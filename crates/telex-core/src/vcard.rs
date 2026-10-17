//! vCard files (.vcf): reading versions 2.1, 3.0 and 4.0, writing 3.0.
//!
//! Reading takes what the address book keeps: the name (FN, else N), the addresses (the
//! preferred one first), phones, the organization, the note, CATEGORIES as groups, and group
//! cards (KIND:group with MEMBER, and Apple's X-ADDRESSBOOKSERVER-KIND). It copes with what old
//! programs write: folded lines, quoted-printable values with soft line breaks, CHARSET
//! parameters (Outlook's Windows-1250), parameters without names (`TEL;WORK;VOICE:`), Apple's
//! `item1.` prefixes, and files that aren't UTF-8. Photos, addresses and the rest are left out.
//!
//! Writing makes vCard 3.0, which every address book reads: one card per contact with its groups
//! as CATEGORIES, and one per address a group holds that isn't a contact, marked
//! `X-TELEX-MEMBER` so Telex reads it back as a member only.

use crate::contacts::{Contact, Group};

/// One card as read from a file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Card {
    /// UID, as written (`urn:uuid:` and all).
    pub uid: String,
    pub name: String,
    /// The preferred address first.
    pub emails: Vec<String>,
    /// The phones, the preferred one first, joined with ", ".
    pub phone: String,
    pub organization: String,
    pub note: String,
    /// CATEGORIES: the groups it belongs to.
    pub categories: Vec<String>,
    /// A group card: its name is `name`, its people `members`.
    pub group: bool,
    /// MEMBER values of a group card: `mailto:` addresses or `urn:uuid:` of other cards.
    pub members: Vec<String>,
    /// Written by Telex for an address a group holds that isn't a contact.
    pub member_only: bool,
}

/// A property line: its name (upper case, without `item1.`), its parameters, its raw value.
struct Line {
    name: String,
    params: Vec<(String, String)>,
    value: Vec<u8>,
}

impl Line {
    /// The parameter's values: `TYPE=a,b;TYPE=c` gives a, b, c. A bare 2.1 parameter
    /// (`TEL;WORK;PREF:`) counts as a TYPE.
    fn types(&self) -> impl Iterator<Item = String> + '_ {
        self.params
            .iter()
            .filter(|(k, _)| k == "TYPE" || k.is_empty())
            .flat_map(|(_, v)| v.split(','))
            .map(|v| v.trim().trim_matches('"').to_ascii_lowercase())
    }

    fn param(&self, name: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.trim_matches('"'))
    }

    fn preferred(&self) -> bool {
        self.types().any(|t| t == "pref")
            || self
                .param("PREF")
                .is_some_and(|p| p.trim().parse::<u32>().is_ok_and(|n| n <= 1))
    }

    fn quoted_printable(&self) -> bool {
        self.param("ENCODING")
            .is_some_and(|e| e.eq_ignore_ascii_case("quoted-printable"))
            || self.types().any(|t| t == "quoted-printable")
    }

    /// The value as text: quoted-printable undone, in its CHARSET (else UTF-8, else
    /// Windows-1250, the likeliest for a Polish file that isn't UTF-8).
    fn text(&self) -> String {
        let bytes = if self.quoted_printable() {
            quoted_printable(&self.value)
        } else {
            self.value.clone()
        };
        let charset = self.param("CHARSET").map(str::to_owned);
        decode(&bytes, charset.as_deref())
    }
}

fn decode(bytes: &[u8], charset: Option<&str>) -> String {
    if let Some(charset) = charset.filter(|c| !c.eq_ignore_ascii_case("utf-8")) {
        if let Some(decoder) =
            mail_parser::decoders::charsets::map::charset_decoder(charset.as_bytes())
        {
            return decoder(bytes);
        }
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => match mail_parser::decoders::charsets::map::charset_decoder(b"windows-1250") {
            Some(decoder) => decoder(bytes),
            None => String::from_utf8_lossy(bytes).into_owned(),
        },
    }
}

