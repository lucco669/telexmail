//! Rules, Telex's answer to The Bat!'s Sorting Office: conditions on a message, and what to
//! do when they match.
//!
//! Rules run on mail that arrives in an Inbox (never on an account's first download, so old
//! mail isn't touched) and on demand on a folder. Every message a rule touches gets a note
//! that says which rule did what. Moves and flags go to the server like the user's own
//! changes. Auto-replies only answer new mail, and follow RFC 3834 so they never answer
//! mailing lists, bulk mail, other robots or the same sender twice in a few days.

use chrono::{DateTime, Local, TimeDelta};
use serde::{Deserialize, Serialize};

use crate::mime::auto_reply_blocker;
use crate::model::{split_folder_key, Account, FolderKind, Message, MessageId, OutgoingDraft};
use crate::smtp::{self, Originals};
use crate::store::{MailStore, StoreError, LOCAL_ID};
use crate::tr;

/// How long a rule waits before auto-replying to the same sender again.
pub const AUTO_REPLY_EVERY: TimeDelta = TimeDelta::days(4);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    #[serde(default)]
    pub id: i64,
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// An account id, or `None` for every account.
    #[serde(default)]
    pub account: Option<String>,
    /// Every condition must match (`true`), or any one (`false`).
    #[serde(default = "yes")]
    pub match_all: bool,
    pub conditions: Vec<Condition>,
    pub actions: Vec<Action>,
    /// Later rules don't run on a message this one matched.
    #[serde(default)]
    pub stop: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Condition {
    pub field: Field,
    pub op: Op,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    /// The sender's address or name.
    From,
    /// Any recipient in To or Cc, by address or name.
    To,
    Subject,
    Body,
    /// Any of the above.
    Anywhere,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    Contains,
    NotContains,
    Is,
    StartsWith,
    EndsWith,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Move to this folder ("account/folder") of the rule's account.
    Move {
        folder: String,
    },
    MarkRead,
    Flag,
    /// Move to Trash.
    Delete,
    /// Answer the sender with this text. New mail only.
    AutoReply {
        body: String,
    },
}

impl Rule {
    /// Catch mistakes before the rule is saved, in words for the rule editor.
    pub fn check(&self, accounts: &[Account]) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err(tr!("Give the rule a name.", "Nadaj regule nazwę."));
        }
        if self.conditions.is_empty() {
            return Err(tr!(
                "Add at least one condition.",
                "Dodaj co najmniej jeden warunek."
            ));
        }
        if self.conditions.iter().any(|c| c.value.trim().is_empty()) {
            return Err(tr!(
                "Every condition needs some text to look for.",
                "Każdy warunek potrzebuje szukanego tekstu."
            ));
        }
        if self.actions.is_empty() {
            return Err(tr!(
                "Add at least one action.",
                "Dodaj co najmniej jedno działanie."
            ));
        }
        let account =
            match &self.account {
                Some(id) => Some(accounts.iter().find(|a| &a.id == id).ok_or_else(|| {
                    tr!("That account no longer exists.", "Tego konta już nie ma.")
                })?),
                None => None,
            };
        let destinations = self
            .actions
            .iter()
            .filter(|a| matches!(a, Action::Move { .. } | Action::Delete))
            .count();
        if destinations > 1 {
            return Err(tr!(
                "A rule can move a message to one place only.",
                "Reguła może przenieść wiadomość tylko w jedno miejsce."
            ));
        }
        for action in &self.actions {
            match action {
                // A folder on this PC takes mail from any account; a server folder only from
                // its own.
                Action::Move { folder } if is_local(folder) => {
                    let here = accounts.iter().find(|a| a.id == LOCAL_ID);
                    if !here
                        .is_some_and(|a| a.folders.iter().any(|f| &a.folder_key(&f.id) == folder))
                    {
                        return Err(tr!(
                            "Pick the folder to move to.",
                            "Wybierz folder, do którego przenieść."
                        ));
                    }
                }
                Action::Move { folder } => {
                    let account = account.ok_or_else(|| {
                        tr!(
                            "To move mail, pick the rule's account.",
                            "Żeby przenosić pocztę, wybierz konto reguły."
                        )
                    })?;
                    if !account
                        .folders
                        .iter()
                        .any(|f| &account.folder_key(&f.id) == folder)
                    {
                        return Err(tr!(
                            "Pick the folder to move to.",
                            "Wybierz folder, do którego przenieść."
                        ));
                    }
                }
                Action::AutoReply { body } => {
                    if account.is_none() {
                        return Err(tr!(
                            "To answer mail, pick the account that answers.",
                            "Żeby odpowiadać na pocztę, wybierz konto, które odpowiada."
                        ));
                    }
                    if body.trim().is_empty() {
                        return Err(tr!(
                            "Write the text of the automatic reply.",
                            "Napisz treść automatycznej odpowiedzi."
                        ));
                    }
                }
                Action::MarkRead | Action::Flag | Action::Delete => {}
            }
        }
        Ok(())
    }

    pub fn matches(&self, m: &Message) -> bool {
        let mut results = self.conditions.iter().map(|c| c.matches(m));
        if self.match_all {
            results.all(|hit| hit)
        } else {
            results.any(|hit| hit)
        }
    }
}

