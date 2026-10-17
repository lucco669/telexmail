//! Settings: how Telex looks and behaves, kept in `settings.json` next to the mail base, and
//! the settings file people export to move Telex to another PC or to share their rules.
//!
//! An exported file holds the settings, the rules, the identities and the accounts' server
//! settings; never passwords, which stay in Windows Credential Manager. Importing reads the
//! file into a plan first (what changes, what is added, what can't come along and why), and
//! applies only the parts the user ticks. Rules and identities find their account by its email
//! address and their folders by id, then by name, so they land right on another PC.
//!
//! Values this version doesn't know (a theme from a newer Telex, a rule action it doesn't
//! have) are left out one by one; the rest of the file still counts. Reasons are codes, not
//! sentences: the UI words them, so they translate with it.

use std::fmt;
use std::path::Path;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::filters::{Action, Condition, Every, Rule, Schedule};
use crate::model::{
    split_folder_key, Account, AccountSetup, FolderTemplate, Identity, ImapSettings, SmtpSettings,
};
use crate::store::{MailStore, StoreError, LOCAL_ID};

/// Themes in the order the settings show them. "system" follows Windows' light or dark mode;
/// each of the others has a block in `ui/themes.css`. `scripts/check-contrast.mjs` checks that
/// the two lists match and that every theme meets its WCAG level.
pub const THEMES: &[&str] = &[
    "system",
    "light",
    "dark",
    "paper",
    "heather",
    "fjord",
    "phosphor",
    "contrast-light",
    "contrast-dark",
];
/// Languages the interface speaks.
pub const LANGUAGES: &[&str] = &["en", "pl"];
/// Text sizes, in percent of the normal size.
pub const TEXT_SIZES: &[u16] = &[90, 100, 110, 125, 150, 175, 200];
/// Minutes between background checks for mail. 0: only when the user asks.
pub const CHECK_INTERVALS: &[u16] = &[0, 5, 10, 15, 30, 60];
/// Seconds sent mail waits in the Outbox, so Send can be undone. 0: it goes at once.
pub const UNDO_SEND: &[u16] = &[0, 5, 10, 20, 30];
/// Milliseconds a message stays on screen before it counts as read ("a moment" first).
pub const READ_DELAYS: &[u16] = &[600, 1000, 2000, 3000, 5000, 10000];
/// How many backups the backup folder keeps; older ones are removed.
pub const BACKUP_KEEP: &[u16] = &[3, 5, 10, 20];

/// The keys of [`Settings`] in the order the settings window shows them.
const KEYS: &[&str] = &[
    "theme",
    "language",
    "toolbar_style",
    "density",
    "account_mark",
    "unread_style",
    "threaded",
    "font",
    "text_size",
    "prefer_text",
    "mark_read",
    "read_delay",
    "check_every",
    "push_mail",
    "notify",
    "mail_paper",
    "risk_checks",
    "spam_filter",
    "compose_format",
    "reply_format",
    "sign_replies",
    "undo_send",
    "ask_subject",
    "ask_attachment",
    "ask_many",
    "spell_check",
    "spell_languages",
    "updates",
    "backup_every",
    "backup_keep",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// One of [`THEMES`].
    pub theme: String,
    /// "auto" follows Windows' display language; otherwise one of [`LANGUAGES`].
    pub language: String,
    /// How the top toolbar shows its actions. Older settings keep the classic layout.
    #[serde(default)]
    pub toolbar_style: ToolbarStyle,
    pub density: Density,
    /// How each account's color shows, once there is more than one.
    pub account_mark: AccountMark,
    /// How unread mail stands out from read mail in the message list.
    pub unread_style: UnreadStyle,
    /// Group related messages in folder lists. Off until the user enables it.
    #[serde(default)]
    pub threaded: bool,
    /// The faces the window is written in.
    pub font: Font,
    /// Percent, one of [`TEXT_SIZES`].
    pub text_size: u16,
    /// Show HTML mail as plain text (H switches).
    pub prefer_text: bool,
    pub mark_read: MarkRead,
    /// Milliseconds, one of [`READ_DELAYS`]: how long "a moment" on screen is (`MarkRead::Moment`).
    pub read_delay: u16,
    /// Minutes, one of [`CHECK_INTERVALS`].
    pub check_every: u16,
    /// New mail in an Inbox shows at once: Telex keeps a connection open (IMAP IDLE) to
    /// each server that offers it.
    pub push_mail: bool,
    /// A system notification for new mail while Telex's window isn't the active one.
    pub notify: bool,
    /// HTML mail in a dark theme: on the theme's dark paper, or on white as sent.
    pub mail_paper: MailPaper,
    /// A line above opened mail that shows scam warning signs (`risk::analyse`), with its reasons.
    #[serde(default = "yes")]
    pub risk_checks: bool,
    /// Telex's own spam filter (`spam.rs`): how sure it must be to move new mail to Spam.
    #[serde(default)]
    pub spam_filter: crate::spam::Level,
    /// What a new message is written in.
    pub compose_format: ComposeFormat,
    /// What a reply or a forward is written in.
    pub reply_format: ReplyFormat,
    /// Replies and forwards open with the identity's signature too (new messages always do).
    #[serde(default = "yes")]
    pub sign_replies: bool,
    /// Seconds, one of [`UNDO_SEND`]: how long sent mail waits so Z can take it back.
    pub undo_send: u16,
    /// Send asks once more when the subject is empty.
    pub ask_subject: bool,
    /// Send asks once more when the text mentions an attachment and none is attached.
    #[serde(default = "yes")]
    pub ask_attachment: bool,
    /// Send asks once more when a reply to all goes to many people.
    #[serde(default = "yes")]
    pub ask_many: bool,
    /// How large pictures put into a message are made (`images::prepare`).
    #[serde(default)]
    pub image_size: ImageSize,
    /// Underline misspelt words while writing.
    pub spell_check: bool,
    /// The languages spelling is checked in ("pl-PL"); a word any of them knows is right.
    /// Empty: Windows' languages and the interface's, as far as there are dictionaries.
    pub spell_languages: Vec<String>,
    pub updates: Updates,
    /// Whether this copy may be counted once a day (`count.rs`). It belongs to this PC, like
    /// `backup_dir`: an exported settings file leaves it out, and importing one keeps this PC's.
    #[serde(default)]
    pub usage_count: UsageCount,
    /// Back up the mail base by itself: never, every day or every week.
    pub backup_every: BackupEvery,
    /// The folder backups go to, empty until the user picks one. It belongs to this PC: an
    /// exported settings file leaves it out, and importing one keeps this PC's.
    pub backup_dir: String,
    /// How many backups to keep there, one of [`BACKUP_KEEP`].
    pub backup_keep: u16,
}

/// How often the mail base backs itself up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupEvery {
    Off,
    Daily,
    Weekly,
}

impl BackupEvery {
    /// How old the newest backup may be before the next one is due.
    pub fn period(self) -> Option<chrono::TimeDelta> {
        match self {
            Self::Off => None,
            Self::Daily => Some(chrono::TimeDelta::days(1)),
            Self::Weekly => Some(chrono::TimeDelta::days(7)),
        }
    }
}

/// The paper HTML mail is shown on in a dark theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MailPaper {
    /// The theme's own dark paper, with the message's colors turned around to match.
    Theme,
    /// White, as the sender designed it.
    White,
}

/// Plain text or HTML.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComposeFormat {
    Plain,
    Html,
}

/// What a reply or a forward is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplyFormat {
    /// HTML when the message has HTML, else plain text.
    Same,
    Plain,
    Html,
}

/// How large a picture put into a message is made: as it is, or smaller (its longest side).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageSize {
    Original,
    /// 1600 pixels.
    #[default]
    Large,
    /// 800 pixels.
    Medium,
}

/// How actions are shown in the top toolbar.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolbarStyle {
    #[default]
    Classic,
    Icons,
    Large,
    Below,
    Text,
}

/// Row height in the lists: 20 px, or 26 px for people who want air.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Density {
    Compact,
    Comfortable,
}

/// How the folder tree (and wherever an account is named) tells accounts apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountMark {
    /// A thin line in the account's color down the side of its folders.
    Rail,
    /// A small dot before the address.
    Dot,
    /// The account's initials on a tint of its color.
    Initials,
    /// A soft wash of its color behind the account's line.
    Tint,
    /// Nothing: the addresses say it.
    None,
}

/// How unread mail stands out in the message list. Every style draws it bold with a dot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnreadStyle {
    /// Read mail in a quieter color, so unread mail stands out at a glance.
    Quiet,
    /// Only the weight: read and unread mail in the same color.
    Bold,
    /// Unread mail in the theme's accent color.
    Color,
}

/// The faces the window is written in: one for its words, one for the message list, and a
/// mono one for dates, sizes and plain-text writing. Plex, Inter, Atkinson and Source are
/// bundled (ui/fonts); System and Classic use the fonts Windows has, and stand-ins elsewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Font {
    /// IBM Plex Sans, Sans Condensed in the list, and Plex Mono.
    Plex,
    /// The system's own: Segoe UI Variable and Cascadia Mono on Windows 11.
    System,
    /// Tahoma and Consolas, as mail programs on Windows used to look.
    Classic,
    /// Inter with JetBrains Mono.
    Inter,
    /// Atkinson Hyperlegible Next and Mono, made for low vision: no two letters alike.
    Atkinson,
    /// Source Sans 3 and Source Code Pro in their light weights: an airy page.
    Source,
}

/// When a message on screen counts as read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkRead {
    /// After it has been on screen for `read_delay`, so arrowing past a message doesn't.
    Moment,
    /// As soon as it is selected.
    AtOnce,
    /// When it is opened in a window of its own; selecting it in the list doesn't.
    Window,
    /// Only when the user marks it (U, the dot in the list).
    Manual,
}

/// Checking for a newer Telex.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Updates {
    /// Not answered yet: a build that can update itself asks in the status bar.
    Ask,
    /// Each time Telex starts, and once a day while it stays open.
    Start,
    /// Once a day.
    Daily,
    Off,
}

