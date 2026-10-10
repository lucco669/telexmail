use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

/// Stable id of a message inside one store.
pub type MessageId = u64;

/// What a folder is for. Custom folders are the user's own ("Klienci", "Faktury").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FolderKind {
    Inbox,
    Drafts,
    Sent,
    Archive,
    Spam,
    Trash,
    Custom,
}

impl FolderKind {
    /// Folders whose messages were written by the user, so the list shows "To" instead of "From".
    pub fn is_outgoing(self) -> bool {
        matches!(self, Self::Sent | Self::Drafts)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    /// Short id, unique within the account ("inbox", "klienci").
    pub id: String,
    pub name: String,
    pub kind: FolderKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub email: String,
    pub folders: Vec<Folder>,
}

impl Account {
    /// Full folder key as used on messages: "<account>/<folder>".
    pub fn folder_key(&self, folder_id: &str) -> String {
        format!("{}/{}", self.id, folder_id)
    }

    pub fn folder_of_kind(&self, kind: FolderKind) -> Option<&Folder> {
        self.folders.iter().find(|f| f.kind == kind)
    }
}

/// Split "<account>/<folder>" into its two parts.
pub fn split_folder_key(key: &str) -> Option<(&str, &str)> {
    key.split_once('/')
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    pub name: String,
    pub size_kb: u32,
}

/// A complete message, including its body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    #[serde(default)]
    pub id: MessageId,
    /// "<account>/<folder>"
    pub folder: String,
    pub from_name: String,
    pub from_email: String,
    /// Recipients as written in the header, e.g. "Piotr Nowak <piotr@nowak-meble.pl>".
    pub to: String,
    #[serde(default)]
    pub cc: String,
    pub subject: String,
    /// Local time the message was received (or written, for outgoing mail).
    pub date: NaiveDateTime,
    pub size_kb: u32,
    #[serde(default)]
    pub unread: bool,
    #[serde(default)]
    pub flagged: bool,
    /// The HTML references remote images. Telex blocks them until the user allows it.
    #[serde(default)]
    pub remote_images: bool,
    /// Why a filter or spam check touched this message, shown above the body.
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    /// Plain-text body. HTML bodies come later, rendered in a sandboxed frame.
    pub body: String,
}

/// One row of the message list. Everything except the body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MessageSummary {
    pub id: MessageId,
    pub folder: String,
    pub from_name: String,
    pub from_email: String,
    pub to: String,
    pub subject: String,
    pub date: NaiveDateTime,
    pub size_kb: u32,
    pub unread: bool,
    pub flagged: bool,
    pub has_attachments: bool,
}

impl From<&Message> for MessageSummary {
    fn from(m: &Message) -> Self {
        Self {
            id: m.id,
            folder: m.folder.clone(),
            from_name: m.from_name.clone(),
            from_email: m.from_email.clone(),
            to: m.to.clone(),
            subject: m.subject.clone(),
            date: m.date,
            size_kb: m.size_kb,
            unread: m.unread,
            flagged: m.flagged,
            has_attachments: !m.attachments.is_empty(),
        }
    }
}

/// A message the user wrote in the compose pane.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OutgoingDraft {
    /// Account id to send from.
    pub account: String,
    pub to: String,
    #[serde(default)]
    pub cc: String,
    pub subject: String,
    pub body: String,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
}