impl Condition {
    /// Case and diacritics don't matter: "faktura" finds "Fakturą".
    pub fn matches(&self, m: &Message) -> bool {
        let needle = fold(self.value.trim());
        let candidates = self.candidates(m);
        let mut hits = candidates.iter().map(|c| {
            let c = fold(c);
            match self.op {
                Op::Contains | Op::NotContains => c.contains(&needle),
                Op::Is => c == needle,
                Op::StartsWith => c.starts_with(&needle),
                Op::EndsWith => c.ends_with(&needle),
            }
        });
        if self.op == Op::NotContains {
            !hits.any(|hit| hit)
        } else {
            hits.any(|hit| hit)
        }
    }

    /// The texts a condition looks at. Addresses count by address and by name, so
    /// "From is anna@x.pl" works as well as "From contains Kowalska".
    fn candidates(&self, m: &Message) -> Vec<String> {
        let from = || vec![m.from_email.clone(), m.from_name.clone()];
        let to = || {
            smtp::split_addresses(&format!("{}, {}", m.to, m.cc))
                .into_iter()
                .flat_map(|a| match a.split_once('<') {
                    Some((name, rest)) => vec![
                        rest.trim_end_matches('>').trim().to_owned(),
                        name.trim().trim_matches('"').to_owned(),
                    ],
                    None => vec![a],
                })
                .collect::<Vec<_>>()
        };
        match self.field {
            Field::From => from(),
            Field::To => to(),
            Field::Subject => vec![m.subject.clone()],
            Field::Body => vec![m.body.clone()],
            Field::Anywhere => {
                let mut all = from();
                all.extend(to());
                all.push(m.subject.clone());
                all.push(m.body.clone());
                all
            }
        }
    }
}

/// Lowercase without diacritics, for matching: "Fakturą Łódź" → "faktura lodz".
pub fn fold(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'ą' | 'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' => 'a',
            'ć' | 'ç' | 'č' => 'c',
            'ď' => 'd',
            'ę' | 'è' | 'é' | 'ê' | 'ë' | 'ě' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ł' => 'l',
            'ń' | 'ñ' | 'ň' => 'n',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'ø' => 'o',
            'ř' => 'r',
            'ś' | 'š' | 'ß' => 's',
            'ť' => 't',
            'ù' | 'ú' | 'û' | 'ü' | 'ů' => 'u',
            'ý' | 'ÿ' => 'y',
            'ź' | 'ż' | 'ž' => 'z',
            other => other,
        })
        .collect()
}

/// Why rules are running: new mail (auto-replies allowed), or the user's "run now" on a
/// folder of existing mail (no auto-replies: old mail shouldn't be answered).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Arrival,
    OnDemand,
}

/// What a run did, for the status bar.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RunReport {
    /// Messages a rule changed.
    pub matched: usize,
    pub moved: usize,
    pub replied: usize,
    pub problems: Vec<String>,
}

/// Where a message goes once its rules have run.
enum Destination {
    Folder(String),
    Trash,
}

