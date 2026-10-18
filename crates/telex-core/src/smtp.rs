//! Sending mail: building the message and handing it to the SMTP server (lettre).
//!
//! Sent mail goes through the Outbox: [`compose`] builds the message, `MailStore::queue_outgoing`
//! puts it in the Outbox, [`send_outbox`] sends what is waiting, and the copy for the
//! server's Sent folder travels with the other queued changes (`imap::push_changes`).
//! Nothing is called sent until the SMTP server has accepted it.

use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Local};
use lettre::address::Envelope;
use lettre::message::header::{ContentType, HeaderName, HeaderValue};
use lettre::message::{Attachment as MimeAttachment, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message as Email, Tokio1Executor};

use crate::html;
use crate::mime::{
    attachments_of, content_type_for, parse_message, parse_message_id, reply_headers, thread_ids,
    FileAttachment,
};
use crate::model::{Account, Message, OutgoingDraft, SmtpSecurity, SmtpSettings};
use crate::openpgp;
use crate::servers::{self, LOG};
use crate::store::{MailStore, OutgoingMail, StoreError};
use crate::tr;

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
    /// The draft (or unsent message) this was edited from: a draft saved again keeps its
    /// Message-ID, and without `replied` the new message answers what that one answered
    /// (a reply begun on a phone stays in its thread).
    pub edited: Option<Vec<u8>>,
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
    build(
        account,
        draft,
        originals,
        now,
        Purpose::Send,
        None,
        &Extras::default(),
    )
}

/// An answer to a calendar invitation (RFC 6047): the text, and the iTIP REPLY as a
/// text/calendar part with method=REPLY beside it, as calendar programs expect.
pub fn compose_invite_reply(
    account: &Account,
    draft: &OutgoingDraft,
    originals: &Originals,
    ics: &str,
    now: DateTime<Local>,
) -> Result<Composed, String> {
    build(
        account,
        draft,
        originals,
        now,
        Purpose::Send,
        Some(ics),
        &Extras::default(),
    )
}

/// Like [`compose`], marked `Auto-Submitted: auto-replied` (RFC 3834) so other robots
/// don't answer it back.
pub fn compose_auto_reply(
    account: &Account,
    draft: &OutgoingDraft,
    originals: &Originals,
    now: DateTime<Local>,
) -> Result<Composed, String> {
    build(
        account,
        draft,
        originals,
        now,
        Purpose::AutoReply,
        None,
        &Extras::default(),
    )
}

pub fn recipient_emails(to: &str) -> Result<Vec<String>, String> {
    if to.contains(['\r', '\n']) {
        return Err(tr!(
            "Enter valid recipient addresses.",
            "Wpisz poprawne adresy odbiorców."
        ));
    }
    split_addresses(to)
        .iter()
        .map(|address| {
            address
                .parse::<Mailbox>()
                .map(|m| m.email.to_string())
                .map_err(|e| unbuildable(&e))
        })
        .collect()
}

pub fn compose_auto_forward(
    account: &Account,
    draft: &OutgoingDraft,
    originals: &Originals,
    now: DateTime<Local>,
) -> Result<Composed, String> {
    build(
        account,
        draft,
        originals,
        now,
        Purpose::AutoForward,
        None,
        &Extras::default(),
    )
}

/// A draft as a whole message, to keep with the draft: its HTML and the forwarded files
/// stay with it until it is edited again. Unlike [`compose`], recipients may be missing
/// or half typed; the ones that aren't addresses yet are left out of the headers (the
/// draft keeps the To and Cc text as typed).
pub fn compose_draft(
    account: &Account,
    draft: &OutgoingDraft,
    originals: &Originals,
    now: DateTime<Local>,
) -> Result<Vec<u8>, String> {
    build(
        account,
        draft,
        originals,
        now,
        Purpose::Draft,
        None,
        &Extras::default(),
    )
    .map(|c| c.raw)
}

/// Like [`compose`], signed or encrypted as `protect` says (openpgp.rs): the message is
/// built as usual, then wrapped (multipart/signed or multipart/encrypted). The copy kept in
/// the Outbox and Sent is the one sent, which the sender's own key can decrypt. Its list
/// line keeps the subject the user typed; its text isn't kept or indexed when encrypted.
pub fn compose_protected(
    account: &Account,
    draft: &OutgoingDraft,
    originals: &Originals,
    now: DateTime<Local>,
    protect: &openpgp::Protect,
) -> Result<Composed, String> {
    let extras = Extras {
        quoted_printable: protect.sign.is_some()
            || protect.smime.as_ref().is_some_and(|s| s.sign.is_some()),
        key_file: protect.key_file.clone(),
    };
    let mut composed = build(account, draft, originals, now, Purpose::Send, None, &extras)?;
    let subject = composed.message.subject.clone();
    composed.raw = match &protect.smime {
        Some(s) => crate::smime::protect(&composed.raw, s),
        None => openpgp::protect(&composed.raw, protect),
    }
    .map_err(|e| e.to_string())?;
    composed.message = parse_message(&composed.raw, "", Some(now.naive_local()));
    composed.message.subject = subject;
    composed.message.unread = false;
    Ok(composed)
}

/// Like [`compose_draft`], encrypted to the sender's own key when `protect` says so.
pub fn compose_draft_protected(
    account: &Account,
    draft: &OutgoingDraft,
    originals: &Originals,
    now: DateTime<Local>,
    protect: &openpgp::Protect,
) -> Result<Vec<u8>, String> {
    let raw = build(
        account,
        draft,
        originals,
        now,
        Purpose::Draft,
        None,
        &Extras::default(),
    )?
    .raw;
    match &protect.smime {
        Some(s) => crate::smime::protect(&raw, s),
        None => openpgp::protect(&raw, protect),
    }
    .map_err(|e| e.to_string())
}

