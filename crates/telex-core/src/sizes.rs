//! How large a message is as it is sent, and the most its account's server takes.
//!
//! A message goes out encoded: files and pictures in base64 (a third more, and a line break
//! every 76 characters), text as 7bit, quoted-printable or base64, whichever lettre picks, and
//! headers and MIME boundaries around it all. [`estimate`] adds that up part by part without
//! building the message (files by their size on disk, pictures by their data: URIs, text through
//! lettre's own encoder), so compose can show it as files and pictures come and go; Send builds
//! the message and checks its exact size ([`check`]).
//!
//! The limit, best source first ([`limit`]): what the server said at its last login (EHLO's
//! SIZE, RFC 1870, kept per account), else the provider list below, else 20 MB. Every limit is
//! for the message as sent, encoded. Recipients' servers have limits of their own, which Telex
//! can't know.

use std::collections::HashSet;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use lettre::message::Body;
use serde::Serialize;

use crate::html;
use crate::i18n;
use crate::model::{Account, OutgoingDraft};
use crate::tr;

const MB: u64 = 1024 * 1024;

/// What Telex goes by when neither the server nor the provider list says.
pub const DEFAULT_LIMIT: u64 = 20 * MB;

/// The most a message may weigh as sent, and who says so.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Limit {
    /// For the message as sent, encoded.
    pub bytes: u64,
    /// "server" (its EHLO), "provider" (the list) or "default".
    pub source: &'static str,
    /// Who: the provider's name where the list knows it, else the server's host.
    pub name: Option<String>,
    /// The provider's own number, in MB, as its help page states it, and whether that is for the
    /// files (most say "attachments") rather than the whole message: compose's tooltip says it.
    pub documented_mb: Option<u32>,
    pub for_files: bool,
}

/// What a provider's help page states for one message: the files in it, or the whole message.
#[derive(Clone, Copy)]
enum Documented {
    /// "Attachments up to n MB": the encoded message may be about 4/3 of that.
    Files(u32),
    /// The whole message, n MB.
    Message(u32),
}

/// A provider's documented limit, matched by the SMTP server's host (or a host under it), else
/// by the address's domain.
struct Provider {
    name: &'static str,
    domains: &'static [&'static str],
    hosts: &'static [&'static str],
    limit: Documented,
}