/// Run the enabled rules, in order, on these messages.
pub fn run(
    store: &mut MailStore,
    ids: &[MessageId],
    trigger: Trigger,
    now: DateTime<Local>,
) -> Result<RunReport, StoreError> {
    let rules: Vec<Rule> = store.rules()?.into_iter().filter(|r| r.enabled).collect();
    let mut report = RunReport::default();
    if rules.is_empty() {
        return Ok(report);
    }
    for &id in ids {
        let Ok(message) = store.get(id) else {
            continue; // gone meanwhile
        };
        let account = account_of(&message.folder).to_owned();
        let mut notes = Vec::new();
        let mut destination = None;
        for rule in rules
            .iter()
            .filter(|r| r.account.as_deref().is_none_or(|a| a == account))
        {
            if !rule.matches(&message) {
                continue;
            }
            let name = &rule.name;
            // What the rule did, said of the message: "flagged it", "oznaczyła ją flagą".
            let mut did = Vec::new();
            for action in &rule.actions {
                match action {
                    Action::MarkRead => {
                        if message.unread {
                            store.set_unread(id, false)?;
                            did.push(tr!("marked it read", "oznaczyła ją jako przeczytaną"));
                        }
                    }
                    Action::Flag => {
                        if !message.flagged {
                            store.set_flagged(id, true)?;
                            did.push(tr!("flagged it", "oznaczyła ją flagą"));
                        }
                    }
                    Action::Move { folder } if folder != &message.folder => {
                        match store.folder_name(folder) {
                            Some(to) if account_of(folder) == account || is_local(folder) => {
                                // The Polish interface names special folders by their kind.
                                let pl_to = store
                                    .kind_of(folder)
                                    .and_then(FolderKind::pl_name)
                                    .unwrap_or(to.as_str());
                                did.push(if is_local(folder) {
                                    tr!(
                                        "moved it to {to} on this PC",
                                        "przeniosła ją do folderu {pl_to} na tym komputerze"
                                    )
                                } else {
                                    tr!("moved it to {to}", "przeniosła ją do folderu {pl_to}")
                                });
                                destination = Some(Destination::Folder(folder.clone()));
                            }
                            _ => report.problems.push(tr!(
                                "rule \"{name}\" moves mail to a folder that no longer exists",
                                "reguła „{name}” przenosi pocztę do folderu, którego już nie ma"
                            )),
                        }
                    }
                    Action::Move { .. } => {}
                    Action::Delete => {
                        if store.kind_of(&message.folder) != Some(FolderKind::Trash) {
                            did.push(tr!("moved it to Trash", "przeniosła ją do Kosza"));
                            destination = Some(Destination::Trash);
                        }
                    }
                    Action::AutoReply { body } if trigger == Trigger::Arrival => {
                        let replier = Replier::Rule(rule.id);
                        match auto_reply(store, replier, &message, "", body, AUTO_REPLY_EVERY, now)
                        {
                            Ok(true) => {
                                did.push(tr!("answered it", "odpowiedziała na nią"));
                                report.replied += 1;
                            }
                            Ok(false) => {}
                            Err(e) => report.problems.push(tr!(
                                "rule \"{name}\" couldn't answer: {e}",
                                "reguła „{name}” nie mogła odpowiedzieć: {e}"
                            )),
                        }
                    }
                    Action::AutoReply { .. } => {}
                }
            }
            // Only what actually changed: running the rules again adds nothing.
            if !did.is_empty() {
                notes.push(rule_note(name, &did));
            }
            if rule.stop {
                break;
            }
        }
        if notes.is_empty() {
            continue;
        }
        report.matched += 1;
        store.add_note(id, &notes.join(" "))?;
        match destination {
            Some(Destination::Folder(key)) => {
                store.move_to(id, &key)?;
                report.moved += 1;
            }
            Some(Destination::Trash) => {
                store.delete(id)?;
                report.moved += 1;
            }
            None => {}
        }
    }
    Ok(report)
}

/// Who answers automatically, and so who remembers whom it answered: a rule, or an account's
/// away reply.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Replier<'a> {
    Rule(i64),
    Away(&'a str),
}

