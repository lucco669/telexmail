//! Thunderbird and the programs that share its profile layout (Betterbird, SeaMonkey,
//! Postbox): `profiles.ini` names the profiles; a profile's `prefs.js` has the accounts, their
//! identities and outgoing servers; its `Mail/` and `ImapMail/` folders hold mail as mbox files
//! (or Maildir), a folder's subfolders in "<name>.sbd"; `msgFilterRules.dat` in a server's
//! folder holds its filters.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::{
    found_folder, identity, mark_present, Blocked, BookPlace, FoundAccount, FoundBook, FoundFolder,
    Place, Program, Source,
};
use crate::filters::fold;
use crate::model::{Account, AccountSetup, Identity, SmtpSecurity};
use crate::settings::{PlanItem, Skip, Status};
use crate::store::StoreError;
use crate::tr;
use crate::vcard::{self, Card};
use crate::tags::{self, Tag};

/// Where each program keeps its profiles.
pub fn roots() -> Vec<(PathBuf, Program)> {
    let mut roots = Vec::new();
    if let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) {
        roots.push((appdata.join("Thunderbird"), Program::Thunderbird));
        roots.push((appdata.join("Betterbird"), Program::Betterbird));
        roots.push((
            appdata.join("Mozilla").join("SeaMonkey"),
            Program::Seamonkey,
        ));
        roots.push((appdata.join("Postbox"), Program::Postbox));
    }
    if !cfg!(windows) {
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            for (dir, program) in [
                (".thunderbird", Program::Thunderbird),
                ("snap/thunderbird/common/.thunderbird", Program::Thunderbird),
                (
                    ".var/app/org.mozilla.Thunderbird/.thunderbird",
                    Program::Thunderbird,
                ),
                (
                    ".var/app/eu.betterbird.Betterbird/.thunderbird",
                    Program::Betterbird,
                ),
                (".icedove", Program::Thunderbird),
                (".mozilla-thunderbird", Program::Thunderbird),
                (".mozilla/seamonkey", Program::Seamonkey),
            ] {
                roots.push((home.join(dir), program));
            }
        }
    }
    roots.retain(|(dir, _)| dir.join("profiles.ini").is_file());
    roots
}

/// Which program a profile folder belongs to, by the folders around it.
pub fn program_of(profile: &Path) -> Program {
    let path = profile.to_string_lossy().to_lowercase();
    if path.contains("betterbird") {
        Program::Betterbird
    } else if path.contains("seamonkey") {
        Program::Seamonkey
    } else if path.contains("postbox") {
        Program::Postbox
    } else {
        Program::Thunderbird
    }
}

/// The profiles `profiles.ini` lists, the one the program starts with first.
pub fn profiles(root: &Path) -> Vec<PathBuf> {
    let Ok(text) = std::fs::read_to_string(root.join("profiles.ini")) else {
        return Vec::new();
    };
    let mut sections: Vec<HashMap<String, String>> = Vec::new();
    let mut names = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            names.push(name.to_owned());
            sections.push(HashMap::new());
        } else if let (Some((k, v)), Some(section)) = (line.split_once('='), sections.last_mut()) {
            section.insert(k.trim().to_owned(), v.trim().to_owned());
        }
    }
    let resolve = |path: &str, relative: bool| {
        if relative {
            root.join(path)
        } else {
            PathBuf::from(path)
        }
    };
    let mut ranked: Vec<(u8, PathBuf)> = Vec::new();
    for (name, section) in names.iter().zip(&sections) {
        if name.starts_with("Install") {
            if let Some(path) = section.get("Default") {
                ranked.push((0, resolve(path, true)));
            }
        } else if let Some(path) = section.get("Path") {
            let relative = section.get("IsRelative").is_none_or(|v| v == "1");
            let rank = if section.get("Default").is_some_and(|v| v == "1") {
                1
            } else {
                2
            };
            ranked.push((rank, resolve(path, relative)));
        }
    }
    ranked.sort_by_key(|(rank, _)| *rank);
    let mut out: Vec<PathBuf> = Vec::new();
    for (_, path) in ranked {
        let path = normalized(&path);
        if path.join("prefs.js").is_file() && !out.contains(&path) {
            out.push(path);
        }
    }
    out
}

/// "Profiles/abc.default" with the separators this system uses.
fn normalized(path: &Path) -> PathBuf {
    path.components().collect()
}

#[derive(Debug, Clone, PartialEq)]
enum Pref {
    Text(String),
    Number(i64),
    Bool(bool),
}

/// `prefs.js`: lines of `user_pref("name", value);`.
#[derive(Debug, Default)]
struct Prefs(HashMap<String, Pref>);

impl Prefs {
    fn parse(text: &str) -> Self {
        let mut prefs = HashMap::new();
        for line in text.lines() {
            let Some(rest) = line.trim().strip_prefix("user_pref(") else {
                continue;
            };
            let Some((name, rest)) = js_string(rest) else {
                continue;
            };
            let rest = rest.trim_start().trim_start_matches(',').trim_start();
            let value = if rest.starts_with('"') {
                js_string(rest).map(|(v, _)| Pref::Text(v))
            } else {
                let word: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
                    .collect();
                match word.as_str() {
                    "true" => Some(Pref::Bool(true)),
                    "false" => Some(Pref::Bool(false)),
                    n => n.parse().ok().map(Pref::Number),
                }
            };
            if let Some(value) = value {
                prefs.insert(name, value);
            }
        }
        Self(prefs)
    }

    fn text(&self, name: &str) -> Option<&str> {
        match self.0.get(name)? {
            Pref::Text(t) if !t.trim().is_empty() => Some(t.as_str()),
            _ => None,
        }
    }

    fn number(&self, name: &str) -> Option<i64> {
        match self.0.get(name)? {
            Pref::Number(n) => Some(*n),
            Pref::Text(t) => t.trim().parse().ok(),
            Pref::Bool(_) => None,
        }
    }

    fn flag(&self, name: &str) -> bool {
        matches!(self.0.get(name), Some(Pref::Bool(true)))
    }
}

/// A JavaScript string at the start of `s` ("…" with \" \\ \n \uXXXX), and what follows it.
fn js_string(s: &str) -> Option<(String, &str)> {
    let mut chars = s.strip_prefix('"')?.char_indices();
    let mut out = String::new();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some((out, &s[i + 2..])),
            '\\' => match chars.next()?.1 {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => {}
                'u' => {
                    let hex: String = (0..4)
                        .filter_map(|_| chars.next().map(|(_, c)| c))
                        .collect();
                    let unit = u16::from_str_radix(&hex, 16).ok()?;
                    out.push(char::from_u32(u32::from(unit)).unwrap_or('\u{fffd}'));
                }
                other => out.push(other),
            },
            c => out.push(c),
        }
    }
    None
}