/// Documented limits, each checked on the provider's own page on 2026-10-18 (the URL above each).
/// Left out, as no page of their own states a number: Purelymail, OVHcloud MX Plan (ssl0.ovh.net),
/// Onet, AOL, home.pl for mail programs (its 100 MB is for its webmail) and Zoho's free plan. For those
/// the server's own SIZE or 20 MB applies. Where a provider has plans, the smallest is kept.
const PROVIDERS: &[Provider] = &[
    // support.google.com/mail/answer/6584 ("For personal Gmail accounts, the limit is 25 MB.");
    // Google Workspace counts content and attachments before encoding, 25 MB too.
    Provider {
        name: "Gmail",
        domains: &["gmail.com", "googlemail.com"],
        hosts: &["smtp.gmail.com"],
        limit: Documented::Files(25),
    },
    // support.microsoft.com, "Sending limits in Outlook.com" ("The attachment size limit for
    // files is 25 MB.").
    Provider {
        name: "Outlook.com",
        domains: &["outlook.com", "hotmail.com", "live.com", "msn.com"],
        hosts: &["smtp-mail.outlook.com"],
        limit: Documented::Files(25),
    },
    // learn.microsoft.com, Exchange Online limits ("default maximum message size … is 35 MB
    // for sending"; an admin can set 1–150 MB).
    Provider {
        name: "Microsoft 365",
        domains: &[],
        hosts: &["smtp.office365.com"],
        limit: Documented::Message(35),
    },
    // help.yahoo.com/kb/SLN5673.html ("must not exceed 25MB in total file size").
    Provider {
        name: "Yahoo Mail",
        domains: &["yahoo.com", "ymail.com", "rocketmail.com"],
        hosts: &["smtp.mail.yahoo.com"],
        limit: Documented::Files(25),
    },
    // support.apple.com/en-us/102198 ("The size of incoming and outgoing messages (20MB").
    Provider {
        name: "iCloud Mail",
        domains: &["icloud.com", "me.com", "mac.com"],
        hosts: &["smtp.mail.me.com"],
        limit: Documented::Message(20),
    },
    // fastmail.help/hc/en-us/articles/360060591253 ("under 70 MB in total size").
    Provider {
        name: "Fastmail",
        domains: &["fastmail.com", "fastmail.fm"],
        hosts: &["smtp.fastmail.com"],
        limit: Documented::Message(70),
    },
    // zoho.com/mail/help/attachments.html ("maximum email size including all attachments is
    // 20MB"), personal accounts.
    Provider {
        name: "Zoho Mail",
        domains: &["zoho.com", "zohomail.com", "zohomail.eu"],
        hosts: &["smtp.zoho.com", "smtp.zoho.eu", "smtp.zoho.in"],
        limit: Documented::Message(20),
    },
    // zoho.com/mail/zohomail-pricing.html ("Maximum email attachment size": Mail Lite 30 MB).
    Provider {
        name: "Zoho Mail",
        domains: &[],
        hosts: &["smtppro.zoho.com", "smtppro.zoho.eu", "smtppro.zoho.in"],
        limit: Documented::Files(30),
    },
    // hilfe.gmx.net/email/empfangen-und-lesen/dateianhaenge.html ("GMX FreeMail 20 MB").
    Provider {
        name: "GMX",
        domains: &["gmx.net", "gmx.de", "gmx.at", "gmx.ch"],
        hosts: &["mail.gmx.net"],
        limit: Documented::Files(20),
    },
    // support.gmx.com/email/composing-and-sending/attachments.html ("The maximum attachment
    // size is 50 MB.").
    Provider {
        name: "GMX",
        domains: &["gmx.com"],
        hosts: &["mail.gmx.com"],
        limit: Documented::Files(50),
    },
    // hilfe.web.de/email/empfangen-und-lesen/dateianhaenge.html ("bis zu 20 MB Anhänge pro
    // E-Mail versenden"), FreeMail.
    Provider {
        name: "WEB.DE",
        domains: &["web.de"],
        hosts: &["smtp.web.de"],
        limit: Documented::Files(20),
    },
    // proton.me/support/attaching-multiple-documents-to-a-message ("25 MB for outgoing emails").
    Provider {
        name: "Proton Mail",
        domains: &["proton.me", "protonmail.com", "pm.me"],
        hosts: &["smtp.protonmail.ch"],
        limit: Documented::Files(25),
    },
    // support.mail.com/email/composing-and-sending/attachments.html ("The maximum attachment
    // size is 30 MB for mail.com FreeMail users").
    Provider {
        name: "mail.com",
        domains: &["mail.com", "email.com", "usa.com", "post.com"],
        hosts: &["smtp.mail.com"],
        limit: Documented::Files(30),
    },
    // yandex.com/support/mail/web/letter/attachments.html (larger than 25 MB goes to Disk as
    // links; over SMTP it can't).
    Provider {
        name: "Yandex Mail",
        domains: &["yandex.com", "yandex.ru", "ya.ru"],
        hosts: &["smtp.yandex.com", "smtp.yandex.ru"],
        limit: Documented::Files(25),
    },
    // help.mail.ru/mail/letters/sent/attach/ (files over 25 MB go to the Cloud as links).
    Provider {
        name: "Mail.ru",
        domains: &["mail.ru", "bk.ru", "inbox.ru", "list.ru"],
        hosts: &["smtp.mail.ru"],
        limit: Documented::Files(25),
    },
    // ovhcloud.com, Email Pro and Hosted Exchange ("SMTP: 100 MB").
    Provider {
        name: "OVHcloud",
        domains: &[],
        hosts: &["mail.ovh.net"],
        limit: Documented::Message(100),
    },
    // nazwa.pl/parametry-bezpieczenstwa/ ("Maksymalny rozmiar wiadomości 1 GB").
    Provider {
        name: "nazwa.pl",
        domains: &[],
        hosts: &["nazwa.pl"],
        limit: Documented::Message(1024),
    },
    // pomoc.wp.pl/regulamin, § 5 ("załączników do e-mail do 100 MB").
    Provider {
        name: "WP Poczta",
        domains: &["wp.pl"],
        hosts: &["smtp.wp.pl"],
        limit: Documented::Files(100),
    },
    // pomoc.o2.pl/regulamin (the same terms) and poczta.o2.pl ("Duże załączniki do 100 MB").
    Provider {
        name: "o2 Poczta",
        domains: &["o2.pl", "tlen.pl", "go2.pl"],
        hosts: &["poczta.o2.pl"],
        limit: Documented::Files(100),
    },
    // pomoc.poczta.interia.pl, free and PocztaPRO ("Maksymalna przesyłka 100MB").
    Provider {
        name: "Interia Poczta",
        domains: &["interia.pl", "interia.eu", "poczta.fm"],
        hosts: &["poczta.interia.pl"],
        limit: Documented::Message(100),
    },
];

