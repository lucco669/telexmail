use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

/// Stable id of a message inside one store.
pub type MessageId = u64;

/// What a folder is for. Custom folders are the user's own ("Klienci", "Faktury").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FolderKind {
    Inbox,
    /// Local only: mail waiting to be sent. Every server account has one.
    Outbox,
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
        matches!(self, Self::Sent | Self::Drafts | Self::Outbox)
    }

    /// The same lowercase name serde uses, for storing in the database.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inbox => "inbox",
            Self::Outbox => "outbox",
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
            "outbox" => Self::Outbox,
            "drafts" => Self::Drafts,
            "sent" => Self::Sent,
            "archive" => Self::Archive,
            "spam" => Self::Spam,
            "trash" => Self::Trash,
            _ => Self::Custom,
        }
    }

    /// Order in the folder tree, as in The Bat!: Inbox, Outbox, Drafts, Sent, the user's
    /// own folders, then Archive, Spam and Trash.
    pub fn rank(self) -> u8 {
        match self {
            Self::Inbox => 0,
            Self::Outbox => 1,
            Self::Drafts => 2,
            Self::Sent => 3,
            Self::Custom => 4,
            Self::Archive => 5,
            Self::Spam => 6,
            Self::Trash => 7,
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
    /// How to send. `None` for the sample mailbox and for accounts added before sending
    /// existed; the UI asks the user to edit the account then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smtp: Option<SmtpSettings>,
    /// Who mail from this account can be from. The first default one is used unless the
    /// user picks another.
    #[serde(default)]
    pub identities: Vec<Identity>,
}

impl Account {
    /// Full folder key as used on messages: "<account>/<folder>".
    pub fn folder_key(&self, folder_id: &str) -> String {
        format!("{}/{}", self.id, folder_id)
    }

    pub fn folder_of_kind(&self, kind: FolderKind) -> Option<&Folder> {
        self.folders.iter().find(|f| f.kind == kind)
    }

    /// The identity with this id, or the default one. An account without identities
    /// (never stored ones) sends as its own name and address.
    pub fn identity(&self, id: Option<i64>) -> Identity {
        id.and_then(|id| self.identities.iter().find(|i| i.id == id))
            .or_else(|| self.identities.iter().find(|i| i.is_default))
            .or_else(|| self.identities.first())
            .cloned()
            .unwrap_or_else(|| Identity::new(&self.id, &self.name, &self.email))
    }
}

/// Who a message is from: a name and an address, with a signature and the templates for
/// replying and forwarding. Templates use {name}, {email}, {from}, {date}, {time} and
/// {subject} of the original message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub account: String,
    pub name: String,
    pub email: String,
    #[serde(default)]
    pub signature: String,
    #[serde(default = "default_reply_header")]
    pub reply_header: String,
    #[serde(default = "default_forward_header")]
    pub forward_header: String,
    #[serde(default)]
    pub is_default: bool,
}

pub const DEFAULT_SIGNATURE: &str = "Pozdrawiam";

fn default_reply_header() -> String {
    "Dnia {date} o {time}, {name} napisał(a):".to_owned()
}

fn default_forward_header() -> String {
    "-------- Przekazana wiadomość --------\nOd: {from}\nData: {date} {time}\nTemat: {subject}"
        .to_owned()
}

impl Identity {
    /// A default identity with the Polish templates.
    pub fn new(account: &str, name: &str, email: &str) -> Self {
        Self {
            id: 0,
            account: account.to_owned(),
            name: name.to_owned(),
            email: email.to_owned(),
            signature: DEFAULT_SIGNATURE.to_owned(),
            reply_header: default_reply_header(),
            forward_header: default_forward_header(),
            is_default: true,
        }
    }

    /// Catch mistakes before saving, in words for the identity editor.
    pub fn check(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Enter the name people see.".into());
        }
        let email = self.email.trim();
        let valid = email
            .split_once('@')
            .is_some_and(|(l, d)| !l.is_empty() && d.contains('.') && !d.contains('@'));
        if !valid || email.contains(char::is_whitespace) {
            return Err(format!("\"{email}\" doesn't look like an email address."));
        }
        Ok(())
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

/// Where an account sends from. It logs in with the IMAP user name and password.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmtpSettings {
    pub host: String,
    pub port: u16,
    pub security: SmtpSecurity,
}

/// How the SMTP connection is encrypted. Both are TLS; they differ in when it starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SmtpSecurity {
    /// TLS from the first byte, usually port 465.
    #[default]
    Tls,
    /// Plain greeting, then STARTTLS before anything else, usually port 587.
    StartTls,
}

impl SmtpSecurity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tls => "tls",
            Self::StartTls => "starttls",
        }
    }

    pub fn from_name(name: &str) -> Self {
        if name == "starttls" {
            Self::StartTls
        } else {
            Self::Tls
        }
    }
}