/// Counting this copy of Telex (`count.rs`): only after the user says yes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageCount {
    /// Not answered yet: a release build asks in the status bar.
    #[default]
    Ask,
    On,
    Off,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            language: "auto".into(),
            toolbar_style: ToolbarStyle::Classic,
            density: Density::Compact,
            account_mark: AccountMark::Rail,
            unread_style: UnreadStyle::Quiet,
            threaded: false,
            font: Font::Plex,
            text_size: 100,
            prefer_text: false,
            mark_read: MarkRead::Moment,
            read_delay: 600,
            check_every: 10,
            push_mail: true,
            notify: true,
            mail_paper: MailPaper::Theme,
            risk_checks: true,
            spam_filter: crate::spam::Level::Careful,
            compose_format: ComposeFormat::Html,
            reply_format: ReplyFormat::Html,
            sign_replies: true,
            undo_send: 5,
            ask_subject: true,
            ask_attachment: true,
            ask_many: true,
            image_size: ImageSize::Large,
            spell_check: true,
            spell_languages: Vec::new(),
            updates: Updates::Ask,
            usage_count: UsageCount::Ask,
            backup_every: BackupEvery::Off,
            backup_dir: String::new(),
            backup_keep: 5,
        }
    }
}

/// `settings.json` as read at start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded {
    pub settings: Settings,
    /// There was no file yet: the first start of a Telex that keeps settings.
    pub fresh: bool,
}

/// One setting an import would change.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SettingChange {
    pub key: String,
    pub from: Value,
    pub to: Value,
}

impl Settings {
    fn is_valid(&self) -> bool {
        THEMES.contains(&self.theme.as_str())
            && (self.language == "auto" || LANGUAGES.contains(&self.language.as_str()))
            && TEXT_SIZES.contains(&self.text_size)
            && READ_DELAYS.contains(&self.read_delay)
            && CHECK_INTERVALS.contains(&self.check_every)
            && UNDO_SEND.contains(&self.undo_send)
            && BACKUP_KEEP.contains(&self.backup_keep)
            && self.spell_languages.len() <= 12
            && self.spell_languages.iter().all(|tag| {
                (2..=20).contains(&tag.len())
                    && tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            })
    }

    fn to_map(&self) -> Map<String, Value> {
        match serde_json::to_value(self) {
            Ok(Value::Object(map)) => map,
            _ => Map::new(),
        }
    }

    /// These settings with the values of `changes` (a JSON object) taken over one by one. A
    /// value this version can't use (a newer theme, a typo, a key it doesn't have) is left
    /// out and its key returned; the others still count.
    pub fn merged(&self, changes: &Value) -> (Settings, Vec<String>) {
        let mut current = self.clone();
        let mut ignored = Vec::new();
        let Some(changes) = changes.as_object() else {
            return (current, ignored);
        };
        for (key, value) in changes {
            let mut map = current.to_map();
            if !map.contains_key(key) {
                ignored.push(key.clone());
                continue;
            }
            map.insert(key.clone(), value.clone());
            match serde_json::from_value::<Settings>(Value::Object(map)) {
                Ok(next) if next.is_valid() => current = next,
                _ => ignored.push(key.clone()),
            }
        }
        (current, ignored)
    }

    /// What differs in `other`, in the order the settings window shows it.
    pub fn changes_to(&self, other: &Settings) -> Vec<SettingChange> {
        let (from, to) = (self.to_map(), other.to_map());
        KEYS.iter()
            .filter(|k| from.get(**k) != to.get(**k))
            .map(|k| SettingChange {
                key: (*k).to_owned(),
                from: from.get(*k).cloned().unwrap_or(Value::Null),
                to: to.get(*k).cloned().unwrap_or(Value::Null),
            })
            .collect()
    }

    /// Read `settings.json`. No file: the defaults, and `fresh`. A damaged file or one with
    /// values this version doesn't know: what can be read, defaults for the rest.
    pub fn load(path: &Path) -> Loaded {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
                    .unwrap_or(Value::Null);
                Loaded {
                    settings: Settings::default().merged(&value).0,
                    fresh: false,
                }
            }
            Err(e) => Loaded {
                settings: Settings::default(),
                fresh: e.kind() == std::io::ErrorKind::NotFound,
            },
        }
    }

    /// Write `settings.json` whole or not at all: into a temporary file, then renamed over
    /// the old one, so a crash halfway leaves the old settings.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, text + "\n")?;
        std::fs::rename(&temp, path)
    }

    /// The language the interface shows: the chosen one, or for "auto" the first of Windows'
    /// display languages that Telex speaks, else English.
    pub fn effective_language(&self, system: &SystemLocale) -> &'static str {
        if let Some(chosen) = LANGUAGES.iter().find(|l| **l == self.language) {
            return chosen;
        }
        system
            .languages
            .iter()
            .find_map(|tag| spoken(tag))
            .unwrap_or("en")
    }

    /// A language to offer once, on the first start: when the choice is "auto" and Windows
    /// shows a language other than the one its region uses, and Telex speaks that one. Common
    /// in Poland: English Windows with Polish formats.
    pub fn language_to_offer(&self, system: &SystemLocale) -> Option<&'static str> {
        if self.language != "auto" {
            return None;
        }
        let regional = spoken(system.region.as_deref()?)?;
        (regional != self.effective_language(system)).then_some(regional)
    }
}

/// What Windows says about the user's languages.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SystemLocale {
    /// Display languages in the user's order, e.g. `["pl-PL", "en-US"]`.
    pub languages: Vec<String>,
    /// The regional format, e.g. "pl-PL": dates, numbers. Often Polish on an English Windows.
    pub region: Option<String>,
    /// The languages the user writes in (Settings › Time & language › Language, with their
    /// keyboards), e.g. `["pl", "en-US"]`: Polish here even when Windows shows English.
    #[serde(default)]
    pub writing: Vec<String>,
}

/// "pl-PL" → "pl" when Telex speaks it.
fn spoken(tag: &str) -> Option<&'static str> {
    let base = tag.split(['-', '_']).next()?.to_ascii_lowercase();
    LANGUAGES.iter().copied().find(|l| *l == base)
}

/// Windows' display languages, in the order the user put them, and its regional format.
#[cfg(windows)]
pub fn system_locale() -> SystemLocale {
    use std::ptr::null_mut;
    use windows_sys::Win32::Globalization::{
        GetUserDefaultLocaleName, GetUserPreferredUILanguages, MUI_LANGUAGE_NAME,
    };

    let mut languages = Vec::new();
    let (mut count, mut len) = (0u32, 0u32);
    // SAFETY: the first call writes only the two counts and asks for the buffer's length; the
    // second fills a buffer of exactly that length with NUL-separated names.
    unsafe {
        if GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, null_mut(), &mut len) != 0
            && len > 0
        {
            let mut buf = vec![0u16; len as usize];
            if GetUserPreferredUILanguages(
                MUI_LANGUAGE_NAME,
                &mut count,
                buf.as_mut_ptr(),
                &mut len,
            ) != 0
            {
                languages = buf
                    .split(|c| *c == 0)
                    .filter(|name| !name.is_empty())
                    .map(String::from_utf16_lossy)
                    .collect();
            }
        }
    }
    // LOCALE_NAME_MAX_LENGTH, NUL included.
    let mut buf = [0u16; 85];
    // SAFETY: Windows writes at most `buf.len()` characters and returns how many, NUL included.
    let written = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) };
    let region = usize::try_from(written)
        .ok()
        .filter(|n| *n > 1)
        .map(|n| String::from_utf16_lossy(&buf[..n - 1]));
    SystemLocale {
        languages,
        region,
        writing: writing_languages(),
    }
}

/// The user's language list, as Windows keeps it for Settings › Language.
#[cfg(windows)]
fn writing_languages() -> Vec<String> {
    use windows_sys::Win32::System::Registry::{
        RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_MULTI_SZ,
    };
    let key: Vec<u16> = "Control Panel\\International\\User Profile\0"
        .encode_utf16()
        .collect();
    let value: Vec<u16> = "Languages\0".encode_utf16().collect();
    let mut buf = vec![0u16; 1024];
    let mut size = (buf.len() * 2) as u32;
    // SAFETY: both names are NUL-terminated; Windows writes at most `size` bytes into `buf`.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_MULTI_SZ,
            std::ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if status != 0 {
        return Vec::new();
    }
    buf.truncate(size as usize / 2);
    buf.split(|c| *c == 0)
        .filter(|name| !name.is_empty())
        .map(String::from_utf16_lossy)
        .collect()
}

/// Elsewhere (the tests on Linux): the LANG variable, e.g. "pl_PL.UTF-8".
#[cfg(not(windows))]
pub fn system_locale() -> SystemLocale {
    let lang = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default();
    let tag = lang.split('.').next().unwrap_or("").replace('_', "-");
    let languages = match tag.as_str() {
        "" | "C" | "POSIX" => Vec::new(),
        _ => vec![tag],
    };
    SystemLocale {
        region: languages.first().cloned(),
        writing: languages.clone(),
        languages,
    }
}

// ---- The settings file people export and import ----

/// What the first line of the file says it is.
pub const FORMAT: &str = "telex-settings";
/// The layout of the file. A newer one is read as far as it goes.
pub const FORMAT_VERSION: u32 = 1;

/// The parts to put in an exported file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct ExportChoice {
    pub settings: bool,
    pub rules: bool,
    pub identities: bool,
    /// The accounts' IMAP and SMTP servers and user names. Never passwords.
    pub servers: bool,
}

#[derive(Serialize)]
struct Bundle<'a> {
    format: &'static str,
    version: u32,
    /// Local time, as dates cross the bridge: "2026-10-10T09:30:00".
    exported: String,
    /// The Telex version that wrote it.
    app: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    settings: Option<&'a Settings>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    accounts: Vec<AccountEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    rules: Vec<RuleEntry>,
    /// Folders' own templates, with the identities.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    templates: Vec<TemplateEntry>,
    /// The tags, with the rules: rules use them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tags: Vec<TagEntry>,
}

