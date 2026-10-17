//! Turning a raw RFC 822 message from the server into a [`Message`].
//!
//! `mail-parser` does the hard part: MIME structure, transfer encodings and old
//! charsets such as Windows-1250 and ISO-8859-2.

use std::collections::HashMap;

use chrono::{DateTime, Local, NaiveDateTime};
use mail_parser::{Address, MessageParser, MessagePart, MimeHeaders, PartType};

use crate::model::{Attachment, Message};
use crate::tr;

/// An attachment's contents, for forwarding it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileAttachment {
    pub name: String,
    /// "application/pdf"
    pub content_type: String,
    pub data: Vec<u8>,
}

/// Parse `raw` into a message filed in `folder`. `fallback_date` (the server's
/// INTERNALDATE) is used when the Date header is missing or unreadable.
pub fn parse_message(raw: &[u8], folder: &str, fallback_date: Option<NaiveDateTime>) -> Message {
    let size_kb = kb(raw.len());
    let Some(p) = MessageParser::default().parse(raw) else {
        return Message {
            id: 0,
            folder: folder.to_owned(),
            from_name: String::new(),
            from_email: String::new(),
            to: String::new(),
            cc: String::new(),
            subject: tr!("(unreadable message)", "(nieczytelna wiadomość)"),
            date: fallback_date.unwrap_or_else(|| Local::now().naive_local()),
            size_kb,
            unread: true,
            flagged: false,
            remote_images: false,
            note: Some(tr!(
                "Telex couldn't read this message's structure.",
                "Telex nie mógł odczytać struktury tej wiadomości."
            )),
            attachments: Vec::new(),
            body: String::from_utf8_lossy(raw).into_owned(),
            message_id: None,
            bcc: String::new(),
            refs: Vec::new(),
        };
    };

    let (from_name, from_email) = p
        .from()
        .and_then(Address::first)
        .map(|a| {
            let email = a.address().unwrap_or_default().to_owned();
            let name = a.name().map(str::trim).unwrap_or_default();
            (
                if name.is_empty() {
                    email.clone()
                } else {
                    name.to_owned()
                },
                email,
            )
        })
        .unwrap_or_default();

    let date = p
        .date()
        .filter(|d| d.is_valid())
        .and_then(|d| DateTime::from_timestamp(d.to_timestamp(), 0))
        .map(|utc| utc.with_timezone(&Local).naive_local())
        .or(fallback_date)
        .unwrap_or_else(|| Local::now().naive_local());

    let body = (0..p.text_body_count())
        .filter_map(|i| p.body_text(i))
        .collect::<Vec<_>>()
        .join("\n")
        .replace("\r\n", "\n");

    let remote_images = p.html_bodies().any(|part| match &part.body {
        PartType::Html(html) => references_remote_content(html),
        _ => false,
    });

    let attachments = files(&p)
        .map(|a| Attachment {
            name: file_name(a),
            size_kb: kb(a.contents().len()),
            path: None,
        })
        .collect();

    let subject = match p.subject().map(str::trim) {
        Some(s) if !s.is_empty() => s.to_owned(),
        _ => tr!("(no subject)", "(bez tematu)"),
    };

    Message {
        id: 0,
        folder: folder.to_owned(),
        from_name,
        from_email,
        to: format_addresses(p.to()),
        cc: format_addresses(p.cc()),
        bcc: format_addresses(p.bcc()),
        subject,
        date,
        size_kb,
        unread: true,
        flagged: false,
        remote_images,
        note: None,
        attachments,
        body,
        message_id: p.message_id().map(str::to_owned),
        refs: answered_ids(&p),
    }
}

