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

    /// The same lowercase name serde uses, for storing in the database.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inbox => "inbox",
            Self::Drafts => "drafts",
            Self::Sent => "sent",
            Self::Archive => "archive",
            Self::Spam => "spam",
            Self::Trash => "trash",
            Self::Custom => "custom",
        }
    }

    /// Inverse of [`FolderKind::as_str`]. Unknown names become `Custom`.
    pub fn from_name(name: &str) -> Self {
        match name {
            "inbox" => Self::Inbox,
            "drafts" => Self::Drafts,
            "sent" => Self::Sent,
            "archive" => Self::Archive,
            "spam" => Self::Spam,
            "trash" => Self::Trash,
            _ => Self::Custom,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    /// Short id, unique within the account ("inbox", "klienci").
    pub id: String,
    pub name: String,
    pub kind: FolderKind,
    /// The folder's name on the IMAP server, spelled exactly as the server does
    /// (modified UTF-7, e.g. "INBOX.Wys&AUI-ane"). `None` for local folders.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub email: String,
    pub folders: Vec<Folder>,
    /// How to reach the account's server. `None` for the sample mailbox.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imap: Option<ImapSettings>,
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

/// Where an account's mail lives. The password is not here: the app keeps it in
/// Windows Credential Manager and hands it over only to connect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImapSettings {
    pub host: String,
    pub port: u16,
    pub username: String,
}

/// What the user types in the account dialog, except the password.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AccountSetup {
    /// Label in the folder list. Defaults to the email's domain.
    #[serde(default)]
    pub name: String,
    pub email: String,
    pub host: String,
    pub port: u16,
    /// Login name. Defaults to the email address, which is what most servers expect.
    #[serde(default)]
    pub username: String,
}

impl AccountSetup {
    /// Trim the fields, fill in defaults and catch typos before Telex dials out.
    pub fn cleaned(self) -> Result<Self, String> {
        let email = self.email.trim().to_owned();
        let domain = email
            .split_once('@')
            .filter(|(local, domain)| {
                !local.is_empty() && domain.contains('.') && !domain.contains('@')
            })
            .map(|(_, domain)| domain.to_ascii_lowercase())
            .ok_or_else(|| format!("\"{email}\" doesn't look like an email address."))?;
        if email.contains(char::is_whitespace) {
            return Err("The email address can't contain spaces.".into());
        }
        let host = self.host.trim();
        let host = host
            .strip_prefix("imaps://")
            .or_else(|| host.strip_prefix("imap://"))
            .unwrap_or(host)
            .trim_end_matches('/')
            .to_ascii_lowercase();
        if host.is_empty() {
            return Err("Enter the IMAP server, e.g. mail.example.com.".into());
        }
        if host.contains(|c: char| c.is_whitespace() || matches!(c, '/' | ':' | '@')) {
            return Err(format!(
                "\"{host}\" isn't a server name. Enter just the name, e.g. mail.example.com."
            ));
        }
        if self.port == 0 {
            return Err("Enter the port. For SSL/TLS it is usually 993.".into());
        }
        let name = match self.name.trim() {
            "" => domain,
            n => n.to_owned(),
        };
        let username = match self.username.trim() {
            "" => email.clone(),
            u => u.to_owned(),
        };
        Ok(Self {
            name,
            email,
            host,
            port: self.port,
            username,
        })
    }

    pub fn imap_settings(&self) -> ImapSettings {
        ImapSettings {
            host: self.host.clone(),
            port: self.port,
            username: self.username.clone(),
        }
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
    /// The Message-ID header, used by sync to recognize a moved message. Not sent to the UI.
    #[serde(default, skip_serializing)]
    pub message_id: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(email: &str, host: &str) -> AccountSetup {
        AccountSetup {
            name: String::new(),
            email: email.into(),
            host: host.into(),
            port: 993,
            username: String::new(),
        }
    }

    #[test]
    fn account_setup_fills_in_defaults() {
        let s = setup("  biuro@TwojaStudio.pl ", " imaps://Mail.TwojaStudio.pl/ ")
            .cleaned()
            .unwrap();
        assert_eq!(s.email, "biuro@TwojaStudio.pl");
        assert_eq!(s.host, "mail.twojastudio.pl");
        assert_eq!(s.name, "twojastudio.pl");
        assert_eq!(s.username, "biuro@TwojaStudio.pl");
    }

    #[test]
    fn account_setup_catches_typos() {
        assert!(setup("biuro", "mail.x.pl").cleaned().is_err());
        assert!(setup("biuro@x.pl", "").cleaned().is_err());
        assert!(setup("biuro@x.pl", "mail.x.pl:993").cleaned().is_err());
        let mut no_port = setup("biuro@x.pl", "mail.x.pl");
        no_port.port = 0;
        assert!(no_port.cleaned().is_err());
    }
}