/// A tag by its keyword; `name` is left out while one of Thunderbird's five has its own.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TagEntry {
    keyword: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AccountEntry {
    #[serde(default)]
    name: String,
    email: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    imap: Option<ImapSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    smtp: Option<SmtpSettings>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    identities: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IdentityEntry {
    name: String,
    email: String,
    #[serde(default)]
    signature: String,
    /// The signature in HTML; missing for a plain-text one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    signature_html: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    new_template: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    reply_template: String,
    /// Missing: Telex's usual (Polish) header, as for a new identity.
    #[serde(default)]
    reply_header: Option<String>,
    #[serde(default)]
    forward_header: Option<String>,
    #[serde(default)]
    is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RuleEntry {
    name: String,
    #[serde(default = "yes")]
    enabled: bool,
    /// The account's email address; none for a rule on every account.
    #[serde(default)]
    account: Option<String>,
    #[serde(default = "yes")]
    match_all: bool,
    conditions: Vec<Condition>,
    actions: Vec<ActionEntry>,
    #[serde(default)]
    stop: bool,
    /// Missing in files from before schedules: those rules all ran on new mail.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    on_arrival: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    schedule: Option<ScheduleEntry>,
}

/// [`Schedule`] with its folders spelled out, so they can be found on another PC.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScheduleEntry {
    every: Every,
    folders: Vec<FolderEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FolderEntry {
    /// Its name, and its id, which is tried first.
    folder: String,
    #[serde(default)]
    folder_id: String,
    /// Its account's address; none for a folder on this PC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    account: Option<String>,
}

/// A folder's own templates, the folder found as a rule's is: its account by address (or
/// `this_pc`), then the folder by id, else by name.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TemplateEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    account: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    this_pc: bool,
    folder: String,
    #[serde(default)]
    folder_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    new_template: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    reply_template: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    reply_header: String,
}

fn yes() -> bool {
    true
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_true(b: &bool) -> bool {
    *b
}

/// [`Action`] with the folder spelled out, so it can be found on another PC.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ActionEntry {
    /// Move to a folder of the rule's account (or with `this_pc`, a folder on this PC): by its
    /// id, else by its name.
    Move {
        folder: String,
        #[serde(default)]
        folder_id: String,
        #[serde(default, skip_serializing_if = "is_false")]
        this_pc: bool,
    },
    Copy {
        folder: String,
        #[serde(default)]
        folder_id: String,
        #[serde(default, skip_serializing_if = "is_false")]
        this_pc: bool,
    },
    Forward {
        to: String,
    },
    MarkRead,
    Flag,
    AddTag {
        tag: String,
    },
    RemoveTag {
        tag: String,
    },
    Delete,
    AutoReply {
        body: String,
    },
}

fn identity_entry(i: &Identity) -> IdentityEntry {
    IdentityEntry {
        name: i.name.clone(),
        email: i.email.clone(),
        signature: i.signature.clone(),
        signature_html: i.signature_html.clone(),
        new_template: i.new_template.clone(),
        reply_template: i.reply_template.clone(),
        reply_header: Some(i.reply_header.clone()),
        forward_header: Some(i.forward_header.clone()),
        is_default: i.is_default,
    }
}

fn template_entry(t: &FolderTemplate, accounts: &[Account]) -> Option<TemplateEntry> {
    let (owner, folder_id) = split_folder_key(&t.folder)?;
    let account = accounts.iter().find(|a| a.id == owner)?;
    let folder = account.folders.iter().find(|f| f.id == folder_id)?;
    let this_pc = owner == LOCAL_ID;
    Some(TemplateEntry {
        account: (!this_pc).then(|| account.email.clone()),
        this_pc,
        folder: folder.name.clone(),
        folder_id: folder.id.clone(),
        new_template: t.new_template.clone(),
        reply_template: t.reply_template.clone(),
        reply_header: t.reply_header.clone(),
    })
}

fn rule_entry(rule: &Rule, accounts: &[Account]) -> RuleEntry {
    let account = rule
        .account
        .as_ref()
        .and_then(|id| accounts.iter().find(|a| &a.id == id));
    let actions = rule
        .actions
        .iter()
        .map(|action| match action {
            Action::Move { folder } | Action::Copy { folder } => {
                let (owner, folder_id) = split_folder_key(folder).unwrap_or(("", folder.as_str()));
                let this_pc = owner == LOCAL_ID;
                let name = if this_pc {
                    accounts.iter().find(|a| a.id == LOCAL_ID)
                } else {
                    account
                }
                .and_then(|a| a.folders.iter().find(|f| f.id == folder_id))
                .map_or(folder_id, |f| f.name.as_str());
                if matches!(action, Action::Copy { .. }) {
                    ActionEntry::Copy {
                        folder: name.to_owned(),
                        folder_id: folder_id.to_owned(),
                        this_pc,
                    }
                } else {
                    ActionEntry::Move {
                        folder: name.to_owned(),
                        folder_id: folder_id.to_owned(),
                        this_pc,
                    }
                }
            }
            Action::Forward { to } => ActionEntry::Forward { to: to.clone() },
            Action::MarkRead => ActionEntry::MarkRead,
            Action::Flag => ActionEntry::Flag,
            Action::AddTag { tag } => ActionEntry::AddTag { tag: tag.clone() },
            Action::RemoveTag { tag } => ActionEntry::RemoveTag { tag: tag.clone() },
            Action::Delete => ActionEntry::Delete,
            Action::AutoReply { body } => ActionEntry::AutoReply { body: body.clone() },
        })
        .collect();
    RuleEntry {
        name: rule.name.clone(),
        enabled: rule.enabled,
        account: account.map(|a| a.email.clone()),
        match_all: rule.match_all,
        conditions: rule.conditions.clone(),
        actions,
        stop: rule.stop,
        on_arrival: rule.on_arrival,
        schedule: rule.schedule.as_ref().map(|s| ScheduleEntry {
            every: s.every,
            folders: s
                .folders
                .iter()
                .map(|key| {
                    let (owner, folder_id) = split_folder_key(key).unwrap_or(("", key.as_str()));
                    let owner = accounts.iter().find(|a| a.id == owner);
                    FolderEntry {
                        folder: owner
                            .and_then(|a| a.folders.iter().find(|f| f.id == folder_id))
                            .map_or(folder_id, |f| f.name.as_str())
                            .to_owned(),
                        folder_id: folder_id.to_owned(),
                        account: owner.filter(|a| a.id != LOCAL_ID).map(|a| a.email.clone()),
                    }
                })
                .collect(),
        }),
    }
}

/// The exported file's text: pretty JSON, readable and diffable.
pub fn export(
    store: &MailStore,
    settings: &Settings,
    choice: ExportChoice,
    app_version: &str,
    now: NaiveDateTime,
) -> Result<String, StoreError> {
    let accounts = store.accounts();
    let entries = if choice.servers || choice.identities {
        accounts
            .iter()
            .filter(|a| a.imap.is_some())
            .map(|a| AccountEntry {
                name: a.name.clone(),
                email: a.email.clone(),
                imap: a.imap.clone().filter(|_| choice.servers),
                smtp: a.smtp.clone().filter(|_| choice.servers),
                identities: if choice.identities {
                    a.identities
                        .iter()
                        .filter_map(|i| serde_json::to_value(identity_entry(i)).ok())
                        .collect()
                } else {
                    Vec::new()
                },
            })
            .collect()
    } else {
        Vec::new()
    };
    let rules = if choice.rules {
        store
            .rules()?
            .iter()
            .map(|r| rule_entry(r, accounts))
            .collect()
    } else {
        Vec::new()
    };
    let tags = if choice.rules {
        store
            .tags()?
            .into_iter()
            .map(|t| TagEntry {
                name: (crate::tags::default_name(&t.keyword).as_deref() != Some(&t.name))
                    .then_some(t.name),
                keyword: t.keyword,
                color: t.color,
            })
            .collect()
    } else {
        Vec::new()
    };
    let templates = if choice.identities {
        store
            .folder_templates()?
            .iter()
            .filter_map(|t| template_entry(t, accounts))
            .collect()
    } else {
        Vec::new()
    };
    // Where backups go, and whether this copy is counted, are this PC's own business.
    let portable = Settings {
        backup_dir: String::new(),
        usage_count: UsageCount::Ask,
        ..settings.clone()
    };
    let bundle = Bundle {
        format: FORMAT,
        version: FORMAT_VERSION,
        exported: now.format("%Y-%m-%dT%H:%M:%S").to_string(),
        app: app_version,
        settings: choice.settings.then_some(&portable),
        accounts: entries,
        rules,
        tags,
        templates,
    };
    serde_json::to_string_pretty(&bundle)
        .map(|text| text + "\n")
        .map_err(|e| StoreError::Database(e.to_string()))
}

/// Why a file can't be imported at all. Crosses the bridge as `{code, detail}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "code", content = "detail", rename_all = "snake_case")]
pub enum ImportError {
    /// The file couldn't be read (the system's words).
    Unreadable(String),
    /// It isn't JSON (the parser's words).
    NotJson(String),
    /// It is JSON, but not a Telex settings file.
    NotTelex,
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable(e) => write!(f, "Telex couldn't read the file: {e}."),
            Self::NotJson(e) => write!(f, "This isn't a Telex settings file ({e})."),
            Self::NotTelex => f.write_str("This isn't a Telex settings file."),
        }
    }
}

impl std::error::Error for ImportError {}

/// What happens to one rule or identity from the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Add,
    /// One with the same name (rules) or address (identities) is here: it gets the file's version.
    Update,
    /// Already here, exactly so.
    Same,
    Skip,
}

