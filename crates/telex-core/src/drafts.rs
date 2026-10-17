//! Starting a draft: who it goes to, its subject and the text it opens with, for a new
//! message, a reply, a reply to all and a forward, as plain text and as HTML. The identity
//! decides the signature (text or HTML), what a new message and a reply open with, and the
//! reply and forward headers (templates with {name}, {first}, {email}, {from}, {date}, {time}
//! and {subject} of the original). A folder's own templates (`FolderTemplate`) take the place
//! of the identity's: the open folder's for a new message, the original's for a reply. A reply
//! or forward in HTML quotes the original's own HTML when it has some (`html::quotable`).

use serde::{Deserialize, Serialize};

use crate::html;
use crate::mime::reply_to_of;
use crate::model::{account_of, Account, Attachment, FolderTemplate, Identity, Message, MessageId};
use crate::smtp::split_addresses;
use crate::store::{MailStore, StoreError, LOCAL_ID};
use crate::tr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DraftKind {
    New,
    Reply,
    ReplyAll,
    Forward,
}

/// What the compose pane opens with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DraftStart {
    pub account: String,
    pub identity: i64,
    pub to: String,
    pub cc: String,
    pub subject: String,
    /// The text in plain text.
    pub body: String,
    /// The same in HTML, for the HTML editor: the signature as written (in its block,
    /// `html::SIGNATURE_CLASS`), and a reply's or forward's original in a blockquote.
    pub html: String,
    /// The line typing starts on (after a greeting): a line of `body`, a top-level block of `html`.
    pub caret_line: usize,
    pub attachments: Vec<Attachment>,
    pub reply_to: Option<MessageId>,
    pub forward_of: Option<MessageId>,
}

/// Start a draft. A new message is from `account`, with the templates of `folder` (the one
/// open, if any); a reply or forward comes from the account of the message `of`, as the
/// identity it was sent to. `sign_replies` false leaves the signature out of replies and
/// forwards (Settings › Writing); new messages always have it.
pub fn start(
    store: &MailStore,
    kind: DraftKind,
    of: Option<MessageId>,
    account: &str,
    folder: Option<&str>,
    sign_replies: bool,
) -> Result<DraftStart, StoreError> {
    let original = match (kind, of) {
        (DraftKind::New, _) => None,
        (_, Some(id)) => Some(store.get(id)?),
        (_, None) => {
            return Err(StoreError::Invalid(tr!(
                "Select a message first.",
                "Najpierw wybierz wiadomość."
            )))
        }
    };
    let account = sender(store, original.as_ref(), account)?;
    let identity = match &original {
        Some(m) => identity_for(account, m),
        None => account.identity(None),
    };
    let signature = if sign_replies || original.is_none() {
        Signature::of(&identity)
    } else {
        Signature::default()
    };
    let template_folder = original
        .as_ref()
        .map_or(folder.unwrap_or_default(), |m| &m.folder);
    let templates = Templates::of(&identity, store.folder_template(template_folder)?.as_ref());
    let mut draft = DraftStart {
        account: account.id.clone(),
        identity: identity.id,
        to: String::new(),
        cc: String::new(),
        subject: String::new(),
        body: String::new(),
        html: String::new(),
        caret_line: 0,
        attachments: Vec::new(),
        reply_to: None,
        forward_of: None,
    };
    let Some(m) = original else {
        let greeting = expand(&templates.new, &Vars::today());
        draft.lay_out(&greeting, &signature, None);
        return Ok(draft);
    };
    let vars = Vars::of(&m);
    match kind {
        DraftKind::Reply | DraftKind::ReplyAll => {
            let outgoing = store.kind_of(&m.folder).is_some_and(|k| k.is_outgoing());
            let raw = store.raw_of(m.id)?;
            // Replying to your own message goes to its recipients again.
            let primary = if outgoing {
                m.to.clone()
            } else {
                raw.as_deref()
                    .and_then(reply_to_of)
                    .unwrap_or_else(|| address_line(&m.from_name, &m.from_email))
            };
            let mut to = split_addresses(&primary);
            let mut cc = Vec::new();
            if kind == DraftKind::ReplyAll {
                let own: Vec<String> = account
                    .identities
                    .iter()
                    .map(|i| key(&i.email))
                    .chain([key(&account.email)])
                    .collect();
                let mut seen: Vec<String> = to.iter().map(|a| key(a)).collect();
                let mut add = |list: &mut Vec<String>, line: &str| {
                    for a in split_addresses(line) {
                        let k = key(&a);
                        if !own.contains(&k) && !seen.contains(&k) {
                            seen.push(k);
                            list.push(a);
                        }
                    }
                };
                if !outgoing {
                    add(&mut to, &m.to);
                }
                add(&mut cc, &m.cc);
            }
            draft.to = to.join(", ");
            draft.cc = cc.join(", ");
            draft.subject = format!("Re: {}", strip_prefixes(&m.subject));
            let header = expand(&templates.reply_header, &vars);
            let quoted = match raw.as_deref().and_then(html::quotable) {
                Some(original) => format!("<blockquote>{original}</blockquote>"),
                None => html::from_text(&quote(&m.body)),
            };
            let tail = Tail {
                text: format!("{header}\n{}", quote(&m.body)),
                html: format!("{}{quoted}", html::from_text(&header)),
            };
            draft.lay_out(&expand(&templates.reply, &vars), &signature, Some(tail));
            draft.reply_to = Some(m.id);
        }
        DraftKind::Forward => {
            draft.subject = format!("Fwd: {}", strip_prefixes(&m.subject));
            draft.attachments = m.attachments.clone();
            draft.forward_of = Some(m.id);
            let header = expand(&identity.forward_header, &vars);
            let text = text_lines(&m.body).join("\n");
            let original = store
                .raw_of(m.id)?
                .as_deref()
                .and_then(html::quotable)
                .unwrap_or_else(|| html::from_text(&text));
            let tail = Tail {
                text: format!("{header}\n\n{text}"),
                html: format!(
                    "{}<blockquote>{original}</blockquote>",
                    html::from_text(&header)
                ),
            };
            draft.lay_out("", &signature, Some(tail));
        }
        DraftKind::New => {}
    }
    Ok(draft)
}