/// The Message-IDs a message answers: In-Reply-To, then References, each once.
fn answered_ids(p: &mail_parser::Message<'_>) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for header in [p.in_reply_to(), p.references()] {
        let found = match (header.as_text_list(), header.as_text()) {
            (Some(list), _) => list.iter().map(|r| r.to_string()).collect(),
            (None, Some(one)) => vec![one.to_owned()],
            (None, None) => Vec::new(),
        };
        for id in found {
            let id = id.trim().trim_matches(['<', '>']).to_owned();
            if !id.is_empty() && !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    ids
}

/// Just the Message-IDs a message answers, without parsing the body: for mail stored before
/// Telex kept them.
pub fn answered_ids_of(raw: &[u8]) -> Vec<String> {
    MessageParser::default()
        .parse_headers(raw)
        .map(|p| answered_ids(&p))
        .unwrap_or_default()
}

/// Just the Message-ID header, without parsing the body.
pub fn parse_message_id(raw: &[u8]) -> Option<String> {
    MessageParser::default()
        .parse_headers(raw)?
        .message_id()
        .map(str::to_owned)
}

/// Why an automatic reply must not answer this message (RFC 3834), or `None`: it was
/// itself sent by a robot, it is bulk mail, or it came from a mailing list.
pub fn auto_reply_blocker(raw: &[u8]) -> Option<&'static str> {
    let Some(p) = MessageParser::default().parse_headers(raw) else {
        return Some("its headers can't be read");
    };
    let header = |name: &str| p.header_raw(name).map(|v| v.trim().to_ascii_lowercase());
    if header("Auto-Submitted").is_some_and(|v| v != "no") {
        Some("it was sent automatically")
    } else if header("Precedence").is_some_and(|v| matches!(v.as_str(), "bulk" | "list" | "junk")) {
        Some("it is bulk mail")
    } else if header("List-Id").is_some() || header("List-Unsubscribe").is_some() {
        Some("it came from a mailing list")
    } else if header("X-Autoreply").is_some() || header("X-Autorespond").is_some() {
        Some("it is an automatic reply")
    } else {
        None
    }
}

/// Where replies should go when the sender asks for that (a Reply-To header), written
/// like a To line; `None` when there is no such header.
pub fn reply_to_of(raw: &[u8]) -> Option<String> {
    let p = MessageParser::default().parse_headers(raw)?;
    let to = format_addresses(p.reply_to());
    (!to.is_empty()).then_some(to)
}

/// The Message-ID and References of a message (without angle brackets), so a reply can
/// say which conversation it belongs to.
pub fn thread_ids(raw: &[u8]) -> (Option<String>, Vec<String>) {
    let Some(p) = MessageParser::default().parse_headers(raw) else {
        return (None, Vec::new());
    };
    let references = match (p.references().as_text_list(), p.references().as_text()) {
        (Some(list), _) => list.iter().map(|r| r.to_string()).collect(),
        (None, Some(one)) => vec![one.to_owned()],
        (None, None) => Vec::new(),
    };
    (p.message_id().map(str::to_owned), references)
}

/// The attachments of a message, with their contents, for forwarding.
pub fn attachments_of(raw: &[u8]) -> Vec<FileAttachment> {
    let Some(p) = MessageParser::default().parse(raw) else {
        return Vec::new();
    };
    files(&p).map(file_attachment).collect()
}

/// One attachment, by its place in the message's list (as [`parse_message`] lists them).
pub fn attachment_at(raw: &[u8], index: usize) -> Option<FileAttachment> {
    let p = MessageParser::default().parse(raw)?;
    let found = files(&p).nth(index).map(file_attachment);
    found
}

