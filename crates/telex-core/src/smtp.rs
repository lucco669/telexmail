//! Sending mail: building the message and handing it to the SMTP server (lettre).
//!
//! Sent mail goes through the Outbox: [`compose`] builds the message, `MailStore::queue_outgoing`
//! puts it in the Outbox, [`send_outbox`] sends what is waiting, and the copy for the
//! server's Sent folder travels with the other queued changes (`imap::push_changes`).
//! Nothing is called sent until the SMTP server has accepted it.

use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Local};
use lettre::address::Envelope;
use lettre::message::header::{ContentType, HeaderName, HeaderValue};
use lettre::message::{Attachment as MimeAttachment, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message as Email, Tokio1Executor};

use crate::mime::{attachments_of, parse_message, thread_ids};
use crate::model::{Account, Message, OutgoingDraft, SmtpSecurity, SmtpSettings};
use crate::store::{MailStore, OutgoingMail, StoreError};

const SMTP_TIMEOUT: Duration = Duration::from_secs(60);

/// An SMTP connection setup. Each send opens its own connection.
pub type Transport = AsyncSmtpTransport<Tokio1Executor>;

/// A message ready for the Outbox.
#[derive(Debug, Clone)]
pub struct Composed {
    /// Parsed back from `raw`, for the list and the preview.
    pub message: Message,
    /// Exactly what is sent, and later saved in Sent.
    pub raw: Vec<u8>,
    /// The SMTP envelope: the sender, and every recipient (To and Cc).
    pub env_from: String,
    pub env_to: Vec<String>,
}

