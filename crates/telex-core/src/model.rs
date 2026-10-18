use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::i18n::Lang;
use crate::tr;

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

    /// The name the Polish interface gives a special folder ("Kosz"); `None` for the user's
    /// own folders, which keep their names.
    pub fn pl_name(self) -> Option<&'static str> {
        self.name_in(Lang::Pl)
    }

    /// The name the interface gives a special folder in the current language ("Trash",
    /// "Kosz", "Papierkorb"), for sentences the core writes; `None` for the user's own.
    pub fn shown_name(self) -> Option<&'static str> {
        self.name_in(crate::i18n::current())
    }

    /// A special folder's name in a language, as the interface's translations name it.
    pub fn name_in(self, lang: Lang) -> Option<&'static str> {
        // Inbox, Outbox, Drafts, Sent, Archive, Spam, Trash; in the order of `Lang::ALL`.
        const NAMES: [[&str; 7]; 9] = [
            [
                "Inbox", "Outbox", "Drafts", "Sent", "Archive", "Spam", "Trash",
            ],
            [
                "Odebrane",
                "Skrzynka nadawcza",
                "Szkice",
                "Wysłane",
                "Archiwum",
                "Spam",
                "Kosz",
            ],
            [
                "Posteingang",
                "Postausgang",
                "Entwürfe",
                "Gesendet",
                "Archiv",
                "Spam",
                "Papierkorb",
            ],
            [
                "Doručená pošta",
                "K odeslání",
                "Koncepty",
                "Odeslaná pošta",
                "Archiv",
                "Spam",
                "Koš",
            ],
            [
                "Doručená pošta",
                "Na odoslanie",
                "Koncepty",
                "Odoslané",
                "Archív",
                "Spam",
                "Kôš",
            ],
            [
                "Вхідні",
                "Вихідні",
                "Чернетки",
                "Надіслані",
                "Архів",
                "Спам",
                "Смітник",
            ],
            [
                "Bandeja de entrada",
                "Bandeja de salida",
                "Borradores",
                "Enviados",
                "Archivo",
                "Spam",
                "Papelera",
            ],
            [
                "Boîte de réception",
                "Boîte d’envoi",
                "Brouillons",
                "Envoyés",
                "Archives",
                "Spam",
                "Corbeille",
            ],
            [
                "Posta in arrivo",
                "Posta in uscita",
                "Bozze",
                "Posta inviata",
                "Archivio",
                "Spam",
                "Cestino",
            ],
        ];
        let i = match self {
            Self::Inbox => 0,
            Self::Outbox => 1,
            Self::Drafts => 2,
            Self::Sent => 3,
            Self::Archive => 4,
            Self::Spam => 5,
            Self::Trash => 6,
            Self::Custom => return None,
        };
        Some(NAMES[lang as usize][i])
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
    /// How sent mail reaches the server's Sent folder: by itself, or as a copy Telex uploads.
    #[serde(default)]
    pub sent_copy: SentCopy,
    /// Whether the server files sent mail in Sent itself, as far as Telex knows without the
    /// user's choice: from the provider list or found out after a send. `None`: not yet known.
    #[serde(default)]
    pub server_files_sent: Option<bool>,
}

/// Who puts a copy of sent mail into the server's Sent folder (Settings › Accounts › Folders).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SentCopy {
    /// Known providers (Gmail, Outlook.com) keep their own; others Telex finds out about
    /// after the first send, by looking for the message in Sent before uploading it.
    #[default]
    Auto,
    /// The server files sent mail itself; Telex uploads nothing.
    Server,
    /// Telex uploads a copy.
    Telex,
}

impl SentCopy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Server => "server",
            Self::Telex => "telex",
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name {
            "server" => Self::Server,
            "telex" => Self::Telex,
            _ => Self::Auto,
        }
    }
}

impl Account {
    /// Whether the server keeps its own copy of sent mail, so Telex uploads none: the user's
    /// choice, else what Telex knows. `None`: not known yet (Telex looks after the next send).
    pub fn server_keeps_sent(&self) -> Option<bool> {
        match self.sent_copy {
            SentCopy::Server => Some(true),
            SentCopy::Telex => Some(false),
            SentCopy::Auto => self.server_files_sent,
        }
    }

    /// Full folder key as used on messages: "<account>/<folder>".
    pub fn folder_key(&self, folder_id: &str) -> String {
        format!("{}/{}", self.id, folder_id)
    }