/// A message's files: its attachments, without the pictures its HTML shows in the text
/// (`cid:`, as in multipart/related), which belong to the text, not to the files.
pub(crate) fn files<'a>(
    p: &'a mail_parser::Message<'a>,
) -> impl Iterator<Item = &'a MessagePart<'a>> + 'a {
    let html = p
        .html_bodies()
        .filter_map(|part| match &part.body {
            PartType::Html(html) => Some(html.to_ascii_lowercase()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(
            "
",
        );
    p.attachments()
        .filter(move |part| !in_the_text(part, &html))
}

fn in_the_text(part: &MessagePart<'_>, html: &str) -> bool {
    if part
        .content_disposition()
        .is_some_and(|d| d.is_attachment())
    {
        return false;
    }
    let Some(cid) = part.content_id() else {
        return false;
    };
    let cid = cid.trim().trim_start_matches('<').trim_end_matches('>');
    !cid.is_empty()
        && mime_type(part).starts_with("image/")
        && html.contains(&format!("cid:{}", cid.to_ascii_lowercase()))
}

fn file_attachment(part: &MessagePart<'_>) -> FileAttachment {
    FileAttachment {
        name: file_name(part),
        content_type: mime_type(part),
        data: part.contents().to_vec(),
    }
}

/// The MIME type a file of this name usually has, for attaching it: "faktura.PDF" →
/// "application/pdf". Unknown kinds are application/octet-stream, which every mail program
/// accepts.
pub fn content_type_for(name: &str) -> &'static str {
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "pdf" => "application/pdf",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "heic" => "image/heic",
        "txt" | "log" => "text/plain",
        "csv" => "text/csv",
        "htm" | "html" => "text/html",
        "ics" => "text/calendar",
        "vcf" => "text/vcard",
        "eml" => "message/rfc822",
        "zip" => "application/zip",
        "7z" => "application/x-7z-compressed",
        "rar" => "application/vnd.rar",
        "gz" => "application/gzip",
        "json" => "application/json",
        "xml" => "application/xml",
        "rtf" => "application/rtf",
        "doc" => "application/msword",
        "xls" => "application/vnd.ms-excel",
        "ppt" => "application/vnd.ms-powerpoint",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "odt" => "application/vnd.oasis.opendocument.text",
        "ods" => "application/vnd.oasis.opendocument.spreadsheet",
        "odp" => "application/vnd.oasis.opendocument.presentation",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        _ => "application/octet-stream",
    }
}

/// A message's HTML body, with the images it embeds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HtmlBody {
    pub html: String,
    /// Content-ID (without angle brackets) → MIME type and bytes, for `cid:` references.
    pub inline: HashMap<String, (String, Vec<u8>)>,
}

/// The HTML body, or `None` for plain-text mail.
pub fn html_body(raw: &[u8]) -> Option<HtmlBody> {
    let p = MessageParser::default().parse(raw)?;
    let html = p.html_bodies().find_map(|part| match &part.body {
        PartType::Html(html) => Some(html.to_string()),
        _ => None,
    })?;
    let inline = p
        .parts
        .iter()
        .filter_map(|part| {
            let cid = part.content_id()?.trim();
            let cid = cid.strip_prefix('<').unwrap_or(cid);
            let cid = cid.strip_suffix('>').unwrap_or(cid);
            Some((cid.to_owned(), (mime_type(part), part.contents().to_vec())))
        })
        .collect();
    Some(HtmlBody { html, inline })
}

/// The HTML of mail that has no plain-text part of its own, whose text is HTML turned into
/// text; `None` when the sender wrote a plain-text part too, or no HTML.
pub fn html_only(raw: &[u8]) -> Option<String> {
    let p = MessageParser::default().parse(raw)?;
    if p.text_bodies()
        .any(|part| matches!(part.body, PartType::Text(_)))
    {
        return None;
    }
    let html = p.html_bodies().find_map(|part| match &part.body {
        PartType::Html(html) => Some(html.to_string()),
        _ => None,
    });
    html
}

/// "image/png", or application/octet-stream when the part doesn't say.
fn mime_type(part: &MessagePart<'_>) -> String {
    part.content_type().map_or_else(
        || "application/octet-stream".to_owned(),
        |ct| match ct.subtype() {
            Some(sub) => format!("{}/{sub}", ct.ctype()).to_ascii_lowercase(),
            None => ct.ctype().to_ascii_lowercase(),
        },
    )
}

fn file_name(part: &MessagePart<'_>) -> String {
    part.attachment_name().map_or_else(
        || {
            if part.is_message() {
                "message.eml"
            } else {
                "attachment"
            }
            .to_owned()
        },
        str::to_owned,
    )
}