/// What the originals of a reply or a forward add to the new message.
#[derive(Debug, Clone, Default)]
pub struct Originals {
    /// The message replied to, for the In-Reply-To and References headers.
    pub replied: Option<Vec<u8>>,
    /// The message forwarded, whose attachments go along.
    pub forwarded: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendError {
    /// The server refused this message for good, e.g. an unknown recipient. Retrying won't help.
    Refused(String),
    /// It couldn't be sent now: no connection, a busy server, a wrong password. Try again later.
    Failed(String),
}

/// Build the message for a draft, from the account's address and with the account's name
/// as the sender name.
pub fn compose(
    account: &Account,
    draft: &OutgoingDraft,
    originals: &Originals,
    now: DateTime<Local>,
) -> Result<Composed, String> {
    build(account, draft, originals, now, false)
}

/// Like [`compose`], marked `Auto-Submitted: auto-replied` (RFC 3834) so other robots
/// don't answer it back.
pub fn compose_auto_reply(
    account: &Account,
    draft: &OutgoingDraft,
    originals: &Originals,
    now: DateTime<Local>,
) -> Result<Composed, String> {
    build(account, draft, originals, now, true)
}

fn build(
    account: &Account,
    draft: &OutgoingDraft,
    originals: &Originals,
    now: DateTime<Local>,
    auto_reply: bool,
) -> Result<Composed, String> {
    let identity = account.identity(draft.identity);
    let from_address = identity
        .email
        .parse()
        .map_err(|_| format!("The sender address \"{}\" isn't valid.", identity.email))?;
    let to = parse_recipients(&draft.to, "To")?;
    let cc = parse_recipients(&draft.cc, "Cc")?;
    if to.is_empty() && cc.is_empty() {
        return Err("Add at least one recipient in To.".into());
    }
    let domain = identity
        .email
        .rsplit_once('@')
        .map_or("telex.invalid", |(_, d)| d);

    let mut builder = Email::builder()
        .from(Mailbox::new(Some(identity.name.clone()), from_address))
        .subject(draft.subject.clone())
        .date(SystemTime::from(now))
        .message_id(Some(format!("<{}>", new_message_id(domain))));
    for mailbox in to {
        builder = builder.to(mailbox);
    }
    for mailbox in cc {
        builder = builder.cc(mailbox);
    }
    if let Some(raw) = &originals.replied {
        let (id, mut references) = thread_ids(raw);
        if let Some(id) = id {
            builder = builder.in_reply_to(format!("<{id}>"));
            references.push(id);
        }
        if !references.is_empty() {
            let refs: Vec<String> = references.iter().map(|r| format!("<{r}>")).collect();
            builder = builder.references(refs.join(" "));
        }
    }

    let text = SinglePart::plain(draft.body.clone());
    // The forwarded message's attachments that are still listed in the draft.
    let files: Vec<_> = originals
        .forwarded
        .as_deref()
        .map(attachments_of)
        .unwrap_or_default()
        .into_iter()
        .filter(|f| draft.attachments.iter().any(|a| a.name == f.name))
        .collect();
    let email = if files.is_empty() {
        builder.singlepart(text)
    } else {
        let mut mixed = MultiPart::mixed().singlepart(text);
        for file in files {
            let content_type = ContentType::parse(&file.content_type)
                .unwrap_or(ContentType::parse("application/octet-stream").expect("valid type"));
            mixed = mixed.singlepart(MimeAttachment::new(file.name).body(file.data, content_type));
        }
        builder.multipart(mixed)
    }
    .map_err(|e| format!("Couldn't build the message: {e}"))?;
    let mut email = email;
    if auto_reply {
        let name =
            HeaderName::new_from_ascii("Auto-Submitted".to_owned()).expect("a valid header name");
        email
            .headers_mut()
            .insert_raw(HeaderValue::new(name, "auto-replied".to_owned()));
    }

    let raw = email.formatted();
    let envelope = email.envelope();
    let mut message = parse_message(&raw, "", Some(now.naive_local()));
    message.unread = false;
    Ok(Composed {
        message,
        env_from: envelope.from().map(ToString::to_string).unwrap_or_default(),
        env_to: envelope.to().iter().map(ToString::to_string).collect(),
        raw,
    })
}

/// An SMTP connection for an account. It logs in with the IMAP user name and password.
pub fn transport(
    settings: &SmtpSettings,
    username: &str,
    password: &str,
) -> Result<Transport, SendError> {
    let builder = match settings.security {
        SmtpSecurity::Tls => Transport::relay(&settings.host),
        SmtpSecurity::StartTls => Transport::starttls_relay(&settings.host),
    }
    .map_err(|e| SendError::Failed(describe(&e)))?;
    Ok(builder
        .port(settings.port)
        .credentials(Credentials::new(username.to_owned(), password.to_owned()))
        .timeout(Some(SMTP_TIMEOUT))
        .build())
}

/// Connect and log in once to check the settings, without sending anything.
pub async fn check(settings: &SmtpSettings, username: &str, password: &str) -> Result<(), String> {
    let transport = transport(settings, username, password).map_err(|e| match e {
        SendError::Refused(m) | SendError::Failed(m) => m,
    })?;
    match transport.test_connection().await {
        Ok(true) => Ok(()),
        Ok(false) => Err("the SMTP server closed the connection".into()),
        Err(e) => Err(describe(&e)),
    }
}

/// Hand one message to the SMTP server.
pub async fn send_raw(
    transport: &Transport,
    env_from: &str,
    env_to: &[String],
    raw: &[u8],
) -> Result<(), SendError> {
    let refused = |what: &str| SendError::Refused(format!("\"{what}\" isn't a valid address"));
    let from = env_from.parse().map_err(|_| refused(env_from))?;
    let to = env_to
        .iter()
        .map(|a| a.parse().map_err(|_| refused(a)))
        .collect::<Result<Vec<_>, _>>()?;
    let envelope = Envelope::new(Some(from), to).map_err(|e| SendError::Refused(e.to_string()))?;
    transport
        .send_raw(&envelope, raw)
        .await
        .map(|_| ())
        .map_err(|e| classify(&e))
}

/// What sending an Outbox did, as sentences for the status bar.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SendReport {
    /// One line per message sent: `"Re: Wycena" to Anna Kowalska`.
    pub sent: Vec<String>,
    pub problems: Vec<String>,
}

