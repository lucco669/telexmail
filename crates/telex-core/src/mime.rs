//! Turning a raw RFC 822 message from the server into a [`Message`].
//!
//! `mail-parser` does the hard part: MIME structure, transfer encodings and old
//! charsets such as Windows-1250 and ISO-8859-2.

use chrono::{DateTime, Local, NaiveDateTime};
use mail_parser::{Address, MessageParser, MimeHeaders, PartType};

use crate::model::{Attachment, Message};

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
            subject: "(unreadable message)".into(),
            date: fallback_date.unwrap_or_else(|| Local::now().naive_local()),
            size_kb,
            unread: true,
            flagged: false,
            remote_images: false,
            note: Some("Telex couldn't read this message's structure.".into()),
            attachments: Vec::new(),
            body: String::from_utf8_lossy(raw).into_owned(),
            message_id: None,
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

    let attachments = p
        .attachments()
        .map(|a| Attachment {
            name: a.attachment_name().map_or_else(
                || {
                    if a.is_message() {
                        "message.eml"
                    } else {
                        "attachment"
                    }
                    .to_owned()
                },
                str::to_owned,
            ),
            size_kb: kb(a.contents().len()),
        })
        .collect();

    let subject = match p.subject().map(str::trim) {
        Some(s) if !s.is_empty() => s.to_owned(),
        _ => "(no subject)".to_owned(),
    };

    Message {
        id: 0,
        folder: folder.to_owned(),
        from_name,
        from_email,
        to: format_addresses(p.to()),
        cc: format_addresses(p.cc()),
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
    }
}

/// Just the Message-ID header, without parsing the body.
pub fn parse_message_id(raw: &[u8]) -> Option<String> {
    MessageParser::default()
        .parse_headers(raw)?
        .message_id()
        .map(str::to_owned)
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
    fn missing_date_and_subject_have_fallbacks() {
        let internal = local(2026, 10, 1, 8, 0);
        let m = parse_message(b"From: a@b.pl\r\n\r\nHej\r\n", "x/inbox", Some(internal));
        assert_eq!(m.date, internal);
        assert_eq!(m.subject, "(no subject)");
        assert_eq!(m.body.trim(), "Hej");
    }
}
