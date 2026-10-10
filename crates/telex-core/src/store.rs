use std::fmt;

use chrono::NaiveDateTime;
use serde::Serialize;

use crate::model::{
    split_folder_key, Account, FolderKind, Message, MessageId, MessageSummary, OutgoingDraft,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    UnknownMessage(MessageId),
    UnknownAccount(String),
    UnknownFolder(String),
    /// Moving a message between accounts needs an upload to the other server, which isn't supported yet.
    CrossAccountMove {
        from: String,
        to: String,
    },
    /// The account has no folder of the kind the operation needs (e.g. no Trash).
    MissingFolder {
        account: String,
        kind: FolderKind,
    },
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownMessage(id) => write!(f, "message {id} does not exist"),
            Self::UnknownAccount(a) => write!(f, "account \"{a}\" does not exist"),
            Self::UnknownFolder(k) => write!(f, "folder \"{k}\" does not exist"),
            Self::CrossAccountMove { from, to } => {
                write!(
                    f,
                    "can't move a message from {from} to {to}: they belong to different accounts"
                )
            }
            Self::MissingFolder { account, kind } => {
                write!(f, "account \"{account}\" has no {kind:?} folder")
            }
        }
    }
}

impl std::error::Error for StoreError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeleteOutcome {
    /// The message is now in the account's Trash and can be moved back.
    MovedToTrash,
    /// The message was already in Trash and is gone for good.
    Removed,
}

/// Everything the UI needs to draw the folder tree and message lists.
#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub accounts: Vec<Account>,
    pub messages: Vec<MessageSummary>,
}

/// In-memory mail store. The public API is what the UI talks to; the storage
/// behind it will move to SQLite when IMAP sync arrives.
#[derive(Debug, Clone)]
pub struct MailStore {
    accounts: Vec<Account>,
    messages: Vec<Message>,
    incoming: Vec<Message>,
    next_id: MessageId,
}

impl MailStore {
    /// Build a store and give every message a fresh id.
    pub fn new(accounts: Vec<Account>, messages: Vec<Message>, incoming: Vec<Message>) -> Self {
        let mut store = Self {
            accounts,
            messages: Vec::new(),
            incoming: Vec::new(),
            next_id: 1,
        };
        for mut m in messages {
            m.id = store.take_id();
            store.messages.push(m);
        }
        for mut m in incoming {
            m.id = store.take_id();
            store.incoming.push(m);
        }
        store
    }

