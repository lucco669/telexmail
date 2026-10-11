//! Outlook keeps a message as MAPI properties, not as the text that went over the wire: a
//! subject, a sender, recipients in a table, a plain-text and an HTML body, attachments as
//! objects of their own. This builds an RFC 822 message from them, so mail from a .pst or a
//! .msg file is stored like any other. Received mail keeps the headers it came with
//! (PR_TRANSPORT_MESSAGE_HEADERS); mail written in Outlook gets headers made from its
//! properties.

use std::collections::HashMap;

use base64::Engine;
use chrono::{DateTime, Local, Utc};

use crate::mailbox::{crlf_lines, FileMessage};

/// A property's value, as far as mail needs it.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Text(String),
    /// Text in the message's code page.
    Ansi(Vec<u8>),
    Bytes(Vec<u8>),
    Int(i64),
    /// 100-nanosecond steps since 1601 (FILETIME).
    Time(i64),
    Bool(bool),
}

/// A bag of properties by id: a message's, a recipient's or an attachment's.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Props {
    values: HashMap<u16, Value>,
    /// For [`Value::Ansi`]: the code page its text is in.
    pub code_page: Option<u32>,
}

impl Props {
    pub fn insert(&mut self, id: u16, value: Value) {
        self.values.insert(id, value);
    }

    pub fn text(&self, id: u16) -> Option<String> {
        let text = match self.values.get(&id)? {
            Value::Text(t) => t.clone(),
            Value::Ansi(bytes) => decode(bytes, self.code_page),
            _ => return None,
        };
        let text = text.trim_end_matches('\0').trim().to_owned();
        (!text.is_empty()).then_some(text)
    }

    pub fn bytes(&self, id: u16) -> Option<&[u8]> {
        match self.values.get(&id)? {
            Value::Bytes(b) | Value::Ansi(b) => Some(b),
            _ => None,
        }
    }

    pub fn int(&self, id: u16) -> Option<i64> {
        match self.values.get(&id)? {
            Value::Int(n) => Some(*n),
            Value::Bool(b) => Some(i64::from(*b)),
            _ => None,
        }
    }

    pub fn time(&self, id: u16) -> Option<DateTime<Utc>> {
        match self.values.get(&id)? {
            Value::Time(ft) if *ft > 0 => {
                DateTime::from_timestamp(ft / 10_000_000 - 11_644_473_600, 0)
            }
            _ => None,
        }
    }
}

/// A message read from Outlook's files.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MapiMessage {
    pub props: Props,
    pub recipients: Vec<Props>,
    pub attachments: Vec<MapiAttachment>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MapiAttachment {
    pub props: Props,
    pub data: Vec<u8>,
    /// An attached message (a forwarded .msg).
    pub embedded: Option<Box<MapiMessage>>,
}

// Property ids (MS-OXPROPS).
pub const MESSAGE_CLASS: u16 = 0x001A;
pub const SUBJECT: u16 = 0x0037;
pub const CLIENT_SUBMIT_TIME: u16 = 0x0039;
pub const SENT_REPRESENTING_NAME: u16 = 0x0042;
pub const SENT_REPRESENTING_EMAIL: u16 = 0x0065;
pub const TRANSPORT_HEADERS: u16 = 0x007D;
pub const RECIPIENT_TYPE: u16 = 0x0C15;
pub const SENDER_NAME: u16 = 0x0C1A;
pub const SENDER_EMAIL: u16 = 0x0C1F;
pub const DELIVERY_TIME: u16 = 0x0E06;
pub const MESSAGE_FLAGS: u16 = 0x0E07;
pub const BODY: u16 = 0x1000;
pub const HTML: u16 = 0x1013;
pub const INTERNET_MESSAGE_ID: u16 = 0x1035;
pub const REFERENCES: u16 = 0x1039;
pub const IN_REPLY_TO: u16 = 0x1042;
pub const FLAG_STATUS: u16 = 0x1090;
pub const DISPLAY_NAME: u16 = 0x3001;
pub const EMAIL_ADDRESS: u16 = 0x3003;
pub const CREATION_TIME: u16 = 0x3007;
pub const ATTACH_DATA: u16 = 0x3701;
pub const ATTACH_FILENAME: u16 = 0x3704;
pub const ATTACH_LONG_FILENAME: u16 = 0x3707;
pub const ATTACH_MIME_TAG: u16 = 0x370E;
pub const ATTACH_CONTENT_ID: u16 = 0x3712;
pub const SMTP_ADDRESS: u16 = 0x39FE;
pub const INTERNET_CPID: u16 = 0x3FDE;
pub const MESSAGE_CODEPAGE: u16 = 0x3FFD;
pub const SENDER_SMTP_ADDRESS: u16 = 0x5D01;
pub const SENT_REPRESENTING_SMTP_ADDRESS: u16 = 0x5D02;