fn quoted_printable(value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len());
    let mut i = 0;
    while i < value.len() {
        if value[i] == b'=' {
            let hex = value.get(i + 1..i + 3).and_then(|h| {
                std::str::from_utf8(h)
                    .ok()
                    .and_then(|h| u8::from_str_radix(h, 16).ok())
            });
            match hex {
                Some(byte) => {
                    out.push(byte);
                    i += 3;
                }
                // A soft line break that wasn't joined, or a stray "=".
                None => i += 1,
            }
        } else {
            out.push(value[i]);
            i += 1;
        }
    }
    out
}

/// The file's text lines as bytes, without line ends; UTF-16 files (with a byte order mark)
/// are turned into UTF-8 first.
fn physical_lines(bytes: &[u8]) -> Vec<Vec<u8>> {
    let owned;
    let bytes = if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let units: Vec<u16> = rest
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect();
        owned = String::from_utf16_lossy(&units).into_bytes();
        &owned[..]
    } else if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        let units: Vec<u16> = rest
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_be_bytes(*c))
            .collect();
        owned = String::from_utf16_lossy(&units).into_bytes();
        &owned[..]
    } else {
        bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes)
    };
    bytes
        .split(|&b| b == b'\n')
        .map(|l| l.strip_suffix(b"\r").unwrap_or(l).to_vec())
        .collect()
}

/// Where the property's name and parameters end: the first ':' outside quotes.
fn colon_at(line: &[u8]) -> Option<usize> {
    let mut quoted = false;
    for (i, &b) in line.iter().enumerate() {
        match b {
            b'"' => quoted = !quoted,
            b':' if !quoted => return Some(i),
            _ => {}
        }
    }
    None
}

/// Lines unfolded: a line starting with a space or tab goes on the one before, and a
/// quoted-printable value ending in "=" goes on with the next line.
fn logical_lines(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut out: Vec<Vec<u8>> = Vec::new();
    let mut soft_break = false;
    for line in physical_lines(bytes) {
        if soft_break {
            let last = out.last_mut().expect("a soft break follows a line");
            last.pop(); // the "="
            last.extend_from_slice(&line);
        } else if matches!(line.first(), Some(b' ' | b'\t')) && !out.is_empty() {
            out.last_mut()
                .expect("checked")
                .extend_from_slice(&line[1..]);
        } else if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        } else {
            out.push(line);
        }
        let last = out.last().expect("a line was added");
        soft_break = last.ends_with(b"=") && {
            let head = colon_at(last).map_or(&last[..], |at| &last[..at]);
            head.to_ascii_uppercase()
                .windows(16)
                .any(|w| w == b"QUOTED-PRINTABLE")
        };
    }
    out
}

fn parse_line(line: &[u8]) -> Option<Line> {
    let at = colon_at(line)?;
    let head = String::from_utf8_lossy(&line[..at]);
    let mut parts = split_params(&head).into_iter();
    let name = parts.next()?;
    // Apple groups properties: "item1.EMAIL".
    let name = name
        .rsplit_once('.')
        .map_or(name.as_str(), |(_, n)| n)
        .trim()
        .to_ascii_uppercase();
    let params = parts
        .map(|p| match p.split_once('=') {
            Some((k, v)) => (k.trim().to_ascii_uppercase(), v.trim().to_owned()),
            None => (String::new(), p.trim().to_owned()),
        })
        .collect();
    Some(Line {
        name,
        params,
        value: line[at + 1..].to_vec(),
    })
}

/// "EMAIL;TYPE=\"a;b\";PREF=1" into its parts, quotes respected.
fn split_params(head: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in head.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                current.push(c);
            }
            ';' if !quoted => parts.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    parts.push(current);
    parts
}

/// A value's text with its escapes undone (`\n`, `\,`, `\;`, `\\`).
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n' | 'N') => out.push('\n'),
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// A value cut at `sep` where it isn't escaped, each part unescaped.
fn components(s: &str, sep: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            current.push(c);
            if let Some(next) = chars.next() {
                current.push(next);
            }
        } else if c == sep {
            parts.push(unescape(&std::mem::take(&mut current)));
        } else {
            current.push(c);
        }
    }
    parts.push(unescape(&current));
    parts
}