/// Why a rule or identity can't come along.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum Skip {
    /// The account isn't in Telex here. Set it up, then import again.
    NoAccount { email: String },
    /// The rule moves mail to a folder this account doesn't have.
    NoFolder { folder: String },
    /// Something this version can't read: from a newer Telex, or edited by hand.
    Unreadable,
    /// It doesn't pass the editor's checks; the sentence says why.
    Invalid { detail: String },
    /// A filter from another program needs something Telex's rules can't do: a condition
    /// or an action, in that program's own words.
    Unsupported { what: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanItem {
    /// The rule's name, the identity's "Name <address>", or the name of the folder whose
    /// templates these are.
    pub name: String,
    /// The address of the account it belongs to; none for a rule on every account.
    pub account: Option<String>,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip: Option<Skip>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AccountPlan {
    pub name: String,
    pub email: String,
    /// Already set up in Telex here.
    pub present: bool,
    /// The servers from the file, to fill in the setup form. No password.
    pub setup: Option<AccountSetup>,
}

/// What an import would do, for the preview.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImportPlan {
    /// When and by which Telex the file was made.
    pub exported: String,
    pub app: String,
    /// Made by a newer Telex: what this one doesn't know is left out.
    pub newer: bool,
    /// The file has settings, even if none of them would change anything.
    pub has_settings: bool,
    pub settings: Vec<SettingChange>,
    /// Settings this version can't use, by key.
    pub settings_ignored: Vec<String>,
    pub identities: Vec<PlanItem>,
    /// Folders' own templates; they come with the identities.
    pub templates: Vec<PlanItem>,
    pub rules: Vec<PlanItem>,
    /// The file's tags, which come with the rules (`name` is the tag's).
    pub tags: Vec<PlanItem>,
    pub accounts: Vec<AccountPlan>,
}

/// The parts of a file to import.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct ImportChoice {
    pub settings: bool,
    pub identities: bool,
    pub rules: bool,
}

/// What an import did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ImportReport {
    /// Settings changed.
    pub settings: usize,
    pub identities_added: usize,
    pub identities_updated: usize,
    /// Folders whose templates were set.
    pub templates: usize,
    pub rules_added: usize,
    pub rules_updated: usize,
    pub tags_added: usize,
    pub tags_updated: usize,
    /// One line per rule or identity that failed to save after all.
    pub problems: Vec<String>,
}

/// A settings file read and matched against this Telex: the plan to show, and the changes
/// that applying it makes.
#[derive(Debug, Clone)]
pub struct Import {
    pub plan: ImportPlan,
    settings: Settings,
    identities: Vec<(Status, Identity)>,
    templates: Vec<FolderTemplate>,
    rules: Vec<(Status, Rule)>,
    tags: Vec<(Status, crate::tags::Tag)>,
}

#[derive(Deserialize)]
struct RawBundle {
    #[serde(default)]
    format: String,
    #[serde(default)]
    version: u32,
    #[serde(default)]
    exported: String,
    #[serde(default)]
    app: String,
    #[serde(default)]
    settings: Option<Value>,
    #[serde(default)]
    accounts: Vec<Value>,
    #[serde(default)]
    rules: Vec<Value>,
    #[serde(default)]
    tags: Vec<Value>,
    #[serde(default)]
    templates: Vec<Value>,
}