/// What a profile has, or `None` when it has neither accounts nor mail.
pub fn read(profile: &Path, program: Program, accounts: &[Account]) -> Option<Source> {
    let prefs = Prefs::parse(&std::fs::read_to_string(profile.join("prefs.js")).ok()?);
    let mut found = Vec::new();
    let mut folders = Vec::new();
    let mut filter_files = Vec::new();
    let mut filters = 0;
    let order = prefs
        .text("mail.accountmanager.accounts")
        .unwrap_or_default();
    for account in order.split(',').map(str::trim).filter(|a| !a.is_empty()) {
        let Some(server) = prefs.text(&format!("mail.account.{account}.server")) else {
            continue;
        };
        let p = |key: &str| format!("mail.server.{server}.{key}");
        let kind = prefs.text(&p("type")).unwrap_or_default();
        let dir = server_dir(profile, &prefs, server, kind);
        let identities: Vec<Identity> = prefs
            .text(&format!("mail.account.{account}.identities"))
            .unwrap_or_default()
            .split(',')
            .filter_map(|id| read_identity(&prefs, id.trim()))
            .collect();
        match kind {
            "imap" | "pop3" => {
                let Some(found_account) = read_account(&prefs, server, kind, &identities) else {
                    continue;
                };
                if kind == "pop3" {
                    // Its mail is only here: it comes along as folders of its own.
                    let prefix = found_account.setup.email.clone();
                    mail_folders(&dir, &[prefix], 0, &mut folders);
                }
                let rules = dir.join("msgFilterRules.dat");
                if kind == "imap" && rules.is_file() {
                    filters += count_filters(&rules);
                    filter_files.push((server.to_owned(), rules));
                }
                found.push(found_account);
            }
            "none" => mail_folders(&dir, &[], 0, &mut folders),
            _ => {} // news, feeds, chat
        }
    }
    let books = address_books(profile, &prefs);
    if found.is_empty() && folders.is_empty() && books.is_empty() {
        return None;
    }
    let tags = read_tags(&prefs);
    mark_present(&mut found, accounts);
    let name = profile
        .file_name()
        .map(|n| n.to_string_lossy())
        .and_then(|n| n.split_once('.').map(|(_, rest)| rest.to_owned()))
        .unwrap_or_else(|| profile.display().to_string());
    Some(
        Source {
            path: profile.display().to_string(),
            program,
            name,
            accounts: found,
            folders,
            filters,
            tags,
            filter_files,
            books,
        }
        .numbered(),
    )
}

/// The profile's address books: abook.sqlite (Personal Address Book), abook-1.sqlite and so on,
/// and history.sqlite (Collected Addresses, unticked). Thunderbird before 78 kept them as .mab
/// (Mork) files, which aren't read.
fn address_books(profile: &Path, prefs: &Prefs) -> Vec<FoundBook> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(profile)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                (n.starts_with("abook") || n == "history.sqlite") && n.ends_with(".sqlite")
            })
        })
        .collect();
    files.sort();
    let mut books = Vec::new();
    for path in files {
        let file = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        // ldap_2.servers.<name>.filename names the file, .description the book.
        let described = prefs.0.iter().find_map(|(key, value)| {
            let server = key
                .strip_prefix("ldap_2.servers.")?
                .strip_suffix(".filename")?;
            match value {
                Pref::Text(f) if *f == file => {
                    prefs.text(&format!("ldap_2.servers.{server}.description"))
                }
                _ => None,
            }
        });
        let collected = file == "history.sqlite";
        let name = match described.filter(|d| !d.starts_with("chrome://")) {
            Some(d) => d.to_owned(),
            None if collected => tr!("Collected Addresses", "Zebrane adresy"),
            None if file == "abook.sqlite" => {
                tr!("Personal Address Book", "Osobista książka adresowa")
            }
            None => file.trim_end_matches(".sqlite").to_owned(),
        };
        let Ok(count) = with_book(&path, |db| {
            db.query_row(
                "SELECT (SELECT COUNT(DISTINCT card) FROM properties)
                      + (SELECT COUNT(*) FROM lists)",
                [],
                |r| r.get::<_, i64>(0),
            )
            .map_err(StoreError::from)
        }) else {
            continue;
        };
        if count > 0 {
            books.push(FoundBook {
                key: String::new(),
                name,
                contacts: u64::try_from(count).unwrap_or(0),
                tick: !collected,
                place: BookPlace::Sqlite(path),
            });
        }
    }
    books
}

/// Open a copy of an address book (Thunderbird may have it open, with changes in its -wal).
fn with_book<T>(
    path: &Path,
    f: impl FnOnce(&rusqlite::Connection) -> Result<T, StoreError>,
) -> Result<T, StoreError> {
    let dir = std::env::temp_dir().join(format!(
        "telex-abook-{}-{}",
        std::process::id(),
        chrono::Local::now()
            .timestamp_nanos_opt()
            .unwrap_or_default()
    ));
    std::fs::create_dir_all(&dir).map_err(|e| StoreError::Database(e.to_string()))?;
    let copy = dir.join("abook.sqlite");
    let result = (|| {
        std::fs::copy(path, &copy).map_err(|e| StoreError::Database(e.to_string()))?;
        let mut wal = path.as_os_str().to_owned();
        wal.push("-wal");
        if Path::new(&wal).is_file() {
            let _ = std::fs::copy(&wal, dir.join("abook.sqlite-wal"));
        }
        let db = rusqlite::Connection::open(&copy)?;
        f(&db)
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// An address book's cards and lists. Since Thunderbird 102 each card keeps a vCard
/// (`_vCard`); older ones only have properties such as DisplayName and PrimaryEmail.
pub fn read_book(path: &Path) -> Result<Vec<Card>, StoreError> {
    with_book(path, |db| {
        let mut cards: Vec<(String, HashMap<String, String>)> = Vec::new();
        let mut index: HashMap<String, usize> = HashMap::new();
        {
            let mut stmt = db.prepare("SELECT card, name, value FROM properties ORDER BY rowid")?;
            let rows = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                ))
            })?;
            for row in rows {
                let (card, name, value) = row?;
                match index.get(&card) {
                    Some(&at) => {
                        cards[at].1.insert(name, value);
                    }
                    None => {
                        index.insert(card.clone(), cards.len());
                        cards.push((card, HashMap::from([(name, value)])));
                    }
                }
            }
        }
        let mut out: Vec<Card> = cards
            .into_iter()
            .map(|(uid, props)| {
                let get = |k: &str| props.get(k).map_or("", |v| v.trim()).to_owned();
                let mut card = props
                    .get("_vCard")
                    .and_then(|v| vcard::parse(v.as_bytes()).into_iter().next())
                    .unwrap_or_else(|| {
                        let name = match get("DisplayName") {
                            n if n.is_empty() => {
                                format!("{} {}", get("FirstName"), get("LastName"))
                                    .trim()
                                    .to_owned()
                            }
                            n => n,
                        };
                        let phones: Vec<String> = ["CellularNumber", "WorkPhone", "HomePhone"]
                            .iter()
                            .map(|k| get(k))
                            .filter(|p| !p.is_empty())
                            .collect();
                        Card {
                            name,
                            emails: [get("PrimaryEmail"), get("SecondEmail")]
                                .into_iter()
                                .filter(|e| !e.is_empty())
                                .collect(),
                            phone: phones.join(", "),
                            organization: get("Company"),
                            note: get("Notes"),
                            ..Card::default()
                        }
                    });
                // Lists name their cards by this id.
                card.uid = uid;
                card
            })
            .collect();
        let lists: Vec<(String, String)> = {
            let mut stmt = db.prepare("SELECT uid, name FROM lists")?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect::<Result<_, _>>()?
        };
        for (list, name) in lists {
            let mut stmt = db.prepare("SELECT card FROM list_cards WHERE list = ?1")?;
            let members = stmt
                .query_map([&list], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            out.push(Card {
                name,
                group: true,
                members,
                ..Card::default()
            });
        }
        Ok(out)
    })
}