impl Documented {
    /// The limit for the encoded message: the files in base64 (Gmail's 25 MB of files comes to
    /// 35,872,340 bytes; its server says 35,882,577).
    fn bytes(self) -> u64 {
        match self {
            Self::Files(mb) => base64_size(u64::from(mb) * MB),
            Self::Message(mb) => u64::from(mb) * MB,
        }
    }
}

/// The provider an account sends through, if the list knows it.
fn provider_of(email: &str, smtp_host: &str) -> Option<&'static Provider> {
    let domain = email
        .rsplit_once('@')
        .map_or("", |(_, d)| d)
        .trim()
        .to_ascii_lowercase();
    let host = smtp_host.trim().trim_end_matches('.').to_ascii_lowercase();
    let under = |name: &str, root: &str| name == root || name.ends_with(&format!(".{root}"));
    PROVIDERS
        .iter()
        .find(|p| p.hosts.iter().any(|h| !host.is_empty() && under(&host, h)))
        .or_else(|| {
            PROVIDERS
                .iter()
                .find(|p| p.domains.iter().any(|d| domain == *d))
        })
}

/// The provider an account sends through, if the list knows it: its name and its limit for the
/// encoded message.
pub fn provider(email: &str, smtp_host: &str) -> Option<(&'static str, u64)> {
    provider_of(email, smtp_host).map(|p| (p.name, p.limit.bytes()))
}

/// The most a message from this account may weigh as sent.
pub fn limit(account: &Account) -> Limit {
    let host = account.smtp.as_ref().map_or("", |s| s.host.as_str());
    let known = provider_of(&account.email, host);
    let (documented_mb, for_files) = match known.map(|p| p.limit) {
        Some(Documented::Files(mb)) => (Some(mb), true),
        Some(Documented::Message(mb)) => (Some(mb), false),
        None => (None, false),
    };
    if let Some(max) = account.smtp_max {
        return Limit {
            bytes: max,
            source: "server",
            // The provider's name where the list knows it ("Gmail"), else the server's.
            name: known
                .map(|p| p.name.to_owned())
                .or_else(|| (!host.is_empty()).then(|| host.to_owned())),
            documented_mb,
            for_files,
        };
    }
    match known {
        Some(p) => Limit {
            bytes: p.limit.bytes(),
            source: "provider",
            name: Some(p.name.to_owned()),
            documented_mb,
            for_files,
        },
        None => Limit {
            bytes: DEFAULT_LIMIT,
            source: "default",
            name: None,
            documented_mb: None,
            for_files: false,
        },
    }
}

/// A file of the message, and what it weighs as sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileWeight {
    pub name: String,
    pub bytes: u64,
}

/// What a message weighs as sent, in parts, against its limit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Weight {
    /// The whole message.
    pub bytes: u64,
    /// Its files, each as sent.
    pub files: Vec<FileWeight>,
    /// The pictures in its text, as sent.
    pub pictures: u64,
    /// Its text (the HTML and its plain copy, or the plain text) and headers.
    pub text: u64,
    pub limit: Limit,
}

/// A part's headers and its boundary line, about.
const PART: u64 = 160;