    /// Its mail comes over POP3, so all its folders are here.
    pub fn is_pop3(&self) -> bool {
        self.imap.as_ref().is_some_and(|i| i.pop3.is_some())
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
/// what new messages and replies open with, and for replying and forwarding. Templates use
/// {name}, {first}, {email}, {from}, {date}, {time} and {subject} of the original message (in
/// a new message only {date} and {time}, today's).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub account: String,
    pub name: String,
    pub email: String,
    /// Its text, for plain-text messages (made from `signature_html` when that is set).
    #[serde(default)]
    pub signature: String,
    /// The signature written in HTML, cleaned as sent mail is (`html::compose_clean`), with its
    /// pictures as `data:`; `None` for a plain-text signature.
    #[serde(default)]
    pub signature_html: Option<String>,
    /// What a new message opens with, above the space to type: a greeting such as "Dzień dobry,".
    #[serde(default)]
    pub new_template: String,
    /// What a reply opens with, above the space to type ("Dzień dobry {first},").
    #[serde(default)]
    pub reply_template: String,
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

/// A new identity's signature, reply header and forward header in a language: Polish for
/// Polish and English (Telex's default), the language's own for the others.
fn defaults(lang: Lang) -> (&'static str, &'static str, &'static str) {
    match lang {
        Lang::En | Lang::Pl => (
            DEFAULT_SIGNATURE,
            "Dnia {date} o {time}, {name} napisał(a):",
            "-------- Przekazana wiadomość --------\nOd: {from}\nData: {date} {time}\nTemat: {subject}",
        ),
        Lang::De => (
            "Viele Grüße",
            "Am {date} um {time} schrieb {name}:",
            "-------- Weitergeleitete Nachricht --------\nVon: {from}\nDatum: {date} {time}\nBetreff: {subject}",
        ),
        Lang::Cs => (
            "S pozdravem",
            "Dne {date} v {time} napsal(a) {name}:",
            "-------- Přeposlaná zpráva --------\nOd: {from}\nDatum: {date} {time}\nPředmět: {subject}",
        ),
        Lang::Sk => (
            "S pozdravom",
            "Dňa {date} o {time} napísal(a) {name}:",
            "-------- Preposlaná správa --------\nOd: {from}\nDátum: {date} {time}\nPredmet: {subject}",
        ),
        Lang::Uk => (
            "З повагою",
            "{date} о {time} {name} пише:",
            "-------- Переслане повідомлення --------\nВід: {from}\nДата: {date} {time}\nТема: {subject}",
        ),
        Lang::Es => (
            "Saludos",
            "El {date} a las {time}, {name} escribió:",
            "-------- Mensaje reenviado --------\nDe: {from}\nFecha: {date} {time}\nAsunto: {subject}",
        ),
        Lang::Fr => (
            "Cordialement",
            "Le {date} à {time}, {name} a écrit\u{a0}:",
            "-------- Message transféré --------\nDe\u{a0}: {from}\nDate\u{a0}: {date} {time}\nObjet\u{a0}: {subject}",
        ),
        Lang::It => (
            "Cordiali saluti",
            "Il {date} alle {time}, {name} ha scritto:",
            "-------- Messaggio inoltrato --------\nDa: {from}\nData: {date} {time}\nOggetto: {subject}",
        ),
    }
}

impl Identity {
    /// A default identity with the templates of the interface's language (Polish for Polish
    /// and English).
    pub fn new(account: &str, name: &str, email: &str) -> Self {
        let (signature, reply, forward) = defaults(crate::i18n::current());
        Self {
            id: 0,
            account: account.to_owned(),
            name: name.to_owned(),
            email: email.to_owned(),
            signature: signature.to_owned(),
            signature_html: None,
            new_template: String::new(),
            reply_template: String::new(),
            reply_header: reply.to_owned(),
            forward_header: forward.to_owned(),
            is_default: true,
        }
    }

    /// Catch mistakes before saving, in words for the identity editor.
    pub fn check(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err(tr!(
                "Enter the name people see.",
                "Wpisz nazwę, którą widzą inni."
            ));
        }
        let email = self.email.trim();
        let valid = email
            .split_once('@')
            .is_some_and(|(l, d)| !l.is_empty() && d.contains('.') && !d.contains('@'));
        if !valid || email.contains(char::is_whitespace) {
            return Err(tr!(
                "\"{email}\" doesn't look like an email address.",
                "„{email}” nie wygląda na adres e-mail."
            ));
        }
        Ok(())
    }

    /// Checked, and ready to keep: an HTML signature cleaned as sent mail is, its text made from
    /// it, and a signature without any formatting kept as plain text.
    pub fn prepared(&self) -> Result<Identity, String> {
        self.check()?;
        let mut identity = self.clone();
        let Some(html) = self.signature_html.as_deref() else {
            return Ok(identity);
        };
        let limits = crate::html::SIGNATURE_IMAGES;
        let clean = crate::html::clean_signature(html);
        if clean.matches("<img").count() < html.matches("<img").count() {
            let kb = limits.total / 1024;
            return Err(tr!(
                "Pictures in a signature can take {kb} KB in all. Use a smaller one.",
                "Obrazy w podpisie mogą zająć łącznie {kb} KB. Użyj mniejszego."
            ));
        }
        identity.signature = crate::html::to_text(&clean);
        identity.signature_html = (!crate::html::is_plain(&clean)).then_some(clean);
        Ok(identity)
    }
}

/// A folder's own templates: each one that isn't empty takes the place of the identity's, for
/// new messages written while the folder is open and for replies to mail in it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderTemplate {
    /// "<account>/<folder>"
    pub folder: String,
    #[serde(default)]
    pub new_template: String,
    #[serde(default)]
    pub reply_template: String,
    #[serde(default)]
    pub reply_header: String,
}