/// Queue an automatic answer, unless RFC 3834 or common sense says not to: not to lists, bulk
/// mail, robots, the account's own address or Spam, and to the same sender at most once in
/// `every`. `Ok(false)` means it was rightly skipped. An empty `subject` is "Re: " and the
/// original's.
pub(crate) fn auto_reply(
    store: &mut MailStore,
    replier: Replier<'_>,
    m: &Message,
    subject: &str,
    body: &str,
    every: TimeDelta,
    now: DateTime<Local>,
) -> Result<bool, String> {
    let account = store
        .account(account_of(&m.folder))
        .map_err(|e| e.to_string())?
        .clone();
    let sender = m.from_email.trim();
    if sender.is_empty()
        || sender.eq_ignore_ascii_case(&account.email)
        || looks_automated(sender)
        || store.kind_of(&m.folder) == Some(FolderKind::Spam)
    {
        return Ok(false);
    }
    let raw = store
        .raw_of(m.id)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    if auto_reply_blocker(&raw).is_some() {
        return Ok(false);
    }
    let last = match replier {
        Replier::Rule(id) => store.last_auto_reply(id, sender),
        Replier::Away(account) => store.last_away_reply(account, sender),
    }
    .map_err(|e| e.to_string())?;
    if last.is_some_and(|at| now.naive_local() - at < every) {
        return Ok(false);
    }
    let to = if m.from_name.trim().is_empty() || m.from_name == m.from_email {
        sender.to_owned()
    } else {
        format!("\"{}\" <{sender}>", m.from_name.replace('"', "'"))
    };
    let theirs = m.subject.trim();
    let draft = OutgoingDraft {
        account: account.id.clone(),
        to,
        subject: if !subject.trim().is_empty() {
            subject.trim().to_owned()
        } else if theirs.to_ascii_lowercase().starts_with("re:") {
            theirs.to_owned()
        } else {
            format!("Re: {theirs}")
        },
        body: body.to_owned(),
        reply_to: Some(m.id),
        ..OutgoingDraft::default()
    };
    let originals = Originals {
        replied: Some(raw),
        forwarded: None,
    };
    let composed = smtp::compose_auto_reply(&account, &draft, &originals, now)?;
    store
        .queue_outgoing(
            &account.id,
            &composed.message,
            &composed.raw,
            &composed.env_from,
            &composed.env_to,
            None,
            None,
        )
        .map_err(|e| e.to_string())?;
    match replier {
        Replier::Rule(id) => store.record_auto_reply(id, sender, now.naive_local()),
        Replier::Away(account) => store.record_away_reply(account, sender, now.naive_local()),
    }
    .map_err(|e| e.to_string())?;
    Ok(true)
}

/// Addresses that are robots, mailing lists or bounces by name.
fn looks_automated(address: &str) -> bool {
    let local = address
        .split('@')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let local = local.split('+').next().unwrap_or_default();
    [
        "mailer-daemon",
        "postmaster",
        "noreply",
        "no-reply",
        "no_reply",
        "donotreply",
        "do-not-reply",
        "do_not_reply",
        "bounce",
        "bounces",
        "listserv",
        "majordomo",
    ]
    .contains(&local)
        || local.starts_with("owner-")
        || local.ends_with("-request")
        || local.starts_with("bounce")
}

/// `Rule "Faktury" moved it to Faktury and flagged it.`, from what the rule did.
fn rule_note(name: &str, did: &[String]) -> String {
    let did = join_and(did);
    tr!("Rule \"{name}\" {did}.", "Reguła „{name}” {did}.")
}

/// "a" · "a and b" · "a, b and c"; in Polish "a, b i c". Neither puts a comma before the
/// last one: the English notes never had one, and Polish never does before "i".
fn join_and(parts: &[String]) -> String {
    match parts {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => {
            let rest = rest.join(", ");
            tr!("{rest} and {last}", "{rest} i {last}")
        }
    }
}

pub(crate) fn account_of(folder_key: &str) -> &str {
    split_folder_key(folder_key).map_or(folder_key, |(a, _)| a)
}