/// What OpenPGP asks of the message being built.
#[derive(Debug, Clone, Default)]
struct Extras {
    /// Text parts in quoted-printable even when 7-bit would do: a signed part must reach the
    /// reader byte for byte, and servers may strip spaces at the ends of 7-bit lines (RFC 3156
    /// §3; the "-- " before a signature has one).
    quoted_printable: bool,
    /// The sender's public key as a file (application/pgp-keys).
    key_file: Option<(String, String)>,
}

/// Why a message is built: to send, to answer automatically, or to keep as a draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Purpose {
    Send,
    AutoReply,
    AutoForward,
    Draft,
}

/// The text of a message: plain, or written in HTML with a plain-text version alongside.
enum Body {
    Plain(SinglePart),
    Html(MultiPart),
}

fn build(
    account: &Account,
    draft: &OutgoingDraft,
    originals: &Originals,
    now: DateTime<Local>,
    purpose: Purpose,
    calendar: Option<&str>,
    extras: &Extras,
) -> Result<Composed, String> {
    let identity = account.identity(draft.identity);
    let from_address: lettre::Address = identity.email.parse().map_err(|_| {
        let email = &identity.email;
        tr!(
            "The sender address \"{email}\" isn't valid.",
            "Adres nadawcy „{email}” jest niepoprawny."
        )
    })?;
    let (to, cc, bcc) = if purpose == Purpose::Draft {
        (
            lenient_recipients(&draft.to),
            lenient_recipients(&draft.cc),
            lenient_recipients(&draft.bcc),
        )
    } else {
        (
            parse_recipients(&draft.to, &tr!("To", "Do"))?,
            parse_recipients(&draft.cc, &tr!("Cc", "DW"))?,
            parse_recipients(&draft.bcc, &tr!("Bcc", "UDW"))?,
        )
    };
    if to.is_empty() && cc.is_empty() && bcc.is_empty() && purpose != Purpose::Draft {
        return Err(tr!(
            "Add at least one recipient in To.",
            "Dodaj co najmniej jednego odbiorcę w polu Do."
        ));
    }
    // Files from this PC, read now: a file moved or deleted since it was attached is named.
    let mut files = Vec::new();
    for a in &draft.attachments {
        let Some(path) = &a.path else { continue };
        let data = std::fs::read(path).map_err(|e| {
            let name = &a.name;
            tr!(
                "Couldn't attach \"{name}\": {e}.",
                "Nie udało się dołączyć pliku „{name}”: {e}."
            )
        })?;
        files.push(FileAttachment {
            name: a.name.clone(),
            content_type: content_type_for(&a.name).to_owned(),
            data,
        });
    }
    let domain = identity
        .email
        .rsplit_once('@')
        .map_or("telex.invalid", |(_, d)| d);

    // A draft keeps its Message-ID from one save to the next; what is sent gets its own.
    let message_id = originals
        .edited
        .as_deref()
        .filter(|_| purpose == Purpose::Draft)
        .and_then(parse_message_id)
        .filter(|id| !id.is_empty() && id.bytes().all(|b| (0x21..0x7f).contains(&b)))
        .unwrap_or_else(|| new_message_id(domain));
    // Bcc stays in Telex's own copy (the Outbox, Sent, drafts); `send_raw` leaves it out of
    // what the recipients get.
    let mut builder = Email::builder()
        .from(Mailbox::new(
            Some(identity.name.clone()),
            from_address.clone(),
        ))
        .subject(draft.subject.clone())
        .date(SystemTime::from(now))
        .message_id(Some(format!("<{message_id}>")))
        .keep_bcc();
    if purpose == Purpose::Draft {
        // A draft goes nowhere, but lettre wants an envelope: the sender to itself.
        let envelope = Envelope::new(Some(from_address.clone()), vec![from_address])
            .map_err(|e| unbuildable(&e))?;
        builder = builder.envelope(envelope);
    }
    for mailbox in to {
        builder = builder.to(mailbox);
    }
    for mailbox in cc {
        builder = builder.cc(mailbox);
    }
    for mailbox in bcc {
        builder = builder.bcc(mailbox);
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
    } else if let Some(raw) = &originals.edited {
        let (in_reply_to, references) = reply_headers(raw);
        if let Some(id) = in_reply_to {
            builder = builder.in_reply_to(format!("<{id}>"));
        }
        if !references.is_empty() {
            let refs: Vec<String> = references.iter().map(|r| format!("<{r}>")).collect();
            builder = builder.references(refs.join(" "));
        }
    }

    // HTML goes out cleaned, with a plain-text version for mail programs that show only text,
    // and its pictures as parts of their own beside it (multipart/related, `cid:`).
    let text = |content_type: ContentType, text: String| {
        if extras.quoted_printable {
            let body = lettre::message::Body::new_with_encoding(
                text,
                lettre::message::header::ContentTransferEncoding::QuotedPrintable,
            )
            .unwrap_or_else(|_| lettre::message::Body::new(Vec::new()));
            SinglePart::builder().header(content_type).body(body)
        } else {
            SinglePart::builder().header(content_type).body(text)
        }
    };
    let body = match draft.html.as_deref().map(html::compose_clean) {
        Some(clean) => Body::Html(html_body(&clean, &message_id, &text)),
        None => Body::Plain(text(ContentType::TEXT_PLAIN, draft.body.clone())),
    };
    let body = match (body, calendar) {
        (Body::Plain(text), Some(ics)) => Body::Html(
            MultiPart::alternative().singlepart(text).singlepart(
                SinglePart::builder()
                    .header(
                        ContentType::parse("text/calendar; charset=utf-8; method=REPLY")
                            .expect("a valid type"),
                    )
                    .body(ics.to_owned()),
            ),
        ),
        (body, _) => body,
    };
    // The forwarded message's attachments that are still listed in the draft, then the
    // files from this PC.
    let forwarded = originals
        .forwarded
        .as_deref()
        .map(attachments_of)
        .unwrap_or_default()
        .into_iter()
        .filter(|f| {
            draft
                .attachments
                .iter()
                .any(|a| a.path.is_none() && a.name == f.name)
        });
    let key_file = extras
        .key_file
        .iter()
        .map(|(name, armored)| FileAttachment {
            name: name.clone(),
            content_type: "application/pgp-keys".to_owned(),
            data: armored.clone().into_bytes(),
        });
    let files: Vec<FileAttachment> = forwarded.chain(files).chain(key_file).collect();
    let email = match body {
        Body::Plain(text) if files.is_empty() => builder.singlepart(text),
        Body::Html(alternative) if files.is_empty() => builder.multipart(alternative),
        body => {
            let mut mixed = match body {
                Body::Plain(text) => MultiPart::mixed().singlepart(text),
                Body::Html(alternative) => MultiPart::mixed().multipart(alternative),
            };
            for file in files {
                let content_type = ContentType::parse(&file.content_type)
                    .unwrap_or(ContentType::parse("application/octet-stream").expect("valid type"));
                mixed =
                    mixed.singlepart(MimeAttachment::new(file.name).body(file.data, content_type));
            }
            builder.multipart(mixed)
        }
    }
    .map_err(|e| unbuildable(&e))?;
    let mut email = email;
    if matches!(purpose, Purpose::AutoReply | Purpose::AutoForward) {
        let name =
            HeaderName::new_from_ascii("Auto-Submitted".to_owned()).expect("a valid header name");
        email.headers_mut().insert_raw(HeaderValue::new(
            name,
            if purpose == Purpose::AutoForward {
                "auto-generated"
            } else {
                "auto-replied"
            }
            .to_owned(),
        ));
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

/// The text of an HTML message: its plain text, then the HTML, with the pictures it shows
/// related to the HTML. Each picture's Content-ID is its number at the message's own id.
fn html_body(
    clean: &str,
    message_id: &str,
    text: &dyn Fn(ContentType, String) -> SinglePart,
) -> MultiPart {
    let (html, pictures) = html::inline_images(clean, message_id);
    let plain = html::to_text(&html);
    let document = html::compose_document(&html);
    if pictures.is_empty() {
        return MultiPart::alternative()
            .singlepart(text(ContentType::TEXT_PLAIN, plain))
            .singlepart(text(ContentType::TEXT_HTML, document));
    }
    let mut related = MultiPart::related().singlepart(text(ContentType::TEXT_HTML, document));
    for picture in pictures {
        let content_type = ContentType::parse(&picture.mime)
            .unwrap_or(ContentType::parse("application/octet-stream").expect("valid type"));
        related = related
            .singlepart(MimeAttachment::new_inline(picture.cid).body(picture.data, content_type));
    }
    MultiPart::alternative()
        .singlepart(text(ContentType::TEXT_PLAIN, plain))
        .multipart(related)
}

/// lettre couldn't put the message together; its reason stays as it wrote it.
fn unbuildable(e: &impl std::fmt::Display) -> String {
    tr!(
        "Couldn't build the message: {e}",
        "Nie udało się utworzyć wiadomości: {e}"
    )
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
    let started = Instant::now();
    let transport = transport(settings, username, password).map_err(|e| match e {
        SendError::Refused(m) | SendError::Failed(m) => m,
    })?;
    let result = match transport.test_connection().await {
        Ok(true) => Ok(()),
        Ok(false) => Err(tr!(
            "the SMTP server closed the connection",
            "serwer SMTP zamknął połączenie"
        )),
        Err(e) => Err(describe(&e)),
    };
    let server = (settings.host.as_str(), settings.port);
    LOG.once(
        servers::Purpose::Test,
        username,
        server,
        started,
        result.clone().err(),
    );
    result
}

/// Hand one message to the SMTP server.
pub async fn send_raw(
    transport: &Transport,
    env_from: &str,
    env_to: &[String],
    raw: &[u8],
) -> Result<(), SendError> {
    let refused = |what: &str| {
        SendError::Refused(tr!(
            "\"{what}\" isn't a valid address",
            "„{what}” nie jest poprawnym adresem"
        ))
    };
    let from = env_from.parse().map_err(|_| refused(env_from))?;
    let to = env_to
        .iter()
        .map(|a| a.parse().map_err(|_| refused(a)))
        .collect::<Result<Vec<_>, _>>()?;
    let envelope = Envelope::new(Some(from), to).map_err(|e| SendError::Refused(e.to_string()))?;
    transport
        .send_raw(&envelope, &without_bcc(raw))
        .await
        .map(|_| ())
        .map_err(|e| classify(&e))
}

/// The message without its Bcc header: blind copies reach their recipients through the
/// envelope, and nobody else may see them. The rest stays byte for byte.
pub fn without_bcc(raw: &[u8]) -> Vec<u8> {
    let end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map_or(raw.len(), |p| p + 2);
    let (head, body) = raw.split_at(end);
    let mut out = Vec::with_capacity(raw.len());
    let mut dropping = false;
    for line in head.split_inclusive(|&b| b == b'\n') {
        let folded = line.first().is_some_and(|b| *b == b' ' || *b == b'\t');
        if !folded {
            dropping = line.len() >= 4 && line[..4].eq_ignore_ascii_case(b"bcc:");
        }
        if !dropping {
            out.extend_from_slice(line);
        }
    }
    out.extend_from_slice(body);
    out
}

/// What sending an Outbox did, as sentences for the status bar.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SendReport {
    /// One line per message sent: `"Re: Wycena" to Anna Kowalska`.
    pub sent: Vec<String>,
    pub problems: Vec<String>,
}

/// Send everything due in the account's Outbox, oldest first. A message the server
/// refuses stays in the Outbox with the reason; a connection problem ends the round and
/// the rest is tried at the next check. Mail queued while this runs is picked up too, and
/// mail still waiting out its moment to undo is left for later.
pub async fn send_outbox(
    account: &Account,
    transport: &Transport,
    store: &Mutex<MailStore>,
) -> Result<SendReport, StoreError> {
    let mut report = SendReport::default();
    let mut after = 0;
    loop {
        let now = Local::now().naive_local();
        let next = lock(store)?.next_outgoing(&account.id, after, now)?;
        let Some(mail) = next else { break };
        after = mail.message;
        let started = Instant::now();
        let sent = send_raw(transport, &mail.env_from, &mail.env_to, &mail.raw).await;
        log_send(account, started, &sent);
        match sent {
            Ok(()) => {
                let at = Local::now().naive_local();
                lock(store)?.outgoing_sent(mail.message, at)?;
                report.sent.push(match mail.send_after {
                    // Telex was closed (or the PC asleep) when it was due.
                    Some(due) if crate::later::late(due, at) => {
                        let (line, due) = (sent_line(&mail), crate::later::short(due));
                        tr!(
                            "{line}, late: it was due {due}",
                            "{line}, z opóźnieniem: miała wyjść {due}"
                        )
                    }
                    _ => sent_line(&mail),
                });
            }
            Err(SendError::Refused(reason)) => {
                lock(store)?.outgoing_failed(mail.message, &reason, true)?;
                let subject = &mail.subject;
                report.problems.push(tr!(
                    "\"{subject}\" wasn't sent: {reason}",
                    "wiadomość „{subject}” nie została wysłana: {reason}"
                ));
            }
            Err(SendError::Failed(reason)) => {
                lock(store)?.outgoing_failed(mail.message, &reason, false)?;
                let subject = &mail.subject;
                report.problems.push(tr!(
                    "couldn't send \"{subject}\": {reason}. It stays in the Outbox",
                    "nie udało się wysłać wiadomości „{subject}”: {reason}. Czeka w Skrzynce \
                     nadawczej"
                ));
                break;
            }
        }
    }
    Ok(report)
}

/// A message handed to the server (or not), as a line in the connection log.
fn log_send(account: &Account, started: Instant, sent: &Result<(), SendError>) {
    let Some(smtp) = &account.smtp else { return };
    let user = account
        .imap
        .as_ref()
        .map_or(&account.email, |i| &i.username);
    let error = match sent {
        Ok(()) => None,
        Err(SendError::Refused(m) | SendError::Failed(m)) => Some(m.clone()),
    };
    LOG.once(
        servers::Purpose::Send,
        user,
        (&smtp.host, smtp.port),
        started,
        error,
    );
}

fn lock(store: &Mutex<MailStore>) -> Result<MutexGuard<'_, MailStore>, StoreError> {
    store.lock().map_err(|_| StoreError::poisoned())
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
    let subject = &mail.subject;
    tr!("\"{subject}\" to {who}", "„{subject}” do {who}")
}