impl FolderTemplate {
    pub fn is_empty(&self) -> bool {
        [&self.new_template, &self.reply_template, &self.reply_header]
            .iter()
            .all(|t| t.trim().is_empty())
    }
}

/// Where an account's mail lives. The password is not here: the app keeps it in
/// Windows Credential Manager and hands it over only to connect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImapSettings {
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(default)]
    pub security: ImapSecurity,
    /// A POP3 server instead of IMAP (`pop3.rs`), and what stays on it: the account's folders
    /// are all here then. `None`: IMAP.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pop3: Option<Pop3Keep>,
}

impl ImapSettings {
    /// "imap" or "pop3", for the connection log and the words around it.
    pub fn protocol(&self) -> &'static str {
        if self.pop3.is_some() {
            "pop3"
        } else {
            "imap"
        }
    }
}

/// What happens to mail on a POP3 server once Telex has it, as in Thunderbird: by default it
/// stays there (a phone still sees it) until it is deleted here for good.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pop3Keep {
    /// Leave it on the server; false removes it once it is here.
    #[serde(default = "yes")]
    pub leave: bool,
    /// While it is left there, remove it after this many days; `None`: no limit.
    #[serde(default)]
    pub days: Option<u16>,
    /// While it is left there, remove it once it is deleted here for good (from Trash).
    #[serde(default = "yes")]
    pub until_deleted: bool,
}

fn yes() -> bool {
    true
}

impl Default for Pop3Keep {
    fn default() -> Self {
        Self {
            leave: true,
            days: None,
            until_deleted: true,
        }
    }
}

/// How the IMAP connection is encrypted: the same two choices as for sending, TLS from the
/// first byte (usually port 993) or STARTTLS (usually port 143). Never unencrypted.
pub type ImapSecurity = SmtpSecurity;

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
    /// SSL/TLS on 993 unless the server takes STARTTLS on 143 only.
    #[serde(default)]
    pub imap_security: ImapSecurity,
    /// SMTP server. Defaults to the IMAP server, which is right for most own domains.
    #[serde(default)]
    pub smtp_host: String,
    /// Defaults to 465 for SSL/TLS and 587 for STARTTLS.
    #[serde(default)]
    pub smtp_port: u16,
    #[serde(default)]
    pub smtp_security: SmtpSecurity,
    /// Mail comes over POP3 from `host` (SSL/TLS on 995 or STLS on 110), with what stays on
    /// the server; `None`: IMAP.
    #[serde(default)]
    pub pop3: Option<Pop3Keep>,
}