/// `n` bytes in base64 as lettre writes it: four characters for three bytes, a line break
/// after every 76.
pub fn base64_size(n: u64) -> u64 {
    let chars = n.div_ceil(3) * 4;
    chars + chars.div_ceil(76) * 2
}

/// A text as lettre encodes it.
fn text_size(text: &str) -> u64 {
    Body::new(text.to_owned()).len() as u64
}

/// What the draft weighs as sent. `originals`: the messages whose files it carries (a forward, a
/// reply taking the original's files, the draft or unsent message it was edited from). Files from this PC are measured on disk; one that
/// can't be read counts nothing (Send names it).
pub fn estimate(draft: &OutgoingDraft, originals: &[&[u8]], limit: Limit) -> Weight {
    let mut files = Vec::new();
    let carried: Vec<crate::mime::FileAttachment> = originals
        .iter()
        .flat_map(|raw| crate::mime::attachments_of(raw))
        .collect();
    for a in &draft.attachments {
        let raw = match &a.path {
            Some(path) => std::fs::metadata(path).map(|m| m.len()).ok(),
            None => carried
                .iter()
                .find(|f| f.name == a.name)
                .map(|f| f.data.len() as u64),
        };
        if let Some(n) = raw {
            let bytes = base64_size(n) + PART + a.name.len() as u64 * 2;
            files.push(FileWeight {
                name: a.name.clone(),
                bytes,
            });
        }
    }
    let (text, pictures) = match draft.html.as_deref() {
        Some(editor) => {
            let clean = html::compose_clean(editor);
            let (without, pictures) = data_pictures(&clean);
            let document = html::compose_document(&without);
            let plain = html::to_text(&without);
            // Two text parts, in an alternative (and a related one with pictures).
            (
                text_size(&document) + text_size(&plain) + 3 * PART,
                pictures,
            )
        }
        None => (text_size(&draft.body), 0),
    };
    // From, To, Cc, Bcc, Subject, Date, Message-ID, MIME and threading headers.
    let headers = 360
        + [&draft.to, &draft.cc, &draft.bcc, &draft.subject]
            .iter()
            .map(|h| h.len() as u64 * 3 / 2)
            .sum::<u64>();
    let all_files: u64 = files.iter().map(|f| f.bytes).sum();
    Weight {
        bytes: headers + text + pictures + all_files,
        files,
        pictures,
        text: headers + text,
        limit,
    }
}

/// Cleaned HTML with its data: pictures taken out (as `cid:` stand-ins of the same length a
/// sent one has), and what those pictures weigh as parts of their own: each once, however
/// often it shows.
fn data_pictures(clean: &str) -> (String, u64) {
    let mut seen = HashSet::new();
    let mut weight = 0;
    let mut out = String::with_capacity(clean.len().min(64 * 1024));
    let mut rest = clean;
    while let Some(at) = rest.find("src=\"data:") {
        out.push_str(&rest[..at]);
        let after = &rest[at + 5..];
        let end = after.find('"').unwrap_or(after.len());
        let uri = &after[..end];
        let payload = uri.split_once(";base64,").map_or("", |(_, p)| p);
        let digest = ring::digest::digest(&ring::digest::SHA256, payload.as_bytes());
        if seen.insert(digest.as_ref().to_vec()) {
            let decoded = STANDARD
                .decode(payload)
                .map_or(payload.len() as u64 * 3 / 4, |d| d.len() as u64);
            weight += base64_size(decoded) + PART;
        }
        out.push_str("src=\"cid:1.00000000000000000.00000.0.telex@example.com\"");
        rest = after.get(end + 1..).unwrap_or_default();
    }
    out.push_str(rest);
    (out, weight)
}

/// "4,2 MB", "25 MB": in MB with one decimal under 100, the decimal sign the language uses.
pub fn megabytes(bytes: u64) -> String {
    let mb = bytes as f64 / MB as f64;
    let text = if mb >= 100.0 || (mb - mb.round()).abs() < 0.05 {
        format!("{:.0}", mb.round())
    } else {
        format!("{mb:.1}")
    };
    let text = if i18n::current().decimal_comma() {
        text.replace('.', ",")
    } else {
        text
    };
    format!("{text} MB")
}