/// The account a draft is from: the original's, or for a new message the one asked for. Mail
/// on this PC belongs to no server, so a reply to it goes from the account whose address or
/// identity it was sent to (or from), else from the one asked for, else the first that can.
fn sender<'s>(
    store: &'s MailStore,
    original: Option<&Message>,
    asked: &str,
) -> Result<&'s Account, StoreError> {
    let servers = || store.accounts().iter().filter(|a| a.imap.is_some());
    if let Some(m) = original {
        let own = store.account(account_of(&m.folder))?;
        if own.id != LOCAL_ID {
            return Ok(own);
        }
        let addressed: Vec<String> =
            split_addresses(&format!("{}, {}, {}", m.to, m.cc, m.from_email))
                .iter()
                .map(|a| key(a))
                .collect();
        let mine = |a: &&Account| {
            addressed.contains(&key(&a.email))
                || a.identities
                    .iter()
                    .any(|i| addressed.contains(&key(&i.email)))
        };
        if let Some(a) = servers().find(mine) {
            return Ok(a);
        }
    }
    match store.account(asked) {
        Ok(a) if a.id != LOCAL_ID => Ok(a),
        _ => servers()
            .next()
            .ok_or_else(|| StoreError::UnknownAccount(asked.to_owned())),
    }
}

/// The identity a reply or forward is from: the one the message was sent to (an alias),
/// or for your own messages the one that sent it, else the account's default.
fn identity_for(account: &Account, m: &Message) -> Identity {
    let addressed: Vec<String> = split_addresses(&format!("{}, {}, {}", m.to, m.cc, m.from_email))
        .iter()
        .map(|a| key(a))
        .collect();
    account
        .identities
        .iter()
        .filter(|i| !i.is_default)
        .find(|i| addressed.contains(&key(&i.email)))
        .cloned()
        .unwrap_or_else(|| account.identity(None))
}

/// The signature a draft opens with, as text and as HTML (empty: none).
#[derive(Default)]
struct Signature {
    text: String,
    html: String,
}