fn same_text(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

/// The name of an entry that didn't parse, if it has one.
fn name_of(value: &Value) -> String {
    value
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_owned()
}

fn skipped(name: String, account: Option<String>, why: Skip) -> PlanItem {
    PlanItem {
        name,
        account,
        status: Status::Skip,
        skip: Some(why),
    }
}

impl Import {
    /// Read a settings file and match it against this store and these settings.
    pub fn read(text: &str, store: &MailStore, current: &Settings) -> Result<Import, ImportError> {
        // Notepad may add a byte-order mark when the file is edited and saved again.
        let value: Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
            .map_err(|e| ImportError::NotJson(e.to_string()))?;
        let raw: RawBundle = serde_json::from_value(value).map_err(|_| ImportError::NotTelex)?;
        if raw.format != FORMAT {
            return Err(ImportError::NotTelex);
        }
        let (mut settings, settings_ignored) = match &raw.settings {
            Some(s) => current.merged(s),
            None => (current.clone(), Vec::new()),
        };
        // Where backups go, and whether this copy is counted, belong to this PC.
        settings.backup_dir.clone_from(&current.backup_dir);
        settings.usage_count = current.usage_count;
        let accounts = store.accounts();
        let entries: Vec<AccountEntry> = raw
            .accounts
            .iter()
            .filter_map(|v| serde_json::from_value(v.clone()).ok())
            .collect();
        let local = |email: &str| {
            accounts
                .iter()
                .find(|a| a.email.eq_ignore_ascii_case(email))
        };

        let mut identities = Vec::new();
        let mut identity_items = Vec::new();
        for entry in &entries {
            for value in &entry.identities {
                let Ok(found) = serde_json::from_value::<IdentityEntry>(value.clone()) else {
                    let why = Skip::Unreadable;
                    identity_items.push(skipped(name_of(value), Some(entry.email.clone()), why));
                    continue;
                };
                let label = format!("{} <{}>", found.name.trim(), found.email.trim());
                let Some(account) = local(&entry.email) else {
                    let why = Skip::NoAccount {
                        email: entry.email.clone(),
                    };
                    identity_items.push(skipped(label, Some(entry.email.clone()), why));
                    continue;
                };
                let fresh = Identity::new(&account.id, &found.name, &found.email);
                let read = Identity {
                    signature: found.signature.clone(),
                    signature_html: found.signature_html.clone(),
                    new_template: found.new_template.clone(),
                    reply_template: found.reply_template.clone(),
                    reply_header: found.reply_header.clone().unwrap_or(fresh.reply_header),
                    forward_header: found.forward_header.clone().unwrap_or(fresh.forward_header),
                    is_default: found.is_default,
                    ..fresh
                };
                let mut identity = match read.prepared() {
                    Ok(identity) => identity,
                    Err(detail) => {
                        let why = Skip::Invalid { detail };
                        identity_items.push(skipped(label, Some(account.email.clone()), why));
                        continue;
                    }
                };
                let existing = account
                    .identities
                    .iter()
                    .find(|i| i.email.trim().eq_ignore_ascii_case(identity.email.trim()));
                let status = match existing {
                    None => Status::Add,
                    Some(e) => {
                        identity.id = e.id;
                        let unchanged = e.name.trim() == identity.name.trim()
                            && e.signature == identity.signature
                            && e.signature_html == identity.signature_html
                            && e.new_template == identity.new_template
                            && e.reply_template == identity.reply_template
                            && e.reply_header == identity.reply_header
                            && e.forward_header == identity.forward_header
                            && e.is_default == identity.is_default;
                        if unchanged {
                            Status::Same
                        } else {
                            Status::Update
                        }
                    }
                };
                identity_items.push(PlanItem {
                    name: label,
                    account: Some(account.email.clone()),
                    status,
                    skip: None,
                });
                if status != Status::Same {
                    identities.push((status, identity));
                }
            }
        }

        let (rule_items, rules) = plan_rules(&raw.rules, store);
        let (tag_items, tags) = plan_tags(&raw.tags, store);
        let (template_items, templates) = plan_templates(&raw.templates, store);

        let account_plans = entries
            .iter()
            .filter(|e| e.imap.is_some() || local(&e.email).is_some())
            .map(|e| AccountPlan {
                name: e.name.clone(),
                email: e.email.clone(),
                present: local(&e.email).is_some(),
                setup: e.imap.as_ref().map(|imap| {
                    let smtp = e.smtp.clone();
                    AccountSetup {
                        name: e.name.clone(),
                        email: e.email.clone(),
                        host: imap.host.clone(),
                        port: imap.port,
                        username: imap.username.clone(),
                        imap_security: imap.security,
                        smtp_host: smtp.as_ref().map(|s| s.host.clone()).unwrap_or_default(),
                        smtp_port: smtp.as_ref().map_or(0, |s| s.port),
                        smtp_security: smtp.map(|s| s.security).unwrap_or_default(),
                    }
                }),
            })
            .collect();

        Ok(Import {
            plan: ImportPlan {
                exported: raw.exported,
                app: raw.app,
                newer: raw.version > FORMAT_VERSION,
                has_settings: raw.settings.is_some(),
                settings: current.changes_to(&settings),
                settings_ignored,
                identities: identity_items,
                templates: template_items,
                rules: rule_items,
                tags: tag_items,
                accounts: account_plans,
            },
            settings,
            identities,
            templates,
            rules,
            tags,
        })
    }

    /// Apply the chosen parts. The new settings come back for the caller to save and show;
    /// rules and identities are saved here, each on its own, so one failing doesn't stop the
    /// others.
    pub fn apply(
        self,
        store: &mut MailStore,
        choice: ImportChoice,
    ) -> (Option<Settings>, ImportReport) {
        let mut report = ImportReport::default();
        let settings = (choice.settings && !self.plan.settings.is_empty()).then(|| {
            report.settings = self.plan.settings.len();
            self.settings
        });
        if choice.identities {
            for (status, identity) in &self.identities {
                match store.save_identity(identity) {
                    Ok(_) if *status == Status::Add => report.identities_added += 1,
                    Ok(_) => report.identities_updated += 1,
                    Err(e) => report.problems.push(format!("{}: {e}", identity.email)),
                }
            }
            for template in &self.templates {
                match store.save_folder_template(template) {
                    Ok(()) => report.templates += 1,
                    Err(e) => report.problems.push(format!("{}: {e}", template.folder)),
                }
            }
        }
        if choice.rules {
            // Tags first: the rules use them.
            for (status, tag) in &self.tags {
                match store.import_tag(tag) {
                    Ok(_) if *status == Status::Add => report.tags_added += 1,
                    Ok(_) => report.tags_updated += 1,
                    Err(e) => report.problems.push(format!("{}: {e}", tag.name)),
                }
            }
            for (status, rule) in &self.rules {
                match store.save_rule(rule) {
                    Ok(_) if *status == Status::Add => report.rules_added += 1,
                    Ok(_) => report.rules_updated += 1,
                    Err(e) => report.problems.push(format!("{}: {e}", rule.name)),
                }
            }
        }
        (settings, report)
    }
}

/// Tags as a settings file holds them, matched by keyword: new ones, and ones whose name or
/// color differ here.
fn plan_tags(
    values: &[Value],
    store: &MailStore,
) -> (Vec<PlanItem>, Vec<(Status, crate::tags::Tag)>) {
    let here = store.tags().unwrap_or_default();
    let mut items = Vec::new();
    let mut tags = Vec::new();
    for value in values {
        let entry = serde_json::from_value::<TagEntry>(value.clone())
            .ok()
            .filter(|e| crate::tags::is_keyword(&e.keyword.to_ascii_lowercase()));
        let Some(entry) = entry else {
            items.push(skipped(name_of(value), None, Skip::Unreadable));
            continue;
        };
        let keyword = entry.keyword.to_ascii_lowercase();
        let name = entry
            .name
            .clone()
            .or_else(|| crate::tags::default_name(&keyword))
            .unwrap_or_else(|| keyword.clone());
        let tag = crate::tags::Tag {
            keyword: keyword.clone(),
            name: name.clone(),
            color: entry.color.clone(),
            count: 0,
        };
        let status = match here.iter().find(|t| t.keyword == keyword) {
            None => Status::Add,
            Some(t) if t.name == tag.name && t.color == tag.color => Status::Same,
            Some(_) => Status::Update,
        };
        if !crate::tags::COLORS.contains(&tag.color.as_str()) {
            let detail = crate::tr!(
                "Pick one of the tag colors.",
                "Wybierz jeden z kolorów etykiet."
            );
            items.push(skipped(name, None, Skip::Invalid { detail }));
            continue;
        }
        items.push(PlanItem {
            name,
            account: None,
            status,
            skip: None,
        });
        if status != Status::Same {
            tags.push((status, tag));
        }
    }
    (items, tags)
}

/// Folders' templates as a settings file holds them, matched against this store: what happens
/// to each folder's, and the ones to save. The folder is found as a rule's move is.
fn plan_templates(values: &[Value], store: &MailStore) -> (Vec<PlanItem>, Vec<FolderTemplate>) {
    let accounts = store.accounts();
    let mut items = Vec::new();
    let mut found = Vec::new();
    for value in values {
        let Ok(entry) = serde_json::from_value::<TemplateEntry>(value.clone()) else {
            let folder = value.get("folder").and_then(Value::as_str).unwrap_or("?");
            let account = value
                .get("account")
                .and_then(Value::as_str)
                .map(str::to_owned);
            items.push(skipped(folder.to_owned(), account, Skip::Unreadable));
            continue;
        };
        let owner = if entry.this_pc {
            store.this_pc()
        } else {
            let email = entry.account.as_deref().unwrap_or_default();
            accounts
                .iter()
                .find(|a| a.imap.is_some() && a.email.eq_ignore_ascii_case(email))
        };
        let Some(owner) = owner else {
            let email = entry.account.clone().unwrap_or_default();
            items.push(skipped(
                entry.folder.clone(),
                entry.account.clone(),
                Skip::NoAccount { email },
            ));
            continue;
        };
        let folder = owner
            .folders
            .iter()
            .find(|f| !entry.folder_id.is_empty() && f.id == entry.folder_id)
            .or_else(|| {
                owner
                    .folders
                    .iter()
                    .find(|f| same_text(&f.name, &entry.folder))
            });
        let account = entry.account.clone();
        let Some(folder) = folder else {
            let why = Skip::NoFolder {
                folder: entry.folder.clone(),
            };
            items.push(skipped(entry.folder.clone(), account, why));
            continue;
        };
        let template = FolderTemplate {
            folder: owner.folder_key(&folder.id),
            new_template: entry.new_template,
            reply_template: entry.reply_template,
            reply_header: entry.reply_header,
        };
        let here = store.folder_template(&template.folder).ok().flatten();
        let status = match here {
            None => Status::Add,
            Some(t) if t == template => Status::Same,
            Some(_) => Status::Update,
        };
        items.push(PlanItem {
            name: folder.name.clone(),
            account,
            status,
            skip: None,
        });
        if status != Status::Same {
            found.push(template);
        }
    }
    (items, found)
}

/// Rules as a settings file holds them, matched against this store: what happens to each,
/// and the rules to save (new ones, and changed ones with their id here). Each finds its
/// account by address and the folders it moves to by id, then by name.
pub fn plan_rules(values: &[Value], store: &MailStore) -> (Vec<PlanItem>, Vec<(Status, Rule)>) {
    let accounts = store.accounts();
    let local = |email: &str| {
        accounts
            .iter()
            .find(|a| a.email.eq_ignore_ascii_case(email))
    };
    let existing_rules = store.rules().unwrap_or_default();
    let mut rules = Vec::new();
    let mut items = Vec::new();
    for value in values {
        let Ok(entry) = serde_json::from_value::<RuleEntry>(value.clone()) else {
            let account = value
                .get("account")
                .and_then(Value::as_str)
                .map(str::to_owned);
            items.push(skipped(name_of(value), account, Skip::Unreadable));
            continue;
        };
        let name = entry.name.trim().to_owned();
        let account = match &entry.account {
            None => None,
            Some(email) => match local(email) {
                Some(a) => Some(a),
                None => {
                    let why = Skip::NoAccount {
                        email: email.clone(),
                    };
                    items.push(skipped(name, Some(email.clone()), why));
                    continue;
                }
            },
        };
        let mut actions = Vec::new();
        let mut missing = None;
        for action in &entry.actions {
            actions.push(match action {
                ActionEntry::Move {
                    folder,
                    folder_id,
                    this_pc,
                }
                | ActionEntry::Copy {
                    folder,
                    folder_id,
                    this_pc,
                } => {
                    let owner = if *this_pc { store.this_pc() } else { account };
                    let found = owner.and_then(|a| {
                        a.folders
                            .iter()
                            .find(|f| !folder_id.is_empty() && f.id == *folder_id)
                            .or_else(|| a.folders.iter().find(|f| same_text(&f.name, folder)))
                            .map(|f| a.folder_key(&f.id))
                    });
                    match found {
                        Some(key) => {
                            if matches!(action, ActionEntry::Copy { .. }) {
                                Action::Copy { folder: key }
                            } else {
                                Action::Move { folder: key }
                            }
                        }
                        None => {
                            missing = Some(folder.clone());
                            break;
                        }
                    }
                }
                ActionEntry::Forward { to } => Action::Forward { to: to.clone() },
                ActionEntry::MarkRead => Action::MarkRead,
                ActionEntry::Flag => Action::Flag,
                ActionEntry::AddTag { tag } => Action::AddTag { tag: tag.clone() },
                ActionEntry::RemoveTag { tag } => Action::RemoveTag { tag: tag.clone() },
                ActionEntry::Delete => Action::Delete,
                ActionEntry::AutoReply { body } => Action::AutoReply { body: body.clone() },
            });
        }
        // The schedule's folders: of the account with that address (or on this PC), by id,
        // else by name.
        let schedule = entry.schedule.as_ref().map(|s| Schedule {
            every: s.every,
            folders: s
                .folders
                .iter()
                .filter_map(|f| {
                    let owner = match &f.account {
                        Some(email) => local(email),
                        None => store.this_pc(),
                    };
                    let found = owner.and_then(|a| {
                        a.folders
                            .iter()
                            .find(|x| !f.folder_id.is_empty() && x.id == f.folder_id)
                            .or_else(|| a.folders.iter().find(|x| same_text(&x.name, &f.folder)))
                            .map(|x| a.folder_key(&x.id))
                    });
                    if found.is_none() && missing.is_none() {
                        missing = Some(f.folder.clone());
                    }
                    found
                })
                .collect(),
        });
        let email = account.map(|a| a.email.clone());
        if let Some(folder) = missing {
            items.push(skipped(name, email, Skip::NoFolder { folder }));
            continue;
        }
        let mut rule = Rule {
            id: 0,
            name: name.clone(),
            enabled: entry.enabled,
            account: account.map(|a| a.id.clone()),
            match_all: entry.match_all,
            conditions: entry.conditions,
            actions,
            stop: entry.stop,
            on_arrival: entry.on_arrival,
            schedule,
            last_run: None,
        };
        if let Err(detail) = rule.check(accounts) {
            items.push(skipped(name, email, Skip::Invalid { detail }));
            continue;
        }
        let existing = existing_rules
            .iter()
            .find(|r| same_text(&r.name, &rule.name) && r.account == rule.account);
        let status = match existing {
            None => Status::Add,
            Some(e) => {
                rule.id = e.id;
                let unchanged = e.enabled == rule.enabled
                    && e.match_all == rule.match_all
                    && e.conditions == rule.conditions
                    && e.actions == rule.actions
                    && e.stop == rule.stop
                    && e.on_arrival == rule.on_arrival
                    && e.schedule == rule.schedule;
                if unchanged {
                    Status::Same
                } else {
                    Status::Update
                }
            }
        };
        items.push(PlanItem {
            name,
            account: email,
            status,
            skip: None,
        });
        if status != Status::Same {
            rules.push((status, rule));
        }
    }
    (items, rules)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filters::{Field, Op};
    use crate::model::{Folder, FolderKind, SmtpSecurity};
    use chrono::NaiveDate;

    fn now() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 10)
            .unwrap()
            .and_hms_opt(9, 30, 0)
            .unwrap()
    }

    fn locale(languages: &[&str], region: &str) -> SystemLocale {
        SystemLocale {
            languages: languages.iter().map(|l| (*l).to_owned()).collect(),
            region: (!region.is_empty()).then(|| region.to_owned()),
            writing: Vec::new(),
        }
    }

    /// A store with one server account at `email` and folders Inbox, Faktury, Trash.
    fn store_with(email: &str, faktury_id: &str) -> (MailStore, Account) {
        let mut store = MailStore::in_memory().unwrap();
        let account = store
            .upsert_account(&AccountSetup {
                name: "Firma".into(),
                email: email.into(),
                host: "mail.firma.pl".into(),
                port: 993,
                username: email.into(),
                imap_security: SmtpSecurity::Tls,
                smtp_host: "smtp.firma.pl".into(),
                smtp_port: 587,
                smtp_security: SmtpSecurity::StartTls,
            })
            .unwrap();
        let folder = |id: &str, name: &str, kind| Folder {
            id: id.into(),
            name: name.into(),
            kind,
            remote: Some(name.into()),
        };
        store
            .set_folders(
                &account.id,
                &[
                    folder("inbox", "Inbox", FolderKind::Inbox),
                    folder(faktury_id, "Faktury", FolderKind::Custom),
                    folder("trash", "Kosz", FolderKind::Trash),
                ],
            )
            .unwrap();
        let account = store.account(&account.id).unwrap().clone();
        (store, account)
    }

    fn invoices_rule(account: &Account, folder_id: &str) -> Rule {
        Rule {
            id: 0,
            name: "Faktury".into(),
            enabled: true,
            account: Some(account.id.clone()),
            match_all: true,
            conditions: vec![Condition {
                header: None,
                field: Field::Subject,
                op: Op::Contains,
                value: "faktura".into(),
            }],
            actions: vec![
                Action::Move {
                    folder: account.folder_key(folder_id),
                },
                Action::Flag,
            ],
            stop: true,
            ..Rule::default()
        }
    }

    const ALL: ExportChoice = ExportChoice {
        settings: true,
        rules: true,
        identities: true,
        servers: true,
    };
    const EVERYTHING: ImportChoice = ImportChoice {
        settings: true,
        identities: true,
        rules: true,
    };

    #[test]
    fn unknown_values_are_left_out_one_by_one() {
        let changes = serde_json::json!({
            "theme": "aurora",          // a theme from a newer Telex
            "language": "pl",
            "text_size": 133,           // not one of the sizes
            "density": "comfortable",
            "account_mark": "initials",
            "unread_style": "color",
            "font": "source",
            "colour": "blue",           // a key Telex doesn't have
        });
        let (s, ignored) = Settings::default().merged(&changes);
        assert_eq!(s.theme, "system");
        assert_eq!(s.language, "pl");
        assert_eq!(s.text_size, 100);
        assert_eq!(s.density, Density::Comfortable);
        assert_eq!(s.account_mark, AccountMark::Initials);
        assert_eq!(s.unread_style, UnreadStyle::Color);
        assert_eq!(s.font, Font::Source);
        let mut ignored = ignored;
        ignored.sort();
        assert_eq!(ignored, ["colour", "text_size", "theme"]);
    }

    #[test]
    fn a_file_from_0_3_keeps_its_values_and_gets_the_new_defaults() {
        let old = serde_json::json!({
            "theme": "fjord", "language": "pl", "density": "compact", "text_size": 110,
            "prefer_text": false, "mark_read": "moment", "check_every": 15, "updates": "daily",
        });
        let (s, ignored) = Settings::default().merged(&old);
        assert!(ignored.is_empty(), "{ignored:?}");
        assert_eq!((s.theme.as_str(), s.check_every), ("fjord", 15));
        assert_eq!(s.mail_paper, MailPaper::Theme);
        assert_eq!(s.account_mark, AccountMark::Rail);
        assert_eq!(s.unread_style, UnreadStyle::Quiet);
        assert!(!s.threaded);
        assert_eq!(s.font, Font::Plex);
        assert_eq!(s.toolbar_style, ToolbarStyle::Classic);
        // Since 0.9.8 new messages and replies start in HTML.
        assert_eq!(s.compose_format, ComposeFormat::Html);
        assert_eq!(s.reply_format, ReplyFormat::Html);
        assert!(s.ask_subject && s.spell_check);
        // Added after 0.9.9: on until the user turns them off.
        assert!(s.ask_attachment && s.ask_many);
        assert!(s.risk_checks && s.sign_replies);
        // 0.9.8 added the spam filter: careful until the user picks.
        assert_eq!(s.spam_filter, crate::spam::Level::Careful);
        let (strict, ignored) = s.merged(&serde_json::json!({"spam_filter": "strict"}));
        assert!(ignored.is_empty());
        assert_eq!(strict.spam_filter, crate::spam::Level::Strict);
        let (_, ignored) = s.merged(&serde_json::json!({"spam_filter": "eager"}));
        assert_eq!(ignored, ["spam_filter"]);

        let (s, ignored) = s.merged(&serde_json::json!({
            "compose_format": "plain", "reply_format": "same", "mail_paper": "white",
            "spell_check": false, "ask_subject": "maybe", "sign_replies": false,
        }));
        assert_eq!(ignored, ["ask_subject"]);
        assert_eq!(s.compose_format, ComposeFormat::Plain);
        assert_eq!(s.reply_format, ReplyFormat::Same);
        assert!(!s.sign_replies);
        assert_eq!(s.mail_paper, MailPaper::White);
        assert!(!s.spell_check && s.ask_subject);

        // 0.5 added these; an older file gets the defaults.
        assert!(s.push_mail && s.notify);
        assert_eq!(s.undo_send, 5);
        let (s, ignored) = s.merged(&serde_json::json!({
            "undo_send": 7, "push_mail": false, "notify": false,
        }));
        assert_eq!(ignored, ["undo_send"], "only the listed delays");
        assert_eq!(s.undo_send, 5);
        assert!(!s.push_mail && !s.notify);

        // 0.7 added how long "a moment" is, and marking read only in a window of its own.
        assert_eq!(s.read_delay, 600);
        let (s, ignored) = s.merged(&serde_json::json!({
            "mark_read": "window", "read_delay": 2000,
        }));
        assert!(ignored.is_empty(), "{ignored:?}");
        assert_eq!((s.mark_read, s.read_delay), (MarkRead::Window, 2000));
        let (s, ignored) = s.merged(&serde_json::json!({"read_delay": 1500, "mark_read": "never"}));
        assert_eq!(ignored.len(), 2, "only the listed delays and choices");
        assert_eq!((s.mark_read, s.read_delay), (MarkRead::Window, 2000));
        let (s, ignored) = s.merged(&serde_json::json!({"updates": "start"}));
        assert!(ignored.is_empty(), "{ignored:?}");
        assert_eq!(s.updates, Updates::Start);
    }

    #[test]
    fn toolbar_styles_survive_saving_and_report_import_changes() {
        let base = Settings::default();
        for (value, style) in [
            ("classic", ToolbarStyle::Classic),
            ("icons", ToolbarStyle::Icons),
            ("large", ToolbarStyle::Large),
            ("below", ToolbarStyle::Below),
            ("text", ToolbarStyle::Text),
        ] {
            let (next, ignored) = base.merged(&serde_json::json!({"toolbar_style": value}));
            assert!(ignored.is_empty());
            assert_eq!(next.toolbar_style, style);
            let saved = serde_json::to_string(&next).unwrap();
            assert_eq!(serde_json::from_str::<Settings>(&saved).unwrap(), next);
            let changes = base.changes_to(&next);
            if style == ToolbarStyle::Classic {
                assert!(changes.is_empty());
            } else {
                assert_eq!(changes.len(), 1);
                assert_eq!(changes[0].key, "toolbar_style");
                assert_eq!(changes[0].to, serde_json::json!(value));
            }
            let (kept, ignored) =
                next.merged(&serde_json::json!({"toolbar_style": "future_style"}));
            assert_eq!(ignored, ["toolbar_style"]);
            assert_eq!(kept.toolbar_style, style);
        }
        let mut older = serde_json::to_value(&base).unwrap();
        older.as_object_mut().unwrap().remove("toolbar_style");
        assert_eq!(
            serde_json::from_value::<Settings>(older)
                .unwrap()
                .toolbar_style,
            ToolbarStyle::Classic
        );
    }

    #[test]
    fn threading_is_optional_and_reports_import_changes() {
        let base = Settings::default();
        assert!(!base.threaded);
        let (next, ignored) = base.merged(&serde_json::json!({"threaded": true}));
        assert!(ignored.is_empty());
        assert!(next.threaded);
        let changes = base.changes_to(&next);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].key, "threaded");
        assert_eq!(changes[0].to, serde_json::json!(true));
        let (kept, ignored) = next.merged(&serde_json::json!({"threaded": "yes"}));
        assert_eq!(ignored, ["threaded"]);
        assert!(kept.threaded);
    }

    #[test]
    fn settings_file_round_trips_and_survives_damage() {
        let dir = std::env::temp_dir().join(format!("telex-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let _ = std::fs::remove_file(&path);

        let first = Settings::load(&path);
        assert!(first.fresh);
        assert_eq!(first.settings, Settings::default());

        let mine = Settings {
            theme: "paper".into(),
            threaded: true,
            check_every: 30,
            mark_read: MarkRead::Manual,
            ..Settings::default()
        };
        mine.save(&path).unwrap();
        let again = Settings::load(&path);
        assert!(!again.fresh);
        assert_eq!(again.settings, mine);

        // Saved again by Notepad, with a byte-order mark and a value Telex doesn't know.
        std::fs::write(&path, "\u{feff}{\"theme\":\"fjord\",\"density\":\"roomy\"}").unwrap();
        let edited = Settings::load(&path).settings;
        assert_eq!(edited.theme, "fjord");
        assert_eq!(edited.density, Density::Compact);

        std::fs::write(&path, "not json at all").unwrap();
        let damaged = Settings::load(&path);
        assert!(!damaged.fresh);
        assert_eq!(damaged.settings, Settings::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn automatic_language_follows_windows() {
        let auto = Settings::default();
        assert_eq!(
            auto.effective_language(&locale(&["pl-PL", "en-US"], "pl-PL")),
            "pl"
        );
        assert_eq!(
            auto.effective_language(&locale(&["en-GB", "pl-PL"], "pl-PL")),
            "en"
        );
        assert_eq!(auto.effective_language(&locale(&["de-DE"], "de-DE")), "en");
        assert_eq!(auto.effective_language(&locale(&[], "")), "en");

        // English Windows with Polish formats: English, but Polish is offered once.
        let english_windows = locale(&["en-US"], "pl-PL");
        assert_eq!(auto.effective_language(&english_windows), "en");
        assert_eq!(auto.language_to_offer(&english_windows), Some("pl"));
        assert_eq!(auto.language_to_offer(&locale(&["pl-PL"], "pl-PL")), None);
        assert_eq!(auto.language_to_offer(&locale(&["en-US"], "en-US")), None);
        assert_eq!(auto.language_to_offer(&locale(&["en-US"], "cs-CZ")), None);

        let chosen = Settings {
            language: "en".into(),
            ..Settings::default()
        };
        assert_eq!(
            chosen.effective_language(&locale(&["pl-PL"], "pl-PL")),
            "en"
        );
        assert_eq!(chosen.language_to_offer(&english_windows), None);
    }

    #[cfg(windows)]
    #[test]
    fn windows_names_its_languages() {
        let system = system_locale();
        println!("{system:?}");
        assert!(!system.languages.is_empty());
        assert!(system
            .languages
            .iter()
            .all(|l| l.len() >= 2 && !l.contains('\0')));
        assert!(system.region.is_some_and(|r| r.len() >= 2));
    }

    #[test]
    fn exported_file_holds_no_password_and_reads_back_as_unchanged() {
        let (mut store, account) = store_with("anna@firma.pl", "faktury");
        store
            .save_rule(&invoices_rule(&account, "faktury"))
            .unwrap();
        let mine = Settings {
            theme: "heather".into(),
            ..Settings::default()
        };
        let text = export(&store, &mine, ALL, "0.3.0", now()).unwrap();
        assert!(text.contains("\"format\": \"telex-settings\""));
        assert!(text.contains("\"exported\": \"2026-10-10T09:30:00\""));
        assert!(text.contains("\"folder\": \"Faktury\""));
        assert!(!text.to_lowercase().contains("password"));

        // Into the same Telex: nothing to do.
        let import = Import::read(&text, &store, &mine).unwrap();
        assert!(import.plan.has_settings && import.plan.settings.is_empty());
        assert_eq!(import.plan.rules.len(), 1);
        assert_eq!(import.plan.rules[0].status, Status::Same);
        assert_eq!(import.plan.identities[0].status, Status::Same);
        assert!(import.plan.accounts[0].present);
        let (settings, report) = import.apply(&mut store, EVERYTHING);
        assert_eq!(settings, None);
        assert_eq!(report, ImportReport::default());
    }

    #[test]
    fn a_yes_to_counting_stays_on_its_pc() {
        let (store, _) = store_with("anna@firma.pl", "faktury");
        let mine = Settings {
            usage_count: UsageCount::On,
            backup_dir: "D:\\Kopie".into(),
            ..Settings::default()
        };
        let text = export(&store, &mine, ALL, "0.9.10", now()).unwrap();
        assert!(text.contains("\"usage_count\": \"ask\""));
        assert!(!text.contains("Kopie"));
        // Another PC that said no keeps its no.
        let theirs = Settings {
            usage_count: UsageCount::Off,
            ..Settings::default()
        };
        let import = Import::read(&text, &store, &theirs).unwrap();
        assert!(import.plan.settings.is_empty());
        // Settings older than the question ask it.
        let old = Settings::default()
            .merged(&serde_json::json!({"theme": "dark"}))
            .0;
        assert_eq!(old.usage_count, UsageCount::Ask);
    }

    #[test]
    fn header_attachment_copy_and_forward_rules_travel_to_another_pc() {
        let (mut old, account) = store_with("anna@firma.pl", "faktury");
        let mut rule = invoices_rule(&account, "faktury");
        rule.conditions = vec![
            Condition {
                field: Field::Header,
                header: Some("List-Id".into()),
                op: Op::Exists,
                value: String::new(),
            },
            Condition {
                field: Field::AttachmentType,
                header: None,
                op: Op::Is,
                value: "application/pdf".into(),
            },
        ];
        rule.actions = vec![
            Action::Copy {
                folder: account.folder_key("faktury"),
            },
            Action::Forward {
                to: "archive@example.com".into(),
            },
        ];
        old.save_rule(&rule).unwrap();
        let text = export(&old, &Settings::default(), ALL, "0.9.5", now()).unwrap();
        let (mut new, here) = store_with("Anna@Firma.pl", "invoices-here");
        let import = Import::read(&text, &new, &Settings::default()).unwrap();
        assert_eq!(import.plan.rules[0].status, Status::Add);
        let (_, report) = import.apply(&mut new, EVERYTHING);
        assert!(report.problems.is_empty(), "{:?}", report.problems);
        let rules = new.rules().unwrap();
        assert_eq!(rules[0].conditions, rule.conditions);
        assert_eq!(
            rules[0].actions,
            [
                Action::Copy {
                    folder: here.folder_key("invoices-here")
                },
                Action::Forward {
                    to: "archive@example.com".into()
                }
            ]
        );
        let again = Import::read(&text, &new, &Settings::default()).unwrap();
        assert_eq!(again.plan.rules[0].status, Status::Same);
    }

    #[test]
    fn scheduled_rules_with_patterns_and_ages_travel_to_another_pc() {
        let (mut old, account) = store_with("anna@firma.pl", "faktury");
        let mut rule = invoices_rule(&account, "faktury");
        let cond = |field, op, value: &str| Condition {
            field,
            op,
            value: value.into(),
            header: None,
        };
        rule.conditions = vec![
            cond(Field::Subject, Op::Matches, r"^fv/\d+"),
            cond(Field::Received, Op::OlderThan, "30"),
            cond(Field::State, Op::Is, "read"),
            cond(Field::Size, Op::LargerThan, "5m"),
        ];
        rule.on_arrival = false;
        rule.schedule = Some(Schedule {
            every: Every::Week,
            folders: vec![account.folder_key("inbox"), account.folder_key("faktury")],
        });
        let saved = old.save_rule(&rule).unwrap();
        old.set_rule_last_run(saved.id, now()).unwrap();
        let text = export(&old, &Settings::default(), ALL, "0.9.12", now()).unwrap();
        assert!(text.contains("\"on_arrival\": false"), "{text}");
        let (mut new, here) = store_with("anna@firma.pl", "invoices-here");
        let import = Import::read(&text, &new, &Settings::default()).unwrap();
        assert_eq!(import.plan.rules[0].status, Status::Add);
        let (_, report) = import.apply(&mut new, EVERYTHING);
        assert!(report.problems.is_empty(), "{:?}", report.problems);
        let rules = new.rules().unwrap();
        assert_eq!(rules[0].conditions, rule.conditions);
        assert!(!rules[0].on_arrival);
        assert_eq!(
            rules[0].schedule,
            Some(Schedule {
                every: Every::Week,
                folders: vec![here.folder_key("inbox"), here.folder_key("invoices-here")],
            }),
            "folders found by id, else by name"
        );
        assert_eq!(rules[0].last_run, None, "this PC's own");
        let again = Import::read(&text, &new, &Settings::default()).unwrap();
        assert_eq!(again.plan.rules[0].status, Status::Same);

        // A scheduled folder that isn't here: skipped, saying which.
        let text = text.replace("\"folder\": \"Inbox\"", "\"folder\": \"Nowhere\"");
        let text = text.replace("\"folder_id\": \"inbox\"", "\"folder_id\": \"nowhere\"");
        let import = Import::read(&text, &new, &Settings::default()).unwrap();
        assert_eq!(
            import.plan.rules[0].skip,
            Some(Skip::NoFolder {
                folder: "Nowhere".into()
            })
        );
    }

    #[test]
    fn tags_travel_with_the_rules_that_use_them() {
        let (mut old, account) = store_with("anna@firma.pl", "faktury");
        let projekt = old
            .save_tag(&crate::tags::Tag {
                keyword: String::new(),
                name: "Projekt X".into(),
                color: "teal".into(),
                count: 0,
            })
            .unwrap();
        let mut work = old.tags().unwrap()[1].clone();
        work.color = "gray".into();
        old.save_tag(&work).unwrap();
        let mut rule = invoices_rule(&account, "faktury");
        rule.conditions = vec![Condition {
            field: Field::Tag,
            header: None,
            op: Op::Is,
            value: "$label1".into(),
        }];
        rule.actions = vec![
            Action::AddTag {
                tag: projekt.keyword.clone(),
            },
            Action::RemoveTag {
                tag: "$label1".into(),
            },
        ];
        old.save_rule(&rule).unwrap();
        let text = export(&old, &Settings::default(), ALL, "0.9.12", now()).unwrap();
        assert!(text.contains("\"keyword\": \"projekt_x\""), "{text}");
        assert!(
            !text.contains("\"name\": \"Work\""),
            "a default tag's own name isn't written"
        );
        let (mut new, _) = store_with("anna@firma.pl", "faktury");
        let import = Import::read(&text, &new, &Settings::default()).unwrap();
        let status = |name: &str| {
            import
                .plan
                .tags
                .iter()
                .find(|t| t.name == name)
                .map(|t| t.status)
        };
        assert_eq!(status("Projekt X"), Some(Status::Add));
        assert_eq!(status("Work"), Some(Status::Update), "its color");
        assert_eq!(status("Later"), Some(Status::Same));
        let (_, report) = import.apply(&mut new, EVERYTHING);
        assert!(report.problems.is_empty(), "{:?}", report.problems);
        assert_eq!((report.tags_added, report.tags_updated), (1, 1));
        let tags = new.tags().unwrap();
        assert!(tags
            .iter()
            .any(|t| t.keyword == "projekt_x" && t.name == "Projekt X" && t.color == "teal"));
        assert_eq!(tags[1].color, "gray");
        assert_eq!(new.rules().unwrap()[0].actions, rule.actions);
        let again = Import::read(&text, &new, &Settings::default()).unwrap();
        assert!(again.plan.tags.iter().all(|t| t.status == Status::Same));
    }

    #[test]
    fn rules_and_identities_find_their_account_and_folder_on_another_pc() {
        let (mut old, account) = store_with("anna@firma.pl", "faktury");
        old.save_rule(&invoices_rule(&account, "faktury")).unwrap();
        let mut sig = account.identity(None);
        sig.signature = "Pozdrawiam serdecznie\nAnna Łęcka".into();
        old.save_identity(&sig).unwrap();
        let mut alias = Identity::new(&account.id, "Biuro Firma", "biuro@firma.pl");
        alias.is_default = false;
        old.save_identity(&alias).unwrap();
        let text = export(&old, &Settings::default(), ALL, "0.3.0", now()).unwrap();

        // The new PC: same address, but the folder got another id there.
        let (mut new, here) = store_with("Anna@Firma.pl", "inbox-faktury");
        let import = Import::read(&text, &new, &Settings::default()).unwrap();
        let statuses: Vec<_> = import.plan.identities.iter().map(|i| i.status).collect();
        assert_eq!(statuses, [Status::Update, Status::Add]);
        assert_eq!(import.plan.rules[0].status, Status::Add);
        let (_, report) = import.apply(&mut new, EVERYTHING);
        assert_eq!(
            (
                report.identities_updated,
                report.identities_added,
                report.rules_added
            ),
            (1, 1, 1)
        );
        assert!(report.problems.is_empty());

        let rules = new.rules().unwrap();
        assert_eq!(rules[0].account.as_deref(), Some(here.id.as_str()));
        assert_eq!(
            rules[0].actions[0],
            Action::Move {
                folder: here.folder_key("inbox-faktury")
            }
        );
        let here = new.account(&here.id).unwrap();
        assert_eq!(here.identities.len(), 2);
        assert_eq!(
            here.identity(None).signature,
            "Pozdrawiam serdecznie\nAnna Łęcka"
        );

        // Importing the same file again changes nothing.
        let again = Import::read(&text, &new, &Settings::default()).unwrap();
        assert!(again.plan.rules.iter().all(|r| r.status == Status::Same));
        assert!(again
            .plan
            .identities
            .iter()
            .all(|i| i.status == Status::Same));
    }

    #[test]
    fn html_signatures_and_folder_templates_travel_to_another_pc() {
        let (mut old, account) = store_with("anna@firma.pl", "faktury");
        let mut sig = account.identity(None);
        sig.signature_html = Some(
            "<div><b>Anna Łęcka</b></div><img src=\"data:image/png;base64,iVBORw0KGgo=\">".into(),
        );
        sig.new_template = "Dzień dobry,".into();
        sig.reply_template = "Dzień dobry {first},".into();
        old.save_identity(&sig).unwrap();
        old.save_folder_template(&FolderTemplate {
            folder: account.folder_key("faktury"),
            reply_header: "On {date}, {name} wrote:".into(),
            ..FolderTemplate::default()
        })
        .unwrap();
        let text = export(&old, &Settings::default(), ALL, "0.9.12", now()).unwrap();
        assert!(
            text.contains("\"signature_html\"") && text.contains("\"templates\""),
            "{text}"
        );

        // The folder has another id there: found by its name, as a rule's is.
        let (mut new, here) = store_with("anna@firma.pl", "inbox-faktury");
        let import = Import::read(&text, &new, &Settings::default()).unwrap();
        assert_eq!(import.plan.templates.len(), 1);
        assert_eq!(
            (
                import.plan.templates[0].name.as_str(),
                import.plan.templates[0].status
            ),
            ("Faktury", Status::Add)
        );
        let (_, report) = import.apply(&mut new, EVERYTHING);
        assert_eq!((report.templates, report.identities_updated), (1, 1));
        let me = new.account(&here.id).unwrap().identity(None);
        assert_eq!(me.signature, "Anna Łęcka");
        assert!(me.signature_html.unwrap().contains("data:image/png"));
        assert_eq!(
            (me.new_template.as_str(), me.reply_template.as_str()),
            ("Dzień dobry,", "Dzień dobry {first},")
        );
        let t = new
            .folder_template(&here.folder_key("inbox-faktury"))
            .unwrap()
            .unwrap();
        assert_eq!(t.reply_header, "On {date}, {name} wrote:");

        let again = Import::read(&text, &new, &Settings::default()).unwrap();
        assert!(again
            .plan
            .templates
            .iter()
            .all(|t| t.status == Status::Same));
        assert!(again
            .plan
            .identities
            .iter()
            .all(|i| i.status == Status::Same));
        // A folder that isn't there says so.
        let (other, _) = store_with("anna@firma.pl", "x");
        let mut gone = Import::read(
            &text.replace("\"Faktury\"", "\"Rachunki\""),
            &other,
            &Settings::default(),
        )
        .unwrap();
        let item = gone.plan.templates.remove(0);
        assert_eq!(
            item.skip,
            Some(Skip::NoFolder {
                folder: "Rachunki".into()
            })
        );
    }

    #[test]
    fn rules_that_file_mail_on_this_pc_find_that_folder_on_another_pc() {
        let (mut old, account) = store_with("anna@firma.pl", "faktury");
        let projects = old.create_local_folder("Projekty").unwrap();
        let mut rule = invoices_rule(&account, "faktury");
        rule.account = None;
        rule.actions = vec![Action::Move { folder: projects }];
        old.save_rule(&rule).unwrap();
        let text = export(&old, &Settings::default(), ALL, "0.6.0", now()).unwrap();
        assert!(text.contains("\"this_pc\": true"), "{text}");
        assert!(text.contains("\"folder\": \"Projekty\""));

        // Another PC whose folder on this PC got another id, but has the name.
        let (mut new, _) = store_with("anna@firma.pl", "faktury");
        new.create_local_folder("Projekty: stare").unwrap(); // takes no slug of "Projekty"
        let there = new.create_local_folder("PROJEKTY").unwrap();
        new.rename_local_folder(&there, "Projekty").unwrap();
        let import = Import::read(&text, &new, &Settings::default()).unwrap();
        assert_eq!(import.plan.rules[0].status, Status::Add);
        import.apply(&mut new, EVERYTHING);
        assert_eq!(
            new.rules().unwrap()[0].actions,
            [Action::Move { folder: there }]
        );

        // One without that folder says so.
        let (bare, _) = store_with("anna@firma.pl", "faktury");
        let import = Import::read(&text, &bare, &Settings::default()).unwrap();
        assert_eq!(
            import.plan.rules[0].skip,
            Some(Skip::NoFolder {
                folder: "Projekty".into()
            })
        );
    }

    #[test]
    fn what_cant_come_along_says_why() {
        let (mut old, account) = store_with("anna@firma.pl", "faktury");
        old.save_rule(&invoices_rule(&account, "faktury")).unwrap();
        let text = export(&old, &Settings::default(), ALL, "0.3.0", now()).unwrap();

        // A Telex where the account isn't set up yet.
        let empty = MailStore::in_memory().unwrap();
        let import = Import::read(&text, &empty, &Settings::default()).unwrap();
        let no_account = Some(Skip::NoAccount {
            email: "anna@firma.pl".into(),
        });
        assert_eq!(import.plan.rules[0].skip, no_account);
        assert_eq!(import.plan.identities[0].skip, no_account);
        let setup = import.plan.accounts[0].setup.clone().unwrap();
        assert!(!import.plan.accounts[0].present);
        assert_eq!((setup.host.as_str(), setup.port), ("mail.firma.pl", 993));
        assert_eq!(
            (setup.smtp_host.as_str(), setup.smtp_security),
            ("smtp.firma.pl", SmtpSecurity::StartTls)
        );

        // A Telex where the account has no folder called Faktury.
        let mut bare = MailStore::in_memory().unwrap();
        let a = bare
            .upsert_account(&AccountSetup {
                email: "anna@firma.pl".into(),
                host: "mail.firma.pl".into(),
                port: 993,
                ..Default::default()
            })
            .unwrap();
        bare.set_folders(
            &a.id,
            &[Folder {
                id: "inbox".into(),
                name: "Inbox".into(),
                kind: FolderKind::Inbox,
                remote: Some("INBOX".into()),
            }],
        )
        .unwrap();
        let import = Import::read(&text, &bare, &Settings::default()).unwrap();
        assert_eq!(
            import.plan.rules[0].skip,
            Some(Skip::NoFolder {
                folder: "Faktury".into()
            })
        );
    }

    #[test]
    fn a_newer_files_unknown_parts_are_skipped_not_fatal() {
        let (store, _) = store_with("anna@firma.pl", "faktury");
        let text = r#"{
            "format": "telex-settings", "version": 7, "app": "1.4.0",
            "settings": {"theme": "aurora", "density": "comfortable"},
            "rules": [
                {"name": "Robots", "conditions": [{"field": "future_field", "op": "is", "value": "x"}],
                 "actions": [{"type": "flag"}]},
                {"name": "Klient", "conditions": [{"field": "from", "op": "contains", "value": "klient.pl"}],
                 "actions": [{"type": "future_action", "to": "x@y.pl"}]},
                {"name": "Wszędzie", "conditions": [{"field": "subject", "op": "contains", "value": "pilne"}],
                 "actions": [{"type": "flag"}]}
            ]
        }"#;
        let import = Import::read(text, &store, &Settings::default()).unwrap();
        assert!(import.plan.newer);
        assert_eq!(import.plan.settings_ignored, ["theme"]);
        assert_eq!(import.plan.settings.len(), 1);
        let skips: Vec<_> = import.plan.rules.iter().map(|r| r.skip.clone()).collect();
        assert_eq!(
            skips,
            [Some(Skip::Unreadable), Some(Skip::Unreadable), None]
        );
        assert_eq!(import.plan.rules[2].status, Status::Add);
    }

    #[test]
    fn files_that_arent_settings_are_refused() {
        let store = MailStore::in_memory().unwrap();
        let s = Settings::default();
        assert!(matches!(
            Import::read("<html>", &store, &s),
            Err(ImportError::NotJson(_))
        ));
        assert_eq!(
            Import::read(r#"{"accounts": []}"#, &store, &s).unwrap_err(),
            ImportError::NotTelex
        );
        assert_eq!(
            Import::read("[1, 2]", &store, &s).unwrap_err(),
            ImportError::NotTelex
        );
    }

    #[test]
    fn only_the_ticked_parts_are_applied() {
        let (mut old, account) = store_with("anna@firma.pl", "faktury");
        old.save_rule(&invoices_rule(&account, "faktury")).unwrap();
        let mine = Settings {
            theme: "phosphor".into(),
            language: "pl".into(),
            ..Settings::default()
        };
        let text = export(&old, &mine, ALL, "0.3.0", now()).unwrap();

        let (mut new, _) = store_with("anna@firma.pl", "faktury");
        let import = Import::read(&text, &new, &Settings::default()).unwrap();
        let keys: Vec<_> = import
            .plan
            .settings
            .iter()
            .map(|c| c.key.as_str())
            .collect();
        assert_eq!(keys, ["theme", "language"]);
        let only_settings = ImportChoice {
            settings: true,
            ..ImportChoice::default()
        };
        let (settings, report) = import.apply(&mut new, only_settings);
        assert_eq!(settings.unwrap().theme, "phosphor");
        assert_eq!(report.settings, 2);
        assert!(new.rules().unwrap().is_empty());
    }
}