/// The tags of `prefs.js`: `mailnews.tags.<keyword>.tag` names one, `.color` gives its color
/// ("#FF9900") and `.ordinal` its place. Thunderbird's five keep Telex's names (in the
/// interface's language) while they still have Thunderbird's own, in English or Polish.
fn read_tags(prefs: &Prefs) -> Vec<Tag> {
    let mut found: Vec<(String, Tag)> = Vec::new();
    for (key, value) in &prefs.0 {
        let Some(own) = key
            .strip_prefix("mailnews.tags.")
            .and_then(|k| k.strip_suffix(".tag"))
        else {
            continue;
        };
        let Pref::Text(name) = value else { continue };
        let keyword = own.to_ascii_lowercase();
        if !tags::is_keyword(&keyword) {
            continue;
        }
        let p = |what: &str| format!("mailnews.tags.{own}.{what}");
        let default = tags::default_name(&keyword);
        let own_default = [crate::i18n::Lang::En, crate::i18n::Lang::Pl]
            .into_iter()
            .filter_map(|lang| crate::i18n::with(lang, || tags::default_name(&keyword)))
            .any(|d| fold(&d) == fold(name));
        let name = match default {
            Some(d) if own_default || name.trim().is_empty() => d,
            _ => name.trim().chars().take(40).collect(),
        };
        if name.is_empty() {
            continue;
        }
        let color = tags::color_from_hex(prefs.text(&p("color")).unwrap_or_default());
        let ordinal = prefs.text(&p("ordinal")).unwrap_or(&keyword).to_owned();
        let tag = Tag {
            keyword,
            name,
            color: color.to_owned(),
            count: 0,
        };
        found.push((ordinal, tag));
    }
    found.sort_by(|a, b| (&a.0, &a.1.keyword).cmp(&(&b.0, &b.1.keyword)));
    found.into_iter().map(|(_, tag)| tag).collect()
}

/// A server's folder: "[ProfD]ImapMail/imap.example.com" is in the profile.
fn server_dir(profile: &Path, prefs: &Prefs, server: &str, kind: &str) -> PathBuf {
    if let Some(rel) = prefs.text(&format!("mail.server.{server}.directory-rel")) {
        if let Some(rest) = rel.strip_prefix("[ProfD]") {
            return profile.join(rest.replace('\\', "/"));
        }
    }
    if let Some(dir) = prefs.text(&format!("mail.server.{server}.directory")) {
        return PathBuf::from(dir);
    }
    let host = prefs
        .text(&format!("mail.server.{server}.hostname"))
        .unwrap_or("Local Folders");
    match kind {
        "imap" => profile.join("ImapMail").join(host),
        _ => profile.join("Mail").join(host),
    }
}

fn read_identity(prefs: &Prefs, id: &str) -> Option<Identity> {
    if id.is_empty() {
        return None;
    }
    let p = |key: &str| format!("mail.identity.{id}.{key}");
    let email = prefs.text(&p("useremail"))?.trim().to_owned();
    let name = prefs.text(&p("fullName")).unwrap_or(&email).to_owned();
    let mut signature = String::new();
    if prefs.flag(&p("attach_signature")) {
        if let Some(file) = prefs.text(&p("sig_file")) {
            if let Ok(text) = std::fs::read(file) {
                let text = String::from_utf8_lossy(&text[..text.len().min(64 * 1024)]).into_owned();
                let html = file.to_lowercase().ends_with(".htm")
                    || file.to_lowercase().ends_with(".html")
                    || text.trim_start().starts_with('<');
                signature = if html {
                    crate::html::to_text(&text)
                } else {
                    text
                };
            }
        }
    }
    if signature.trim().is_empty() {
        if let Some(text) = prefs.text(&p("htmlSigText")) {
            signature = if prefs.flag(&p("htmlSigFormat")) {
                crate::html::to_text(text)
            } else {
                text.to_owned()
            };
        }
    }
    Some(identity(&name, &email, &signature))
}

fn read_account(
    prefs: &Prefs,
    server: &str,
    kind: &str,
    identities: &[Identity],
) -> Option<FoundAccount> {
    let p = |key: &str| format!("mail.server.{server}.{key}");
    let host = prefs
        .text(&p("realhostname"))
        .or_else(|| prefs.text(&p("hostname")))?
        .to_owned();
    let username = prefs
        .text(&p("realuserName"))
        .or_else(|| prefs.text(&p("userName")))
        .unwrap_or_default()
        .to_owned();
    let email = identities
        .first()
        .map(|i| i.email.clone())
        .or_else(|| username.contains('@').then(|| username.clone()))?;
    // socketType: 3 SSL/TLS, 2 STARTTLS, 1 STARTTLS if offered, 0 none. Telex never goes
    // unencrypted, so the last two try STARTTLS, and the check says whether it works.
    let imap_security = if prefs.number(&p("socketType")) == Some(3) {
        SmtpSecurity::Tls
    } else {
        SmtpSecurity::StartTls
    };
    let port = prefs
        .number(&p("port"))
        .and_then(|n| u16::try_from(n).ok())
        .unwrap_or(0);
    let (smtp_host, smtp_port, smtp_security) = outgoing(prefs, server).unwrap_or_default();
    let name = identities
        .first()
        .map(|i| i.name.clone())
        .or_else(|| prefs.text(&p("name")).map(str::to_owned))
        .unwrap_or_default();
    let blocked = if kind == "pop3" {
        Some(Blocked::Pop3)
    } else if prefs.number(&p("authMethod")) == Some(10) {
        Some(Blocked::Oauth)
    } else {
        None
    };
    Some(FoundAccount {
        key: server.to_owned(),
        setup: AccountSetup {
            name,
            email,
            host,
            port: if kind == "pop3" { 0 } else { port },
            username,
            imap_security,
            smtp_host,
            smtp_port,
            smtp_security,
        },
        identities: identities.to_vec(),
        present: false,
        blocked,
    })
}