impl Signature {
    fn of(identity: &Identity) -> Self {
        let text = identity.signature.trim_end().to_owned();
        let html = match &identity.signature_html {
            Some(h) if !h.trim().is_empty() => h.clone(),
            _ => html::from_text(&text),
        };
        Self { text, html }
    }
}

/// The templates that apply: a folder's own where it has them, else the identity's.
struct Templates {
    new: String,
    reply: String,
    reply_header: String,
}

impl Templates {
    fn of(identity: &Identity, folder: Option<&FolderTemplate>) -> Self {
        let pick = |own: Option<&String>, theirs: &String| match own {
            Some(t) if !t.trim().is_empty() => t.trim_end().to_owned(),
            _ => theirs.trim_end().to_owned(),
        };
        Self {
            new: pick(folder.map(|f| &f.new_template), &identity.new_template),
            reply: pick(folder.map(|f| &f.reply_template), &identity.reply_template),
            reply_header: pick(folder.map(|f| &f.reply_header), &identity.reply_header),
        }
    }
}

/// What follows the signature: the reply's quote or the forwarded message, as text and HTML.
struct Tail {
    text: String,
    html: String,
}

impl DraftStart {
    /// The text a draft opens with: a greeting, room to type, the signature, then the quote
    /// or forward. The same in HTML, line for line, with the signature as written.
    fn lay_out(&mut self, greeting: &str, signature: &Signature, tail: Option<Tail>) {
        let greeting = greeting.trim_end();
        let head = if greeting.is_empty() {
            String::new()
        } else {
            format!("{greeting}\n\n")
        };
        self.caret_line = if greeting.is_empty() {
            0
        } else {
            greeting.split('\n').count() + 1
        };
        self.body = head.clone() + &layout(&signature.text, tail.as_ref().map(|t| t.text.as_str()));
        let mut html = html::from_text(&format!("{head}\n"));
        if !signature.text.is_empty() {
            html.push_str(&format!(
                "<div><br></div><div class=\"{}\">{}</div>",
                html::SIGNATURE_CLASS,
                signature.html
            ));
        }
        if let Some(tail) = tail {
            html.push_str("<div><br></div>");
            html.push_str(&tail.html);
        }
        self.html = html;
    }
}

/// The text a draft opens with: room to type, the signature, then the quote or forward.
fn layout(signature: &str, tail: Option<&str>) -> String {
    match (signature.is_empty(), tail) {
        (false, None) => format!("\n\n{signature}\n"),
        (true, None) => "\n".to_owned(),
        (false, Some(tail)) => format!("\n\n{signature}\n\n{tail}"),
        (true, Some(tail)) => format!("\n\n{tail}"),
    }
}

/// The values templates can use.
struct Vars {
    name: String,
    /// The first word of the name ("Anna" of "Anna Kowalska"); empty without a name.
    first: String,
    email: String,
    from: String,
    date: String,
    time: String,
    subject: String,
}

impl Vars {
    fn of(m: &Message) -> Self {
        let name = if m.from_name.trim().is_empty() {
            m.from_email.clone()
        } else {
            m.from_name.clone()
        };
        Self {
            from: address_line(&m.from_name, &m.from_email),
            first: m
                .from_name
                .split_whitespace()
                .next()
                .filter(|w| !w.contains('@'))
                .unwrap_or_default()
                .trim_matches(['"', ','])
                .to_owned(),
            name,
            email: m.from_email.clone(),
            date: m.date.format("%d.%m.%Y").to_string(),
            time: m.date.format("%H:%M").to_string(),
            subject: m.subject.clone(),
        }
    }
}

impl Vars {
    /// A new message has no original: only today's date and time.
    fn today() -> Self {
        let now = chrono::Local::now();
        Self {
            name: String::new(),
            first: String::new(),
            email: String::new(),
            from: String::new(),
            date: now.format("%d.%m.%Y").to_string(),
            time: now.format("%H:%M").to_string(),
            subject: String::new(),
        }
    }
}

