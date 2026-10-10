//! Starting a draft: who it goes to, its subject and the text it opens with, for a new
//! message, a reply, a reply to all and a forward. The identity decides the signature
//! and the reply and forward headers (templates with {name}, {email}, {from}, {date},
//! {time} and {subject} of the original).

use serde::{Deserialize, Serialize};

use crate::mime::reply_to_of;
use crate::model::{split_folder_key, Account, Attachment, Identity, Message, MessageId};
use crate::smtp::split_addresses;
use crate::store::{MailStore, StoreError};

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
    pub body: String,
    pub attachments: Vec<Attachment>,
    pub reply_to: Option<MessageId>,
    pub forward_of: Option<MessageId>,
}

/// Start a draft. A new message is from `account`; a reply or forward comes from the
/// account of the message `of`, as the identity it was sent to.
pub fn start(
    store: &MailStore,
    kind: DraftKind,
    of: Option<MessageId>,
    account: &str,
) -> Result<DraftStart, StoreError> {
    let original = match (kind, of) {
        (DraftKind::New, _) => None,
        (_, Some(id)) => Some(store.get(id)?),
        (_, None) => return Err(StoreError::Invalid("Select a message first.".into())),
    };
    let account = store.account(original.as_ref().map_or(account, |m| account_of(&m.folder)))?;
    let identity = match &original {
        Some(m) => identity_for(account, m),
        None => account.identity(None),
    };
    let signature = identity.signature.trim_end().to_owned();
    let mut draft = DraftStart {
        account: account.id.clone(),
        identity: identity.id,
        to: String::new(),
        cc: String::new(),
        subject: String::new(),
        body: layout(&signature, None),
        attachments: Vec::new(),
        reply_to: None,
        forward_of: None,
    };
    let Some(m) = original else {
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
            let header = expand(&identity.reply_header, &vars);
            draft.body = layout(&signature, Some(&format!("{header}\n{}", quote(&m.body))));
            draft.reply_to = Some(m.id);
        }
        DraftKind::Forward => {
            draft.subject = format!("Fwd: {}", strip_prefixes(&m.subject));
            draft.attachments = m.attachments.clone();
            draft.forward_of = Some(m.id);
            let header = expand(&identity.forward_header, &vars);
            draft.body = layout(
                &signature,
                Some(&format!("{header}\n\n{}", text_lines(&m.body).join("\n"))),
            );
        }
        DraftKind::New => {}
    }
    Ok(draft)
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
            name,
            email: m.from_email.clone(),
            date: m.date.format("%d.%m.%Y").to_string(),
            time: m.date.format("%H:%M").to_string(),
            subject: m.subject.clone(),
        }
    }
}

/// Fill in a template. Unknown {words} stay as they are.
fn expand(template: &str, v: &Vars) -> String {
    template
        .replace("{name}", &v.name)
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

fn address_line(name: &str, email: &str) -> String {
    let name = name.trim();
    if name.is_empty() || name == email {
        email.to_owned()
    } else if name.contains([',', ';', '"']) {
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

fn account_of(folder_key: &str) -> &str {
    split_folder_key(folder_key).map_or(folder_key, |(a, _)| a)
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
        let d = start(&store, DraftKind::New, None, "firma").unwrap();
        assert_eq!(d.body, "\n\nPozdrawiam\n");
        assert_eq!(d.account, "firma");
        assert!(d.identity > 0);
    }

    #[test]
    fn a_reply_quotes_with_the_polish_header() {
        let store = sample_store();
        let m = first_in(&store, "firma/inbox", "Poprawki");
        let d = start(&store, DraftKind::Reply, Some(m.id), "firma").unwrap();
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
    fn a_forward_carries_the_text_and_attachments() {
        let store = sample_store();
        let m = store
            .snapshot()
            .unwrap()
            .messages
            .into_iter()
            .find(|s| s.has_attachments && s.folder.ends_with("/inbox"))
            .map(|s| store.get(s.id).unwrap())
            .unwrap();
        let d = start(&store, DraftKind::Forward, Some(m.id), "firma").unwrap();
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
        let d = start(&store, DraftKind::Reply, Some(id), &account.id).unwrap();
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
    fn reply_to_all_leaves_you_out_and_names_everyone_once() {
        let (mut store, account) = store_with_alias();
        let id = deliver(
            &mut store,
            &account,
            "From: Anna <anna@x.pl>\r\nTo: biuro@twojastudio.pl, Piotr <piotr@y.pl>\r\nCc: ANNA@x.pl, jan@twojastudio.pl, ewa@z.pl\r\n",
        );
        let d = start(&store, DraftKind::ReplyAll, Some(id), &account.id).unwrap();
        assert_eq!(d.to, "Anna <anna@x.pl>, Piotr <piotr@y.pl>");
        assert_eq!(d.cc, "ewa@z.pl");
    }

    #[test]
    fn replying_to_your_own_sent_mail_goes_to_its_recipients() {
        let store = sample_store();
        let m = first_in(&store, "firma/sent", "");
        let d = start(&store, DraftKind::Reply, Some(m.id), "firma").unwrap();
        assert_eq!(d.to, split_addresses(&m.to).join(", "));
    }

    #[test]
    fn an_empty_signature_leaves_no_gap() {
        assert_eq!(layout("", None), "\n");
        assert_eq!(layout("", Some("Dnia…")), "\n\nDnia…");
    }
}
