//! Mail as Markdown, to paste into notes or an AI chat: each message's subject as a heading,
//! who wrote it to whom and when, what was attached, then its text. Several messages (a
//! conversation, a selection) go oldest first, a rule between them.

use crate::html;
use crate::mime;
use crate::model::{Attachment, Message};
use crate::tr;

/// One message. `raw` is the message as stored: mail written only in HTML has its text made
/// from the HTML, so links keep their addresses.
pub fn message(m: &Message, raw: Option<&[u8]>) -> String {
    let text = raw
        .and_then(mime::html_only)
        .map(|h| html::received_text(&h))
        .unwrap_or_else(|| m.body.replace("\r\n", "\n"));
    let mut out = format!("# {}\n\n", one_line(&m.subject));
    let from = match (m.from_name.trim(), m.from_email.trim()) {
        ("", email) => email.to_owned(),
        (name, "") => name.to_owned(),
        (name, email) if name.eq_ignore_ascii_case(email) => email.to_owned(),
        (name, email) => format!("{name} <{email}>"),
    };
    let fields = [
        (tr!("From", "Od"), from),
        (tr!("To", "Do"), m.to.clone()),
        (tr!("Cc", "DW"), m.cc.clone()),
        (tr!("Bcc", "UDW"), m.bcc.clone()),
        (
            tr!("Date", "Data"),
            m.date.format("%Y-%m-%d %H:%M").to_string(),
        ),
        (tr!("Attachments", "Załączniki"), files(&m.attachments)),
    ];
    for (label, value) in fields {
        let value = one_line(&value);
        if !value.is_empty() {
            out.push_str(&format!("- **{label}:** {value}\n"));
        }
    }
    let text = text.trim_matches('\n').trim_end();
    if !text.is_empty() {
        out.push('\n');
        out.push_str(text);
        out.push('\n');
    }
    out
}

/// Several messages, given oldest first.
pub fn messages<'a>(all: impl IntoIterator<Item = (&'a Message, Option<&'a [u8]>)>) -> String {
    all.into_iter()
        .map(|(m, raw)| message(m, raw))
        .collect::<Vec<_>>()
        .join("\n---\n\n")
}

/// Header values on one line: folded headers and stray line breaks would end the list item.
fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn files(attachments: &[Attachment]) -> String {
    attachments
        .iter()
        .map(|a| format!("{} ({})", a.name, size(a.size_kb)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// "34 KB", "1.2 MB" (in Polish "1,2 MB").
pub(crate) fn size(kb: u32) -> String {
    if kb < 1024 {
        return format!("{} KB", kb.max(1));
    }
    let mb = format!("{:.1}", f64::from(kb) / 1024.0);
    match crate::i18n::current() {
        crate::i18n::Lang::Pl => format!("{} MB", mb.replace('.', ",")),
        crate::i18n::Lang::En => format!("{mb} MB"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{self, Lang};
    use chrono::NaiveDate;

    fn sample() -> Message {
        Message {
            id: 1,
            folder: "firma/inbox".into(),
            from_name: "Anna Kowalska".into(),
            from_email: "anna@kowalska-architekci.pl".into(),
            to: "biuro@twojastudio.pl".into(),
            cc: String::new(),
            bcc: String::new(),
            subject: "Poprawki na stronie\r\n głównej".into(),
            date: NaiveDate::from_ymd_opt(2026, 10, 10)
                .unwrap()
                .and_hms_opt(6, 12, 0)
                .unwrap(),
            size_kb: 40,
            unread: true,
            flagged: false,
            remote_images: false,
            note: None,
            attachments: vec![
                Attachment {
                    name: "projekt.pdf".into(),
                    size_kb: 1229,
                    path: None,
                },
                Attachment {
                    name: "logo.png".into(),
                    size_kb: 34,
                    path: None,
                },
            ],
            body: "Dzień dobry,\r\n\r\nprzesyłam poprawki.\r\n\r\n> Wcześniej\r\n".into(),
            message_id: None,
            refs: Vec::new(),
        }
    }

    #[test]
    fn a_message_is_a_heading_its_header_and_its_text() {
        let md = i18n::with(Lang::En, || message(&sample(), None));
        assert_eq!(
            md,
            "# Poprawki na stronie głównej\n\n\
             - **From:** Anna Kowalska <anna@kowalska-architekci.pl>\n\
             - **To:** biuro@twojastudio.pl\n\
             - **Date:** 2026-10-10 06:12\n\
             - **Attachments:** projekt.pdf (1.2 MB), logo.png (34 KB)\n\n\
             Dzień dobry,\n\nprzesyłam poprawki.\n\n> Wcześniej\n"
        );
        let pl = i18n::with(Lang::Pl, || message(&sample(), None));
        assert!(pl.contains("- **Od:** Anna"));
        assert!(pl.contains("- **Załączniki:** projekt.pdf (1,2 MB)"));
    }

    #[test]
    fn html_only_mail_keeps_its_links_and_table_rows() {
        let raw = b"From: a@b.pl\r\nSubject: Oferta\r\nContent-Type: text/html; charset=utf-8\r\n\r\n\
            <html><head><style>p{color:red}</style></head><body>\
            <p>Zobacz <a href=\"https://sklep.pl/oferta\">nasz sklep</a>.</p>\
            <table><tr><td>Krzes\xc5\x82o</td><td>200 z\xc5\x82</td></tr><tr><td>St\xc3\xb3\xc5\x82</td><td>900 z\xc5\x82</td></tr></table>\
            <script>alert(1)</script></body></html>";
        let mut m = sample();
        m.body = "flattened".into();
        m.attachments.clear();
        let md = i18n::with(Lang::En, || message(&m, Some(raw)));
        let text = md
            .split_once("\n\n")
            .unwrap()
            .1
            .split_once("\n\n")
            .unwrap()
            .1;
        assert_eq!(
            text,
            "Zobacz nasz sklep <https://sklep.pl/oferta>.\n\nKrzesło 200 zł\nStół 900 zł\n"
        );
        // A plain-text part of the sender's own is used as it is.
        let both = b"Subject: x\r\nContent-Type: multipart/alternative; boundary=b\r\n\r\n\
            --b\r\nContent-Type: text/plain\r\n\r\nplain\r\n--b\r\nContent-Type: text/html\r\n\r\n<p>html</p>\r\n--b--\r\n";
        assert!(message(&m, Some(both)).ends_with("flattened\n"));
    }

    #[test]
    fn several_messages_go_with_a_rule_between_them() {
        let (a, mut b) = (sample(), sample());
        b.subject = "Re: Poprawki".into();
        b.from_name = "biuro@twojastudio.pl".into();
        b.from_email = "biuro@twojastudio.pl".into();
        b.body = String::new();
        let md = i18n::with(Lang::En, || messages([(&a, None), (&b, None)]));
        let parts: Vec<&str> = md.split("\n---\n\n").collect();
        assert_eq!(parts.len(), 2);
        assert!(parts[1].starts_with("# Re: Poprawki\n\n- **From:** biuro@twojastudio.pl\n"));
        assert!(parts[1].ends_with("(34 KB)\n"), "no text, no empty lines");
    }
}