/// Every card in a .vcf file, in order. Cards without a name, an address or a phone (and that
/// aren't groups) are left out.
pub fn parse(bytes: &[u8]) -> Vec<Card> {
    let mut cards = Vec::new();
    let mut depth = 0i32;
    let mut card = Card::default();
    let (mut emails, mut phones) = (Vec::new(), Vec::new());
    let mut structured_name = String::new();
    for raw in logical_lines(bytes) {
        let Some(line) = parse_line(&raw) else {
            continue;
        };
        let text = || line.text();
        match line.name.as_str() {
            "BEGIN" if text().trim().eq_ignore_ascii_case("vcard") => {
                depth += 1;
                if depth == 1 {
                    card = Card::default();
                    emails.clear();
                    phones.clear();
                    structured_name.clear();
                }
            }
            "END" if text().trim().eq_ignore_ascii_case("vcard") => {
                depth -= 1;
                if depth == 0 {
                    // The preferred first; otherwise in the file's order.
                    emails.sort_by_key(|(pref, _): &(bool, String)| !pref);
                    phones.sort_by_key(|(pref, _): &(bool, String)| !pref);
                    for (_, email) in emails.drain(..) {
                        if !card
                            .emails
                            .iter()
                            .any(|e: &String| e.eq_ignore_ascii_case(&email))
                        {
                            card.emails.push(email);
                        }
                    }
                    card.phone = phones
                        .drain(..)
                        .map(|(_, p)| p)
                        .collect::<Vec<_>>()
                        .join(", ");
                    if card.name.is_empty() {
                        card.name = std::mem::take(&mut structured_name);
                    }
                    if card.group
                        || !card.name.is_empty()
                        || !card.emails.is_empty()
                        || !card.phone.is_empty()
                    {
                        cards.push(std::mem::take(&mut card));
                    }
                }
                depth = depth.max(0);
            }
            _ if depth != 1 => {} // outside a card, or a card inside one (AGENT)
            "UID" => card.uid = unescape(&text()).trim().to_owned(),
            "FN" => card.name = one_line(&unescape(&text())),
            "N" => {
                // Family; given; additional; prefixes; suffixes.
                let parts = components(&text(), ';');
                let part = |i: usize| parts.get(i).map_or("", |p| p.trim());
                structured_name = [part(3), part(1), part(2), part(0), part(4)]
                    .into_iter()
                    .filter(|p| !p.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
            }
            "EMAIL" => {
                let email = unescape(&text())
                    .trim()
                    .trim_start_matches("mailto:")
                    .to_owned();
                if email.contains('@') {
                    emails.push((line.preferred(), email));
                }
            }
            "TEL" => {
                let phone = unescape(&text())
                    .trim()
                    .trim_start_matches("tel:")
                    .to_owned();
                if !phone.is_empty() {
                    phones.push((line.preferred(), phone));
                }
            }
            "ORG" => {
                card.organization = components(&text(), ';')
                    .iter()
                    .map(|p| p.trim())
                    .filter(|p| !p.is_empty())
                    .collect::<Vec<_>>()
                    .join(", ");
            }
            "NOTE" => {
                let note = unescape(&text()).replace("\r\n", "\n");
                card.note = note.trim().to_owned();
            }
            "CATEGORIES" => {
                for c in components(&text(), ',') {
                    let c = one_line(&c);
                    if !c.is_empty() && !card.categories.contains(&c) {
                        card.categories.push(c);
                    }
                }
            }
            "KIND" | "X-ADDRESSBOOKSERVER-KIND" => {
                card.group = text().trim().eq_ignore_ascii_case("group");
            }
            "MEMBER" | "X-ADDRESSBOOKSERVER-MEMBER" => {
                let member = unescape(&text()).trim().to_owned();
                if !member.is_empty() {
                    card.members.push(member);
                }
            }
            "X-TELEX-MEMBER" => card.member_only = !text().trim().eq_ignore_ascii_case("false"),
            _ => {}
        }
    }
    cards
}

/// Spaces collapsed, no line breaks: names and categories.
fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A UID compared with another: without `urn:uuid:`, case ignored.
pub(crate) fn same_uid(a: &str, b: &str) -> bool {
    let bare = |s: &str| {
        let s = s.trim();
        s.get(..9)
            .filter(|p| p.eq_ignore_ascii_case("urn:uuid:"))
            .map_or(s, |_| &s[9..])
            .to_ascii_lowercase()
    };
    !a.trim().is_empty() && bare(a) == bare(b)
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            ',' => out.push_str("\\,"),
            ';' => out.push_str("\\;"),
            c => out.push(c),
        }
    }
    out
}