/// Send everything waiting in the account's Outbox, oldest first. A message the server
/// refuses stays in the Outbox with the reason; a connection problem ends the round and
/// the rest is tried at the next check. Mail queued while this runs is picked up too.
pub async fn send_outbox(
    account: &Account,
    transport: &Transport,
    store: &Mutex<MailStore>,
) -> Result<SendReport, StoreError> {
    let mut report = SendReport::default();
    let mut after = 0;
    loop {
        let next = lock(store)?.next_outgoing(&account.id, after)?;
        let Some(mail) = next else { break };
        after = mail.message;
        match send_raw(transport, &mail.env_from, &mail.env_to, &mail.raw).await {
            Ok(()) => {
                lock(store)?.outgoing_sent(mail.message, Local::now().naive_local())?;
                report.sent.push(sent_line(&mail));
            }
            Err(SendError::Refused(reason)) => {
                lock(store)?.outgoing_failed(mail.message, &reason, true)?;
                report
                    .problems
                    .push(format!("\"{}\" wasn't sent: {reason}", mail.subject));
            }
            Err(SendError::Failed(reason)) => {
                lock(store)?.outgoing_failed(mail.message, &reason, false)?;
                report.problems.push(format!(
                    "couldn't send \"{}\": {reason}. It stays in the Outbox",
                    mail.subject
                ));
                break;
            }
        }
    }
    Ok(report)
}

fn lock(store: &Mutex<MailStore>) -> Result<MutexGuard<'_, MailStore>, StoreError> {
    store.lock().map_err(|_| {
        StoreError::Database("another task crashed while using it; restart Telex".into())
    })
}

/// `"Re: Wycena" to Anna Kowalska`
fn sent_line(mail: &OutgoingMail) -> String {
    let first = split_addresses(&mail.to)
        .into_iter()
        .next()
        .unwrap_or_default();
    let who = match first.split_once('<') {
        Some((name, _)) if !name.trim().is_empty() => name.trim().trim_matches('"').to_owned(),
        _ => first.trim_matches(['<', '>']).to_owned(),
    };
    format!("\"{}\" to {who}", mail.subject)
}

/// "Anna <a@x.pl>, b@y.pl" → mailboxes, naming the part that doesn't parse.
fn parse_recipients(input: &str, field: &str) -> Result<Vec<Mailbox>, String> {
    split_addresses(input)
        .into_iter()
        .map(|part| {
            Mailbox::from_str(&part)
                .map_err(|_| format!("\"{part}\" in {field} isn't a valid address."))
        })
        .collect()
}

/// Split an address list at commas and semicolons, except inside quotes and angle
/// brackets: `"Kowalska, Anna" <a@x.pl>, b@y.pl` is two addresses.
pub(crate) fn split_addresses(input: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let (mut quoted, mut angled) = (false, false);
    for c in input.chars() {
        match c {
            '"' => quoted = !quoted,
            '<' if !quoted => angled = true,
            '>' if !quoted => angled = false,
            ',' | ';' if !quoted && !angled => {
                parts.push(std::mem::take(&mut current));
                continue;
            }
            _ => {}
        }
        current.push(c);
    }
    parts.push(current);
    parts
        .into_iter()
        .map(|p| p.trim().to_owned())
        .filter(|p| !p.is_empty())
        .collect()
}

/// A Message-ID unique to this message: time, process and a counter, at the sender's domain.
fn new_message_id(domain: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{nanos:x}.{:x}.{n}.telex@{domain}", std::process::id())
}

/// A login problem belongs to the account, not the message, so it is retried after the
/// user fixes the password. Other permanent (5xx) answers are refusals.
fn classify(e: &lettre::transport::smtp::Error) -> SendError {
    let text = describe(e);
    if e.is_permanent() && !is_login_problem(e) {
        SendError::Refused(text)
    } else {
        SendError::Failed(text)
    }
}

fn is_login_problem(e: &lettre::transport::smtp::Error) -> bool {
    let code = e.status().map(|c| c.to_string());
    matches!(code.as_deref(), Some("530" | "534" | "535" | "538"))
}