/// "Anna <a@x.pl>, b@y.pl" → mailboxes, naming the part that doesn't parse.
fn parse_recipients(input: &str, field: &str) -> Result<Vec<Mailbox>, String> {
    split_addresses(input)
        .into_iter()
        .map(|part| {
            mailbox(&part).ok_or_else(|| {
                tr!(
                    "\"{part}\" in {field} isn't a valid address.",
                    "„{part}” w polu {field} nie jest poprawnym adresem."
                )
            })
        })
        .collect()
}

/// The addresses of an address line that parse, as plain addresses (compose's OpenPGP state
/// while the user is still typing).
pub fn lenient_emails(input: &str) -> Vec<String> {
    lenient_recipients(input)
        .into_iter()
        .map(|m| m.email.to_string())
        .collect()
}

/// The addresses that parse; a draft may hold half-typed ones.
fn lenient_recipients(input: &str) -> Vec<Mailbox> {
    split_addresses(input)
        .iter()
        .filter_map(|part| mailbox(part))
        .collect()
}

/// One entry of an address list. Mail often names a person by their address
/// (`a@x.pl <a@x.pl>`) or with a dot or @ in an unquoted name, which RFC 5322 wants quoted:
/// the name is taken as it is (and quoted when the message is written), or left out when it
/// only repeats the address.
fn mailbox(entry: &str) -> Option<Mailbox> {
    if let Ok(m) = Mailbox::from_str(entry) {
        return Some(m);
    }
    let (name, address) = mailbox_parts(entry);
    let address = lettre::Address::from_str(&address).ok()?;
    let name = (!name.is_empty() && !name.eq_ignore_ascii_case(address.as_ref())).then_some(name);
    Some(Mailbox::new(name, address))
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

/// The name and the address in one entry of an address list: `"Kowalska, Anna" <a@x.pl>` →
/// ("Kowalska, Anna", "a@x.pl"); a bare address has no name.
pub(crate) fn mailbox_parts(entry: &str) -> (String, String) {
    match entry.rsplit_once('<') {
        Some((name, rest)) => (
            name.trim().trim_matches('"').trim().to_owned(),
            rest.trim_end().trim_end_matches('>').trim().to_owned(),
        ),
        None => (String::new(), entry.trim().to_owned()),
    }
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
        tr!(
            "the SMTP server rejected the user name or password",
            "serwer SMTP odrzucił nazwę użytkownika lub hasło"
        )
    } else if e.is_tls() {
        tr!(
            "no secure connection to the SMTP server ({e})",
            "brak bezpiecznego połączenia z serwerem SMTP ({e})"
        )
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
    fn names_that_are_addresses_or_need_quotes_still_send() {
        let got = parse_recipients(
            "m.mostowski@owner.pl <m.mostowski@owner.pl>, J. Nowak (biuro@x.pl) <jan@x.pl>, ewa@y.pl",
            "To",
        )
        .unwrap();
        assert_eq!(got.len(), 3);
        assert_eq!(
            got[0].name, None,
            "a name that only repeats the address goes"
        );
        assert_eq!(got[0].email.to_string(), "m.mostowski@owner.pl");
        assert_eq!(got[1].name.as_deref(), Some("J. Nowak (biuro@x.pl)"));
        assert_eq!(got[1].email.to_string(), "jan@x.pl");
        assert!(parse_recipients("anna@", "To").is_err());
        assert_eq!(
            drafts_line("m.mostowski@owner.pl", "M.Mostowski@owner.pl"),
            "M.Mostowski@owner.pl"
        );
    }

    fn drafts_line(name: &str, email: &str) -> String {
        crate::drafts::address_line(name, email)
    }

    #[test]
    fn html_mail_goes_out_with_a_plain_text_version() {
        let mut d = draft("anna@kowalska-architekci.pl");
        d.body = "ignored".into();
        d.html = Some(
            "<div>Dzień <b>dobry</b>,</div><div><br></div><ul><li>wycena</li></ul>\
             <div style=\"color:red\" onclick=\"x()\">Pozdrawiam</div><script>alert(1)</script>"
                .into(),
        );
        let composed = compose(&account(), &d, &Originals::default(), now()).unwrap();
        let raw = String::from_utf8_lossy(&composed.raw);
        assert!(raw.contains("multipart/alternative"), "{raw}");
        let parsed = MessageParser::default().parse(&composed.raw).unwrap();
        let text = parsed.body_text(0).unwrap().replace("\r\n", "\n");
        assert_eq!(text.trim_end(), "Dzień dobry,\n\n- wycena\nPozdrawiam");
        let html = parsed.body_html(0).unwrap();
        assert!(html.contains("<b>dobry</b>"), "{html}");
        for gone in ["onclick", "script", "color:red", "ignored"] {
            assert!(!html.contains(gone), "{gone}: {html}");
        }
        // The local copy shows the text, and the HTML view finds the HTML.
        assert!(composed.message.body.starts_with("Dzień dobry,"));
        assert!(html::render(&composed.raw, &Default::default(), &html::Paper::Light).is_some());
    }

    #[test]
    fn pictures_go_as_related_parts_and_come_back_for_the_editor() {
        let png = "data:image/png;base64,iVBORw0KGgo=";
        let mut d = draft("anna@kowalska-architekci.pl");
        d.html = Some(format!(
            "<div>Plan <span style=\"color:#c00000\">pilny</span>:</div><img src=\"{png}\" alt=\"plan\">\
             <div class=\"telex-signature\">Anna<br><img src=\"{png}\"></div>\
             <img src=\"https://t.pl/x.gif\">"
        ));
        let composed = compose(&account(), &d, &Originals::default(), now()).unwrap();
        let raw = String::from_utf8_lossy(&composed.raw).replace("\r\n", "\n");
        let alternative = raw.find("multipart/alternative").unwrap();
        let related = raw.find("multipart/related").unwrap();
        assert!(
            alternative < related,
            "the pictures go with the HTML: {raw}"
        );
        assert_eq!(
            raw.matches("Content-ID: <").count(),
            1,
            "one part for one picture"
        );
        assert!(!raw.contains("t.pl"), "no remote pictures");
        let parsed = MessageParser::default().parse(&composed.raw).unwrap();
        let html = parsed.body_html(0).unwrap();
        let id = parsed.message_id().unwrap();
        assert!(
            html.contains(&format!("src=\"cid:1.{id}\" alt=\"plan\"")),
            "{html}"
        );
        assert!(html.contains("style=\"color:#c00000\""));
        assert!(html.contains("<div class=\"telex-signature\">"));
        assert_eq!(
            parsed
                .body_text(0)
                .unwrap()
                .replace("\r\n", "\n")
                .trim_end(),
            "Plan pilny:\nAnna"
        );
        assert!(
            composed.message.attachments.is_empty(),
            "pictures in the text aren't files: {:?}",
            composed.message.attachments
        );
        // Shown in Telex, and opened again with E, the picture is in the message itself.
        let shown = html::render(&composed.raw, &Default::default(), &html::Paper::Light).unwrap();
        assert!(shown.document.contains(png) && shown.blocked == 0);
        let editable = html::editable(&composed.raw).unwrap();
        assert!(
            editable.contains(&format!("<img src=\"{png}\" alt=\"plan\">")),
            "{editable}"
        );
        let draft_raw = compose_draft(&account(), &d, &Originals::default(), now()).unwrap();
        assert_eq!(
            html::editable(&draft_raw).unwrap(),
            editable,
            "a draft keeps them too"
        );
    }

    #[test]
    fn a_draft_keeps_its_html_and_half_typed_recipients_are_fine() {
        let mut d = draft("anna@kowalska-architekci.pl, piotr@");
        d.html = Some("<div><i>Szkic</i></div>".into());
        assert!(compose(&account(), &d, &Originals::default(), now()).is_err());
        let raw = compose_draft(&account(), &d, &Originals::default(), now()).unwrap();
        let parsed = MessageParser::default().parse(&raw).unwrap();
        assert_eq!(
            parsed.to().unwrap().iter().count(),
            1,
            "only the whole address"
        );
        assert!(parsed.body_html(0).unwrap().contains("<i>Szkic</i>"));

        let empty = OutgoingDraft {
            to: String::new(),
            ..draft("")
        };
        assert!(compose_draft(&account(), &empty, &Originals::default(), now()).is_ok());
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
            ..Originals::default()
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
            path: None,
        }];
        let forwarded = Originals {
            forwarded: Some(original.clone()),
            ..Originals::default()
        };
        let forward = compose(&account(), &fwd, &forwarded, now()).unwrap();
        let files = attachments_of(&forward.raw);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "makiety.pdf");
        assert_eq!(files[0].content_type, "application/pdf");
        assert_eq!(files[0].data, b"%PDF-1.4\n");

        // A reply that takes the original's files along: threaded, and with them.
        let mut with_files = draft("anna@kowalska-architekci.pl");
        with_files.attachments = fwd.attachments.clone();
        let both = Originals {
            replied: Some(original.clone()),
            forwarded: Some(original),
            ..Originals::default()
        };
        let reply = compose(&account(), &with_files, &both, now()).unwrap();
        let parsed = MessageParser::default().parse(&reply.raw).unwrap();
        assert_eq!(
            parsed.in_reply_to().as_text(),
            Some("wycena.2@kowalska-architekci.pl")
        );
        assert_eq!(attachments_of(&reply.raw)[0].name, "makiety.pdf");
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

    #[test]
    fn sent_lines_and_bad_addresses_in_polish() {
        use crate::i18n::{self, Lang};
        let mail = OutgoingMail {
            message: 1,
            account: "twojastudio".into(),
            env_from: "biuro@twojastudio.pl".into(),
            env_to: vec!["anna@x.pl".into()],
            raw: Vec::new(),
            subject: "Re: Wycena".into(),
            to: "\"Kowalska, Anna\" <anna@x.pl>, piotr@y.pl".into(),
            attempts: 0,
            send_after: None,
        };
        assert_eq!(sent_line(&mail), "\"Re: Wycena\" to Kowalska, Anna");
        assert_eq!(
            i18n::with(Lang::Pl, || sent_line(&mail)),
            "„Re: Wycena” do Kowalska, Anna"
        );
        let mut d = draft("anna@x.pl");
        d.cc = "piotr@".into();
        let err = i18n::with(Lang::Pl, || {
            compose(&account(), &d, &Originals::default(), now()).unwrap_err()
        });
        assert_eq!(err, "„piotr@” w polu DW nie jest poprawnym adresem.");
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
                        None,
                    )
                    .unwrap()
            })
            .collect();
        (Mutex::new(store), account, ids)
    }

    /// The moment the Outbox is looked at.
    fn due() -> chrono::NaiveDateTime {
        Local::now().naive_local()
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
            ChangeKind::Append {
                to: account.folder_key("sent"),
                flags: None,
                draft: false
            }
        );
        assert!(!s.has_outgoing(&account.id, due()).unwrap());
    }

    #[tokio::test]
    async fn the_original_is_answered_only_once_smtp_took_the_reply() {
        let (port, _server) = start_fake_smtp("nobody@").await;
        let (store, account, ids) = store_with_queued(&[
            "nobody@kowalska-architekci.pl", // refused
            "anna@kowalska-architekci.pl",
        ]);
        let inbox = account.folder_key("inbox");
        let originals: Vec<crate::model::MessageId> = {
            let mut s = store.lock().unwrap();
            let originals: Vec<crate::model::MessageId> = (1..=3)
                .map(|n| {
                    let raw = format!(
                        "From: Anna <anna@kowalska-architekci.pl>\r\nTo: biuro@twojastudio.pl\r\n\
                         Subject: Nr {n}\r\nMessage-ID: <nr{n}@kowalska.pl>\r\n\
                         Date: Fri, 16 Oct 2026 09:00:00 +0200\r\n\r\nTekst\r\n"
                    );
                    let message = crate::mime::parse_message(raw.as_bytes(), &inbox, None);
                    s.add_synced(
                        &inbox,
                        &[crate::store::SyncedMessage {
                            uid: n,
                            message,
                            raw: raw.into_bytes(),
                        }],
                    )
                    .unwrap()[0]
                })
                .collect();
            s.note_answering(ids[0], Some(originals[0]), None).unwrap();
            s.note_answering(ids[1], Some(originals[1]), None).unwrap();
            // Sent later: it waits in the Outbox, and so does the forward's mark.
            let c = compose(
                &account,
                &draft("piotr@nowak.pl"),
                &Originals::default(),
                now(),
            )
            .unwrap();
            let later = s
                .queue_outgoing(
                    &account.id,
                    &c.message,
                    &c.raw,
                    &c.env_from,
                    &c.env_to,
                    None,
                    Some(due() + chrono::TimeDelta::hours(1)),
                )
                .unwrap();
            s.note_answering(later, None, Some(originals[2])).unwrap();
            originals
        };
        let state = |id| {
            let m = store.lock().unwrap().summary(id).unwrap();
            (m.answered, m.forwarded)
        };
        assert_eq!(state(originals[1]), (false, false), "queued isn't sent");

        let report = send_outbox(&account, &plain_transport(port), &store)
            .await
            .unwrap();
        assert_eq!(report.sent.len(), 1, "{report:?}");
        assert_eq!(
            state(originals[0]),
            (false, false),
            "the server refused that reply"
        );
        assert_eq!(state(originals[1]), (true, false));
        assert_eq!(
            state(originals[2]),
            (false, false),
            "the forward goes in an hour"
        );
        let s = store.lock().unwrap();
        let answered: Vec<_> = s
            .pending_changes(&account.id)
            .unwrap()
            .into_iter()
            .filter(|c| c.kind == ChangeKind::Answered(true))
            .map(|c| (c.message, c.uid))
            .collect();
        assert_eq!(
            answered,
            [(Some(originals[1]), Some(2))],
            "queued for the server"
        );
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
        assert!(!s.has_outgoing(&account.id, due()).unwrap());
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
            s.has_outgoing(&account.id, due()).unwrap(),
            "both are tried again later"
        );
    }

    #[test]
    fn moving_a_message_out_of_the_outbox_cancels_it() {
        let (store, account, ids) = store_with_queued(&["anna@kowalska-architekci.pl"]);
        let mut s = store.lock().unwrap();
        s.move_to(ids[0], &account.folder_key("inbox")).unwrap();
        assert!(!s.has_outgoing(&account.id, due()).unwrap());
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
                None,
            )
            .unwrap();
        assert!(s.get(ids[0]).is_err());
        let outbox = s.list(&account.folder_key("outbox")).unwrap();
        assert_eq!(outbox.len(), 1);
        assert_eq!(outbox[0].id, new);
    }

    #[tokio::test]
    async fn blind_copies_reach_their_people_and_nobody_sees_them() {
        let (port, server) = start_fake_smtp("nobody@").await;
        let (store, account, _) = store_with_queued(&[]);
        let mut d = draft("anna@kowalska-architekci.pl");
        d.bcc = "Szef <szef@twojastudio.pl>".into();
        let c = compose(&account, &d, &Originals::default(), now()).unwrap();
        assert_eq!(
            c.env_to,
            ["anna@kowalska-architekci.pl", "szef@twojastudio.pl"]
        );
        assert_eq!(
            c.message.bcc, "Szef <szef@twojastudio.pl>",
            "kept in our copy"
        );
        store
            .lock()
            .unwrap()
            .queue_outgoing(
                &account.id,
                &c.message,
                &c.raw,
                &c.env_from,
                &c.env_to,
                None,
                None,
            )
            .unwrap();
        let report = send_outbox(&account, &plain_transport(port), &store)
            .await
            .unwrap();
        assert_eq!(report.sent.len(), 1, "{report:?}");
        let accepted = server.lock().unwrap().accepted.clone();
        assert_eq!(
            accepted[0].1,
            ["anna@kowalska-architekci.pl", "szef@twojastudio.pl"]
        );
        assert!(
            !accepted[0].2.to_ascii_lowercase().contains("bcc"),
            "{}",
            accepted[0].2
        );
        assert!(accepted[0].2.contains("Subject: "));
        // The copy in Sent still says who got a blind copy.
        let s = store.lock().unwrap();
        let sent = s.list(&account.folder_key("sent")).unwrap();
        assert_eq!(s.get(sent[0].id).unwrap().bcc, "Szef <szef@twojastudio.pl>");
    }

    #[test]
    fn bcc_lines_go_and_everything_else_stays() {
        let raw = b"From: a@x.pl\r\nBcc: b@x.pl,\r\n c@x.pl\r\nSubject: Bcc: in the subject\r\n\
BCC: d@x.pl\r\n\r\nBcc: in the body stays\r\n";
        assert_eq!(
            without_bcc(raw),
            b"From: a@x.pl\r\nSubject: Bcc: in the subject\r\n\r\nBcc: in the body stays\r\n"
        );
        let plain = b"From: a@x.pl\r\n\r\nHej\r\n";
        assert_eq!(without_bcc(plain), plain);
    }

    #[test]
    fn files_from_this_pc_go_along() {
        let path = std::env::temp_dir().join(format!("telex-attach-{}.pdf", std::process::id()));
        std::fs::write(&path, b"%PDF-1.4\n").unwrap();
        let mut d = draft("anna@kowalska-architekci.pl");
        d.attachments = vec![Attachment {
            name: "wycena.pdf".into(),
            size_kb: 1,
            path: Some(path.display().to_string()),
        }];
        let c = compose(&account(), &d, &Originals::default(), now()).unwrap();
        let files = attachments_of(&c.raw);
        assert_eq!(files.len(), 1);
        assert_eq!(
            (files[0].name.as_str(), files[0].content_type.as_str()),
            ("wycena.pdf", "application/pdf")
        );
        assert_eq!(files[0].data, b"%PDF-1.4\n");
        // A file gone since it was attached is named.
        std::fs::remove_file(&path).unwrap();
        let err = compose(&account(), &d, &Originals::default(), now()).unwrap_err();
        assert!(err.contains("Couldn't attach \"wycena.pdf\""), "{err}");
    }

    #[tokio::test]
    async fn sending_waits_its_moment_and_can_be_taken_back() {
        let (port, server) = start_fake_smtp("nobody@").await;
        let (store, account, _) = store_with_queued(&[]);
        let queue = |to: &str, wait: i64| {
            let c = compose(&account, &draft(to), &Originals::default(), now()).unwrap();
            store
                .lock()
                .unwrap()
                .queue_outgoing(
                    &account.id,
                    &c.message,
                    &c.raw,
                    &c.env_from,
                    &c.env_to,
                    None,
                    Some(due() + chrono::TimeDelta::seconds(wait)),
                )
                .unwrap()
        };
        let later = queue("anna@kowalska-architekci.pl", 60);
        let taken_back = queue("piotr@nowak-meble.pl", 60);
        assert!(!store
            .lock()
            .unwrap()
            .has_outgoing(&account.id, due())
            .unwrap());
        let report = send_outbox(&account, &plain_transport(port), &store)
            .await
            .unwrap();
        assert!(report.sent.is_empty(), "nothing is due yet: {report:?}");
        assert!(server.lock().unwrap().accepted.is_empty());

        {
            let mut s = store.lock().unwrap();
            s.hold_outgoing(taken_back, due()).unwrap();
            let held = s.get(taken_back).unwrap();
            assert_eq!(
                held.folder,
                account.folder_key("outbox"),
                "it stays to be edited"
            );
            assert!(held.note.unwrap().contains("Press E"));
            assert!(s.hold_outgoing(taken_back, due()).is_err(), "only once");
            // A minute later the other one is due, and too late to take back.
            let a_minute_on = due() + chrono::TimeDelta::seconds(61);
            assert!(s.has_outgoing(&account.id, a_minute_on).unwrap());
            assert!(s.hold_outgoing(later, a_minute_on).is_err());
            assert_eq!(
                s.next_outgoing(&account.id, 0, a_minute_on)
                    .unwrap()
                    .unwrap()
                    .message,
                later,
                "the held one is never sent"
            );
        }
    }

    #[tokio::test]
    async fn mail_sent_later_waits_its_time_and_says_when_it_went_late() {
        use crate::later::{self, Later};
        let (port, server) = start_fake_smtp("nobody@").await;
        let (store, account, _) = store_with_queued(&[]);
        let queue = |to: &str, at| {
            let c = compose(&account, &draft(to), &Originals::default(), now()).unwrap();
            store
                .lock()
                .unwrap()
                .queue_outgoing(
                    &account.id,
                    &c.message,
                    &c.raw,
                    &c.env_from,
                    &c.env_to,
                    None,
                    Some(at),
                )
                .unwrap()
        };
        let monday = later::when(Later::Monday, due()).unwrap();
        let on_monday = queue("anna@kowalska-architekci.pl", monday);
        // Due an hour ago: Telex was closed then.
        let missed_at = due() - chrono::TimeDelta::hours(1);
        let missed = queue("piotr@nowak-meble.pl", missed_at);
        {
            let s = store.lock().unwrap();
            assert_eq!(s.next_scheduled().unwrap(), Some(missed_at));
            assert_eq!(
                s.scheduled_due(due()).unwrap(),
                std::slice::from_ref(&account.id)
            );
            assert_eq!(s.goes_at(on_monday).unwrap(), Some(monday));
            let row = s.summary(on_monday).unwrap();
            assert_eq!(row.goes, Some(monday), "the Outbox list shows when it goes");
            assert_eq!(
                s.inspect_message(on_monday).unwrap().goes,
                Some(monday),
                "and so does the open message"
            );
        }
        let report = send_outbox(&account, &plain_transport(port), &store)
            .await
            .unwrap();
        let due_then = later::short(missed_at);
        assert_eq!(
            report.sent,
            [format!(
                "\"Wycena – październik\" to piotr@nowak-meble.pl, late: it was due {due_then}"
            )]
        );
        assert_eq!(server.lock().unwrap().accepted.len(), 1, "Monday's waits");
        {
            let s = store.lock().unwrap();
            assert_eq!(s.get(missed).unwrap().folder, account.folder_key("sent"));
            assert_eq!(s.next_scheduled().unwrap(), Some(monday));
            assert!(s.scheduled_due(due()).unwrap().is_empty());
            assert!(s.scheduled_due(monday).unwrap().len() == 1);
        }

        // Sent now instead: it goes at once, and not late.
        let owner = store.lock().unwrap().send_now(on_monday).unwrap();
        assert_eq!(owner, account.id);
        let report = send_outbox(&account, &plain_transport(port), &store)
            .await
            .unwrap();
        assert_eq!(
            report.sent,
            ["\"Wycena – październik\" to anna@kowalska-architekci.pl"]
        );
        let mut s = store.lock().unwrap();
        assert_eq!(s.next_scheduled().unwrap(), None);
        assert!(s.send_now(on_monday).is_err(), "it went");
    }

    #[test]
    fn held_mail_can_be_sent_now_but_refused_mail_cannot() {
        let (store, account, ids) = store_with_queued(&["anna@x.pl", "piotr@y.pl"]);
        let mut s = store.into_inner().unwrap();
        let later = due() + chrono::TimeDelta::hours(2);
        let c = compose(&account, &draft("ewa@z.pl"), &Originals::default(), now()).unwrap();
        let scheduled = s
            .queue_outgoing(
                &account.id,
                &c.message,
                &c.raw,
                &c.env_from,
                &c.env_to,
                None,
                Some(later),
            )
            .unwrap();
        s.hold_outgoing(scheduled, due()).unwrap();
        assert_eq!(s.goes_at(scheduled).unwrap(), None, "held: it doesn't go");
        assert_eq!(s.summary(scheduled).unwrap().goes, None);
        assert_eq!(s.next_scheduled().unwrap(), None);
        s.send_now(scheduled).unwrap();
        assert!(
            s.get(scheduled).unwrap().note.is_none(),
            "the held note goes"
        );
        assert!(s.has_outgoing(&account.id, due()).unwrap());
        s.outgoing_failed(ids[0], "550 no such user", true).unwrap();
        let error = s.send_now(ids[0]).unwrap_err().to_string();
        assert!(error.contains("Press E"), "{error}");
        // A failed try (no connection) may go again now.
        s.outgoing_failed(ids[1], "connection refused", false)
            .unwrap();
        s.send_now(ids[1]).unwrap();
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