/// Fill in a template. Unknown {words} stay as they are.
fn expand(template: &str, v: &Vars) -> String {
    template
        .replace("{name}", &v.name)
        .replace("{first}", &v.first)
        .replace("{email}", &v.email)
        .replace("{from}", &v.from)
        .replace("{date}", &v.date)
        .replace("{time}", &v.time)
        .replace("{subject}", &v.subject)
}

/// "> " before every line; a line that is already a quote gets one more ">".
fn quote(body: &str) -> String {
    text_lines(body)
        .iter()
        .map(|l| {
            if l.starts_with('>') {
                format!(">{l}")
            } else {
                format!("> {l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The lines of a body, without the sample mailbox's image placeholders.
fn text_lines(body: &str) -> Vec<&str> {
    body.trim_end_matches('\n')
        .split('\n')
        .filter(|l| !l.starts_with("[img:"))
        .collect()
}

/// Words that say something is attached, folded: "attached", "attachment", "enclosed",
/// "załączam", "w załączniku", "załączone".
const ATTACHED: &[&str] = &["attach", "enclos", "zalacz"];

/// Whether the text the user wrote says a file is attached, so Send can ask when none is.
/// Quoted lines and the line that introduces them, a forwarded message and the signature
/// (often a disclaimer about attachments) are left out.
pub fn mentions_attachment(text: &str, signature: &str) -> bool {
    let signature = signature.trim();
    let text = match text.find(signature) {
        Some(at) if !signature.is_empty() => {
            format!("{}{}", &text[..at], &text[at + signature.len()..])
        }
        _ => text.to_owned(),
    };
    let lines: Vec<&str> = text.split('\n').collect();
    let quoted = |l: &str| l.trim_start().starts_with('>');
    let mut own = String::new();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        // "-------- Forwarded message --------": what follows isn't the user's.
        if trimmed.starts_with("-----") && trimmed.ends_with("-----") && trimmed.len() > 12 {
            break;
        }
        let introduces = trimmed.ends_with(':') && lines.get(i + 1).is_some_and(|n| quoted(n));
        if !quoted(line) && !introduces {
            own.push_str(line);
            own.push('\n');
        }
    }
    crate::filters::fold(&own)
        .split(|c: char| !c.is_alphanumeric())
        .any(|w| ATTACHED.iter().any(|stem| w.starts_with(stem)))
}

/// "Re: Odp: FW: Oferta" → "Oferta". Covers English, Polish (Odp, PD) and German (AW, WG).
fn strip_prefixes(subject: &str) -> String {
    let mut s = subject.trim();
    loop {
        let lower = s.to_ascii_lowercase();
        let Some(prefix) = ["re:", "fwd:", "fw:", "odp:", "pd:", "aw:", "wg:"]
            .iter()
            .find(|p| lower.starts_with(*p))
        else {
            return s.to_owned();
        };
        s = s[prefix.len()..].trim_start();
    }
}

pub(crate) fn address_line(name: &str, email: &str) -> String {
    let name = name.trim();
    if name.is_empty() || name.eq_ignore_ascii_case(email) {
        email.to_owned()
    } else if name.contains([',', ';', '"', '@', '<', '>']) {
        format!("\"{}\" <{email}>", name.replace('"', "'"))
    } else {
        format!("{name} <{email}>")
    }
}

/// The address part, lowercased, for comparing: "Anna <A@x.pl>" → "a@x.pl".
fn key(address: &str) -> String {
    let a = address.trim();
    let inner = match (a.rfind('<'), a.rfind('>')) {
        (Some(open), Some(close)) if open < close => &a[open + 1..close],
        _ => a,
    };
    inner.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AccountSetup, Folder, FolderKind};
    use crate::sample::sample_store;
    use crate::store::SyncedMessage;

    fn first_in(store: &MailStore, folder: &str, subject: &str) -> Message {
        let id = store
            .list(folder)
            .unwrap()
            .into_iter()
            .find(|m| m.subject.contains(subject))
            .unwrap_or_else(|| panic!("no {subject} in {folder}"))
            .id;
        store.get(id).unwrap()
    }

    #[test]
    fn a_new_message_opens_with_the_signature() {
        let store = sample_store();
        let d = start(&store, DraftKind::New, None, "firma", None, true).unwrap();
        assert_eq!(d.body, "\n\nPozdrawiam\n");
        assert_eq!(d.account, "firma");
        assert!(d.identity > 0);
    }

    #[test]
    fn a_reply_quotes_with_the_polish_header() {
        let store = sample_store();
        let m = first_in(&store, "firma/inbox", "Poprawki");
        let d = start(&store, DraftKind::Reply, Some(m.id), "firma", None, true).unwrap();
        assert_eq!(d.to, "Anna Kowalska <anna@kowalska-architekci.pl>");
        assert_eq!(d.subject, format!("Re: {}", m.subject));
        assert_eq!(d.reply_to, Some(m.id));
        let header = format!(
            "Dnia {} o {}, Anna Kowalska napisał(a):",
            m.date.format("%d.%m.%Y"),
            m.date.format("%H:%M")
        );
        assert!(
            d.body
                .starts_with(&format!("\n\nPozdrawiam\n\n{header}\n> ")),
            "{:?}",
            d.body
        );
        assert!(d.body.contains("> Dzień dobry,"));
    }

    #[test]
    fn subjects_dont_pile_up_prefixes() {
        assert_eq!(strip_prefixes("Re: Odp: FW: Oferta"), "Oferta");
        assert_eq!(strip_prefixes("PD:Faktura"), "Faktura");
        assert_eq!(
            strip_prefixes("Reklamacja"),
            "Reklamacja",
            "words that start like a prefix stay"
        );
    }

    #[test]
    fn a_mention_of_an_attachment_is_found_in_the_users_own_text() {
        let none = "";
        assert!(mentions_attachment(
            "Hi Anna,\nthe plans are attached.",
            none
        ));
        assert!(mentions_attachment(
            "Dzień dobry,\nw załączniku przesyłam wycenę.",
            none
        ));
        assert!(mentions_attachment("Załączam umowę do podpisu.", none));
        assert!(mentions_attachment(
            "Please see the enclosed invoice.",
            none
        ));
        assert!(
            !mentions_attachment("Dołącz do spotkania o 10.", none),
            "joining isn't attaching"
        );
        assert!(!mentions_attachment("Thanks, see you on Friday.", none));
        // What someone else wrote, and a forwarded message, aren't the user's.
        let reply =
            "Got it, thanks!\n\nDnia 10.10.2026 o 06:12, Anna napisała:\n> W załączniku plany.";
        assert!(!mentions_attachment(reply, none));
        let forward =
            "FYI\n\n-------- Forwarded message --------\nSubject: Plans\nThe plans are attached.";
        assert!(!mentions_attachment(forward, none));
        // A signature's disclaimer about attachments doesn't count; the text above it does.
        let signature = "Sam Carter\nThis email and any attachments are confidential.";
        assert!(!mentions_attachment(
            &format!("See you tomorrow.\n\n{signature}"),
            signature
        ));
        assert!(mentions_attachment(
            &format!("Attached: the plans.\n\n{signature}"),
            signature
        ));
    }

    #[test]
    fn a_forward_carries_the_text_and_attachments() {
        let store = sample_store();
        let m = ["firma/inbox", "gmail/inbox"]
            .iter()
            .flat_map(|f| store.list(f).unwrap())
            .find(|s| s.has_attachments)
            .map(|s| store.get(s.id).unwrap())
            .unwrap();
        let d = start(&store, DraftKind::Forward, Some(m.id), "firma", None, true).unwrap();
        assert!(d.subject.starts_with("Fwd: "));
        assert_eq!(d.attachments, m.attachments);
        assert_eq!(d.forward_of, Some(m.id));
        assert!(d
            .body
            .contains("-------- Przekazana wiadomość --------\nOd: "));
        assert!(!d.body.contains("[img:"));
        assert!(d.to.is_empty());
    }

    /// A server account with two identities (the default biuro@ and the alias jan@).
    fn store_with_alias() -> (MailStore, Account) {
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
        store
            .set_folders(
                &account.id,
                &[Folder {
                    id: "inbox".into(),
                    name: "Inbox".into(),
                    kind: FolderKind::Inbox,
                    remote: Some("INBOX".into()),
                }],
            )
            .unwrap();
        let mut jan = Identity::new(&account.id, "Jan Kowalski", "jan@twojastudio.pl");
        jan.is_default = false;
        jan.signature = "Jan Kowalski\nStudio".into();
        jan.reply_header = "On {date}, {from} wrote:".into();
        store.save_identity(&jan).unwrap();
        let account = store.account(&account.id).unwrap().clone();
        (store, account)
    }

    fn deliver(store: &mut MailStore, account: &Account, headers: &str) -> MessageId {
        let raw = format!(
            "{headers}Subject: Pytanie\r\nDate: Fri, 09 Oct 2026 14:30:00 +0000\r\n\r\nHej\r\n"
        );
        let mut message = crate::mime::parse_message(raw.as_bytes(), "", None);
        message.folder = account.folder_key("inbox");
        store
            .add_synced(
                &account.folder_key("inbox"),
                &[SyncedMessage {
                    uid: 1 + store.list(&account.folder_key("inbox")).unwrap().len() as u32,
                    message,
                    raw: raw.into_bytes(),
                }],
            )
            .unwrap()[0]
    }

    #[test]
    fn replies_go_to_reply_to_and_come_from_the_alias_it_was_sent_to() {
        let (mut store, account) = store_with_alias();
        let id = deliver(
            &mut store,
            &account,
            "From: Formularz <www@sklep.pl>\r\nReply-To: Anna Klientka <anna@x.pl>\r\nTo: jan@twojastudio.pl\r\n",
        );
        let d = start(&store, DraftKind::Reply, Some(id), &account.id, None, true).unwrap();
        assert_eq!(d.to, "Anna Klientka <anna@x.pl>", "Reply-To wins over From");
        let jan = account
            .identities
            .iter()
            .find(|i| i.email == "jan@twojastudio.pl")
            .unwrap();
        assert_eq!(d.identity, jan.id);
        assert!(
            d.body.starts_with(
                "\n\nJan Kowalski\nStudio\n\nOn 09.10.2026, Formularz <www@sklep.pl> wrote:\n> Hej"
            ),
            "{:?}",
            d.body
        );
    }

    #[test]
    fn replies_to_mail_on_this_pc_come_from_the_account_it_was_sent_to() {
        let (mut store, account) = store_with_alias();
        let id = deliver(
            &mut store,
            &account,
            "From: Anna <anna@x.pl>\r\nTo: jan@twojastudio.pl\r\n",
        );
        store.move_to(id, "local/archive").unwrap();
        let d = start(&store, DraftKind::Reply, Some(id), LOCAL_ID, None, true).unwrap();
        assert_eq!(d.account, account.id);
        let jan = account
            .identities
            .iter()
            .find(|i| i.email == "jan@twojastudio.pl")
            .unwrap();
        assert_eq!(d.identity, jan.id, "the alias it was sent to");
        // A new message while a folder on this PC is open: from an account that can send.
        let d = start(&store, DraftKind::New, None, LOCAL_ID, None, true).unwrap();
        assert_eq!(d.account, account.id);
    }

    #[test]
    fn reply_to_all_leaves_you_out_and_names_everyone_once() {
        let (mut store, account) = store_with_alias();
        let id = deliver(
            &mut store,
            &account,
            "From: Anna <anna@x.pl>\r\nTo: biuro@twojastudio.pl, Piotr <piotr@y.pl>\r\nCc: ANNA@x.pl, jan@twojastudio.pl, ewa@z.pl\r\n",
        );
        let d = start(
            &store,
            DraftKind::ReplyAll,
            Some(id),
            &account.id,
            None,
            true,
        )
        .unwrap();
        assert_eq!(d.to, "Anna <anna@x.pl>, Piotr <piotr@y.pl>");
        assert_eq!(d.cc, "ewa@z.pl");
    }

    #[test]
    fn replying_to_your_own_sent_mail_goes_to_its_recipients() {
        let store = sample_store();
        let m = first_in(&store, "firma/sent", "");
        let d = start(&store, DraftKind::Reply, Some(m.id), "firma", None, true).unwrap();
        assert_eq!(d.to, split_addresses(&m.to).join(", "));
    }

    #[test]
    fn greetings_come_from_the_identity_unless_the_folder_has_its_own() {
        let (mut store, account) = store_with_alias();
        let mut biuro = account.identity(None);
        biuro.new_template = "Dzień dobry,".into();
        biuro.reply_template = "Dzień dobry {first},\ndziękuję za wiadomość.".into();
        store.save_identity(&biuro).unwrap();
        let inbox = account.folder_key("inbox");

        let d = start(
            &store,
            DraftKind::New,
            None,
            &account.id,
            Some(&inbox),
            true,
        )
        .unwrap();
        assert_eq!(d.body, "Dzień dobry,\n\n\n\nPozdrawiam\n");
        assert_eq!(d.caret_line, 2, "below the greeting and a blank line");
        assert!(
            d.html.starts_with("<div>Dzień dobry,</div><div><br></div><div><br></div><div><br></div><div class=\"telex-signature\"><div>Pozdrawiam</div></div>"),
            "{}",
            d.html
        );

        let id = deliver(
            &mut store,
            &account,
            "From: Anna Kowalska <anna@x.pl>\r\nTo: biuro@twojastudio.pl\r\n",
        );
        let d = start(&store, DraftKind::Reply, Some(id), &account.id, None, true).unwrap();
        assert!(
            d.body.starts_with(
                "Dzień dobry Anna,\ndziękuję za wiadomość.\n\n\n\nPozdrawiam\n\nDnia 09.10.2026"
            ),
            "{:?}",
            d.body
        );
        assert_eq!(d.caret_line, 3);

        // The Inbox's own templates win; the one left empty is still the identity's.
        store
            .save_folder_template(&FolderTemplate {
                folder: inbox.clone(),
                new_template: "Hello,".into(),
                reply_template: String::new(),
                reply_header: "On {date} {name} wrote:".into(),
            })
            .unwrap();
        let d = start(
            &store,
            DraftKind::New,
            None,
            &account.id,
            Some(&inbox),
            true,
        )
        .unwrap();
        assert!(d.body.starts_with("Hello,\n\n"), "{:?}", d.body);
        let d = start(&store, DraftKind::Reply, Some(id), &account.id, None, true).unwrap();
        assert!(
            d.body.starts_with("Dzień dobry Anna,\n")
                && d.body.contains("On 09.10.2026 Anna Kowalska wrote:\n> Hej"),
            "{:?}",
            d.body
        );
        // Another folder, or none: the identity's.
        let d = start(&store, DraftKind::New, None, &account.id, None, true).unwrap();
        assert!(d.body.starts_with("Dzień dobry,\n"));
        assert_eq!(store.folder_templates().unwrap().len(), 1);
        store
            .save_folder_template(&FolderTemplate {
                folder: inbox.clone(),
                ..FolderTemplate::default()
            })
            .unwrap();
        assert!(
            store.folder_templates().unwrap().is_empty(),
            "all empty: gone"
        );
        assert!(store
            .save_folder_template(&FolderTemplate {
                folder: account.folder_key("nowhere"),
                new_template: "x".into(),
                ..FolderTemplate::default()
            })
            .is_err());
    }

    #[test]
    fn an_html_signature_goes_in_as_written_and_as_text_in_plain_mail() {
        let (mut store, account) = store_with_alias();
        let mut biuro = account.identity(None);
        biuro.signature_html = Some(
            "<div><b>Anna Łęcka</b></div><div><span style=\"color:#1d4ed8\">Studio</span> \
             <img src=\"data:image/png;base64,iVBORw0KGgo=\"></div><script>x()</script>"
                .into(),
        );
        let saved = store.save_identity(&biuro).unwrap();
        assert_eq!(
            saved.signature, "Anna Łęcka\nStudio",
            "its text, for plain mail"
        );
        let html = saved.signature_html.clone().unwrap();
        assert!(
            !html.contains("script") && html.contains("<b>Anna Łęcka</b>"),
            "{html}"
        );
        let d = start(&store, DraftKind::New, None, &account.id, None, true).unwrap();
        assert_eq!(d.body, "\n\nAnna Łęcka\nStudio\n");
        assert!(
            d.html
                .contains(&format!("<div class=\"telex-signature\">{html}</div>")),
            "{}",
            d.html
        );

        // Typed without formatting, a signature stays plain text.
        let plain = store
            .save_identity(&Identity {
                signature_html: Some("<div>Pozdrawiam</div><div>Anna</div>".into()),
                ..saved.clone()
            })
            .unwrap();
        assert_eq!(
            (plain.signature.as_str(), plain.signature_html),
            ("Pozdrawiam\nAnna", None)
        );
        // A picture too large for a signature is said, not dropped quietly.
        let big = format!(
            "<img src=\"data:image/png;base64,{}\">",
            "A".repeat(300_000)
        );
        let err = store
            .save_identity(&Identity {
                signature_html: Some(big),
                ..saved
            })
            .unwrap_err();
        assert!(err.to_string().contains("200 KB"), "{err}");
    }

    #[test]
    fn a_reply_in_html_quotes_the_originals_html() {
        let (mut store, account) = store_with_alias();
        let raw = "From: Anna <anna@x.pl>\r\nTo: biuro@twojastudio.pl\r\nSubject: Plany\r\n\
Date: Fri, 09 Oct 2026 14:30:00 +0000\r\nMIME-Version: 1.0\r\n\
Content-Type: multipart/alternative; boundary=\"b\"\r\n\r\n\
--b\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nPlany na *piątek*\r\n\
--b\r\nContent-Type: text/html; charset=utf-8\r\n\r\n\
<p style=\"color:#fff;background:#000\">Plany na <b>piątek</b></p><img src=\"https://t.pl/x.gif\">\r\n\
--b--\r\n";
        let mut message = crate::mime::parse_message(raw.as_bytes(), "", None);
        message.folder = account.folder_key("inbox");
        let id = store
            .add_synced(
                &account.folder_key("inbox"),
                &[SyncedMessage {
                    uid: 1,
                    message,
                    raw: raw.as_bytes().to_vec(),
                }],
            )
            .unwrap()[0];
        let d = start(&store, DraftKind::Reply, Some(id), &account.id, None, true).unwrap();
        assert!(
            d.html.ends_with(
                "napisał(a):</div><blockquote><p>Plany na <b>piątek</b></p></blockquote>"
            ),
            "{}",
            d.html
        );
        assert!(
            d.body.contains("> Plany na *piątek*"),
            "the text quotes the text"
        );
        let d = start(
            &store,
            DraftKind::Forward,
            Some(id),
            &account.id,
            None,
            true,
        )
        .unwrap();
        assert!(
            d.html.ends_with(
                "<div>Temat: Plany</div><blockquote><p>Plany na <b>piątek</b></p></blockquote>"
            ),
            "{}",
            d.html
        );
        // Plain-text mail: its lines, quoted as before.
        let plain = deliver(&mut store, &account, "From: Ewa <ewa@y.pl>\r\n");
        let d = start(
            &store,
            DraftKind::Reply,
            Some(plain),
            &account.id,
            None,
            false,
        )
        .unwrap();
        assert!(
            d.html
                .ends_with("napisał(a):</div><blockquote><div>Hej</div></blockquote>"),
            "{}",
            d.html
        );
    }

    #[test]
    fn an_empty_signature_leaves_no_gap() {
        assert_eq!(layout("", None), "\n");
        assert_eq!(layout("", Some("Dnia…")), "\n\nDnia…");
    }

    #[test]
    fn replies_can_go_without_the_signature() {
        let store = sample_store();
        let m = first_in(&store, "firma/inbox", "Poprawki");
        let d = start(&store, DraftKind::Reply, Some(m.id), "firma", None, false).unwrap();
        assert!(d.body.starts_with("\n\nDnia "), "{:?}", d.body);
        let d = start(&store, DraftKind::Forward, Some(m.id), "firma", None, false).unwrap();
        assert!(
            d.body.starts_with("\n\n-------- Przekazana"),
            "{:?}",
            d.body
        );
        // A new message keeps it: the setting is about replies and forwards.
        let d = start(&store, DraftKind::New, None, "firma", None, false).unwrap();
        assert_eq!(d.body, "\n\nPozdrawiam\n");
    }
}