/// What the user types in the account dialog, except the password. Also what the wizard finds.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AccountSetup {
    /// Label in the folder list, and the sender name. Defaults to the email's domain.
    #[serde(default)]
    pub name: String,
    pub email: String,
    pub host: String,
    pub port: u16,
    /// Login name. Defaults to the email address, which is what most servers expect.
    #[serde(default)]
    pub username: String,
    /// SMTP server. Defaults to the IMAP server, which is right for most own domains.
    #[serde(default)]
    pub smtp_host: String,
    /// Defaults to 465 for SSL/TLS and 587 for STARTTLS.
    #[serde(default)]
    pub smtp_port: u16,
    #[serde(default)]
    pub smtp_security: SmtpSecurity,
}

/// "imaps://Mail.Example.com/" → "mail.example.com", or an error for something that isn't
/// a host name.
fn clean_host(input: &str, what: &str) -> Result<String, String> {
    let host = input.trim();
    let host = ["imaps://", "imap://", "smtps://", "smtp://"]
        .iter()
        .find_map(|p| host.strip_prefix(p))
        .unwrap_or(host)
        .trim_end_matches('/')
        .to_ascii_lowercase();
    if host.is_empty() {
        return Err(format!("Enter the {what} server, e.g. mail.example.com."));
    }
    if host.contains(|c: char| c.is_whitespace() || matches!(c, '/' | ':' | '@')) {
        return Err(format!(
            "\"{host}\" isn't a server name. Enter just the name, e.g. mail.example.com."
        ));
    }
    Ok(host)
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
        let host = clean_host(&self.host, "IMAP")?;
        if self.port == 0 {
            return Err("Enter the port. For SSL/TLS it is usually 993.".into());
        }
        let smtp_host = if self.smtp_host.trim().is_empty() {
            host.clone()
        } else {
            clean_host(&self.smtp_host, "SMTP")?
        };
        let smtp_port = match (self.smtp_port, self.smtp_security) {
            (0, SmtpSecurity::Tls) => 465,
            (0, SmtpSecurity::StartTls) => 587,
            (p, _) => p,
        };
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
            smtp_host,
            smtp_port,
            smtp_security: self.smtp_security,
        })
    }

    pub fn imap_settings(&self) -> ImapSettings {
        ImapSettings {
            host: self.host.clone(),
            port: self.port,
            username: self.username.clone(),
        }
    }

    pub fn smtp_settings(&self) -> SmtpSettings {
        SmtpSettings {
            host: self.smtp_host.clone(),
            port: self.smtp_port,
            security: self.smtp_security,
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
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct OutgoingDraft {
    /// Account id to send from.
    pub account: String,
    pub to: String,
    #[serde(default)]
    pub cc: String,
    pub subject: String,
    pub body: String,
    /// Attachments carried over from `forward_of`. New files come later.
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    /// The message this replies to, for the In-Reply-To and References headers.
    #[serde(default)]
    pub reply_to: Option<MessageId>,
    /// The message this forwards; its attachments are sent along.
    #[serde(default)]
    pub forward_of: Option<MessageId>,
    /// The draft or unsent message this was edited from. It is removed once this is saved.
    #[serde(default)]
    pub replaces: Option<MessageId>,
    /// Who it is from; `None` is the account's default identity.
    #[serde(default)]
    pub identity: Option<i64>,
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
            smtp_host: String::new(),
            smtp_port: 0,
            smtp_security: SmtpSecurity::Tls,
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
        assert_eq!(
            s.smtp_host, "mail.twojastudio.pl",
            "SMTP defaults to the IMAP server"
        );
        assert_eq!(s.smtp_port, 465);

        let mut starttls = setup("biuro@x.pl", "mail.x.pl");
        starttls.smtp_security = SmtpSecurity::StartTls;
        starttls.smtp_host = "smtp://SMTP.x.pl".into();
        let s = starttls.cleaned().unwrap();
        assert_eq!((s.smtp_host.as_str(), s.smtp_port), ("smtp.x.pl", 587));
    }

    #[test]
    fn account_setup_catches_typos() {
        assert!(setup("biuro", "mail.x.pl").cleaned().is_err());
        assert!(setup("biuro@x.pl", "").cleaned().is_err());
        assert!(setup("biuro@x.pl", "mail.x.pl:993").cleaned().is_err());
        let mut no_port = setup("biuro@x.pl", "mail.x.pl");
        no_port.port = 0;
        assert!(no_port.cleaned().is_err());
        let mut bad_smtp = setup("biuro@x.pl", "mail.x.pl");
        bad_smtp.smtp_host = "smtp.x.pl:587".into();
        assert!(bad_smtp.cleaned().is_err());
    }

    #[test]
    fn folder_kinds_round_trip_and_sort() {
        for kind in [
            FolderKind::Inbox,
            FolderKind::Outbox,
            FolderKind::Drafts,
            FolderKind::Sent,
            FolderKind::Archive,
            FolderKind::Spam,
            FolderKind::Trash,
            FolderKind::Custom,
        ] {
            assert_eq!(FolderKind::from_name(kind.as_str()), kind);
        }
        assert!(FolderKind::Outbox.rank() < FolderKind::Sent.rank());
        assert!(FolderKind::Custom.rank() < FolderKind::Trash.rank());
    }
}