/// "imaps://Mail.Example.com/" → "mail.example.com", or an error for something that isn't
/// a host name.
fn clean_host(input: &str, what: &str) -> Result<String, String> {
    let host = input.trim();
    let host = [
        "imaps://", "imap://", "pop3s://", "pop3://", "pop://", "smtps://", "smtp://",
    ]
    .iter()
    .find_map(|p| host.strip_prefix(p))
    .unwrap_or(host)
    .trim_end_matches('/')
    .to_ascii_lowercase();
    if host.is_empty() {
        return Err(tr!(
            "Enter the {what} server, e.g. mail.example.com.",
            "Wpisz serwer {what}, np. mail.example.com."
        ));
    }
    if host.contains(|c: char| c.is_whitespace() || matches!(c, '/' | ':' | '@')) {
        return Err(tr!(
            "\"{host}\" isn't a server name. Enter just the name, e.g. mail.example.com.",
            "„{host}” nie jest nazwą serwera. Wpisz samą nazwę, np. mail.example.com."
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
            .ok_or_else(|| {
                tr!(
                    "\"{email}\" doesn't look like an email address.",
                    "„{email}” nie wygląda na adres e-mail."
                )
            })?;
        if email.contains(char::is_whitespace) {
            return Err(tr!(
                "The email address can't contain spaces.",
                "Adres e-mail nie może zawierać spacji."
            ));
        }
        let pop3 = self.pop3.map(|keep| Pop3Keep {
            days: keep.days.map(|d| d.clamp(1, 3650)),
            ..keep
        });
        let host = clean_host(&self.host, if pop3.is_some() { "POP3" } else { "IMAP" })?;
        let port = match (self.port, self.imap_security, pop3.is_some()) {
            (0, SmtpSecurity::Tls, false) => 993,
            (0, SmtpSecurity::StartTls, false) => 143,
            (0, SmtpSecurity::Tls, true) => 995,
            (0, SmtpSecurity::StartTls, true) => 110,
            (p, ..) => p,
        };
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
            port,
            username,
            imap_security: self.imap_security,
            smtp_host,
            smtp_port,
            smtp_security: self.smtp_security,
            pop3,
        })
    }

    pub fn imap_settings(&self) -> ImapSettings {
        ImapSettings {
            host: self.host.clone(),
            port: self.port,
            username: self.username.clone(),
            security: self.imap_security,
            pop3: self.pop3,
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

/// "firma/inbox" → "firma".
pub fn account_of(folder_key: &str) -> &str {
    split_folder_key(folder_key).map_or(folder_key, |(a, _)| a)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    pub name: String,
    pub size_kb: u32,
    /// A file on this PC that goes with a message being written. `None` for attachments
    /// of stored mail, and for those a forward or an edited draft carries over.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
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
    /// Blind copies, on mail the user wrote: drafts and Telex's own Sent copies.
    #[serde(default)]
    pub bcc: String,
    pub subject: String,
    /// Local time the message was received (or written, for outgoing mail).
    pub date: NaiveDateTime,
    pub size_kb: u32,
    #[serde(default)]
    pub unread: bool,
    #[serde(default)]
    pub flagged: bool,
    /// Its tags: keywords of `tags::Tag`s, lowercase, A to Z.
    #[serde(default)]
    pub tags: Vec<String>,
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
    /// The Message-IDs it answers (In-Reply-To, then References), for conversations.
    #[serde(default, skip_serializing)]
    pub refs: Vec<String>,
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
    /// In an Outbox: when it goes (sent later, or waiting out its moment to undo).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goes: Option<NaiveDateTime>,
    /// Keywords of its tags; left out when it has none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// The user replied to it (\Answered), forwarded it ($Forwarded); left out when not.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub answered: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub forwarded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread: Option<ThreadRow>,
}

/// Presentation of a message in a grouped folder list; actions still target its own id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ThreadRow {
    pub id: MessageId,
    pub head: MessageId,
    pub count: usize,
    pub unread: usize,
    pub child: bool,
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
            goes: None,
            tags: m.tags.clone(),
            answered: false,
            forwarded: false,
            thread: None,
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
    /// Blind copies: they get the message, the others don't see them.
    #[serde(default)]
    pub bcc: String,
    pub subject: String,
    /// The text as typed in plain-text compose. Ignored when `html` is set.
    #[serde(default)]
    pub body: String,
    /// The editor's HTML when the message is written in HTML. It is cleaned before it is
    /// sent or saved, and the plain-text version is made from it.
    #[serde(default)]
    pub html: Option<String>,
    /// Attachments: files from this PC (with a `path`), and those carried over from
    /// `forward_of` (without one), found there by name.
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
    /// The last autosave of this same message: a draft of its own, so the one in `replaces`
    /// stays as it was until the user saves or sends. Removed once this is saved or sent.
    #[serde(default)]
    pub autosaved: Option<MessageId>,
    /// Who it is from; `None` is the account's default identity.
    #[serde(default)]
    pub identity: Option<i64>,
    /// Saved by compose's autosave: it stays on this PC (the server's Drafts gets explicit
    /// saves only), and the draft in `replaces` stays as it was.
    #[serde(default)]
    pub autosave: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_identities_speak_the_interface_language() {
        use crate::i18n;
        let polish = i18n::with(Lang::En, || Identity::new("a", "Anna", "anna@x.pl"));
        assert_eq!(polish.signature, "Pozdrawiam");
        assert_eq!(polish.reply_header, default_reply_header());
        let german = i18n::with(Lang::De, || Identity::new("a", "Anna", "anna@x.pl"));
        assert_eq!(german.reply_header, "Am {date} um {time} schrieb {name}:");
        assert!(german.forward_header.contains("Betreff: {subject}"));
        for lang in Lang::ALL {
            let (_, reply, forward) = defaults(lang);
            assert!(
                reply.contains("{name}") && reply.contains("{date}"),
                "{lang:?}"
            );
            assert!(
                forward.contains("{from}") && forward.contains("{subject}"),
                "{lang:?}"
            );
        }
    }

    fn setup(email: &str, host: &str) -> AccountSetup {
        AccountSetup {
            name: String::new(),
            email: email.into(),
            host: host.into(),
            port: 993,
            username: String::new(),
            imap_security: SmtpSecurity::Tls,
            smtp_host: String::new(),
            smtp_port: 0,
            smtp_security: SmtpSecurity::Tls,
            pop3: None,
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

        // No IMAP port: the usual one for the chosen security.
        let mut no_port = setup("biuro@x.pl", "mail.x.pl");
        no_port.port = 0;
        assert_eq!(no_port.clone().cleaned().unwrap().port, 993);
        no_port.imap_security = SmtpSecurity::StartTls;
        let s = no_port.cleaned().unwrap();
        assert_eq!(s.port, 143);
        assert_eq!(s.imap_settings().security, SmtpSecurity::StartTls);
    }

    #[test]
    fn pop3_setups_take_pop3_ports() {
        let mut pop = setup("biuro@x.pl", "pop3s://POP.x.pl");
        pop.port = 0;
        pop.pop3 = Some(Pop3Keep {
            days: Some(0),
            ..Pop3Keep::default()
        });
        let s = pop.clone().cleaned().unwrap();
        assert_eq!((s.host.as_str(), s.port), ("pop.x.pl", 995));
        assert_eq!(s.pop3.unwrap().days, Some(1), "at least a day");
        assert_eq!(s.imap_settings().protocol(), "pop3");
        pop.imap_security = SmtpSecurity::StartTls;
        assert_eq!(pop.cleaned().unwrap().port, 110);
        assert_eq!(
            setup("biuro@x.pl", "mail.x.pl").imap_settings().protocol(),
            "imap"
        );
        let keep: Pop3Keep = serde_json::from_str("{}").unwrap();
        assert_eq!(
            keep,
            Pop3Keep::default(),
            "left on the server until deleted here"
        );
        assert!(keep.leave && keep.until_deleted && keep.days.is_none());
    }

    #[test]
    fn account_setup_catches_typos() {
        assert!(setup("biuro", "mail.x.pl").cleaned().is_err());
        assert!(setup("biuro@x.pl", "").cleaned().is_err());
        assert!(setup("biuro@x.pl", "mail.x.pl:993").cleaned().is_err());
        let mut bad_smtp = setup("biuro@x.pl", "mail.x.pl");
        bad_smtp.smtp_host = "smtp.x.pl:587".into();
        assert!(bad_smtp.cleaned().is_err());
    }

    #[test]
    fn typos_are_explained_in_polish_too() {
        use crate::i18n::{self, Lang};
        let polish = |s: AccountSetup| i18n::with(Lang::Pl, || s.cleaned().unwrap_err());
        assert_eq!(
            polish(setup("biuro", "mail.x.pl")),
            "„biuro” nie wygląda na adres e-mail."
        );
        assert_eq!(
            polish(setup("biuro@x.pl", " ")),
            "Wpisz serwer IMAP, np. mail.example.com."
        );
        assert_eq!(
            setup("biuro@x.pl", " ").cleaned().unwrap_err(),
            "Enter the IMAP server, e.g. mail.example.com.",
            "English stays the default"
        );
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