/// A folder on this PC, which takes mail from every account.
fn is_local(folder_key: &str) -> bool {
    account_of(folder_key) == LOCAL_ID
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AccountSetup, Folder, SmtpSecurity};
    use crate::sample::sample_store;
    use crate::store::SyncedMessage;
    use chrono::TimeZone;

    fn when(day: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, day, 9, 0, 0).unwrap()
    }

    fn cond(field: Field, op: Op, value: &str) -> Condition {
        Condition {
            field,
            op,
            value: value.into(),
        }
    }

    fn rule(
        name: &str,
        account: Option<&str>,
        conditions: Vec<Condition>,
        actions: Vec<Action>,
    ) -> Rule {
        Rule {
            id: 0,
            name: name.into(),
            enabled: true,
            account: account.map(Into::into),
            match_all: true,
            conditions,
            actions,
            stop: false,
        }
    }

    fn message(from: (&str, &str), to: &str, subject: &str, body: &str) -> Message {
        Message {
            id: 0,
            folder: "firma/inbox".into(),
            from_name: from.0.into(),
            from_email: from.1.into(),
            to: to.into(),
            cc: String::new(),
            subject: subject.into(),
            date: when(10).naive_local(),
            size_kb: 1,
            unread: true,
            flagged: false,
            remote_images: false,
            note: None,
            attachments: vec![],
            body: body.into(),
            message_id: None,
            bcc: String::new(),
            refs: Vec::new(),
        }
    }

    #[test]
    fn conditions_ignore_case_and_diacritics() {
        let m = message(
            ("Paweł Mazur", "pawel@mazur-ksiegowosc.pl"),
            "\"Studio, biuro\" <biuro@twojastudio.pl>, anna@x.pl",
            "Faktura za październik",
            "W załączniku fakturą.",
        );
        let hit = |c: Condition| c.matches(&m);
        assert!(hit(cond(
            Field::Subject,
            Op::Contains,
            "FAKTURA ZA PAZDZIERNIK"
        )));
        assert!(hit(cond(Field::From, Op::Is, "pawel@mazur-ksiegowosc.pl")));
        assert!(hit(cond(Field::From, Op::Contains, "mazur")));
        assert!(hit(cond(Field::From, Op::EndsWith, "@mazur-ksiegowosc.pl")));
        assert!(hit(cond(Field::To, Op::Is, "anna@x.pl")));
        assert!(
            hit(cond(Field::To, Op::Is, "Studio, biuro")),
            "names count too"
        );
        assert!(hit(cond(Field::Body, Op::Contains, "faktura")));
        assert!(!hit(cond(Field::Anywhere, Op::Contains, "lodz")));
        assert!(hit(cond(Field::Subject, Op::NotContains, "oferta")));
        assert!(!hit(cond(Field::Subject, Op::NotContains, "faktura")));
        assert!(hit(cond(Field::Subject, Op::StartsWith, "faktura")));
    }

    #[test]
    fn all_or_any() {
        let m = message(("A", "a@x.pl"), "b@y.pl", "Oferta", "");
        let mut r = rule(
            "r",
            None,
            vec![
                cond(Field::From, Op::Is, "a@x.pl"),
                cond(Field::Subject, Op::Is, "nope"),
            ],
            vec![Action::Flag],
        );
        assert!(!r.matches(&m));
        r.match_all = false;
        assert!(r.matches(&m));
    }

    fn accounts() -> Vec<Account> {
        sample_store().accounts().to_vec()
    }

    #[test]
    fn rules_are_checked_before_saving() {
        let ok = rule(
            "Faktury",
            Some("firma"),
            vec![cond(Field::Subject, Op::Contains, "faktura")],
            vec![
                Action::Move {
                    folder: "firma/faktury".into(),
                },
                Action::Flag,
            ],
        );
        assert_eq!(ok.check(&accounts()), Ok(()));
        let mut bad = ok.clone();
        bad.account = None;
        assert!(bad
            .check(&accounts())
            .unwrap_err()
            .contains("pick the rule's account"));
        let mut bad = ok.clone();
        bad.actions = vec![Action::Move {
            folder: "gmail/inbox".into(),
        }];
        assert!(bad.check(&accounts()).unwrap_err().contains("folder"));
        let mut bad = ok.clone();
        bad.actions.push(Action::Delete);
        assert!(bad.check(&accounts()).unwrap_err().contains("one place"));
        let mut bad = ok;
        bad.conditions[0].value = "  ".into();
        assert!(bad.check(&accounts()).is_err());
    }

    #[test]
    fn matching_mail_is_sorted_and_says_why() {
        let mut store = sample_store();
        let faktury = store
            .save_rule(&rule(
                "Faktury",
                Some("firma"),
                vec![cond(Field::Subject, Op::Contains, "dokumenty")],
                vec![
                    Action::Move {
                        folder: "firma/faktury".into(),
                    },
                    Action::Flag,
                    Action::MarkRead,
                ],
            ))
            .unwrap();
        assert!(faktury.id > 0);
        let target = store
            .list("firma/inbox")
            .unwrap()
            .into_iter()
            .find(|m| m.subject.contains("Dokumenty"))
            .unwrap();
        let others = store.list("firma/inbox").unwrap().len() - 1;
        let ids: Vec<_> = store
            .list("firma/inbox")
            .unwrap()
            .iter()
            .map(|m| m.id)
            .collect();

        let report = run(&mut store, &ids, Trigger::OnDemand, when(10)).unwrap();
        assert_eq!((report.matched, report.moved), (1, 1));
        let m = store.get(target.id).unwrap();
        assert_eq!(m.folder, "firma/faktury");
        assert!(m.flagged && !m.unread);
        assert_eq!(
            m.note.as_deref(),
            Some("Rule \"Faktury\" moved it to Faktury and flagged it.")
        );
        assert_eq!(store.list("firma/inbox").unwrap().len(), others);

        // Running again changes nothing and doesn't repeat the note.
        let again = run(&mut store, &[target.id], Trigger::OnDemand, when(10)).unwrap();
        assert_eq!(again.moved, 0);
        assert_eq!(
            store.get(target.id).unwrap().note.as_deref(),
            Some("Rule \"Faktury\" moved it to Faktury and flagged it.")
        );
    }

    #[test]
    fn notes_and_checks_speak_polish_when_asked() {
        use crate::i18n::{self, Lang};
        let mut store = sample_store();
        let m = store.list("firma/inbox").unwrap()[0].clone();
        store.set_unread(m.id, true).unwrap();
        store.set_flagged(m.id, false).unwrap();
        let archive = rule(
            "Porządki",
            Some("firma"),
            vec![cond(Field::Subject, Op::Is, &m.subject)],
            vec![
                Action::Move {
                    folder: "firma/archive".into(),
                },
                Action::Flag,
                Action::MarkRead,
            ],
        );
        store.save_rule(&archive).unwrap();
        let report = i18n::with(Lang::Pl, || {
            run(&mut store, &[m.id], Trigger::OnDemand, when(10)).unwrap()
        });
        assert_eq!(report.moved, 1);
        assert_eq!(
            store.get(m.id).unwrap().note.as_deref(),
            Some(
                "Reguła „Porządki” przeniosła ją do folderu Archiwum, oznaczyła ją flagą \
                 i oznaczyła ją jako przeczytaną."
            ),
            "the Archive folder by its Polish name, no comma before \"i\""
        );

        let parts = ["a", "b", "c"].map(String::from);
        assert_eq!(join_and(&parts), "a, b and c");
        assert_eq!(i18n::with(Lang::Pl, || join_and(&parts[..2])), "a i b");

        let mut unnamed = archive;
        unnamed.account = None;
        assert_eq!(
            i18n::with(Lang::Pl, || unnamed.check(&accounts()).unwrap_err()),
            "Żeby przenosić pocztę, wybierz konto reguły."
        );
    }

    #[test]
    fn rules_run_in_order_and_stop_stops() {
        let mut store = sample_store();
        let mut first = rule(
            "Anna",
            None,
            vec![cond(Field::From, Op::Contains, "kowalska")],
            vec![Action::MarkRead],
        );
        first.stop = true;
        store.save_rule(&first).unwrap();
        store
            .save_rule(&rule(
                "Wszystko",
                Some("firma"),
                vec![cond(Field::Anywhere, Op::Contains, "a")],
                vec![Action::Move {
                    folder: "firma/archive".into(),
                }],
            ))
            .unwrap();
        let anna = store.list("firma/inbox").unwrap()[0].clone();
        assert_eq!(anna.from_name, "Anna Kowalska");
        run(&mut store, &[anna.id], Trigger::OnDemand, when(10)).unwrap();
        let m = store.get(anna.id).unwrap();
        assert!(!m.unread);
        assert_eq!(m.folder, "firma/inbox", "the second rule didn't run");
        assert_eq!(m.note.as_deref(), Some("Rule \"Anna\" marked it read."));

        // Moving the second rule up changes the order.
        let rules = store.rules().unwrap();
        store.move_rule(rules[1].id, -1).unwrap();
        assert_eq!(store.rules().unwrap()[0].name, "Wszystko");
    }

    #[test]
    fn rules_for_one_account_leave_the_others_alone() {
        let mut store = sample_store();
        store
            .save_rule(&rule(
                "Gmail only",
                Some("gmail"),
                vec![cond(Field::Anywhere, Op::Contains, "e")],
                vec![Action::Flag],
            ))
            .unwrap();
        let firma: Vec<_> = store
            .list("firma/inbox")
            .unwrap()
            .iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(
            run(&mut store, &firma, Trigger::OnDemand, when(10))
                .unwrap()
                .matched,
            0
        );
    }

    #[test]
    fn delete_moves_to_trash_but_never_deletes_for_good() {
        let mut store = sample_store();
        store
            .save_rule(&rule(
                "Spam words",
                Some("firma"),
                vec![cond(Field::Subject, Op::Contains, "google")],
                vec![Action::Delete],
            ))
            .unwrap();
        let spam: Vec<_> = store
            .list("firma/spam")
            .unwrap()
            .iter()
            .map(|m| m.id)
            .collect();
        let target = spam[0];
        store.move_to(target, "firma/trash").unwrap();
        run(&mut store, &[target], Trigger::OnDemand, when(10)).unwrap();
        assert!(store.get(target).is_ok(), "already in Trash: left alone");
    }

    // ---- Auto-reply ----

    /// A server account with Inbox, Sent and Trash, one auto-reply rule, and a way to
    /// deliver a message into the Inbox.
    fn holiday_store() -> (MailStore, Account, Rule) {
        let mut store = MailStore::in_memory().unwrap();
        let account = store
            .upsert_account(&AccountSetup {
                name: "Studio".into(),
                email: "biuro@twojastudio.pl".into(),
                host: "mail.twojastudio.pl".into(),
                port: 993,
                smtp_security: SmtpSecurity::Tls,
                ..Default::default()
            })
            .unwrap();
        let folder = |id: &str, kind| Folder {
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
                    folder("spam", FolderKind::Spam),
                ],
            )
            .unwrap();
        let account = store.account(&account.id).unwrap().clone();
        let rule = store
            .save_rule(&rule(
                "Urlop",
                Some(&account.id),
                vec![cond(Field::To, Op::Contains, "biuro@")],
                vec![Action::AutoReply {
                    body: "Dzień dobry, wracam 20 października.".into(),
                }],
            ))
            .unwrap();
        (store, account, rule)
    }

    fn deliver(store: &mut MailStore, account: &Account, uid: u32, headers: &str) -> MessageId {
        let raw = format!(
            "Message-ID: <m{uid}@x.pl>\r\n{headers}To: biuro@twojastudio.pl\r\nSubject: Pytanie\r\n\r\nHej\r\n"
        );
        let mut message = crate::mime::parse_message(raw.as_bytes(), "", None);
        message.folder = account.folder_key("inbox");
        store
            .add_synced(
                &account.folder_key("inbox"),
                &[SyncedMessage {
                    uid,
                    message,
                    raw: raw.into_bytes(),
                }],
            )
            .unwrap()[0]
    }

    fn outbox(store: &MailStore, account: &Account) -> usize {
        store.list(&account.folder_key("outbox")).unwrap().len()
    }

    #[test]
    fn auto_replies_answer_people_once_in_a_while() {
        let (mut store, account, _) = holiday_store();
        let id = deliver(
            &mut store,
            &account,
            1,
            "From: Anna Kowalska <anna@x.pl>\r\n",
        );
        let report = run(&mut store, &[id], Trigger::Arrival, when(10)).unwrap();
        assert_eq!(report.replied, 1, "{report:?}");
        assert_eq!(outbox(&store, &account), 1);
        let reply = store.list(&account.folder_key("outbox")).unwrap().remove(0);
        assert_eq!(reply.subject, "Re: Pytanie");
        assert!(reply.to.contains("anna@x.pl"));
        let raw = String::from_utf8(store.raw_of(reply.id).unwrap().unwrap()).unwrap();
        assert!(raw.contains("Auto-Submitted: auto-replied"), "{raw}");
        assert!(raw.contains("In-Reply-To: <m1@x.pl>"));
        assert_eq!(
            store.get(id).unwrap().note.as_deref(),
            Some("Rule \"Urlop\" answered it.")
        );

        // The same sender, two days later: no second reply. Six days later: yes.
        let again = deliver(&mut store, &account, 2, "From: anna@x.pl\r\n");
        run(&mut store, &[again], Trigger::Arrival, when(12)).unwrap();
        assert_eq!(outbox(&store, &account), 1);
        let later = deliver(&mut store, &account, 3, "From: anna@x.pl\r\n");
        run(&mut store, &[later], Trigger::Arrival, when(16)).unwrap();
        assert_eq!(outbox(&store, &account), 2);
    }

    #[test]
    fn auto_replies_skip_robots_lists_and_old_mail() {
        let (mut store, account, _) = holiday_store();
        let cases = [
            "From: newsletter@sklep.pl\r\nList-Unsubscribe: <mailto:u@sklep.pl>\r\n",
            "From: news@sklep.pl\r\nPrecedence: bulk\r\n",
            "From: anna@x.pl\r\nAuto-Submitted: auto-replied\r\n",
            "From: noreply@bank.pl\r\n",
            "From: MAILER-DAEMON@mail.x.pl\r\n",
            "From: biuro@twojastudio.pl\r\n",
        ];
        for (i, headers) in cases.iter().enumerate() {
            let id = deliver(&mut store, &account, 10 + i as u32, headers);
            let report = run(&mut store, &[id], Trigger::Arrival, when(10)).unwrap();
            assert_eq!(report.replied, 0, "answered: {headers}");
        }
        // A real person, but the user ran the rules on old mail: no answer either.
        let id = deliver(&mut store, &account, 30, "From: piotr@y.pl\r\n");
        run(&mut store, &[id], Trigger::OnDemand, when(10)).unwrap();
        assert_eq!(outbox(&store, &account), 0);
        // In Spam: no answer.
        let id = deliver(&mut store, &account, 31, "From: piotr@y.pl\r\n");
        store.move_to(id, &account.folder_key("spam")).unwrap();
        run(&mut store, &[id], Trigger::Arrival, when(10)).unwrap();
        assert_eq!(outbox(&store, &account), 0);
    }

    #[test]
    fn rules_file_mail_from_any_account_on_this_pc() {
        let (mut store, account, holiday) = holiday_store();
        store.delete_rule(holiday.id).unwrap();
        let archive = rule(
            "Newslettery",
            None,
            vec![cond(Field::Subject, Op::Contains, "pytanie")],
            vec![Action::Move {
                folder: "local/archive".into(),
            }],
        );
        assert_eq!(
            archive.check(store.accounts()),
            Ok(()),
            "no account needed for this PC"
        );
        let mut gone = archive.clone();
        gone.actions = vec![Action::Move {
            folder: "local/nowhere".into(),
        }];
        assert!(gone.check(store.accounts()).is_err());
        store.save_rule(&archive).unwrap();
        let id = deliver(&mut store, &account, 1, "From: Anna <anna@x.pl>\r\n");
        let report = run(&mut store, &[id], Trigger::Arrival, when(10)).unwrap();
        assert_eq!((report.matched, report.moved), (1, 1));
        let m = store.get(id).unwrap();
        assert_eq!(m.folder, "local/archive");
        assert_eq!(
            m.note.as_deref(),
            Some("Rule \"Newslettery\" moved it to Archive on this PC.")
        );
        assert_eq!(
            store.pending_changes(&account.id).unwrap()[0].kind,
            crate::store::ChangeKind::Expunge,
            "it leaves the server in the same sync"
        );
    }
}