fn describe(e: &lettre::transport::smtp::Error) -> String {
    if is_login_problem(e) {
        "the SMTP server rejected the user name or password".into()
    } else if e.is_tls() {
        format!("no secure connection to the SMTP server ({e})")
    } else {
        e.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AccountSetup, Attachment, FolderKind};
    use crate::store::ChangeKind;
    use chrono::TimeZone;
    use mail_parser::MessageParser;
    use std::sync::Arc;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;

    fn account() -> Account {
        let mut store = MailStore::in_memory().unwrap();
        store
            .upsert_account(&AccountSetup {
                name: "Studio Kowalski".into(),
                email: "biuro@twojastudio.pl".into(),
                host: "mail.twojastudio.pl".into(),
                port: 993,
                ..Default::default()
            })
            .unwrap()
    }

    fn draft(to: &str) -> OutgoingDraft {
        OutgoingDraft {
            account: "twojastudio".into(),
            to: to.into(),
            cc: String::new(),
            subject: "Wycena – październik".into(),
            body: "Dzień dobry,\nw załączniku wycena.\n\nPozdrawiam\n".into(),
            ..OutgoingDraft::default()
        }
    }

    fn now() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 10, 9, 30, 0).unwrap()
    }

    #[test]
    fn addresses_split_at_commas_outside_quotes() {
        assert_eq!(
            split_addresses("\"Kowalska, Anna\" <anna@x.pl>; piotr@y.pl, , Paweł <p@z.pl>"),
            [
                "\"Kowalska, Anna\" <anna@x.pl>",
                "piotr@y.pl",
                "Paweł <p@z.pl>"
            ]
        );
        assert!(split_addresses("  ").is_empty());
    }

    #[test]
    fn a_composed_message_reads_back_correctly() {
        let mut d = draft("\"Kowalska, Anna\" <anna@kowalska-architekci.pl>, piotr@nowak-meble.pl");
        d.cc = "Paweł Mazur <pawel@mazur-ksiegowosc.pl>".into();
        let composed = compose(&account(), &d, &Originals::default(), now()).unwrap();
        assert!(composed.raw.windows(2).any(|w| w == b"\r\n"));
        assert!(
            !composed
                .raw
                .windows(2)
                .any(|w| w[1] == b'\n' && w[0] != b'\r'),
            "every line ends in CRLF"
        );
        let parsed = MessageParser::default().parse(&composed.raw).unwrap();
        assert_eq!(parsed.subject(), Some("Wycena – październik"));
        let from = parsed.from().unwrap().first().unwrap();
        assert_eq!(from.name(), Some("Studio Kowalski"));
        assert_eq!(from.address(), Some("biuro@twojastudio.pl"));
        assert_eq!(parsed.to().unwrap().iter().count(), 2);
        assert_eq!(
            parsed
                .body_text(0)
                .unwrap()
                .replace("\r\n", "\n")
                .trim_end(),
            "Dzień dobry,\nw załączniku wycena.\n\nPozdrawiam"
        );
        assert!(parsed
            .message_id()
            .unwrap()
            .ends_with(".telex@twojastudio.pl"));
        assert_eq!(composed.env_from, "biuro@twojastudio.pl");
        assert_eq!(
            composed.env_to,
            [
                "anna@kowalska-architekci.pl",
                "piotr@nowak-meble.pl",
                "pawel@mazur-ksiegowosc.pl"
            ]
        );
        // The local copy is the same message, parsed.
        assert_eq!(composed.message.subject, "Wycena – październik");
        assert_eq!(composed.message.message_id.as_deref(), parsed.message_id());
        assert!(!composed.message.unread);
    }

    #[test]
    fn a_reply_joins_the_conversation_and_a_forward_carries_attachments() {
        let original = b"Message-ID: <wycena.2@kowalska-architekci.pl>\r\n\
References: <wycena.1@kowalska-architekci.pl>\r\n\
From: anna@kowalska-architekci.pl\r\n\
Subject: Makiety\r\n\
MIME-Version: 1.0\r\n\
Content-Type: multipart/mixed; boundary=\"b\"\r\n\
\r\n\
--b\r\n\
Content-Type: text/plain\r\n\
\r\n\
Makiety w za\xc5\x82\xc4\x85czniku.\r\n\
--b\r\n\
Content-Type: application/pdf\r\n\
Content-Disposition: attachment; filename=\"makiety.pdf\"\r\n\
Content-Transfer-Encoding: base64\r\n\
\r\n\
JVBERi0xLjQK\r\n\
--b--\r\n"
            .to_vec();

        let replied = Originals {
            replied: Some(original.clone()),
            forwarded: None,
        };
        let reply = compose(
            &account(),
            &draft("anna@kowalska-architekci.pl"),
            &replied,
            now(),
        )
        .unwrap();
        let parsed = MessageParser::default().parse(&reply.raw).unwrap();
        assert_eq!(
            parsed.in_reply_to().as_text(),
            Some("wycena.2@kowalska-architekci.pl")
        );
        assert_eq!(
            parsed.references().as_text_list().map(|l| l.len()),
            Some(2),
            "the old references plus the original"
        );

        let mut fwd = draft("piotr@nowak-meble.pl");
        fwd.attachments = vec![Attachment {
            name: "makiety.pdf".into(),
            size_kb: 1,
        }];
        let forwarded = Originals {
            replied: None,
            forwarded: Some(original),
        };
        let forward = compose(&account(), &fwd, &forwarded, now()).unwrap();
        let files = attachments_of(&forward.raw);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "makiety.pdf");
        assert_eq!(files[0].content_type, "application/pdf");
        assert_eq!(files[0].data, b"%PDF-1.4\n");
    }

    #[test]
    fn mail_is_from_the_chosen_identity() {
        let mut store = MailStore::in_memory().unwrap();
        let account = store
            .upsert_account(&AccountSetup {
                name: "Studio".into(),
                email: "biuro@twojastudio.pl".into(),
                host: "mail.twojastudio.pl".into(),
                port: 993,
                ..Default::default()
            })
            .unwrap();
        let mut jan =
            crate::model::Identity::new(&account.id, "Jan Kowalski", "jan@twojastudio.pl");
        jan.is_default = false;
        let jan = store.save_identity(&jan).unwrap();
        let account = store.account(&account.id).unwrap().clone();

        let mut d = draft("anna@x.pl");
        d.identity = Some(jan.id);
        let c = compose(&account, &d, &Originals::default(), now()).unwrap();
        let parsed = MessageParser::default().parse(&c.raw).unwrap();
        let from = parsed.from().unwrap().first().unwrap();
        assert_eq!(
            (from.name(), from.address()),
            (Some("Jan Kowalski"), Some("jan@twojastudio.pl"))
        );
        assert_eq!(c.env_from, "jan@twojastudio.pl");
        assert!(parsed.message_id().unwrap().ends_with("@twojastudio.pl"));
        // No choice: the default identity.
        let c = compose(&account, &draft("anna@x.pl"), &Originals::default(), now()).unwrap();
        assert_eq!(c.env_from, "biuro@twojastudio.pl");
    }

    #[test]
    fn bad_addresses_are_named() {
        let err = compose(
            &account(),
            &draft("anna@, piotr@y.pl"),
            &Originals::default(),
            now(),
        )
        .unwrap_err();
        assert!(err.contains("\"anna@\" in To"), "{err}");
        let err = compose(&account(), &draft("  "), &Originals::default(), now()).unwrap_err();
        assert!(err.contains("Add at least one recipient"), "{err}");
    }

    // ---- A fake SMTP server: just enough of RFC 5321 for one message per connection ----

    #[derive(Default)]
    struct FakeSmtp {
        /// (envelope sender, recipients, message) per message accepted.
        accepted: Vec<(String, Vec<String>, String)>,
    }

    /// Recipients containing `reject` get "550 no such user".
    async fn start_fake_smtp(reject: &'static str) -> (u16, Arc<Mutex<FakeSmtp>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = Arc::new(Mutex::new(FakeSmtp::default()));
        let shared = server.clone();
        tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                let (read, mut write) = socket.into_split();
                let mut lines = BufReader::new(read).lines();
                let _ = write.write_all(b"220 fake.test ESMTP\r\n").await;
                let (mut from, mut to, mut data, mut in_data) =
                    (String::new(), Vec::new(), String::new(), false);
                while let Ok(Some(line)) = lines.next_line().await {
                    let reply = if in_data {
                        if line == "." {
                            in_data = false;
                            shared.lock().unwrap().accepted.push((
                                std::mem::take(&mut from),
                                std::mem::take(&mut to),
                                std::mem::take(&mut data),
                            ));
                            "250 2.0.0 Ok: queued\r\n".to_owned()
                        } else {
                            data.push_str(&line);
                            data.push('\n');
                            continue;
                        }
                    } else {
                        let upper = line.to_ascii_uppercase();
                        let address = || {
                            line.split_once('<')
                                .and_then(|(_, r)| r.split_once('>'))
                                .map(|(a, _)| a.to_owned())
                                .unwrap_or_default()
                        };
                        if upper.starts_with("EHLO") || upper.starts_with("HELO") {
                            "250-fake.test\r\n250 8BITMIME\r\n".to_owned()
                        } else if upper.starts_with("MAIL FROM") {
                            from = address();
                            "250 2.1.0 Ok\r\n".to_owned()
                        } else if upper.starts_with("RCPT TO") {
                            let a = address();
                            if a.contains(reject) {
                                format!(
                                    "550 5.1.1 <{a}>: Recipient address rejected: User unknown\r\n"
                                )
                            } else {
                                to.push(a);
                                "250 2.1.5 Ok\r\n".to_owned()
                            }
                        } else if upper == "DATA" {
                            in_data = true;
                            "354 End data with <CR><LF>.<CR><LF>\r\n".to_owned()
                        } else if upper == "QUIT" {
                            let _ = write.write_all(b"221 2.0.0 Bye\r\n").await;
                            break;
                        } else {
                            "250 2.0.0 Ok\r\n".to_owned()
                        }
                    };
                    if write.write_all(reply.as_bytes()).await.is_err() {
                        break;
                    }
                }
            }
        });
        (port, server)
    }

    fn plain_transport(port: u16) -> Transport {
        Transport::builder_dangerous("127.0.0.1")
            .port(port)
            .timeout(Some(Duration::from_secs(5)))
            .build()
    }

    /// A store with one server account (Inbox, Sent on the server) and `to` queued.
    fn store_with_queued(to: &[&str]) -> (Mutex<MailStore>, Account, Vec<u64>) {
        let mut store = MailStore::in_memory().unwrap();
        let account = store
            .upsert_account(&AccountSetup {
                name: "Studio".into(),
                email: "biuro@twojastudio.pl".into(),
                host: "mail.twojastudio.pl".into(),
                port: 993,
                ..Default::default()
            })
            .unwrap();
        let folder = |id: &str, kind| crate::model::Folder {
            id: id.into(),
            name: id.into(),
            kind,
            remote: Some(id.to_uppercase()),
        };
        store
            .set_folders(
                &account.id,
                &[
                    folder("inbox", FolderKind::Inbox),
                    folder("sent", FolderKind::Sent),
                ],
            )
            .unwrap();
        let account = store.account(&account.id).unwrap().clone();
        let ids = to
            .iter()
            .map(|addr| {
                let c = compose(&account, &draft(addr), &Originals::default(), now()).unwrap();
                store
                    .queue_outgoing(
                        &account.id,
                        &c.message,
                        &c.raw,
                        &c.env_from,
                        &c.env_to,
                        None,
                    )
                    .unwrap()
            })
            .collect();
        (Mutex::new(store), account, ids)
    }

    #[tokio::test]
    async fn the_outbox_is_sent_and_filed_in_sent() {
        let (port, server) = start_fake_smtp("nobody@").await;
        let (store, account, ids) = store_with_queued(&["anna@kowalska-architekci.pl"]);
        assert_eq!(
            store
                .lock()
                .unwrap()
                .list(&account.folder_key("outbox"))
                .unwrap()
                .len(),
            1
        );

        let report = send_outbox(&account, &plain_transport(port), &store)
            .await
            .unwrap();
        assert_eq!(
            report.sent,
            ["\"Wycena – październik\" to anna@kowalska-architekci.pl"]
        );
        assert!(report.problems.is_empty());

        let accepted = server.lock().unwrap().accepted.clone();
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].0, "biuro@twojastudio.pl");
        assert_eq!(accepted[0].1, ["anna@kowalska-architekci.pl"]);
        assert!(accepted[0].2.contains("Subject: "));

        let s = store.lock().unwrap();
        assert!(s.list(&account.folder_key("outbox")).unwrap().is_empty());
        let sent = s.list(&account.folder_key("sent")).unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].id, ids[0]);
        let changes = s.pending_changes(&account.id).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(
            changes[0].kind,
            ChangeKind::Append(account.folder_key("sent"))
        );
        assert!(!s.has_outgoing(&account.id).unwrap());
    }

    #[tokio::test]
    async fn a_refused_recipient_keeps_the_message_and_says_why() {
        let (port, server) = start_fake_smtp("nobody@").await;
        let (store, account, ids) = store_with_queued(&[
            "nobody@kowalska-architekci.pl",
            "anna@kowalska-architekci.pl",
        ]);
        let report = send_outbox(&account, &plain_transport(port), &store)
            .await
            .unwrap();
        assert_eq!(
            report.sent.len(),
            1,
            "the next message still goes: {report:?}"
        );
        assert_eq!(report.problems.len(), 1);
        assert!(
            report.problems[0].contains("Recipient address rejected"),
            "{report:?}"
        );
        assert_eq!(server.lock().unwrap().accepted.len(), 1);

        let s = store.lock().unwrap();
        let stuck = s.get(ids[0]).unwrap();
        assert_eq!(stuck.folder, account.folder_key("outbox"));
        let note = stuck.note.unwrap();
        assert!(
            note.starts_with("Not sent:") && note.contains("Press E"),
            "{note}"
        );
        // Refused for good: it isn't picked up again until the user edits it.
        assert!(!s.has_outgoing(&account.id).unwrap());
    }

    #[tokio::test]
    async fn without_a_connection_the_mail_waits() {
        // Nothing listens on this port once the listener is dropped.
        let port = {
            let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
            l.local_addr().unwrap().port()
        };
        let (store, account, ids) =
            store_with_queued(&["anna@kowalska-architekci.pl", "piotr@nowak-meble.pl"]);
        let report = send_outbox(&account, &plain_transport(port), &store)
            .await
            .unwrap();
        assert!(report.sent.is_empty());
        assert_eq!(
            report.problems.len(),
            1,
            "one try, not one per message: {report:?}"
        );
        let s = store.lock().unwrap();
        assert!(s
            .get(ids[0])
            .unwrap()
            .note
            .unwrap()
            .starts_with("Not sent yet:"));
        assert!(
            s.has_outgoing(&account.id).unwrap(),
            "both are tried again later"
        );
    }

    #[test]
    fn moving_a_message_out_of_the_outbox_cancels_it() {
        let (store, account, ids) = store_with_queued(&["anna@kowalska-architekci.pl"]);
        let mut s = store.lock().unwrap();
        s.move_to(ids[0], &account.folder_key("inbox")).unwrap();
        assert!(!s.has_outgoing(&account.id).unwrap());
    }

    #[test]
    fn editing_an_unsent_message_replaces_it() {
        let (store, account, ids) = store_with_queued(&["anna@kowalska-architekci.pl"]);
        let mut s = store.lock().unwrap();
        let c = compose(
            &account,
            &draft("anna@kowalska-architekci.pl"),
            &Originals::default(),
            now(),
        )
        .unwrap();
        let new = s
            .queue_outgoing(
                &account.id,
                &c.message,
                &c.raw,
                &c.env_from,
                &c.env_to,
                Some(ids[0]),
            )
            .unwrap();
        assert!(s.get(ids[0]).is_err());
        let outbox = s.list(&account.folder_key("outbox")).unwrap();
        assert_eq!(outbox.len(), 1);
        assert_eq!(outbox[0].id, new);
    }

    /// Needs the internet: `cargo test -p telex-core -- --ignored`. Checks that lettre's TLS
    /// (rustls with ring) reaches a real server, both ways. No login: greeting, NOOP, QUIT.
    #[tokio::test]
    #[ignore = "needs the internet"]
    async fn smtp_tls_reaches_a_real_server() {
        let implicit: Transport = Transport::relay("smtp.gmail.com")
            .unwrap()
            .port(465)
            .build();
        assert!(implicit.test_connection().await.unwrap());
        let starttls: Transport = Transport::starttls_relay("smtp.gmail.com")
            .unwrap()
            .port(587)
            .build();
        assert!(starttls.test_connection().await.unwrap());
    }
}