/// Before a message goes into the Outbox (Send, Send later): its exact size as built against
/// the account's limit. The sentence says by how much and what to do, naming the provider or
/// the server where one is known (a sentence for each case, so every language can say it).
pub fn check(bytes: u64, limit: &Limit) -> Result<(), String> {
    if bytes <= limit.bytes {
        return Ok(());
    }
    let (size, over, max) = (
        megabytes(bytes),
        megabytes(bytes - limit.bytes),
        megabytes(limit.bytes),
    );
    Err(match (limit.source, &limit.name) {
        ("default", _) | (_, None) => tr!(
            "This message is {size} as sent: {over} more than most servers take ({max}). Take files off, make the pictures smaller or send it in parts.",
            "Ta wiadomość ma {size} po wysłaniu: o {over} więcej, niż przyjmuje większość serwerów ({max}). Usuń pliki, zmniejsz obrazy albo wyślij ją w częściach."
        ),
        (_, Some(name)) => tr!(
            "This message is {size} as sent: {over} more than {name} takes ({max}). Take files off, make the pictures smaller or send it in parts.",
            "Ta wiadomość ma {size} po wysłaniu: o {over} więcej, niż przyjmuje {name} ({max}). Usuń pliki, zmniejsz obrazy albo wyślij ją w częściach."
        ),
    })
}

/// Why a message in the Outbox isn't tried: the server said at its login that it takes less.
pub fn too_large_for_server(bytes: u64, max: u64) -> String {
    let (size, max) = (megabytes(bytes), megabytes(max));
    tr!(
        "it is {size} as sent, more than the {max} the server takes. Take files off or make the pictures smaller, then send it again",
        "ma {size} po wysłaniu, więcej niż {max}, które przyjmuje serwer. Usuń pliki albo zmniejsz obrazy i wyślij ją ponownie"
    )
}

/// Whether an SMTP refusal says the message is too large: 552 (except a full mailbox,
/// 5.2.2), or the enhanced codes 5.3.4 (too big for the system) and 5.2.3 (too long).
pub fn is_size_refusal(code: Option<&str>, text: &str) -> bool {
    (code == Some("552") && !text.contains("5.2.2"))
        || text.contains("5.3.4")
        || text.contains("5.2.3")
}