/// The outgoing server of the account's first identity, or Thunderbird's default one.
fn outgoing(prefs: &Prefs, server: &str) -> Option<(String, u16, SmtpSecurity)> {
    let first_identity = prefs
        .0
        .iter()
        .filter_map(|(k, v)| match v {
            Pref::Text(t) if k.ends_with(".server") && t == server => {
                k.strip_prefix("mail.account.")?.strip_suffix(".server")
            }
            _ => None,
        })
        .find_map(|account| prefs.text(&format!("mail.account.{account}.identities")))
        .and_then(|ids| ids.split(',').next().map(str::trim));
    let smtp = first_identity
        .and_then(|id| prefs.text(&format!("mail.identity.{id}.smtpServer")))
        .or_else(|| prefs.text("mail.smtp.defaultserver"))?;
    let p = |key: &str| format!("mail.smtpserver.{smtp}.{key}");
    let host = prefs.text(&p("hostname"))?.to_owned();
    let security = if prefs.number(&p("try_ssl")) == Some(3) {
        SmtpSecurity::Tls
    } else {
        SmtpSecurity::StartTls
    };
    let port = prefs
        .number(&p("port"))
        .and_then(|n| u16::try_from(n).ok())
        .unwrap_or(0);
    Some((host, port, security))
}

/// Not folders: Thunderbird's own files next to them.
fn is_mail_file(path: &Path) -> bool {
    let has_index = {
        let mut msf = path.as_os_str().to_owned();
        msf.push(".msf");
        Path::new(&msf).is_file()
    };
    has_index || super::looks_like_mbox(path)
}

/// The folders under a server's folder: mbox files (or Maildir folders), each folder's
/// subfolders in "<name>.sbd".
fn mail_folders(dir: &Path, parents: &[String], depth: usize, out: &mut Vec<FoundFolder>) {
    if depth > 12 {
        return;
    }
    let mut entries: Vec<(String, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| Some((e.file_name().to_str()?.to_owned(), e.path())))
        .collect();
    entries.sort_by_key(|(name, _)| folder_rank(name));
    for (name, path) in &entries {
        if name.ends_with(".msf") || name.ends_with(".sbd") {
            continue;
        }
        let maildir = path.is_dir() && path.join("cur").is_dir();
        if !(maildir || path.is_file() && is_mail_file(path)) {
            continue;
        }
        let mut path_names = parents.to_vec();
        path_names.push(name.clone());
        let bytes = if maildir {
            super::dir_size(path)
        } else {
            std::fs::metadata(path).map_or(0, |m| m.len())
        };
        if bytes > 0 {
            let label = path_names.join(" / ");
            out.push(found_folder(&label, Place::Path(path.clone()), bytes));
        }
        let children = dir.join(format!("{name}.sbd"));
        if children.is_dir() {
            mail_folders(&children, &path_names, depth + 1, out);
        }
    }
}

/// Inbox first, then the other folders by name, as Thunderbird lists them.
fn folder_rank(name: &str) -> (u8, String) {
    let lower = name.to_lowercase();
    let rank = match lower.as_str() {
        "inbox" => 0,
        "drafts" | "templates" => 1,
        "sent" => 2,
        "archives" => 3,
        "junk" | "trash" | "unsent messages" => 5,
        _ => 4,
    };
    (rank, lower)
}

/// A filters file's lines as (key, value): `name="Faktury"`.
fn filter_lines(text: &str) -> Vec<(&str, String)> {
    text.lines()
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            let value = value.trim().strip_prefix('"')?.strip_suffix('"')?;
            let mut out = String::new();
            let mut chars = value.chars();
            while let Some(c) = chars.next() {
                if c == '\\' {
                    out.extend(chars.next());
                } else {
                    out.push(c);
                }
            }
            Some((key.trim(), out))
        })
        .collect()
}

fn count_filters(file: &Path) -> usize {
    std::fs::read_to_string(file).map_or(0, |text| {
        filter_lines(&text)
            .iter()
            .filter(|(k, _)| *k == "name")
            .count()
    })
}

/// One condition: "(subject,contains,faktura)", the value possibly in quotes.
struct Term {
    and: bool,
    field: String,
    op: String,
    value: String,
}

/// "AND (from,contains,anna@x.pl) OR (subject,contains,\"a, b\")"; `None` for "ALL" and for
/// anything unreadable.
fn parse_terms(text: &str) -> Option<Vec<Term>> {
    let mut terms = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        let (and, after) = match rest.strip_prefix("AND") {
            Some(r) => (true, r),
            None => (false, rest.strip_prefix("OR")?),
        };
        let after = after.trim_start().strip_prefix('(')?;
        let (field, after) = quoted_or_until(after, ',')?;
        let (op, after) = quoted_or_until(after.strip_prefix(',')?, ',')?;
        let (value, after) = quoted_or_until(after.strip_prefix(',')?, ')')?;
        rest = after.strip_prefix(')')?.trim_start();
        terms.push(Term {
            and,
            field: field.trim().to_lowercase(),
            op: op.trim().to_lowercase(),
            value,
        });
    }
    (!terms.is_empty()).then_some(terms)
}

/// A value in quotes (with \" and \\), or the text up to `end`.
fn quoted_or_until(s: &str, end: char) -> Option<(String, &str)> {
    let s = s.trim_start();
    if let Some(inner) = s.strip_prefix('"') {
        let mut out = String::new();
        let mut chars = inner.char_indices();
        while let Some((i, c)) = chars.next() {
            match c {
                '\\' => out.extend(chars.next().map(|(_, c)| c)),
                '"' => return Some((out, &inner[i + 1..])),
                c => out.push(c),
            }
        }
        None
    } else {
        let at = s.find(end)?;
        Some((s[..at].to_owned(), &s[at..]))
    }
}