    fn take_id(&mut self) -> MessageId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn accounts(&self) -> &[Account] {
        &self.accounts
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            accounts: self.accounts.clone(),
            messages: self.messages.iter().map(MessageSummary::from).collect(),
        }
    }

    /// Messages in one folder, newest first.
    pub fn list(&self, folder: &str) -> Vec<MessageSummary> {
        let mut rows: Vec<MessageSummary> = self
            .messages
            .iter()
            .filter(|m| m.folder == folder)
            .map(MessageSummary::from)
            .collect();
        rows.sort_by(|a, b| b.date.cmp(&a.date).then(b.id.cmp(&a.id)));
        rows
    }

    pub fn unread_count(&self, folder: &str) -> usize {
        self.messages
            .iter()
            .filter(|m| m.folder == folder && m.unread)
            .count()
    }

    pub fn get(&self, id: MessageId) -> Result<&Message, StoreError> {
        self.messages
            .iter()
            .find(|m| m.id == id)
            .ok_or(StoreError::UnknownMessage(id))
    }

    fn get_mut(&mut self, id: MessageId) -> Result<&mut Message, StoreError> {
        self.messages
            .iter_mut()
            .find(|m| m.id == id)
            .ok_or(StoreError::UnknownMessage(id))
    }

    fn account(&self, id: &str) -> Result<&Account, StoreError> {
        self.accounts
            .iter()
            .find(|a| a.id == id)
            .ok_or_else(|| StoreError::UnknownAccount(id.to_owned()))
    }

    /// Check that "<account>/<folder>" exists and return the account id.
    fn check_folder<'k>(&self, key: &'k str) -> Result<&'k str, StoreError> {
        let unknown = || StoreError::UnknownFolder(key.to_owned());
        let (acct, folder) = split_folder_key(key).ok_or_else(unknown)?;
        let account = self.account(acct).map_err(|_| unknown())?;
        if account.folders.iter().any(|f| f.id == folder) {
            Ok(acct)
        } else {
            Err(unknown())
        }
    }

    fn folder_kind(&self, key: &str) -> Option<FolderKind> {
        let (acct, folder) = split_folder_key(key)?;
        let account = self.accounts.iter().find(|a| a.id == acct)?;
        account
            .folders
            .iter()
            .find(|f| f.id == folder)
            .map(|f| f.kind)
    }

    fn folder_key_of_kind(&self, account: &str, kind: FolderKind) -> Result<String, StoreError> {
        let a = self.account(account)?;
        a.folder_of_kind(kind)
            .map(|f| a.folder_key(&f.id))
            .ok_or_else(|| StoreError::MissingFolder {
                account: account.to_owned(),
                kind,
            })
    }

    pub fn set_unread(&mut self, id: MessageId, unread: bool) -> Result<(), StoreError> {
        self.get_mut(id)?.unread = unread;
        Ok(())
    }

    pub fn set_flagged(&mut self, id: MessageId, flagged: bool) -> Result<(), StoreError> {
        self.get_mut(id)?.flagged = flagged;
        Ok(())
    }

    /// Move a message to another folder of the same account.
    pub fn move_to(&mut self, id: MessageId, folder: &str) -> Result<(), StoreError> {
        let target_acct = self.check_folder(folder)?.to_owned();
        let msg = self.get_mut(id)?;
        let source_acct = split_folder_key(&msg.folder)
            .map(|(a, _)| a)
            .unwrap_or_default();
        if source_acct != target_acct {
            return Err(StoreError::CrossAccountMove {
                from: msg.folder.clone(),
                to: folder.to_owned(),
            });
        }
        msg.folder = folder.to_owned();
        Ok(())
    }

    /// Delete like a mail client does: first to Trash, and from Trash for good.
    pub fn delete(&mut self, id: MessageId) -> Result<DeleteOutcome, StoreError> {
        let folder = self.get(id)?.folder.clone();
        if self.folder_kind(&folder) == Some(FolderKind::Trash) {
            self.messages.retain(|m| m.id != id);
            return Ok(DeleteOutcome::Removed);
        }
        let account = split_folder_key(&folder)
            .map(|(a, _)| a.to_owned())
            .unwrap_or_default();
        let trash = self.folder_key_of_kind(&account, FolderKind::Trash)?;
        self.get_mut(id)?.folder = trash;
        Ok(DeleteOutcome::MovedToTrash)
    }

    /// Store a message the user wrote, in Sent or Drafts of the sending account.
    /// Sending over SMTP is not wired up yet, so "Sent" only files the message.
    pub fn save_outgoing(
        &mut self,
        draft: OutgoingDraft,
        kind: FolderKind,
        now: NaiveDateTime,
    ) -> Result<MessageId, StoreError> {
        debug_assert!(kind.is_outgoing(), "outgoing mail goes to Sent or Drafts");
        let account = self.account(&draft.account)?.clone();
        let folder = self.folder_key_of_kind(&account.id, kind)?;
        let attachments_kb: u32 = draft.attachments.iter().map(|a| a.size_kb).sum();
        let body_kb = u32::try_from(draft.body.len().div_ceil(1024)).unwrap_or(u32::MAX);
        let id = self.take_id();
        self.messages.push(Message {
            id,
            folder,
            from_name: account.name.clone(),
            from_email: account.email.clone(),
            to: draft.to,
            cc: draft.cc,
            subject: draft.subject,
            date: now,
            size_kb: (body_kb + attachments_kb).max(1),
            unread: false,
            flagged: false,
            remote_images: false,
            note: None,
            attachments: draft.attachments,
            body: draft.body,
        });
        Ok(id)
    }

    /// Deliver waiting messages into their folders and return their ids.
    /// Stand-in for an IMAP check until sync exists.
    pub fn fetch_new(&mut self) -> Vec<MessageId> {
        let arrived: Vec<Message> = std::mem::take(&mut self.incoming);
        let ids = arrived.iter().map(|m| m.id).collect();
        self.messages.extend(arrived);
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::sample_store;
    use chrono::NaiveDate;
    use std::collections::HashSet;

    fn now() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 10)
            .unwrap()
            .and_hms_opt(7, 0, 0)
            .unwrap()
    }

    fn draft(account: &str) -> OutgoingDraft {
        OutgoingDraft {
            account: account.into(),
            to: "Anna Kowalska <anna@kowalska-architekci.pl>".into(),
            cc: String::new(),
            subject: "Re: Poprawki".into(),
            body: "Dzień dobry,\n\nwdrożę do środy.\n".into(),
            attachments: vec![],
        }
    }

    #[test]
    fn sample_mailbox_is_consistent() {
        let store = sample_store();
        assert_eq!(store.accounts().len(), 2);
        let snap = store.snapshot();
        assert!(snap.messages.len() >= 20);
        let ids: HashSet<_> = snap.messages.iter().map(|m| m.id).collect();
        assert_eq!(ids.len(), snap.messages.len(), "ids are unique");
        for m in &snap.messages {
            assert!(
                store.check_folder(&m.folder).is_ok(),
                "{} points to a real folder",
                m.folder
            );
        }
    }

    #[test]
    fn list_is_newest_first() {
        let store = sample_store();
        let rows = store.list("firma/inbox");
        assert!(rows.len() > 3);
        assert!(rows.windows(2).all(|w| w[0].date >= w[1].date));
        assert_eq!(rows[0].from_name, "Anna Kowalska");
    }

    #[test]
    fn polish_text_survives_the_round_trip() {
        let store = sample_store();
        let first = &store.list("firma/inbox")[0];
        let msg = store.get(first.id).unwrap();
        assert!(msg.subject.contains("głównej"));
        assert!(msg.body.contains("Dzień dobry"));
    }

    #[test]
    fn flags_and_read_state() {
        let mut store = sample_store();
        let id = store.list("firma/inbox")[0].id;
        let before = store.unread_count("firma/inbox");
        store.set_unread(id, false).unwrap();
        assert_eq!(store.unread_count("firma/inbox"), before - 1);
        store.set_flagged(id, false).unwrap();
        assert!(!store.get(id).unwrap().flagged);
        assert_eq!(
            store.set_flagged(99_999, true),
            Err(StoreError::UnknownMessage(99_999))
        );
    }

    #[test]
    fn delete_goes_to_trash_then_removes() {
        let mut store = sample_store();
        let id = store.list("firma/inbox")[0].id;
        assert_eq!(store.delete(id), Ok(DeleteOutcome::MovedToTrash));
        assert_eq!(store.get(id).unwrap().folder, "firma/trash");
        assert_eq!(store.delete(id), Ok(DeleteOutcome::Removed));
        assert!(store.get(id).is_err());
    }

    #[test]
    fn undo_is_a_move_back() {
        let mut store = sample_store();
        let id = store.list("gmail/inbox")[0].id;
        store.delete(id).unwrap();
        store.move_to(id, "gmail/inbox").unwrap();
        assert_eq!(store.get(id).unwrap().folder, "gmail/inbox");
    }

    #[test]
    fn moves_stay_inside_one_account() {
        let mut store = sample_store();
        let id = store.list("firma/inbox")[0].id;
        assert!(matches!(
            store.move_to(id, "gmail/archive"),
            Err(StoreError::CrossAccountMove { .. })
        ));
        assert!(matches!(
            store.move_to(id, "firma/nope"),
            Err(StoreError::UnknownFolder(_))
        ));
        store.move_to(id, "firma/klienci").unwrap();
        assert_eq!(store.list("firma/klienci")[0].id, id);
    }

    #[test]
    fn outgoing_mail_is_filed_in_the_right_folder() {
        let mut store = sample_store();
        let sent = store
            .save_outgoing(draft("firma"), FolderKind::Sent, now())
            .unwrap();
        let msg = store.get(sent).unwrap();
        assert_eq!(msg.folder, "firma/sent");
        assert_eq!(msg.from_email, "biuro@twojastudio.pl");
        assert_eq!(msg.size_kb, 1);
        let saved = store
            .save_outgoing(draft("gmail"), FolderKind::Drafts, now())
            .unwrap();
        assert_eq!(store.get(saved).unwrap().folder, "gmail/drafts");
        assert!(store
            .save_outgoing(draft("nope"), FolderKind::Sent, now())
            .is_err());
    }

    #[test]
    fn new_mail_arrives_once() {
        let mut store = sample_store();
        let before = store.list("firma/inbox").len();
        let first = store.fetch_new();
        assert_eq!(first.len(), 1);
        assert_eq!(store.list("firma/inbox").len(), before + 1);
        assert!(store.fetch_new().is_empty());
    }
}