/// A size refusal in words that say what to do, with the server's own.
pub fn refused_by_server(server_said: &str) -> String {
    tr!(
        "the server refused it as too large ({server_said}). Take files off or make the pictures smaller, then send it again",
        "serwer odrzucił ją jako za dużą ({server_said}). Usuń pliki albo zmniejsz obrazy i wyślij ją ponownie"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Attachment, SmtpSecurity, SmtpSettings};

    #[test]
    fn base64_grows_by_a_third_and_its_line_breaks() {
        assert_eq!(base64_size(0), 0);
        assert_eq!(base64_size(3), 4 + 2);
        assert_eq!(base64_size(57), 76 + 2, "57 bytes fill one line");
        assert_eq!(base64_size(58), 80 + 4);
        let mb = base64_size(MB);
        assert!((1_398_000..1_435_000).contains(&mb), "{mb}");
    }

    #[test]
    fn sizes_read_in_megabytes_in_each_language() {
        assert_eq!(megabytes(25 * MB), "25 MB");
        assert_eq!(megabytes(4 * MB + MB / 5), "4.2 MB");
        assert_eq!(
            i18n::with(i18n::Lang::Pl, || megabytes(4 * MB + MB / 5)),
            "4,2 MB"
        );
    }

    #[test]
    fn the_limit_comes_from_the_server_then_the_provider_then_the_default() {
        let mut account = Account {
            id: "a".into(),
            name: "Anna".into(),
            email: "anna@gmail.com".into(),
            folders: Vec::new(),
            imap: None,
            smtp: Some(SmtpSettings {
                host: "smtp.gmail.com".into(),
                port: 465,
                security: SmtpSecurity::Tls,
            }),
            identities: Vec::new(),
            sent_copy: Default::default(),
            server_files_sent: None,
            smtp_max: None,
        };
        assert_eq!(
            (limit(&account).source, limit(&account).name.as_deref()),
            ("provider", Some("Gmail"))
        );
        account.smtp_max = Some(35_882_577);
        assert_eq!(
            limit(&account),
            Limit {
                bytes: 35_882_577,
                source: "server",
                name: Some("Gmail".into()),
                documented_mb: Some(25),
                for_files: true,
            }
        );
        account.smtp_max = None;
        account.email = "biuro@firma.pl".into();
        account.smtp.as_mut().unwrap().host = "mail.firma.pl".into();
        assert_eq!(limit(&account).bytes, DEFAULT_LIMIT);
        // By the host when the address is one's own domain.
        assert_eq!(
            provider("ja@moja-firma.pl", "smtp.office365.com"),
            Some(("Microsoft 365", 35 * MB))
        );
        // Gmail's 25 MB of files is about 34 MB as sent, as its server says (35,882,577 bytes).
        let gmail = provider("anna@gmail.com", "").unwrap().1;
        assert!((34 * MB..35_882_577).contains(&gmail), "{gmail}");
        assert_eq!(
            provider("biuro@firma.pl", "pro2.mail.ovh.net").map(|p| p.0),
            Some("OVHcloud")
        );
        assert_eq!(
            provider("biuro@firma.pl", "ssl0.ovh.net"),
            None,
            "MX Plan states none"
        );
        assert_eq!(provider("ja@o2.pl", "").map(|p| p.0), Some("o2 Poczta"));
        assert_eq!(provider("ja@x.pl", "notsmtp.gmail.com.evil.pl"), None);
    }

    #[test]
    fn too_large_says_by_how_much_and_what_to_do() {
        let gmail = Limit {
            bytes: 25 * MB,
            source: "provider",
            name: Some("Gmail".into()),
            documented_mb: Some(25),
            for_files: true,
        };
        assert!(check(25 * MB, &gmail).is_ok());
        let err = check(27 * MB, &gmail).unwrap_err();
        assert!(
            err.contains("27 MB") && err.contains("2 MB more than Gmail takes (25 MB)"),
            "{err}"
        );
        let pl = i18n::with(i18n::Lang::Pl, || check(27 * MB, &gmail).unwrap_err());
        assert!(pl.starts_with("Ta wiadomość ma 27 MB"), "{pl}");
        assert!(is_size_refusal(
            Some("552"),
            "5.3.4 Message size exceeds fixed limit"
        ));
        assert!(is_size_refusal(Some("554"), "5.3.4 too big"));
        assert!(!is_size_refusal(Some("552"), "5.2.2 Mailbox full"));
        assert!(!is_size_refusal(Some("550"), "5.1.1 User unknown"));
    }

    #[test]
    fn files_pictures_and_text_add_up() {
        let path = std::env::temp_dir().join(format!("telex-size-{}.bin", std::process::id()));
        std::fs::write(&path, vec![7u8; 300_000]).unwrap();
        let png = format!(
            "data:image/png;base64,{}",
            STANDARD.encode(vec![1u8; 30_000])
        );
        let draft = OutgoingDraft {
            to: "anna@x.pl".into(),
            subject: "Plany".into(),
            html: Some(format!(
                "<div>Plany</div><img src=\"{png}\"><img src=\"{png}\">"
            )),
            attachments: vec![Attachment {
                name: "plany.bin".into(),
                size_kb: 293,
                path: Some(path.display().to_string()),
            }],
            ..OutgoingDraft::default()
        };
        let w = estimate(
            &draft,
            &[],
            Limit {
                bytes: DEFAULT_LIMIT,
                source: "default",
                name: None,
                documented_mb: None,
                for_files: false,
            },
        );
        std::fs::remove_file(&path).unwrap();
        assert_eq!(w.files.len(), 1);
        assert!(
            (405_000..415_000).contains(&w.files[0].bytes),
            "{:?}",
            w.files
        );
        assert!(
            (40_000..42_500).contains(&w.pictures),
            "one picture shown twice is one part: {}",
            w.pictures
        );
        assert_eq!(w.bytes, w.text + w.pictures + w.files[0].bytes);
    }
}