/// A content line folded at 75 octets (never inside a character), with CRLF.
fn fold_into(out: &mut String, line: &str) {
    let mut width = 0;
    for c in line.chars() {
        let len = c.len_utf8();
        if width + len > 75 {
            out.push_str("\r\n ");
            width = 1;
        }
        out.push(c);
        width += len;
    }
    out.push_str("\r\n");
}

/// "Anna Maria Kowalska" as N: Kowalska;Anna Maria. "Kowalska, Anna" is already that way round.
fn structured(name: &str) -> String {
    if let Some((family, given)) = name.split_once(", ") {
        return format!("{};{};;;", escape(family.trim()), escape(given.trim()));
    }
    match name.trim().rsplit_once(' ') {
        Some((given, family)) => format!("{};{};;;", escape(family), escape(given.trim())),
        None => format!("{};;;;", escape(name.trim())),
    }
}

/// The contacts as one vCard 3.0 file, groups as CATEGORIES. Members of `groups` that aren't
/// contacts follow as cards of their own, marked as members only.
pub fn write(contacts: &[Contact], groups: &[Group]) -> String {
    let mut out = String::new();
    let group_names = |contact: Option<i64>, email: &str| -> Vec<String> {
        groups
            .iter()
            .filter(|g| {
                g.members.iter().any(|m| match contact {
                    Some(id) => m.contact == Some(id),
                    None => m.contact.is_none() && m.email.eq_ignore_ascii_case(email),
                })
            })
            .map(|g| escape(&g.name))
            .collect()
    };
    let mut card = |uid: &str,
                    name: &str,
                    emails: &[String],
                    phone: &str,
                    organization: &str,
                    note: &str,
                    categories: Vec<String>,
                    member_only: bool| {
        let shown = if name.trim().is_empty() {
            emails.first().map_or("", String::as_str)
        } else {
            name
        };
        fold_into(&mut out, "BEGIN:VCARD");
        fold_into(&mut out, "VERSION:3.0");
        if !uid.is_empty() {
            fold_into(&mut out, &format!("UID:{}", escape(uid)));
        }
        fold_into(&mut out, &format!("FN:{}", escape(shown)));
        fold_into(&mut out, &format!("N:{}", structured(name)));
        for (i, email) in emails.iter().enumerate() {
            let kind = if i == 0 && emails.len() > 1 {
                "INTERNET,PREF"
            } else {
                "INTERNET"
            };
            fold_into(&mut out, &format!("EMAIL;TYPE={kind}:{}", escape(email)));
        }
        for p in phone
            .split([',', ';'])
            .map(str::trim)
            .filter(|p| !p.is_empty())
        {
            fold_into(&mut out, &format!("TEL:{}", escape(p)));
        }
        if !organization.is_empty() {
            fold_into(&mut out, &format!("ORG:{}", escape(organization)));
        }
        if !note.is_empty() {
            fold_into(&mut out, &format!("NOTE:{}", escape(note)));
        }
        if !categories.is_empty() {
            fold_into(&mut out, &format!("CATEGORIES:{}", categories.join(",")));
        }
        if member_only {
            fold_into(&mut out, "X-TELEX-MEMBER:TRUE");
        }
        fold_into(&mut out, "END:VCARD");
    };
    for c in contacts {
        card(
            &c.uid,
            &c.name,
            &c.emails,
            &c.phone,
            &c.organization,
            &c.note,
            group_names(Some(c.id), ""),
            false,
        );
    }
    let mut written: Vec<String> = Vec::new();
    for m in groups.iter().flat_map(|g| &g.members) {
        let key = m.email.to_lowercase();
        if m.contact.is_some() || m.email.is_empty() || written.contains(&key) {
            continue;
        }
        written.push(key);
        card(
            "",
            &m.name,
            std::slice::from_ref(&m.email),
            "",
            "",
            "",
            group_names(None, &m.email),
            true,
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contacts::Member;

    #[test]
    fn version_4_with_groups_and_preferences() {
        let file = "BEGIN:VCARD\r\nVERSION:4.0\r\nUID:urn:uuid:4fbe8971-0bc3-424c-9c26-36c3e1eff6b1\r\nFN:Anna Kowalska\r\nN:Kowalska;Anna;;;\r\nEMAIL;TYPE=work:anna@studio.pl\r\nEMAIL;PREF=1:anna.kowalska@gmail.com\r\nTEL;VALUE=uri;TYPE=cell:tel:+48-600-100-200\r\nORG:Studio Łódź;Grafika\r\nNOTE:Pierwsza linia\\nDruga\\, z przecinkiem\r\nCATEGORIES:Zarząd,Klub\r\nEND:VCARD\r\nBEGIN:VCARD\r\nVERSION:4.0\r\nKIND:group\r\nFN:Rodzina\r\nMEMBER:urn:uuid:4fbe8971-0bc3-424c-9c26-36c3e1eff6b1\r\nMEMBER:mailto:babcia@onet.pl\r\nEND:VCARD\r\n";
        let cards = parse(file.as_bytes());
        assert_eq!(cards.len(), 2);
        let anna = &cards[0];
        assert_eq!(anna.name, "Anna Kowalska");
        assert_eq!(
            anna.emails,
            ["anna.kowalska@gmail.com", "anna@studio.pl"],
            "the preferred one first"
        );
        assert_eq!(anna.phone, "+48-600-100-200");
        assert_eq!(anna.organization, "Studio Łódź, Grafika");
        assert_eq!(anna.note, "Pierwsza linia\nDruga, z przecinkiem");
        assert_eq!(anna.categories, ["Zarząd", "Klub"]);
        let family = &cards[1];
        assert!(family.group);
        assert_eq!(family.name, "Rodzina");
        assert_eq!(family.members.len(), 2);
        assert!(same_uid(&family.members[0], &anna.uid));
        assert!(same_uid("4FBE8971-0BC3-424C-9C26-36C3E1EFF6B1", &anna.uid));
        assert!(!same_uid("", ""));
    }

    #[test]
    fn old_outlook_exports_with_quoted_printable_and_windows_1250() {
        // vCard 2.1 as Outlook wrote it: bare parameters, quoted-printable with a soft line
        // break, Polish letters in Windows-1250 (ł = 0xB3, ó = 0xF3, ż = 0xBF, ą = 0xB9).
        let mut file: Vec<u8> = Vec::new();
        file.extend_from_slice(b"BEGIN:VCARD\r\nVERSION:2.1\r\n");
        file.extend_from_slice(
            b"N;CHARSET=Windows-1250;ENCODING=QUOTED-PRINTABLE:Wr=F3blewska;Ma=B3gorzata\r\n",
        );
        file.extend_from_slice(
            b"FN;CHARSET=Windows-1250;ENCODING=QUOTED-PRINTABLE:Ma=B3gorzata Wr=F3ble=\r\nwska\r\n",
        );
        file.extend_from_slice(b"ORG;CHARSET=Windows-1250:Pi\xbfama Sp. z o.o.\r\n");
        file.extend_from_slice(b"TEL;WORK;VOICE:22 111 22 33\r\nTEL;CELL;PREF:600 700 800\r\n");
        file.extend_from_slice(b"EMAIL;PREF;INTERNET:malgosia@pizama.pl\r\n");
        file.extend_from_slice(
            b"NOTE;ENCODING=QUOTED-PRINTABLE;CHARSET=Windows-1250:Zam=F3wienia=0D=0Aw pi=B9tki\r\n",
        );
        file.extend_from_slice(b"END:VCARD\r\n");
        let cards = parse(&file);
        assert_eq!(cards.len(), 1);
        let m = &cards[0];
        assert_eq!(m.name, "Małgorzata Wróblewska");
        assert_eq!(m.organization, "Piżama Sp. z o.o.");
        assert_eq!(m.phone, "600 700 800, 22 111 22 33");
        assert_eq!(m.emails, ["malgosia@pizama.pl"]);
        assert_eq!(m.note, "Zamówienia\nw piątki");
    }

    #[test]
    fn folded_lines_apple_items_and_names_from_n() {
        let file = "BEGIN:VCARD\nVERSION:3.0\nN:Nowak;Piotr;;;\nitem1.EMAIL;type=INTERNET;type=pref:piotr@nowak-\n meble.pl\nitem1.X-ABLabel:_$!<Work>!$_\nNOTE:Bardzo długa notatka, która została złożona w\n\t dwie linie\nPHOTO;ENCODING=b;TYPE=JPEG:AAAA\n BBBB\nEND:VCARD\nBEGIN:VCARD\nVERSION:3.0\nX-ADDRESSBOOKSERVER-KIND:group\nFN:Meble\nX-ADDRESSBOOKSERVER-MEMBER:urn:uuid:abc\nEND:VCARD\nBEGIN:VCARD\nVERSION:3.0\nEND:VCARD\n";
        let cards = parse(file.as_bytes());
        assert_eq!(cards.len(), 2, "an empty card is left out");
        assert_eq!(cards[0].name, "Piotr Nowak");
        assert_eq!(cards[0].emails, ["piotr@nowak-meble.pl"]);
        assert_eq!(
            cards[0].note,
            "Bardzo długa notatka, która została złożona w dwie linie"
        );
        assert!(cards[1].group);
        assert_eq!(cards[1].members, ["urn:uuid:abc"]);
    }

    #[test]
    fn utf16_files_are_read() {
        let text = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Łukasz Żółć\r\nEMAIL:lz@x.pl\r\nEND:VCARD\r\n";
        let mut file = vec![0xFF, 0xFE];
        for unit in text.encode_utf16() {
            file.extend_from_slice(&unit.to_le_bytes());
        }
        let cards = parse(&file);
        assert_eq!(cards[0].name, "Łukasz Żółć");
    }

    #[test]
    fn written_cards_read_back_the_same() {
        let contacts = vec![
            Contact {
                id: 1,
                uid: "urn:uuid:11111111-2222-3333-4444-555555555555".into(),
                name: "Zażółć Gęślą-Jaźń".into(),
                emails: vec!["jazn@przyklad.pl".into(), "zg@firma.pl".into()],
                phone: "+48 600 100 200, 22 123 45 67".into(),
                organization: "Spółka; z \"cudzysłowem\"".into(),
                note: "Pierwsza linia\nDruga, z przecinkiem i średnikiem; i ukośnikiem \\ oraz bardzo, bardzo długi tekst, który trzeba złożyć w kilka linii".into(),
                groups: vec![7],
            },
            Contact {
                id: 2,
                uid: "telex-2".into(),
                name: "Kowalska, Anna".into(),
                emails: vec!["anna@studio.pl".into()],
                ..Contact::default()
            },
        ];
        let groups = vec![Group {
            id: 7,
            name: "Zarząd".into(),
            members: vec![
                Member {
                    contact: Some(1),
                    ..Member::default()
                },
                Member {
                    contact: None,
                    name: "Ewa Świątek".into(),
                    email: "ewa@x.pl".into(),
                },
            ],
        }];
        let file = write(&contacts, &groups);
        assert!(file.lines().all(|l| l.len() <= 76), "folded:\n{file}");
        assert!(file.contains("\r\n"));
        let cards = parse(file.as_bytes());
        assert_eq!(cards.len(), 3);
        let first = &cards[0];
        assert_eq!(first.uid, contacts[0].uid);
        assert_eq!(first.name, contacts[0].name);
        assert_eq!(first.emails, contacts[0].emails);
        assert_eq!(first.phone, contacts[0].phone);
        assert_eq!(first.organization, contacts[0].organization);
        assert_eq!(first.note, contacts[0].note);
        assert_eq!(first.categories, ["Zarząd"]);
        assert!(!first.member_only);
        assert_eq!(cards[1].name, "Kowalska, Anna");
        assert!(file.contains("N:Kowalska;Anna;;;"));
        assert!(file.contains("N:Gęślą-Jaźń;Zażółć;;;"));
        let ewa = &cards[2];
        assert!(ewa.member_only);
        assert_eq!(ewa.name, "Ewa Świątek");
        assert_eq!(ewa.emails, ["ewa@x.pl"]);
        assert_eq!(ewa.categories, ["Zarząd"]);
    }
}