/// Message classes that are mail: notes, meeting requests (mail with an invitation), reports
/// such as bounces, and posts. Not contacts, appointments, tasks or notes.
pub fn is_mail(class: Option<&str>) -> bool {
    let class = class.unwrap_or("IPM.Note").to_ascii_lowercase();
    ["ipm.note", "ipm.schedule.meeting", "report.", "ipm.post"]
        .iter()
        .any(|c| class.starts_with(c))
}

/// Text in a Windows code page, through mail-parser's decoders.
fn decode(bytes: &[u8], code_page: Option<u32>) -> String {
    let name = match code_page.unwrap_or(1252) {
        65001 | 1200 => return String::from_utf8_lossy(bytes).into_owned(),
        20127 => "us-ascii",
        28591 => "iso-8859-1",
        28592 => "iso-8859-2",
        28605 => "iso-8859-15",
        932 => "shift_jis",
        936 => "gb2312",
        949 => "euc-kr",
        950 => "big5",
        n @ 1250..=1258 => return decode_named(bytes, &format!("windows-{n}")),
        _ => "windows-1252",
    };
    decode_named(bytes, name)
}

fn decode_named(bytes: &[u8], charset: &str) -> String {
    match mail_parser::decoders::charsets::map::charset_decoder(charset.as_bytes()) {
        Some(decoder) => decoder(bytes),
        None => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// The message as an RFC 822 file message, with its read state and flag.
pub fn file_message(m: &MapiMessage) -> FileMessage {
    let date = m
        .props
        .time(DELIVERY_TIME)
        .or_else(|| m.props.time(CLIENT_SUBMIT_TIME))
        .or_else(|| m.props.time(CREATION_TIME))
        .map(|d| d.with_timezone(&Local).naive_local());
    FileMessage {
        raw: to_rfc822(m),
        seen: m.props.int(MESSAGE_FLAGS).is_some_and(|f| f & 1 != 0),
        flagged: m.props.int(FLAG_STATUS) == Some(2),
        date,
    }
}

/// The subject without the marker some stores put before a prefix ("\u{1}\u{4}RE: ").
fn subject(props: &Props) -> String {
    let s = props.text(SUBJECT).unwrap_or_default();
    match s.strip_prefix('\u{1}') {
        Some(rest) => rest.chars().skip(1).collect(),
        None => s,
    }
}

/// Headers Telex writes itself: the MIME structure is new.
fn is_mime_header(line: &str) -> bool {
    let name = line.split(':').next().unwrap_or_default().trim();
    [
        "content-type",
        "content-transfer-encoding",
        "mime-version",
        "content-disposition",
    ]
    .iter()
    .any(|h| name.eq_ignore_ascii_case(h))
}

/// The headers the message came with, without those about its MIME structure.
fn kept_headers(headers: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for line in headers.replace("\r\n", "\n").lines() {
        if line.trim().is_empty() {
            break;
        }
        if line.starts_with([' ', '\t']) {
            if !skipping {
                out.push_str(line);
                out.push_str("\r\n");
            }
            continue;
        }
        skipping = is_mime_header(line);
        if !skipping {
            out.push_str(line);
            out.push_str("\r\n");
        }
    }
    out
}

/// A header value in RFC 2047 words when it isn't plain ASCII.
fn encoded(text: &str) -> String {
    if text.is_ascii() && !text.contains("=?") {
        return text.to_owned();
    }
    let mut words = Vec::new();
    let mut chunk = String::new();
    for c in text.chars() {
        if chunk.len() + c.len_utf8() > 45 {
            words.push(std::mem::take(&mut chunk));
        }
        chunk.push(c);
    }
    words.push(chunk);
    words
        .iter()
        .map(|w| {
            format!(
                "=?UTF-8?B?{}?=",
                base64::engine::general_purpose::STANDARD.encode(w)
            )
        })
        .collect::<Vec<_>>()
        .join("\r\n ")
}

/// "Anna Kowalska <anna@x.pl>"; the name alone when Outlook kept no Internet address
/// (an Exchange address book entry).
fn address(name: Option<String>, email: Option<String>) -> Option<String> {
    let email = email.filter(|e| e.contains('@') && !e.starts_with('/'));
    let name = name.filter(|n| Some(n) != email.as_ref());
    match (name, email) {
        (Some(n), Some(e)) if n.is_ascii() => {
            Some(format!("\"{}\" <{e}>", n.replace(['"', '\\'], "")))
        }
        (Some(n), Some(e)) => Some(format!("{} <{e}>", encoded(&n))),
        (None, Some(e)) => Some(format!("<{e}>")),
        (Some(n), None) => Some(encoded(&n)),
        (None, None) => None,
    }
}

fn recipients(m: &MapiMessage, kind: i64) -> Option<String> {
    let list: Vec<String> = m
        .recipients
        .iter()
        .filter(|r| r.int(RECIPIENT_TYPE).unwrap_or(1) & 0xF == kind)
        .filter_map(|r| {
            address(
                r.text(DISPLAY_NAME),
                r.text(SMTP_ADDRESS).or_else(|| r.text(EMAIL_ADDRESS)),
            )
        })
        .collect();
    (!list.is_empty()).then(|| list.join(",\r\n "))
}

/// Headers made from the properties, for mail written in Outlook (it has no transport
/// headers).
fn made_headers(m: &MapiMessage) -> String {
    let p = &m.props;
    let mut h = String::new();
    let mut add = |name: &str, value: Option<String>| {
        if let Some(v) = value {
            h.push_str(&format!("{name}: {v}\r\n"));
        }
    };
    let date = p
        .time(CLIENT_SUBMIT_TIME)
        .or_else(|| p.time(DELIVERY_TIME))
        .or_else(|| p.time(CREATION_TIME));
    add("Date", date.map(|d| d.to_rfc2822()));
    let from = address(
        p.text(SENT_REPRESENTING_NAME)
            .or_else(|| p.text(SENDER_NAME)),
        p.text(SENT_REPRESENTING_SMTP_ADDRESS)
            .or_else(|| p.text(SENDER_SMTP_ADDRESS))
            .or_else(|| p.text(SENT_REPRESENTING_EMAIL))
            .or_else(|| p.text(SENDER_EMAIL)),
    );
    add("From", from);
    add("To", recipients(m, 1));
    add("Cc", recipients(m, 2));
    add("Bcc", recipients(m, 3));
    add("Subject", Some(encoded(&subject(p))));
    add("Message-ID", p.text(INTERNET_MESSAGE_ID));
    add("In-Reply-To", p.text(IN_REPLY_TO));
    add("References", p.text(REFERENCES));
    h
}

/// A part's lines of base64.
fn base64_lines(data: &[u8]) -> String {
    let text = base64::engine::general_purpose::STANDARD.encode(data);
    let mut out = String::with_capacity(text.len() + text.len() / 76 * 2 + 2);
    for chunk in text.as_bytes().chunks(76) {
        out.push_str(std::str::from_utf8(chunk).unwrap_or_default());
        out.push_str("\r\n");
    }
    out
}

/// One MIME part: headers, a blank line, the body.
struct Part {
    headers: String,
    body: Vec<u8>,
}

impl Part {
    fn text(kind: &str, text: &str) -> Self {
        Self {
            headers: format!(
                "Content-Type: {kind}; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n"
            ),
            body: base64_lines(text.as_bytes()).into_bytes(),
        }
    }

    fn multipart(kind: &str, parts: Vec<Part>, n: &mut u32) -> Self {
        *n += 1;
        let boundary = format!("=_telex_{n}_{:x}", parts.len() * 7919 + *n as usize);
        let mut body = Vec::new();
        for part in parts {
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            body.extend_from_slice(part.headers.as_bytes());
            body.extend_from_slice(b"\r\n");
            body.extend_from_slice(&part.body);
        }
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        Self {
            headers: format!("Content-Type: multipart/{kind}; boundary=\"{boundary}\"\r\n"),
            body,
        }
    }

    fn file(a: &MapiAttachment, n: &mut u32) -> Self {
        let p = &a.props;
        if let Some(inner) = &a.embedded {
            let name = p
                .text(DISPLAY_NAME)
                .unwrap_or_else(|| subject(&inner.props));
            let _ = n;
            return Self {
                headers: format!(
                    "Content-Type: message/rfc822\r\nContent-Disposition: attachment; filename=\"{}.eml\"\r\n",
                    encoded(&name).replace('"', "")
                ),
                body: to_rfc822(inner),
            };
        }
        let name = p
            .text(ATTACH_LONG_FILENAME)
            .or_else(|| p.text(ATTACH_FILENAME))
            .or_else(|| p.text(DISPLAY_NAME))
            .unwrap_or_else(|| "attachment".to_owned());
        let kind = p
            .text(ATTACH_MIME_TAG)
            .unwrap_or_else(|| crate::mime::content_type_for(&name).to_owned());
        let mut headers = format!(
            "Content-Type: {kind}; name=\"{0}\"\r\nContent-Transfer-Encoding: base64\r\n",
            encoded(&name).replace('"', "")
        );
        match p.text(ATTACH_CONTENT_ID) {
            Some(cid) => headers.push_str(&format!(
                "Content-ID: <{}>\r\nContent-Disposition: inline; filename=\"{}\"\r\n",
                cid.trim_matches(['<', '>']),
                encoded(&name).replace('"', "")
            )),
            None => headers.push_str(&format!(
                "Content-Disposition: attachment; filename=\"{}\"\r\n",
                encoded(&name).replace('"', "")
            )),
        }
        Self {
            headers,
            body: base64_lines(&a.data).into_bytes(),
        }
    }
}

/// The HTML body in its own code page (PR_INTERNET_CPID), as text.
fn html_body(p: &Props) -> Option<String> {
    let bytes = p.bytes(HTML)?;
    let cpid = p.int(INTERNET_CPID).and_then(|n| u32::try_from(n).ok());
    let html = match cpid {
        Some(cp) => decode(bytes, Some(cp)),
        None => match std::str::from_utf8(bytes) {
            Ok(t) => t.to_owned(),
            Err(_) => decode(bytes, p.code_page),
        },
    };
    let html = html.trim_end_matches('\0').to_owned();
    (!html.trim().is_empty()).then_some(html)
}

/// The whole message as RFC 822, CRLF line ends.
pub fn to_rfc822(m: &MapiMessage) -> Vec<u8> {
    let p = &m.props;
    let mut headers = match p.text(TRANSPORT_HEADERS) {
        Some(h) if h.contains(':') => kept_headers(&h),
        _ => made_headers(m),
    };
    headers.push_str("MIME-Version: 1.0\r\n");
    let html = html_body(p);
    let text = p
        .text(BODY)
        .or_else(|| html.as_deref().map(crate::html::to_text))
        .unwrap_or_default();
    let mut n = 0;
    let mut body = match &html {
        Some(html) => Part::multipart(
            "alternative",
            vec![
                Part::text("text/plain", &text),
                Part::text("text/html", html),
            ],
            &mut n,
        ),
        None => Part::text("text/plain", &text),
    };
    // Pictures the HTML shows (cid:) go with it; the rest are attachments.
    let (inline, files): (Vec<&MapiAttachment>, Vec<&MapiAttachment>) =
        m.attachments.iter().partition(|a| {
            let cid = a.props.text(ATTACH_CONTENT_ID);
            a.embedded.is_none()
                && html.as_deref().zip(cid.as_deref()).is_some_and(|(h, c)| {
                    h.contains(&format!("cid:{}", c.trim_matches(['<', '>'])))
                })
        });
    if !inline.is_empty() {
        let mut parts = vec![body];
        parts.extend(inline.into_iter().map(|a| Part::file(a, &mut n)));
        body = Part::multipart("related", parts, &mut n);
    }
    if !files.is_empty() {
        let mut parts = vec![body];
        for a in files {
            let mut part = Part::file(a, &mut n);
            // A file named in an inline Content-ID the HTML doesn't use is an attachment.
            part.headers = part.headers.replace(
                "Content-Disposition: inline",
                "Content-Disposition: attachment",
            );
            parts.push(part);
        }
        body = Part::multipart("mixed", parts, &mut n);
    }
    let mut raw = headers.into_bytes();
    raw.extend_from_slice(body.headers.as_bytes());
    raw.extend_from_slice(b"\r\n");
    raw.extend_from_slice(&body.body);
    crlf_lines(&raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mime::parse_message;

    fn text(s: &str) -> Value {
        Value::Text(s.to_owned())
    }

    #[test]
    fn mail_written_in_outlook_gets_headers_from_its_properties() {
        let mut m = MapiMessage::default();
        m.props.insert(SUBJECT, text("\u{1}\u{4}RE: Wycena łóżka"));
        m.props.insert(SENDER_NAME, text("Anna Kowalska"));
        m.props
            .insert(SENDER_EMAIL, text("/O=FIRMA/OU=EXCHANGE/CN=ANNA"));
        m.props.insert(SENDER_SMTP_ADDRESS, text("anna@firma.pl"));
        m.props
            .insert(CLIENT_SUBMIT_TIME, Value::Time(134_042_256_000_000_000));
        m.props.insert(INTERNET_MESSAGE_ID, text("<w1@firma.pl>"));
        m.props
            .insert(BODY, text("Dzień dobry,\r\nw załączniku wycena.\r\n"));
        m.props.insert(MESSAGE_FLAGS, Value::Int(1));
        m.props.insert(FLAG_STATUS, Value::Int(2));
        let mut to = Props::default();
        to.insert(RECIPIENT_TYPE, Value::Int(1));
        to.insert(DISPLAY_NAME, text("Piotr Nowak"));
        to.insert(SMTP_ADDRESS, text("piotr@nowak-meble.pl"));
        let mut cc = Props::default();
        cc.insert(RECIPIENT_TYPE, Value::Int(2));
        cc.insert(DISPLAY_NAME, text("Biuro"));
        cc.insert(EMAIL_ADDRESS, text("biuro@firma.pl"));
        m.recipients = vec![to, cc];
        let mut file = MapiAttachment::default();
        file.props.insert(ATTACH_LONG_FILENAME, text("wycena.pdf"));
        file.data = b"%PDF-1.7".to_vec();
        m.attachments.push(file);

        let f = file_message(&m);
        assert!(f.seen && f.flagged);
        let parsed = parse_message(&f.raw, "local/inbox", None);
        assert_eq!(parsed.subject, "RE: Wycena łóżka", "the marker is gone");
        assert_eq!(
            parsed.from_email, "anna@firma.pl",
            "the Internet address, not Exchange's"
        );
        assert_eq!(parsed.from_name, "Anna Kowalska");
        assert!(parsed.to.contains("piotr@nowak-meble.pl"));
        assert!(parsed.cc.contains("biuro@firma.pl"));
        assert_eq!(parsed.message_id.as_deref(), Some("w1@firma.pl"));
        assert!(parsed.body.contains("w załączniku wycena"));
        assert_eq!(parsed.attachments.len(), 1);
        assert_eq!(parsed.attachments[0].name, "wycena.pdf");
        let raw = String::from_utf8(f.raw).unwrap();
        assert!(
            !raw.contains("\r\r") && !raw.replace("\r\n", "").contains('\n'),
            "CRLF only"
        );
    }

    #[test]
    fn received_mail_keeps_its_headers_and_html_with_pictures() {
        let mut m = MapiMessage::default();
        m.props.insert(
            TRANSPORT_HEADERS,
            text("Received: from mx.x.pl\r\n\tby mx.firma.pl\r\nFrom: Sklep <sklep@x.pl>\r\nTo: anna@firma.pl\r\nSubject: =?UTF-8?Q?Zam=C3=B3wienie?=\r\nMessage-ID: <z1@x.pl>\r\nDate: Fri, 09 Oct 2026 14:30:00 +0200\r\nMIME-Version: 1.0\r\nContent-Type: multipart/related;\r\n\tboundary=\"old\"\r\n\r\n"),
        );
        m.props.insert(
            HTML,
            Value::Bytes(
                "<p>Dziękujemy <img src=\"cid:logo@x\"></p>"
                    .as_bytes()
                    .to_vec(),
            ),
        );
        m.props.insert(INTERNET_CPID, Value::Int(65001));
        let mut logo = MapiAttachment::default();
        logo.props.insert(ATTACH_LONG_FILENAME, text("logo.png"));
        logo.props.insert(ATTACH_MIME_TAG, text("image/png"));
        logo.props.insert(ATTACH_CONTENT_ID, text("logo@x"));
        logo.data = vec![0x89, b'P', b'N', b'G'];
        m.attachments.push(logo);

        let raw = to_rfc822(&m);
        let text = String::from_utf8(raw.clone()).unwrap();
        assert!(text.starts_with("Received: from mx.x.pl\r\n\tby mx.firma.pl\r\n"));
        assert!(
            !text.contains("boundary=\"old\""),
            "the old structure's header is gone"
        );
        assert_eq!(text.matches("MIME-Version").count(), 1);
        let parsed = parse_message(&raw, "local/inbox", None);
        assert_eq!(parsed.subject, "Zamówienie");
        assert_eq!(
            parsed.body.trim(),
            "Dziękujemy",
            "plain text made from the HTML"
        );
        let html = crate::mime::html_body(&raw).unwrap();
        assert!(html.html.contains("cid:logo@x"));
        assert!(
            text.contains("multipart/related") && text.contains("Content-ID: <logo@x>"),
            "the picture goes with the HTML"
        );
    }

    #[test]
    fn old_code_pages_decode() {
        let mut p = Props {
            code_page: Some(1250),
            ..Props::default()
        };
        p.insert(SUBJECT, Value::Ansi(vec![0xA3, 0xF3, 0x9F, 0x62]));
        assert_eq!(p.text(SUBJECT).as_deref(), Some("Łóźb"));
        assert!(is_mail(Some("IPM.Note.SMIME")) && is_mail(None));
        assert!(!is_mail(Some("IPM.Contact")) && !is_mail(Some("IPM.Appointment")));
    }
}