/// "%40" → "@", in UTF-8.
fn percent_decoded(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(b) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Where a "Move to folder" goes: a folder of the rule's own account, or one on this PC.
fn move_target(uri: &str, account: &FoundAccount, source: &Source) -> Option<Value> {
    let (scheme, rest) = uri.split_once("://")?;
    let (authority, path) = rest.split_once('/')?;
    let (user, host) = authority.rsplit_once('@').unwrap_or(("", authority));
    let (user, host) = (percent_decoded(user), percent_decoded(host));
    let parts: Vec<String> = path.split('/').map(percent_decoded).collect();
    if scheme == "mailbox" && host == "Local Folders" {
        let name = parts.join(" / ");
        return Some(json!({"type": "move", "folder": name, "this_pc": true}));
    }
    let same = account.setup.host.eq_ignore_ascii_case(&host)
        && (user.is_empty() || account.setup.username.eq_ignore_ascii_case(&user));
    let other = source.accounts.iter().any(|a| {
        a.key != account.key
            && a.setup.host.eq_ignore_ascii_case(&host)
            && a.setup.username.eq_ignore_ascii_case(&user)
    });
    if scheme != "imap" || !same || other {
        return None;
    }
    // As Telex names a server folder: "INBOX / Klienci" is "Klienci".
    let shown = match parts.as_slice() {
        [first, rest @ ..] if !rest.is_empty() && first.eq_ignore_ascii_case("INBOX") => rest,
        all => all,
    };
    Some(json!({"type": "move", "folder": shown.join(" / ")}))
}

/// Thunderbird's filter types: 1 runs on new mail, 32 after junk mail is sorted out.
const ON_NEW_MAIL: i64 = 1 | 32;

/// The filters of one account as rule entries of a settings file, or what can't come along.
pub fn filters(
    file: &Path,
    account: &FoundAccount,
    source: &Source,
) -> Vec<Result<Value, PlanItem>> {
    let Ok(text) = std::fs::read_to_string(file) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut current: Option<Filter> = None;
    for (key, value) in filter_lines(&text) {
        match key {
            "name" => {
                out.extend(current.take().map(|f| f.entry(account, source)));
                current = Some(Filter {
                    name: value,
                    enabled: true,
                    kind: 17,
                    actions: Vec::new(),
                    condition: String::new(),
                });
            }
            _ => {
                let Some(f) = current.as_mut() else { continue };
                match key {
                    "enabled" => f.enabled = value == "yes",
                    "type" => f.kind = value.parse().unwrap_or(17),
                    "action" => f.actions.push((value, String::new())),
                    "actionValue" => {
                        if let Some(last) = f.actions.last_mut() {
                            last.1 = value;
                        }
                    }
                    "condition" => f.condition = value,
                    _ => {}
                }
            }
        }
    }
    out.extend(current.map(|f| f.entry(account, source)));
    out
}

struct Filter {
    name: String,
    enabled: bool,
    kind: i64,
    actions: Vec<(String, String)>,
    condition: String,
}

impl Filter {
    fn entry(self, account: &FoundAccount, source: &Source) -> Result<Value, PlanItem> {
        let email = account.setup.email.clone();
        let skip = |what: &str| PlanItem {
            name: self.name.clone(),
            account: Some(email.clone()),
            status: Status::Skip,
            skip: Some(Skip::Unsupported {
                what: what.to_owned(),
            }),
        };
        let terms = parse_terms(&self.condition).ok_or_else(|| skip(&self.condition))?;
        let mut conditions = Vec::new();
        for t in &terms {
            if t.field == "tag" {
                // Thunderbird's "is" and "contains" both mean the message has the tag.
                let op = match t.op.as_str() {
                    "is" | "contains" => "is",
                    "isn't" | "doesn't contain" => "not_contains",
                    "is empty" => "not_exists",
                    "isn't empty" => "exists",
                    other => return Err(skip(other)),
                };
                let value = if op.ends_with("exists") {
                    String::new()
                } else {
                    t.value.trim().to_ascii_lowercase()
                };
                conditions.push(json!({"field": "tag", "op": op, "value": value}));
                continue;
            }
            let field = match t.field.as_str() {
                "from" => "from",
                "to" | "cc" | "to or cc" => "to",
                "subject" => "subject",
                "body" => "body",
                other => return Err(skip(other)),
            };
            let op = match t.op.as_str() {
                "contains" => "contains",
                "doesn't contain" => "not_contains",
                "is" => "is",
                "begins with" => "starts_with",
                "ends with" => "ends_with",
                other => return Err(skip(other)),
            };
            conditions.push(json!({"field": field, "op": op, "value": t.value}));
        }
        let match_all = terms[0].and;
        if terms.iter().any(|t| t.and != match_all) {
            return Err(skip(&self.condition));
        }
        let mut actions = Vec::new();
        let mut stop = false;
        let mut left_out = Vec::new();
        for (action, value) in &self.actions {
            match action.as_str() {
                "Move to folder" => match move_target(value, account, source) {
                    Some(target) => actions.push(target),
                    None => left_out.push(action.clone()),
                },
                "Mark read" => actions.push(json!({"type": "mark_read"})),
                "Mark flagged" => actions.push(json!({"type": "flag"})),
                "AddTag" | "Add tag" | "Remove tag" | "RemoveTag" if !value.trim().is_empty() => {
                    let kind = if action.starts_with("Add") {
                        "add_tag"
                    } else {
                        "remove_tag"
                    };
                    let tag = value.trim().to_ascii_lowercase();
                    actions.push(json!({"type": kind, "tag": tag}));
                }
                // Before tags, Thunderbird had five labels: "Label" 1–5 is $label1–5.
                "Label" if matches!(value.trim(), "1" | "2" | "3" | "4" | "5") => {
                    let tag = format!("$label{}", value.trim());
                    actions.push(json!({"type": "add_tag", "tag": tag}));
                }
                "Delete" => actions.push(json!({"type": "delete"})),
                "Stop execution" => stop = true,
                other => left_out.push(other.to_owned()),
            }
        }
        if actions.is_empty() {
            return Err(skip(&left_out.join(", ")));
        }
        Ok(json!({
            "name": self.name,
            "enabled": self.enabled && self.kind & ON_NEW_MAIL != 0,
            "account": email,
            "match_all": match_all,
            "conditions": conditions,
            "actions": actions,
            "stop": stop,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrate::tests::temp_dir;

    const PREFS: &str = r##"// Mozilla User Preferences
user_pref("mail.accountmanager.accounts", "account2,account1,account3,account4");
user_pref("mail.account.account1.server", "server1");
user_pref("mail.account.account1.identities", "id1,id3");
user_pref("mail.account.account2.server", "server2");
user_pref("mail.account.account3.server", "server3");
user_pref("mail.account.account3.identities", "id2");
user_pref("mail.account.account4.server", "server4");
user_pref("mail.account.account4.identities", "id4");
user_pref("mail.server.server1.type", "imap");
user_pref("mail.server.server1.hostname", "imap.twojastudio.pl");
user_pref("mail.server.server1.userName", "biuro@twojastudio.pl");
user_pref("mail.server.server1.socketType", 3);
user_pref("mail.server.server1.directory-rel", "[ProfD]ImapMail/imap.twojastudio.pl");
user_pref("mail.server.server2.type", "none");
user_pref("mail.server.server2.hostname", "Local Folders");
user_pref("mail.server.server2.directory-rel", "[ProfD]Mail/Local Folders");
user_pref("mail.server.server3.type", "imap");
user_pref("mail.server.server3.hostname", "imap.gmail.com");
user_pref("mail.server.server3.userName", "anna.kowalska@gmail.com");
user_pref("mail.server.server3.authMethod", 10);
user_pref("mail.server.server3.socketType", 3);
user_pref("mail.server.server4.type", "pop3");
user_pref("mail.server.server4.hostname", "pop.poczta.pl");
user_pref("mail.server.server4.userName", "piotr");
user_pref("mail.server.server4.directory-rel", "[ProfD]Mail/pop.poczta.pl");
user_pref("mail.identity.id1.fullName", "Anna Kowalska");
user_pref("mail.identity.id1.useremail", "biuro@twojastudio.pl");
user_pref("mail.identity.id1.htmlSigFormat", true);
user_pref("mail.identity.id1.htmlSigText", "<b>Anna Kowalska</b><br>Twoja Studio \u0142\u00f3d\u017a");
user_pref("mail.identity.id1.smtpServer", "smtp2");
user_pref("mail.identity.id3.fullName", "Faktury");
user_pref("mail.identity.id3.useremail", "faktury@twojastudio.pl");
user_pref("mail.identity.id2.fullName", "Anna");
user_pref("mail.identity.id2.useremail", "anna.kowalska@gmail.com");
user_pref("mail.identity.id4.useremail", "piotr@poczta.pl");
user_pref("mail.smtp.defaultserver", "smtp1");
user_pref("mail.smtpserver.smtp1.hostname", "smtp.gmail.com");
user_pref("mail.smtpserver.smtp2.hostname", "smtp.twojastudio.pl");
user_pref("mail.smtpserver.smtp2.port", 587);
user_pref("mail.smtpserver.smtp2.try_ssl", 2);
user_pref("mailnews.tags.$label1.tag", "Important");
user_pref("mailnews.tags.$label1.color", "#FF0000");
user_pref("mailnews.tags.$label2.tag", "Klienci");
user_pref("mailnews.tags.$label2.color", "#FF9900");
user_pref("mailnews.tags.$label4.tag", "To Do");
user_pref("mailnews.tags.$label4.color", "#3333FF");
user_pref("mailnews.tags.projekt_x.tag", "Projekt X");
user_pref("mailnews.tags.projekt_x.color", "#00CCCC");
user_pref("mailnews.tags.pilne.tag", "Pilne");
user_pref("mailnews.tags.pilne.color", "#FFCC00");
user_pref("mailnews.tags.bad word.tag", "Zepsuta");
"##;

    const FILTERS: &str = r#"version="9"
logging="no"
name="Faktury"
enabled="yes"
type="17"
action="Move to folder"
actionValue="imap://biuro%40twojastudio.pl@imap.twojastudio.pl/INBOX/Faktury"
action="Mark read"
condition="AND (from,contains,ksiegowosc@biuro.pl) AND (subject,contains,\"faktura, nr\")"
name="Archiwum"
enabled="no"
type="1"
action="Move to folder"
actionValue="mailbox://nobody@Local%20Folders/Archives/2019"
condition="OR (to or cc,is,stare@twojastudio.pl) OR (subject,begins with,[stare])"
name="Wazne"
enabled="yes"
type="16"
action="Mark flagged"
action="Stop execution"
condition="AND (from,ends with,@klient.pl)"
name="Priorytet"
enabled="yes"
type="17"
action="Change priority"
actionValue="Highest"
condition="AND (from,contains,szef)"
name="Daty"
enabled="yes"
type="17"
action="Delete"
condition="AND (date,is before,01-Jan-2020)"
name="Projekty"
enabled="yes"
type="17"
action="AddTag"
actionValue="projekt_x"
action="Remove tag"
actionValue="$label4"
condition="AND (tag,contains,$label2) AND (tag,isn't,PILNE)"
name="Bez etykiet"
enabled="yes"
type="17"
action="Label"
actionValue="1"
condition="AND (tag,is empty,)"
name="Nieznana"
enabled="yes"
type="17"
action="AddTag"
actionValue="nie_ma"
condition="AND (subject,contains,x)"
"#;

    fn profile() -> PathBuf {
        let root = temp_dir("tb");
        let profile = root.join("Profiles").join("ab12cd34.default-release");
        std::fs::create_dir_all(&profile).unwrap();
        std::fs::write(
            root.join("profiles.ini"),
            "[Profile0]\nName=default-release\nIsRelative=1\nPath=Profiles/ab12cd34.default-release\n\n[Profile1]\nName=old\nIsRelative=1\nPath=Profiles/missing.old\nDefault=1\n\n[InstallD78BF5DD33499EC2]\nDefault=Profiles/ab12cd34.default-release\nLocked=1\n",
        )
        .unwrap();
        std::fs::write(profile.join("prefs.js"), PREFS).unwrap();
        let local = profile.join("Mail").join("Local Folders");
        std::fs::create_dir_all(local.join("Archives.sbd")).unwrap();
        let mbox = |subject: &str, status: &str| {
            format!("From - Sat Oct 10 22:35:00 2026\nX-Mozilla-Status: {status}\nSubject: {subject}\nMessage-ID: <{subject}@x.pl>\n\nTreść\n\n")
        };
        // Tagged in Thunderbird: two tags and a keyword that isn't one.
        let tagged = mbox("lokalna", "0001").replacen(
            "Subject:",
            "X-Mozilla-Keys: projekt_x $Label2 NonJunk                    \nSubject:",
            1,
        );
        std::fs::write(local.join("Inbox"), tagged).unwrap();
        std::fs::write(local.join("Inbox.msf"), "").unwrap();
        std::fs::write(local.join("Trash"), mbox("wyrzucona", "0001")).unwrap();
        std::fs::write(local.join("Archives"), "").unwrap();
        std::fs::write(local.join("Archives.msf"), "").unwrap();
        std::fs::write(
            local.join("Archives.sbd").join("2019"),
            mbox("stara", "0001") + &mbox("skasowana", "0009"),
        )
        .unwrap();
        std::fs::write(local.join("msgFilterRules.dat"), "version=\"9\"\n").unwrap();
        let pop = profile.join("Mail").join("pop.poczta.pl");
        std::fs::create_dir_all(&pop).unwrap();
        std::fs::write(pop.join("Inbox"), mbox("z-pop3", "0000")).unwrap();
        let imap = profile.join("ImapMail").join("imap.twojastudio.pl");
        std::fs::create_dir_all(&imap).unwrap();
        std::fs::write(imap.join("msgFilterRules.dat"), FILTERS).unwrap();
        root
    }

    #[test]
    fn a_profile_brings_its_accounts_identities_and_local_mail() {
        let root = profile();
        let profiles = profiles(&root);
        assert_eq!(profiles.len(), 1, "the profile that exists, once");
        let source = crate::i18n::with(crate::i18n::Lang::En, || {
            read(&profiles[0], Program::Thunderbird, &[]).unwrap()
        });
        assert_eq!(source.name, "default-release");
        assert_eq!(source.filters, 8);

        let tags: Vec<(&str, &str, &str)> = source
            .tags
            .iter()
            .map(|t| (t.keyword.as_str(), t.name.as_str(), t.color.as_str()))
            .collect();
        assert_eq!(
            tags,
            [
                ("$label1", "Important", "red"),
                ("$label2", "Klienci", "orange"),
                // Thunderbird's own name for it: Telex's, in the interface's language.
                ("$label4", "To do", "blue"),
                ("pilne", "Pilne", "yellow"),
                ("projekt_x", "Projekt X", "teal"),
            ],
            "the keyword that isn't one is left out"
        );
        let pl = crate::i18n::with(crate::i18n::Lang::Pl, || {
            read(&profiles[0], Program::Thunderbird, &[]).unwrap()
        });
        assert_eq!(pl.tags[2].name, "Do zrobienia");

        let keys: Vec<&str> = source.accounts.iter().map(|a| a.key.as_str()).collect();
        assert_eq!(
            keys,
            ["server1", "server3", "server4"],
            "Thunderbird's order"
        );
        let studio = &source.accounts[0];
        assert_eq!(studio.setup.email, "biuro@twojastudio.pl");
        assert_eq!(studio.setup.name, "Anna Kowalska");
        assert_eq!(studio.setup.host, "imap.twojastudio.pl");
        assert_eq!(studio.setup.imap_security, SmtpSecurity::Tls);
        assert_eq!(
            (
                studio.setup.smtp_host.as_str(),
                studio.setup.smtp_port,
                studio.setup.smtp_security
            ),
            ("smtp.twojastudio.pl", 587, SmtpSecurity::StartTls),
            "the identity's own outgoing server"
        );
        assert_eq!(studio.identities.len(), 2);
        assert_eq!(
            studio.identities[0].signature,
            "Anna Kowalska\nTwoja Studio łódź"
        );
        assert_eq!(studio.identities[1].email, "faktury@twojastudio.pl");
        assert!(studio.blocked.is_none());
        assert_eq!(source.accounts[1].blocked, Some(Blocked::Oauth));
        assert_eq!(source.accounts[2].blocked, Some(Blocked::Pop3));

        let folders: Vec<(&str, bool)> = source
            .folders
            .iter()
            .map(|f| (f.name.as_str(), f.tick))
            .collect();
        assert_eq!(
            folders,
            [
                ("Inbox", true),
                ("Archives / 2019", true),
                ("Trash", false),
                ("piotr@poczta.pl / Inbox", true),
            ],
            "{folders:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn moving_in_adds_accounts_mail_and_rules() {
        use std::sync::Mutex;
        let root = profile();
        let path = profiles(&root).remove(0);
        let en = crate::i18n::Lang::En;
        let source = crate::i18n::with(en, || read(&path, Program::Thunderbird, &[]).unwrap());
        let store = Mutex::new(crate::store::MailStore::in_memory().unwrap());
        {
            let mut s = store.lock().unwrap();
            let studio = &source.accounts[0];
            let account = s
                .add_found_account(&studio.setup.clone().cleaned().unwrap(), &studio.identities)
                .unwrap();
            assert_eq!(account.identities.len(), 2);
            assert!(account.identities[0].is_default);
            assert_eq!(account.identities[0].name, "Anna Kowalska");
            // Its folders, as a first check would list them.
            let folder = |id: &str, name: &str, kind| crate::model::Folder {
                id: id.into(),
                name: name.into(),
                kind,
                remote: Some(name.into()),
            };
            s.set_folders(
                &account.id,
                &[
                    folder("inbox", "Inbox", crate::model::FolderKind::Inbox),
                    folder("inbox-faktury", "Faktury", crate::model::FolderKind::Custom),
                ],
            )
            .unwrap();
        }
        // Tags first, as `move_in` brings them: the mail and the rules use them.
        let tag_items = crate::i18n::with(en, || {
            super::super::tags(&source, &mut store.lock().unwrap())
        });
        let statuses: Vec<(&str, Status)> = tag_items
            .iter()
            .map(|i| (i.name.as_str(), i.status))
            .collect();
        assert_eq!(
            statuses,
            [
                ("Important", Status::Same),
                ("Klienci", Status::Update),
                ("To do", Status::Same),
                ("Pilne", Status::Add),
                ("Projekt X", Status::Add),
            ]
        );
        {
            let s = store.lock().unwrap();
            let here = crate::i18n::with(en, || s.tags().unwrap());
            let named = |k: &str| here.iter().find(|t| t.keyword == k).unwrap().clone();
            assert_eq!(named("$label2").name, "Klienci");
            assert_eq!(named("projekt_x").color, "teal");
            assert_eq!(
                here.last().unwrap().keyword,
                "projekt_x",
                "Thunderbird's order"
            );
        }
        let keys: Vec<String> = source
            .folders
            .iter()
            .filter(|f| f.tick)
            .map(|f| f.key.clone())
            .collect();
        let report = super::super::import_mail(&store, &source, &keys, &|_| {}).unwrap();
        assert_eq!(report.added, 3, "{report:?}");
        let mut s = store.lock().unwrap();
        let lokalna = s.list("local/inbox").unwrap()[0].clone();
        assert_eq!(lokalna.subject, "lokalna");
        assert_eq!(
            s.keywords_of(lokalna.id).unwrap(),
            ["$label2", "projekt_x"],
            "X-Mozilla-Keys: its tags, not NonJunk"
        );
        let old = s.list("local/archives-2019").unwrap();
        assert_eq!(old.len(), 1, "the message Thunderbird deleted stays out");

        let items = super::super::rules(&source, &mut s);
        let by_name = |n: &str| items.iter().find(|i| i.name == n).unwrap();
        assert_eq!(by_name("Faktury").status, Status::Add);
        assert_eq!(by_name("Archiwum").status, Status::Add);
        assert_eq!(by_name("Wazne").status, Status::Add);
        assert_eq!(
            by_name("Priorytet").status,
            Status::Skip,
            "only an action Telex hasn't"
        );
        assert_eq!(by_name("Daty").status, Status::Skip, "a condition on dates");
        let rules = s.rules().unwrap();
        let faktury = rules.iter().find(|r| r.name == "Faktury").unwrap();
        assert_eq!(faktury.conditions[1].value, "faktura, nr");
        assert_eq!(
            faktury.actions,
            [
                crate::filters::Action::Move {
                    folder: "twojastudio/inbox-faktury".into()
                },
                crate::filters::Action::MarkRead
            ]
        );
        let archiwum = rules.iter().find(|r| r.name == "Archiwum").unwrap();
        assert!(!archiwum.enabled && !archiwum.match_all);
        assert_eq!(
            archiwum.actions,
            [crate::filters::Action::Move {
                folder: "local/archives-2019".into()
            }]
        );
        let wazne = rules.iter().find(|r| r.name == "Wazne").unwrap();
        assert!(!wazne.enabled, "a filter run only by hand");
        assert!(wazne.stop);

        use crate::filters::{Action, Condition, Field, Op};
        let tag = |op, value: &str| Condition {
            field: Field::Tag,
            op,
            value: value.into(),
            header: None,
        };
        let projekty = rules.iter().find(|r| r.name == "Projekty").unwrap();
        assert_eq!(
            projekty.conditions,
            [tag(Op::Is, "$label2"), tag(Op::NotContains, "pilne")]
        );
        assert_eq!(
            projekty.actions,
            [
                Action::AddTag {
                    tag: "projekt_x".into()
                },
                Action::RemoveTag {
                    tag: "$label4".into()
                }
            ]
        );
        let bare = rules.iter().find(|r| r.name == "Bez etykiet").unwrap();
        assert_eq!(bare.conditions, [tag(Op::NotExists, "")]);
        assert_eq!(
            bare.actions,
            [Action::AddTag {
                tag: "$label1".into()
            }],
            "an old Label action"
        );
        assert!(
            matches!(&by_name("Nieznana").skip, Some(Skip::Invalid { .. })),
            "a tag that isn't here: {:?}",
            by_name("Nieznana")
        );
        drop(s);
        // Again: nothing new.
        let mut s = store.lock().unwrap();
        let again = super::super::rules(&source, &mut s);
        assert!(again
            .iter()
            .all(|i| i.status == Status::Same || i.status == Status::Skip));
        let tags_again = crate::i18n::with(en, || super::super::tags(&source, &mut s));
        assert!(tags_again.iter().all(|i| i.status == Status::Same));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// An address book as Thunderbird 102+ keeps it.
    fn write_book(path: &Path, cards: &[(&str, &[(&str, &str)])], lists: &[(&str, &str, &[&str])]) {
        let db = rusqlite::Connection::open(path).unwrap();
        db.execute_batch(
            "CREATE TABLE properties (card TEXT, name TEXT, value TEXT);
             CREATE TABLE lists (uid TEXT PRIMARY KEY, name TEXT, nickName TEXT, description TEXT);
             CREATE TABLE list_cards (list TEXT, card TEXT, PRIMARY KEY(list, card));",
        )
        .unwrap();
        for (card, props) in cards {
            for (name, value) in *props {
                db.execute(
                    "INSERT INTO properties VALUES (?1, ?2, ?3)",
                    [card, name, value],
                )
                .unwrap();
            }
        }
        for (uid, name, members) in lists {
            db.execute("INSERT INTO lists VALUES (?1, ?2, '', '')", [uid, name])
                .unwrap();
            for m in *members {
                db.execute("INSERT INTO list_cards VALUES (?1, ?2)", [uid, m])
                    .unwrap();
            }
        }
    }

    #[test]
    fn address_books_come_with_their_lists() {
        let root = profile();
        let path = profiles(&root).remove(0);
        let vcard = "BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Zażółć Gęślą\r\nEMAIL;PREF=1:jazn@przyklad.pl\r\nTEL:600 100 200\r\nUID:c1\r\nEND:VCARD\r\n";
        write_book(
            &path.join("abook.sqlite"),
            &[
                (
                    "c1",
                    &[("_vCard", vcard), ("PrimaryEmail", "jazn@przyklad.pl")],
                ),
                // Before Thunderbird 102: properties only.
                (
                    "c2",
                    &[
                        ("FirstName", "Piotr"),
                        ("LastName", "Nowak"),
                        ("PrimaryEmail", "piotr@nowak-meble.pl"),
                        ("SecondEmail", "pn@gmail.com"),
                        ("Company", "Nowak Meble"),
                    ],
                ),
            ],
            &[("l1", "Klub książki", &["c1", "c2"])],
        );
        write_book(
            &path.join("history.sqlite"),
            &[("h1", &[("PrimaryEmail", "ktos@x.pl")])],
            &[],
        );
        let source = read(&path, Program::Thunderbird, &[]).unwrap();
        let books: Vec<(&str, &str, u64, bool)> = source
            .books
            .iter()
            .map(|b| (b.key.as_str(), b.name.as_str(), b.contacts, b.tick))
            .collect();
        assert_eq!(
            books,
            [
                ("b0", "Personal Address Book", 3, true),
                ("b1", "Collected Addresses", 1, false),
            ]
        );
        let mut store = crate::store::MailStore::in_memory().unwrap();
        let report = super::super::import_books(&mut store, &source, &["b0".to_owned()]).unwrap();
        assert_eq!((report.added, report.groups), (2, 1));
        let contacts = store.contacts().unwrap();
        let names: Vec<&str> = contacts.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Piotr Nowak", "Zażółć Gęślą"]);
        assert_eq!(contacts[0].emails, ["piotr@nowak-meble.pl", "pn@gmail.com"]);
        assert_eq!(contacts[0].organization, "Nowak Meble");
        assert_eq!(contacts[1].phone, "600 100 200");
        let groups = store.contact_groups().unwrap();
        assert_eq!(groups[0].name, "Klub książki");
        assert_eq!(groups[0].members.len(), 2);
        // Again: nothing twice.
        let again = super::super::import_books(&mut store, &source, &["b0".to_owned()]).unwrap();
        assert_eq!((again.added, again.same, again.groups), (0, 2, 0));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn prefs_strings_and_conditions_are_read_whole() {
        let prefs = Prefs::parse(
            "user_pref(\"a\", \"x \\\"y\\\" \\u0105\");\nuser_pref(\"b\", 42);\nuser_pref(\"c\", true);",
        );
        assert_eq!(prefs.text("a"), Some("x \"y\" ą"));
        assert_eq!(prefs.number("b"), Some(42));
        assert!(prefs.flag("c"));
        let terms =
            parse_terms("AND (subject,contains,\"a, (b)\") AND (\"x-spam\",is,yes)").unwrap();
        assert_eq!(terms[0].value, "a, (b)");
        assert_eq!(terms[1].field, "x-spam");
        assert!(parse_terms("ALL").is_none());
        assert_eq!(percent_decoded("Wys%C5%82ane%20rzeczy"), "Wysłane rzeczy");
    }
}