/// "Anna Kowalska <anna@x.pl>, piotr@y.pl", the way the compose pane writes recipients.
fn format_addresses(addresses: Option<&Address<'_>>) -> String {
    let Some(addresses) = addresses else {
        return String::new();
    };
    addresses
        .iter()
        .filter_map(|a| {
            let name = a.name().map(str::trim).filter(|n| !n.is_empty());
            match (name, a.address()) {
                (Some(n), Some(e)) if n.contains([',', ';', '"']) => {
                    Some(format!("\"{}\" <{e}>", n.replace('"', "'")))
                }
                (Some(n), Some(e)) => Some(format!("{n} <{e}>")),
                (None, Some(e)) => Some(e.to_owned()),
                (Some(n), None) => Some(n.to_owned()),
                (None, None) => None,
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Does the HTML load anything from the network (tracking pixels, remote images)?
fn references_remote_content(html: &str) -> bool {
    let html = html.to_ascii_lowercase();
    [
        "src=\"http",
        "src='http",
        "src=http",
        "background=\"http",
        "url(http",
        "url(\"http",
        "url('http",
    ]
    .iter()
    .any(|p| html.contains(p))
}

fn kb(bytes: usize) -> u32 {
    u32::try_from(bytes.div_ceil(1024))
        .unwrap_or(u32::MAX)
        .max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, TimeZone, Utc};

    fn local(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> NaiveDateTime {
        Utc.from_utc_datetime(
            &NaiveDate::from_ymd_opt(y, mo, d)
                .unwrap()
                .and_hms_opt(h, mi, 0)
                .unwrap(),
        )
        .with_timezone(&Local)
        .naive_local()
    }

    #[test]
    fn windows_1250_mail_decodes() {
        // Body in Windows-1250, subject and sender name as ISO-8859-2 encoded words.
        let raw = b"From: =?ISO-8859-2?Q?Pawe=B3_Mazur?= <pawel@mazur-ksiegowosc.pl>\r\n\
To: Studio <biuro@twojastudio.pl>, \"Kowalska, Anna\" <anna@x.pl>\r\n\
Subject: =?ISO-8859-2?Q?Faktura_za_pa=BCdziernik?=\r\n\
Date: Fri, 09 Oct 2026 14:30:00 +0200\r\n\
MIME-Version: 1.0\r\n\
Content-Type: text/plain; charset=windows-1250\r\n\
Content-Transfer-Encoding: 8bit\r\n\
\r\n\
Dzie\xf1 dobry,\r\n\
za\xbf\xf3\xb3\xe6 g\xea\x9cl\xb9 ja\x9f\xf1.\r\n";
        let m = parse_message(raw, "firma/inbox", None);
        assert_eq!(m.folder, "firma/inbox");
        assert_eq!(m.from_name, "Paweł Mazur");
        assert_eq!(m.from_email, "pawel@mazur-ksiegowosc.pl");
        assert_eq!(m.subject, "Faktura za październik");
        assert_eq!(
            m.to,
            "Studio <biuro@twojastudio.pl>, \"Kowalska, Anna\" <anna@x.pl>"
        );
        assert_eq!(m.body, "Dzień dobry,\nzażółć gęślą jaźń.\n");
        assert_eq!(m.date, local(2026, 10, 9, 12, 30));
        assert!(m.attachments.is_empty());
        assert!(!m.remote_images);
    }

    #[test]
    fn html_only_mail_gets_a_text_body_and_flags_remote_images() {
        let raw = b"From: newsletter@sklep.pl\r\n\
Subject: Promocja\r\n\
Content-Type: text/html; charset=iso-8859-2\r\n\
Content-Transfer-Encoding: quoted-printable\r\n\
\r\n\
<html><body><p>Rabat na =BFaluzje</p><img src=3D\"https://track.sklep.pl/p.gif\"></body></html>\r\n";
        let m = parse_message(raw, "firma/inbox", None);
        assert_eq!(
            m.from_name, "newsletter@sklep.pl",
            "no name: show the address"
        );
        assert!(m.body.contains("Rabat na żaluzje"), "{:?}", m.body);
        assert!(!m.body.contains("<p>"));
        assert!(m.remote_images);
    }

    #[test]
    fn attachments_are_listed() {
        let raw = b"From: Anna <anna@x.pl>\r\n\
Subject: Makiety\r\n\
MIME-Version: 1.0\r\n\
Content-Type: multipart/mixed; boundary=\"b1\"\r\n\
\r\n\
--b1\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
\r\n\
W za\xc5\x82\xc4\x85czniku makiety.\r\n\
--b1\r\n\
Content-Type: application/pdf\r\n\
Content-Disposition: attachment; filename*=utf-8''makiety%20v3%20%C5%BC%C3%B3%C5%82te.pdf\r\n\
Content-Transfer-Encoding: base64\r\n\
\r\n\
JVBERi0xLjQK\r\n\
--b1--\r\n";
        let m = parse_message(raw, "firma/inbox", None);
        assert_eq!(m.body.trim(), "W załączniku makiety.");
        assert_eq!(m.attachments.len(), 1);
        assert_eq!(m.attachments[0].name, "makiety v3 żółte.pdf");
        assert_eq!(m.attachments[0].size_kb, 1);
    }

    #[test]
    fn message_id_is_read_with_and_without_the_body() {
        let raw = b"Message-ID: <abc.123@x.pl>\r\nSubject: Hej\r\n\r\nTre\xc5\x9b\xc4\x87\r\n";
        assert_eq!(
            parse_message(raw, "x/inbox", None).message_id.as_deref(),
            Some("abc.123@x.pl")
        );
        assert_eq!(parse_message_id(raw).as_deref(), Some("abc.123@x.pl"));
        assert_eq!(parse_message_id(b"Subject: none\r\n\r\n"), None);
    }

    #[test]
    fn replies_know_what_they_answer_and_drafts_keep_blind_copies() {
        let raw = b"Message-ID: <c@x.pl>\r\nIn-Reply-To: <b@x.pl>\r\n\
References: <a@x.pl>\r\n <b@x.pl>\r\nBcc: Szef <szef@x.pl>\r\nSubject: Re: Re: Wycena\r\n\r\nOK\r\n";
        let m = parse_message(raw, "x/sent", None);
        assert_eq!(m.refs, ["b@x.pl", "a@x.pl"], "In-Reply-To first, each once");
        assert_eq!(answered_ids_of(raw), m.refs);
        assert_eq!(m.bcc, "Szef <szef@x.pl>");
        assert!(answered_ids_of(b"Subject: Hej\r\n\r\nx").is_empty());
    }

    #[test]
    fn one_attachment_by_its_place_and_types_by_name() {
        let raw = b"Subject: Pliki\r\nMIME-Version: 1.0\r\n\
Content-Type: multipart/mixed; boundary=\"b\"\r\n\r\n\
--b\r\nContent-Type: text/plain\r\n\r\nDwa pliki.\r\n\
--b\r\nContent-Type: application/pdf\r\nContent-Disposition: attachment; filename=\"a.pdf\"\r\n\
Content-Transfer-Encoding: base64\r\n\r\nJVBERi0xLjQK\r\n\
--b\r\nContent-Type: text/csv\r\nContent-Disposition: attachment; filename=\"b.csv\"\r\n\r\n\
x;y\r\n--b--\r\n";
        let listed = parse_message(raw, "x/inbox", None).attachments;
        assert_eq!(listed.len(), 2);
        let second = attachment_at(raw, 1).unwrap();
        assert_eq!(
            (second.name.as_str(), second.content_type.as_str()),
            ("b.csv", "text/csv")
        );
        assert_eq!(attachment_at(raw, 0).unwrap().data, b"%PDF-1.4\n");
        assert!(attachment_at(raw, 2).is_none());
        assert_eq!(content_type_for("Faktura 10.PDF"), "application/pdf");
        assert_eq!(content_type_for("zdjęcie.jpeg"), "image/jpeg");
        assert_eq!(content_type_for("README"), "application/octet-stream");
    }

    #[test]
    fn missing_date_and_subject_have_fallbacks() {
        let internal = local(2026, 10, 1, 8, 0);
        let m = parse_message(b"From: a@b.pl\r\n\r\nHej\r\n", "x/inbox", Some(internal));
        assert_eq!(m.date, internal);
        assert_eq!(m.subject, "(no subject)");
        assert_eq!(m.body.trim(), "Hej");
    }
}
