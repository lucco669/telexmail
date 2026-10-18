//! Tauri shell: exposes `telex_core::MailStore` to the UI as commands.
//! Keep logic out of here; put it in `crates/telex-core` where it can be tested.

mod away;
mod backup;
mod contacts;
mod count;
mod export;
mod files;
mod folders;
mod keys;
mod migrate;
mod native;
mod secrets;
mod settings;
mod spell;
mod tray;
mod updates;
mod watch;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use chrono::{Local, NaiveDateTime, TimeDelta};
use futures_util::future::join_all;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_opener::OpenerExt;
use telex_core::autoconfig::{self, Detected, Step};
use telex_core::calendar::{invites::Invite, Answer};
use telex_core::drafts::{self, DraftKind, DraftStart};
use telex_core::filters::{self, Rule, RunReport, Trigger};
use telex_core::html::{self, FetchLimits, Palette, Paper};
use telex_core::imap::{self, Scope, SyncProgress};
use telex_core::later::{self, Later};
use telex_core::openpgp::Choice;
use telex_core::recipients::Recipient;
use telex_core::servers;
use telex_core::smtp::{self, Originals, SendError};
use telex_core::tr;
use telex_core::unsubscribe::Method;
use telex_core::usage::Usage;
use telex_core::{
    Account, AccountSetup, DeleteReport, FolderKind, FolderTemplate, FolderTree, Identity,
    ListView, MailStore, MessageId, OutgoingDraft, Page, PageAt, SentCopy, StoreError, UnreadAt,
};

use crate::files::FilesState;
use crate::settings::SettingsState;

struct AppState {
    store: Mutex<MailStore>,
    /// The user's mail base: %APPDATA%\app.telexmail.desktop\telex.db.
    db_path: PathBuf,
    /// Telex is talking to the IMAP servers (a check or a push). Another one waits its
    /// turn instead of sending the same changes twice.
    busy: AtomicBool,
    /// An Outbox is being sent. A second sender would send the same mail twice, so senders
    /// take turns; restoring a backup and installing an update only go when none is running.
    sending: tokio::sync::Mutex<()>,
    /// A backup is being written, or the mail base restored or compacted.
    backing_up: AtomicBool,
    /// The backup last looked at in Settings › Backups: the only one Restore takes.
    restorable: Mutex<Option<PathBuf>>,
}

/// Errors cross the IPC bridge as plain, user-readable strings.
type CmdResult<T> = Result<T, String>;

/// A task that stopped (a thread that panicked, a lock held by one), in a sentence.
pub(crate) fn failed(e: impl std::fmt::Display) -> String {
    tr!("It didn't work: {e}", "Nie udało się: {e}")
}

fn with_store<T>(
    state: &AppState,
    f: impl FnOnce(&mut MailStore) -> Result<T, StoreError>,
) -> CmdResult<T> {
    let mut store = state.store.lock().map_err(|_| {
        tr!(
            "The mail store stopped responding. Restart Telex.",
            "Baza poczty przestała odpowiadać. Uruchom Telexa ponownie."
        )
    })?;
    f(&mut store).map_err(|e| e.to_string())
}

/// The user's mail base, or an empty one in memory until the first account creates the file.
/// A file that is there but doesn't open is left as it is, and the sentence says why.
fn open_store(db_path: &Path) -> (MailStore, Option<String>) {
    let mut problem = None;
    if db_path.exists() {
        match MailStore::open(db_path) {
            Ok(store) => return (store, None),
            Err(e) => {
                problem = Some(tr!(
                    "Telex couldn't open the mail base {path}: {e}. It is left as it is; Telex starts without it.",
                    "Telex nie mógł otworzyć bazy poczty {path}: {e}. Zostaje nietknięta; Telex startuje bez niej.",
                    path = db_path.display()
                ))
            }
        }
    }
    let empty = MailStore::in_memory().expect("an in-memory database always opens");
    (empty, problem)
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Accounts and their folders, with how many messages and unread ones each folder holds.
#[tauri::command(async)]
fn folder_tree(app: AppHandle, state: State<'_, AppState>) -> CmdResult<FolderTree> {
    let tree = with_store(&state, |s| s.folder_tree())?;
    // The window reads the tree after every change: the tray's count follows it.
    tray::refresh(&app, &tree);
    Ok(tree)
}

/// One page of a message list (a folder, or all mail) in the chosen order, with the
/// quick filter. The window asks for the rows it is about to show.
#[tauri::command(async)]
fn list_page(
    state: State<'_, AppState>,
    view: ListView,
    at: PageAt,
    limit: usize,
) -> CmdResult<Page> {
    with_store(&state, |s| s.page(&view, at, limit))
}

#[tauri::command(async)]
fn get_message(
    state: State<'_, AppState>,
    keys: State<'_, keys::KeysState>,
    id: MessageId,
) -> CmdResult<telex_core::risk::InspectedMessage> {
    let inspected = with_store(&state, |s| s.inspect_message(id))?;
    // Encrypted to a key whose passphrase the user asked Telex to remember: try it (the
    // password store is asked outside the store's lock), then open again.
    let locked: Vec<(String, bool)> = inspected
        .pgp
        .iter()
        .flat_map(|v| v.locked.iter().map(|k| (k.fingerprint.clone(), v.smime)))
        .collect();
    let mut opened = false;
    for (fingerprint, smime) in &locked {
        opened |= if *smime {
            keys::unlock_remembered_cert(&state, &keys, fingerprint)
        } else {
            keys::unlock_remembered(&state, &keys, fingerprint)
        };
    }
    if opened {
        return with_store(&state, |s| s.inspect_message(id));
    }
    Ok(inspected)
}

#[tauri::command(async)]
fn set_unread(state: State<'_, AppState>, id: MessageId, unread: bool) -> CmdResult<()> {
    with_store(&state, |s| s.set_unread(id, unread))
}

#[tauri::command(async)]
fn set_flagged(state: State<'_, AppState>, id: MessageId, flagged: bool) -> CmdResult<()> {
    with_store(&state, |s| s.set_flagged(id, flagged))
}

/// Mark several messages read or unread. Returns how many changed.
#[tauri::command(async)]
fn set_unread_many(
    state: State<'_, AppState>,
    ids: Vec<MessageId>,
    unread: bool,
) -> CmdResult<usize> {
    with_store(&state, |s| s.set_unread_many(&ids, unread))
}

/// Flag or unflag several messages. Returns how many changed.
#[tauri::command(async)]
fn set_flagged_many(
    state: State<'_, AppState>,
    ids: Vec<MessageId>,
    flagged: bool,
) -> CmdResult<usize> {
    with_store(&state, |s| s.set_flagged_many(&ids, flagged))
}

/// Add a tag to messages (`on`) or take it off. Returns how many changed.
#[tauri::command(async)]
fn set_tag_many(
    state: State<'_, AppState>,
    ids: Vec<MessageId>,
    keyword: String,
    on: bool,
) -> CmdResult<usize> {
    with_store(&state, |s| s.set_tag_many(&ids, &keyword, on))
}

/// Save a tag (empty keyword: a new one). Returns it as saved.
#[tauri::command(async)]
fn save_tag(
    state: State<'_, AppState>,
    tag: telex_core::tags::Tag,
) -> CmdResult<telex_core::tags::Tag> {
    with_store(&state, |s| s.save_tag(&tag))
}

/// Delete a tag; it comes off every message. Returns how many lost it.
#[tauri::command(async)]
fn delete_tag(state: State<'_, AppState>, keyword: String) -> CmdResult<usize> {
    with_store(&state, |s| s.delete_tag(&keyword))
}

/// Move a tag up (-1) or down (1).
#[tauri::command(async)]
fn move_tag(state: State<'_, AppState>, keyword: String, delta: i64) -> CmdResult<()> {
    with_store(&state, |s| s.move_tag(&keyword, delta))
}

/// Messages as Markdown, oldest first, for the clipboard (Copy as Markdown).
#[tauri::command(async)]
fn message_markdown(state: State<'_, AppState>, ids: Vec<MessageId>) -> CmdResult<String> {
    with_store(&state, |s| s.markdown(&ids))
}

/// Move several messages to a folder of their account, all or none. Returns where each
/// one was, for undo.
#[tauri::command(async)]
fn move_messages(
    state: State<'_, AppState>,
    ids: Vec<MessageId>,
    folder: String,
) -> CmdResult<Vec<(MessageId, String)>> {
    with_store(&state, |s| s.move_many(&ids, &folder))
}

/// The menu's Spam (`spam` true) and Not spam: to each account's Spam folder or back to its
/// Inbox, and the spam filter learns from it. Returns where each one was, for undo.
#[tauri::command(async)]
fn mark_spam(
    state: State<'_, AppState>,
    ids: Vec<MessageId>,
    spam: bool,
) -> CmdResult<Vec<(MessageId, String)>> {
    with_store(&state, |s| s.mark_spam(&ids, spam))
}

/// What the spam filter has learned, for Settings › Mail.
#[tauri::command(async)]
fn spam_status(state: State<'_, AppState>) -> CmdResult<telex_core::spam::Status> {
    with_store(&state, |s| s.spam_status())
}

/// Delete several messages: to Trash, or for good from Trash.
#[tauri::command(async)]
fn delete_messages(state: State<'_, AppState>, ids: Vec<MessageId>) -> CmdResult<DeleteReport> {
    with_store(&state, |s| s.delete_many(&ids))
}

/// Mark every message in a folder read. Returns how many.
#[tauri::command(async)]
fn mark_folder_read(state: State<'_, AppState>, folder: String) -> CmdResult<usize> {
    with_store(&state, |s| s.mark_folder_read(&folder))
}

/// Use a server folder for sent mail (drafts, archive, spam, trash) from now on.
#[tauri::command(async)]
fn choose_folder(state: State<'_, AppState>, folder: String, kind: FolderKind) -> CmdResult<()> {
    with_store(&state, |s| s.choose_folder(&folder, kind))
}

/// Who files sent mail in the account's Sent folder: automatic, the server, or Telex.
#[tauri::command(async)]
fn set_sent_copy(state: State<'_, AppState>, account: String, mode: SentCopy) -> CmdResult<()> {
    with_store(&state, |s| s.set_sent_copy(&account, mode))
}

/// The messages of a list between two (Shift selection), or all of it (Ctrl+A).
#[tauri::command(async)]
fn list_ids(
    state: State<'_, AppState>,
    view: ListView,
    from: Option<MessageId>,
    to: Option<MessageId>,
) -> CmdResult<Vec<MessageId>> {
    with_store(&state, |s| s.ids_between(&view, from, to))
}

/// The next unread message of a list after one (before it with `back`): Space.
#[tauri::command(async)]
fn next_unread(
    state: State<'_, AppState>,
    view: ListView,
    after: Option<MessageId>,
    back: bool,
) -> CmdResult<Option<UnreadAt>> {
    with_store(&state, |s| s.next_unread(&view, after, back))
}

/// The people the user mails with, most likely first, for suggestions in To, Cc and Bcc.
#[tauri::command(async)]
fn recipients(state: State<'_, AppState>) -> CmdResult<Vec<Recipient>> {
    with_store(&state, |s| s.recipients())
}

/// The account a draft is from, and the raw messages it replies to and forwards, and the
/// draft it was edited from (whose Message-ID and thread it keeps).
fn draft_context(s: &MailStore, draft: &OutgoingDraft) -> Result<(Account, Originals), StoreError> {
    let account = s.account(&draft.account)?.clone();
    let raw = |id: Option<MessageId>| id.map(|id| s.readable_raw(id)).transpose();
    let originals = Originals {
        replied: raw(draft.reply_to)?.flatten(),
        forwarded: raw(draft.forward_of)?.flatten(),
        edited: raw(draft.replaces.or(draft.autosaved))?.flatten(),
    };
    Ok((account, originals))
}

/// Save the compose pane's content as a draft, kept as a whole message so its HTML and
/// forwarded files stay with it. Sending goes through `send_message`. Async, so reading
/// big attachments for an autosave doesn't hold up the window.
#[tauri::command]
async fn save_outgoing(
    state: State<'_, AppState>,
    files: State<'_, FilesState>,
    keys: State<'_, keys::KeysState>,
    draft: OutgoingDraft,
    pgp: Option<Choice>,
) -> CmdResult<MessageId> {
    files.check(&draft.attachments)?;
    let (account, originals) = with_store(&state, |s| draft_context(s, &draft))?;
    // With Encrypt on, the draft is kept encrypted to the sender's own key.
    let choice = pgp.unwrap_or_default();
    let protect = if choice.encrypt {
        Some(keys::protection_for(
            &state, &keys, &account, &draft, choice, false, true,
        )?)
    } else {
        None
    };
    // The attached files are read with the store free: the window goes on meanwhile.
    let composing = draft.clone();
    let raw = tauri::async_runtime::spawn_blocking(move || match &protect {
        Some(p) => smtp::compose_draft_protected(&account, &composing, &originals, Local::now(), p)
            .map(Some),
        None => Ok(smtp::compose_draft(&account, &composing, &originals, Local::now()).ok()),
    })
    .await
    .map_err(failed)??;
    with_store(&state, |s| {
        s.save_outgoing(
            draft,
            raw.as_deref(),
            FolderKind::Drafts,
            Local::now().naive_local(),
        )
    })
}

/// Drop an autosaved draft for good (compose's Discard). Only drafts on this PC; true when
/// one was removed.
#[tauri::command(async)]
fn discard_draft(state: State<'_, AppState>, id: MessageId) -> CmdResult<bool> {
    with_store(&state, |s| s.discard_draft(id))
}

/// The HTML of a draft or an unsent message, cleaned for the editor, or `null` when it
/// was written as plain text.
#[tauri::command(async)]
fn draft_html(state: State<'_, AppState>, id: MessageId) -> CmdResult<Option<String>> {
    // An encrypted draft reads from what was decrypted when it was opened.
    let raw = with_store(&state, |s| s.readable_raw(id))?;
    Ok(raw.as_deref().and_then(html::editable))
}

/// Compose switches from HTML to plain text: the text the plain version would have.
#[tauri::command(async)]
fn html_to_text(html: String) -> String {
    html::to_text(&html::compose_clean(&html))
}

/// Whether the text being sent says a file is attached (Send asks when none is). The
/// signature is left out of it.
#[tauri::command(async)]
fn mentions_attachment(text: String, signature: String) -> bool {
    drafts::mentions_attachment(&text, &signature)
}

/// Compose switches from plain text to HTML, or a reply opens in the HTML editor.
#[tauri::command(async)]
fn text_to_html(text: String) -> String {
    html::from_text(&text)
}

/// HTML pasted into the editor, cleaned to what Telex sends: no scripts, no styles but the
/// palette's, pictures only as data: (remote ones are left out, never fetched).
#[tauri::command(async)]
fn clean_html(html: String) -> String {
    html::compose_clean(&html)
}

/// What the mail takes and how it moves, for the Usage page and the status-bar meter.
#[tauri::command(async)]
fn usage(state: State<'_, AppState>) -> CmdResult<Usage> {
    with_store(&state, |s| s.usage(Local::now().date_naive()))
}

#[derive(Debug, Serialize)]
struct Connections {
    servers: Vec<servers::ServerState>,
    log: Vec<servers::Attempt>,
}

/// Settings › Connections: every server reached since Telex started, and the last connections,
/// newest first.
#[tauri::command(async)]
fn connections() -> Connections {
    Connections {
        servers: servers::SERVERS.states(),
        log: servers::LOG.newest_first(),
    }
}

/// "Try now" on a resting server: its rest ends, so the next check connects.
#[tauri::command(async)]
fn wake_server(address: String) {
    servers::SERVERS.answered(&address);
}

/// Senders whose images load without asking, for Settings.
#[tauri::command(async)]
fn trusted_senders(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    with_store(&state, |s| s.trusted_senders())
}

#[derive(Debug, Serialize)]
struct HtmlView {
    /// A complete, inert document for the sandboxed frame.
    document: String,
    /// Remote images still left out, and how many were loaded.
    blocked: usize,
    loaded: usize,
    sender: String,
    /// The sender's images load without asking (outside Spam).
    trusted: bool,
}

/// The message's HTML, or `null` for plain-text mail. Remote images are downloaded here,
/// never by the window. `load_images`: `true` loads them, `false` never does (the text
/// view only asks whether there is HTML), `null` loads them when the sender is trusted and
/// the message isn't spam. `dark`: the theme's colors when the message goes on dark paper,
/// with its own colors turned around to match; `null` shows it on white.
#[tauri::command]
async fn get_html(
    state: State<'_, AppState>,
    id: MessageId,
    load_images: Option<bool>,
    dark: Option<Palette>,
) -> CmdResult<Option<HtmlView>> {
    let paper = dark.map_or(Paper::Light, Paper::Dark);
    let (raw, trust) = with_store(&state, |s| Ok((s.readable_raw(id)?, s.image_trust(id)?)))?;
    let Some(raw) = raw else { return Ok(None) };
    let load = load_images.unwrap_or(trust.auto);
    // Cleaning a large message, and fetching its images, is work for a thread of its own.
    let rendered = tauri::async_runtime::spawn_blocking(move || {
        let first = html::render(&raw, &HashMap::new(), &paper)?;
        if !load || first.remote.is_empty() {
            return Some((first, 0));
        }
        let images = html::fetch_images(&first.remote, &FetchLimits::default());
        let loaded = images.len();
        Some((html::render(&raw, &images, &paper).unwrap_or(first), loaded))
    })
    .await
    .map_err(failed)?;
    let Some((rendered, loaded)) = rendered else {
        return Ok(None);
    };
    Ok(Some(HtmlView {
        document: rendered.document,
        blocked: rendered.blocked,
        loaded,
        sender: trust.sender,
        trusted: trust.trusted,
    }))
}

/// Load a sender's remote images without asking from now on, or stop that.
#[tauri::command(async)]
fn trust_sender(state: State<'_, AppState>, email: String, trusted: bool) -> CmdResult<()> {
    with_store(&state, |s| s.set_sender_trusted(&email, trusted))
}

/// What the compose pane opens with: recipients, subject and text for a new message
/// (from `account`), or a reply or forward of `of`, with the right identity's signature
/// and templates.
#[tauri::command(async)]
fn start_draft(
    state: State<'_, AppState>,
    settings: State<'_, SettingsState>,
    kind: DraftKind,
    of: Option<MessageId>,
    account: String,
    folder: Option<String>,
) -> CmdResult<DraftStart> {
    let sign_replies = settings.get().sign_replies;
    with_store(&state, |s| {
        drafts::start(s, kind, of, &account, folder.as_deref(), sign_replies)
    })
}

/// Every folder's own templates (Settings › Writing).
#[tauri::command(async)]
fn folder_templates(state: State<'_, AppState>) -> CmdResult<Vec<FolderTemplate>> {
    with_store(&state, |s| s.folder_templates())
}

/// Keep a folder's templates; all empty takes them away.
#[tauri::command(async)]
fn save_folder_template(state: State<'_, AppState>, template: FolderTemplate) -> CmdResult<()> {
    with_store(&state, |s| s.save_folder_template(&template))
}

/// Add (id 0) or update an identity. Mistakes come back as sentences.
#[tauri::command(async)]
fn save_identity(state: State<'_, AppState>, identity: Identity) -> CmdResult<Identity> {
    with_store(&state, |s| s.save_identity(&identity))
}

#[tauri::command(async)]
fn delete_identity(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    with_store(&state, |s| s.delete_identity(id))
}

/// Every rule, in the order they run.
#[tauri::command(async)]
fn list_rules(state: State<'_, AppState>) -> CmdResult<Vec<Rule>> {
    with_store(&state, |s| s.rules())
}

/// Add (id 0) or update a rule. Mistakes come back as sentences for the editor.
#[tauri::command(async)]
fn save_rule(state: State<'_, AppState>, rule: Rule) -> CmdResult<Rule> {
    with_store(&state, |s| s.save_rule(&rule))
}

#[tauri::command(async)]
fn preview_rule(
    state: State<'_, AppState>,
    rule: Rule,
    view: telex_core::ListView,
) -> CmdResult<telex_core::filters::RulePreview> {
    with_store(&state, |s| filters::preview(s, &rule, &view))
}

#[tauri::command(async)]
fn list_saved_searches(
    state: State<'_, AppState>,
) -> CmdResult<Vec<telex_core::saved_searches::SavedSearch>> {
    with_store(&state, |s| s.saved_searches())
}

#[tauri::command(async)]
fn save_search(
    state: State<'_, AppState>,
    search: telex_core::saved_searches::SavedSearch,
) -> CmdResult<telex_core::saved_searches::SavedSearch> {
    with_store(&state, |s| s.save_search(&search))
}

#[tauri::command(async)]
fn delete_search(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    with_store(&state, |s| s.delete_search(id))
}

#[tauri::command(async)]
fn delete_rule(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    with_store(&state, |s| s.delete_rule(id))
}

/// Move a rule up (-1) or down (1).
#[tauri::command(async)]
fn move_rule(state: State<'_, AppState>, id: i64, delta: i64) -> CmdResult<()> {
    with_store(&state, |s| s.move_rule(id, delta))
}

/// Move an account up (-1) or down (1) in the folder tree.
#[tauri::command(async)]
fn move_account(state: State<'_, AppState>, id: String, delta: i64) -> CmdResult<()> {
    with_store(&state, |s| s.move_account(&id, delta))
}

/// Run the rules now on every message in a folder. Auto-replies don't run here: they
/// answer new mail only.
#[tauri::command(async)]
fn run_rules(state: State<'_, AppState>, folder: String) -> CmdResult<RunReport> {
    with_store(&state, |s| {
        let ids: Vec<MessageId> = s.list(&folder)?.iter().map(|m| m.id).collect();
        filters::run(s, &ids, Trigger::OnDemand, Local::now())
    })
}

/// Run the rules whose schedule is due (the window asks every minute while Telex runs). Like
/// Run now, no auto-replies or forwards; the window pushes the changes as after any other.
#[tauri::command(async)]
fn run_scheduled_rules(state: State<'_, AppState>) -> CmdResult<Vec<filters::ScheduledRun>> {
    with_store(&state, |s| filters::run_scheduled(s, Local::now()))
}

/// What Telex's provider list knows about an address, without going online: for tips
/// while the user types it.
#[tauri::command(async)]
fn known_provider(email: String) -> Option<Detected> {
    autoconfig::known(&email)
}

/// Find the servers for an address: the provider list, the domain's setup file, its DNS
/// zone (SRV, MX) and the usual names. `null` when nothing answered. The steps reach the
/// UI as "detect-progress" events.
#[tauri::command]
async fn detect_settings(app: AppHandle, email: String) -> Option<Detected> {
    let progress = |step: Step| {
        let _ = app.emit("detect-progress", step);
    };
    autoconfig::detect(&email, &progress).await
}

/// Open a message in a window of its own: the same page, showing just that message, its
/// header and its buttons (index.html?message=<id>). A message already open in one comes to
/// the front.
///
/// Async on purpose: a plain command runs on the main thread inside WebView2's message
/// handler, and building a webview there waits for that same thread forever (both windows
/// freeze, white and unclosable).
#[tauri::command]
async fn open_window(app: AppHandle, id: MessageId) -> CmdResult<()> {
    let label = format!("message-{id}");
    if let Some(open) = app.get_webview_window(&label) {
        let _ = open.unminimize();
        return open.set_focus().map_err(|e| e.to_string());
    }
    WebviewWindowBuilder::new(
        &app,
        label,
        WebviewUrl::App(format!("index.html?message={id}").into()),
    )
    .title("Telex")
    .inner_size(880.0, 720.0)
    .min_inner_size(480.0, 360.0)
    .build()
    .map(|_| ())
    .map_err(|e| {
        tr!(
            "Couldn't open a window for the message: {e}",
            "Nie udało się otworzyć okna wiadomości: {e}"
        )
    })
}

/// Open a web link from a message in the default browser.
#[tauri::command(async)]
fn open_link(app: AppHandle, url: String) -> CmdResult<()> {
    let url = url.trim();
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return Err(tr!(
            "Telex opens only web links (http and https).",
            "Telex otwiera tylko linki do stron (http i https)."
        ));
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| {
        tr!(
            "Couldn't open the link: {e}",
            "Nie udało się otworzyć linku: {e}"
        )
    })
}

/// Check the settings by logging in to IMAP and SMTP, then save the account and its
/// password. The first account creates the user's mail base on disk.
/// Adding an address that already exists updates it; an empty password then keeps the
/// saved one.
#[tauri::command]
async fn add_account(
    app: AppHandle,
    state: State<'_, AppState>,
    setup: AccountSetup,
    password: String,
) -> CmdResult<Account> {
    let setup = setup.cleaned()?;
    let known = with_store(&state, |s| {
        Ok(s.accounts()
            .iter()
            .any(|a| a.imap.is_some() && a.email.eq_ignore_ascii_case(&setup.email)))
    })?;
    let password = match (password.is_empty(), known) {
        (false, _) => password,
        (true, true) => secrets::load_password(&setup.email).map_err(|e| capitalize(&e))?,
        (true, false) => return Err(tr!("Enter the password.", "Wpisz hasło.")),
    };
    let folders = imap::check_account(&setup.imap_settings(), &password)
        .await
        .map_err(|e| capitalize(&e.to_string()))?;
    smtp::check(&setup.smtp_settings(), &setup.username, &password)
        .await
        .map_err(|e| {
            let incoming = setup.imap_settings().protocol().to_uppercase();
            tr!(
                "{incoming} works, but sending doesn't: {e}.",
                "{incoming} działa, ale wysyłanie nie: {e}."
            )
        })?;
    secrets::save_password(&setup.email, &password)?;
    let account = with_store(&state, |s| {
        if s.is_in_memory() {
            *s = MailStore::open(&state.db_path)?;
        }
        let account = s.upsert_account(&setup)?;
        let plan = imap::plan_folders(&folders, &s.folder_hints(&account.id)?);
        s.set_folders(&account.id, &plan)?;
        s.account(&account.id).cloned()
    })?;
    watch::restart(&app);
    Ok(account)
}

/// Remove an account, its local mail and its saved password. The server's mail is not
/// touched; an away reply Telex put on the server is taken off first, as nobody could later.
#[tauri::command]
async fn remove_account(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<()> {
    // No check may start on the account while it goes.
    let Some(_busy) = BusyGuard::take(&state.busy) else {
        return Err(tr!(
            "Telex is talking to the mail server. Try again in a moment.",
            "Telex rozmawia z serwerem poczty. Spróbuj za chwilę."
        ));
    };
    let (account, away) = with_store(&state, |s| Ok((s.account(&id)?.clone(), s.away(&id)?)))?;
    if away.place == telex_core::away::Place::Server {
        if let (Some(imap), Ok(password)) = (&account.imap, secrets::load_password(&account.email))
        {
            let off = telex_core::away::Away {
                enabled: false,
                ..away
            };
            // Best effort: the account goes either way.
            let _ = telex_core::away::apply(&account, imap, &password, off).await;
        }
    }
    let email = with_store(&state, |s| {
        let email = s.account(&id)?.email.clone();
        s.remove_account(&id)?;
        Ok(email)
    })?;
    secrets::delete_password(&email);
    watch::restart(&app);
    Ok(())
}

#[derive(Debug, Default, Serialize)]
struct SyncOutcome {
    /// Ids of messages that arrived.
    new: Vec<MessageId>,
    /// The folder the newest of them is in now, after the rules.
    new_in: Option<String>,
    /// One line per message sent from an Outbox: `"Re: Wycena" to Anna Kowalska`.
    sent: Vec<String>,
    /// New messages the rules changed.
    sorted: usize,
    /// New messages the spam filter moved to Spam.
    spam: usize,
    /// One line per problem, e.g. "Studio: can't reach mail.x.pl: …".
    errors: Vec<String>,
    /// Another check or push was running, so this one didn't start. Try again shortly.
    busy: bool,
}

impl SyncOutcome {
    fn busy() -> Self {
        Self {
            busy: true,
            ..Self::default()
        }
    }
}

/// Holds the "busy" flag while Telex talks to the servers, and clears it however that
/// ends, even on an early return.
struct BusyGuard<'a>(&'a AtomicBool);

impl<'a> BusyGuard<'a> {
    fn take(flag: &'a AtomicBool) -> Option<Self> {
        (!flag.swap(true, Ordering::SeqCst)).then_some(Self(flag))
    }
}

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// The accounts that live on a server.
pub(crate) fn server_accounts(state: &AppState) -> CmdResult<Vec<Account>> {
    with_store(state, |s| {
        Ok(s.accounts()
            .iter()
            .filter(|a| a.imap.is_some())
            .cloned()
            .collect())
    })
}

/// Full sync of every account: send queued changes, download new mail and flag changes,
/// send what changed meanwhile. The accounts sync at the same time, each on its own
/// connection: a sync mostly waits on its server, so one after another a check took as long
/// as all of them added up. Progress reaches the UI as "sync-progress" events. One account
/// failing doesn't stop the others. With `account`, only that account's Inbox: what IDLE
/// said changed.
#[tauri::command]
async fn check_mail(
    app: AppHandle,
    state: State<'_, AppState>,
    account: Option<String>,
) -> CmdResult<SyncOutcome> {
    let Some(_busy) = BusyGuard::take(&state.busy) else {
        return Ok(SyncOutcome::busy());
    };
    let progress = |p: SyncProgress| {
        let _ = app.emit("sync-progress", p);
    };
    let scope = if account.is_some() {
        Scope::Inbox
    } else {
        Scope::All
    };
    let mut outcome = SyncOutcome::default();
    let mut arrived = Vec::new();
    let accounts: Vec<Account> = server_accounts(&state)?
        .into_iter()
        .filter(|a| account.as_ref().is_none_or(|only| &a.id == only))
        .collect();
    // Send first, so the copies for Sent go to the server in this same sync. One Outbox at a
    // time: only one sender may run (an empty Outbox costs nothing).
    for account in &accounts {
        send_account_outbox(&state, account, &mut outcome.sent, &mut outcome.errors).await;
    }
    let results = join_all(accounts.iter().map(|account| async {
        let result = match secrets::load_password(&account.email) {
            Ok(password) => imap::sync_account(account, &password, &state.store, &progress, scope)
                .await
                .map_err(|e| e.to_string()),
            Err(e) => Err(e),
        };
        let error = result.as_ref().err().map(String::as_str);
        let _ = with_store(&state, |s| {
            s.record_sync(&account.id, Local::now().naive_local(), error)
        });
        result
    }))
    .await;
    for (account, result) in accounts.iter().zip(results) {
        match result {
            Ok(report) => {
                outcome.new.extend(report.new);
                arrived.extend(report.arrived);
                outcome.sorted += report.sorted;
                outcome.spam += report.spam;
                let refused = report.refused.iter();
                outcome
                    .errors
                    .extend(refused.map(|r| format!("{}: {r}", account.name)));
                // Rules may have queued automatic replies: send them now, not next time.
                if report.answered > 0 {
                    send_account_outbox(&state, account, &mut outcome.sent, &mut outcome.errors)
                        .await;
                }
            }
            Err(e) => outcome.errors.push(format!("{}: {e}", account.name)),
        }
    }
    if let Some(&newest) = outcome.new.last() {
        outcome.new_in = with_store(&state, |s| s.folder_of(newest)).unwrap_or_default();
    }
    watch::announce(&app, &arrived);
    Ok(outcome)
}

#[derive(Debug, Default, Serialize)]
struct OlderOutcome {
    /// How many messages came down.
    new: usize,
    /// A check or push was running, so this didn't start. Try again shortly.
    busy: bool,
}

/// Download up to `limit` older messages of a folder: those the first download left on the
/// server. Progress reaches the UI as "sync-progress" events.
#[tauri::command]
async fn download_older(
    app: AppHandle,
    state: State<'_, AppState>,
    folder: String,
    limit: usize,
) -> CmdResult<OlderOutcome> {
    let Some(_busy) = BusyGuard::take(&state.busy) else {
        return Ok(OlderOutcome {
            busy: true,
            ..OlderOutcome::default()
        });
    };
    let account_id = folder.split('/').next().unwrap_or_default();
    let account = with_store(&state, |s| s.account(account_id).cloned())?;
    let password = secrets::load_password(&account.email)?;
    let progress = |p: SyncProgress| {
        let _ = app.emit("sync-progress", p);
    };
    let new = imap::download_older(
        &account,
        &password,
        &state.store,
        &folder,
        limit.clamp(1, 5000),
        &progress,
    )
    .await
    .map_err(|e| capitalize(&e.to_string()))?;
    Ok(OlderOutcome {
        new: new.len(),
        busy: false,
    })
}

#[derive(Debug, Default, Clone, Serialize)]
struct SendOutcome {
    /// The message's id in the Outbox.
    queued: MessageId,
    /// Seconds it waits before it goes, so it can be taken back; 0: it was sent right away.
    wait: u16,
    /// Sent later: when it goes. It waits in the Outbox until then.
    goes: Option<NaiveDateTime>,
    /// One line per message the SMTP server accepted, this one and any that waited.
    sent: Vec<String>,
    errors: Vec<String>,
}

/// Send a message: build it, put it in the Outbox and send it. With Settings › Writing's
/// moment to undo, it waits that long first (`hold_message` takes it back) and goes out in
/// the background; the result comes as an "outbox-sent" event. It only counts as sent once
/// the SMTP server accepts it; until then it waits in the Outbox and goes out at the next
/// check. With `later`, it waits in the Outbox until then (`send_timer` sends it).
// A command's arguments are what the window passes and the states it needs.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn send_message(
    app: AppHandle,
    state: State<'_, AppState>,
    files: State<'_, FilesState>,
    settings: State<'_, SettingsState>,
    keys: State<'_, keys::KeysState>,
    draft: OutgoingDraft,
    later: Option<Later>,
    pgp: Option<Choice>,
) -> CmdResult<SendOutcome> {
    files.check(&draft.attachments)?;
    let current = settings.get();
    let goes = later
        .map(|l| later::when(l, Local::now().naive_local()))
        .transpose()?;
    // Mail sent later needs no moment to undo: it waits anyway, and can be held or edited.
    let wait = if goes.is_some() { 0 } else { current.undo_send };
    let (account, originals) = with_store(&state, |s| draft_context(s, &draft))?;
    // OpenPGP: signed or encrypted as compose says, and the Autocrypt header.
    let choice = pgp.unwrap_or_default();
    let protect = keys::protection_for(
        &state,
        &keys,
        &account,
        &draft,
        choice,
        current.pgp_autocrypt,
        false,
    )?;
    let composed = if protect.is_empty() {
        smtp::compose(&account, &draft, &originals, Local::now())?
    } else {
        smtp::compose_protected(&account, &draft, &originals, Local::now(), &protect)?
    };
    if pgp.is_some() {
        keys::remember(&state, &draft, choice);
    }
    let send_after = goes.or_else(|| {
        (wait > 0).then(|| Local::now().naive_local() + TimeDelta::seconds(i64::from(wait)))
    });
    let queued = with_store(&state, |s| {
        let queued = s.queue_outgoing(
            &account.id,
            &composed.message,
            &composed.raw,
            &composed.env_from,
            &composed.env_to,
            draft.replaces,
            send_after,
        )?;
        if let Some(autosave) = draft.autosaved {
            s.discard_draft(autosave)?;
        }
        Ok(queued)
    })?;
    let mut outcome = SendOutcome {
        queued,
        wait,
        goes,
        ..SendOutcome::default()
    };
    if goes.is_some() {
        return Ok(outcome);
    }
    if wait == 0 {
        send_account_outbox(&state, &account, &mut outcome.sent, &mut outcome.errors).await;
        return Ok(outcome);
    }
    tauri::async_runtime::spawn(async move {
        // A moment more than the wait, so the message is due when the Outbox is read.
        tokio::time::sleep(Duration::from_millis(u64::from(wait) * 1000 + 300)).await;
        let state = app.state::<AppState>();
        let mut done = SendOutcome {
            queued,
            ..SendOutcome::default()
        };
        send_account_outbox(&state, &account, &mut done.sent, &mut done.errors).await;
        let _ = app.emit("outbox-sent", done);
    });
    Ok(outcome)
}

/// Take back a message still waiting out its moment in the Outbox. It stays there, held,
/// for the user to edit (E) or drop.
#[tauri::command(async)]
fn hold_message(state: State<'_, AppState>, id: MessageId) -> CmdResult<()> {
    with_store(&state, |s| s.hold_outgoing(id, Local::now().naive_local()))
}

/// The Send later menu as it is now: in an hour, tomorrow morning, Monday morning.
#[tauri::command(async)]
fn send_later_choices() -> Vec<later::Choice> {
    later::choices(Local::now().naive_local())
}

/// A message waiting in the Outbox (sent later, held, or whose last try failed) goes now.
#[tauri::command]
async fn send_now(state: State<'_, AppState>, id: MessageId) -> CmdResult<SendOutcome> {
    let account = with_store(&state, |s| {
        let account = s.send_now(id)?;
        s.account(&account).cloned()
    })?;
    let mut outcome = SendOutcome {
        queued: id,
        ..SendOutcome::default()
    };
    send_account_outbox(&state, &account, &mut outcome.sent, &mut outcome.errors).await;
    Ok(outcome)
}

/// Mail sent later goes when it is due while Telex runs, and mail that was due while it was
/// closed goes a moment after it starts (its line says it went late). The result comes as
/// an "outbox-sent" event, like undo send's. Only mail never tried: one whose try failed
/// waits for the next check, and an account that can't send now (no SMTP server, no saved
/// password) is left alone for a while instead of being asked every second.
fn send_timer(app: AppHandle) {
    const MOST: Duration = Duration::from_secs(30);
    const REST: TimeDelta = TimeDelta::minutes(10);
    tauri::async_runtime::spawn(async move {
        // A moment after the start, so the window listens and the first check may go first.
        tokio::time::sleep(Duration::from_secs(5)).await;
        let mut resting: HashMap<String, NaiveDateTime> = HashMap::new();
        loop {
            let state = app.state::<AppState>();
            let now = Local::now().naive_local();
            resting.retain(|_, until| *until > now);
            let due = with_store(&state, |s| s.scheduled_due(now)).unwrap_or_default();
            let mut done = SendOutcome::default();
            for id in due.iter().filter(|id| !resting.contains_key(*id)) {
                let Ok(account) = with_store(&state, |s| s.account(id).cloned()) else {
                    continue;
                };
                send_account_outbox(&state, &account, &mut done.sent, &mut done.errors).await;
            }
            if !done.sent.is_empty() || !done.errors.is_empty() {
                let _ = app.emit("outbox-sent", done);
            }
            // Still due and never tried: the account couldn't send at all.
            let now = Local::now().naive_local();
            for id in with_store(&state, |s| s.scheduled_due(now)).unwrap_or_default() {
                resting.entry(id).or_insert(now + REST);
            }
            let next = with_store(&state, |s| s.next_scheduled()).ok().flatten();
            let wait = next
                .and_then(|at| (at - Local::now().naive_local()).to_std().ok())
                .map_or(MOST, |left| left.min(MOST))
                .max(Duration::from_millis(500));
            tokio::time::sleep(wait).await;
        }
    });
}

#[derive(Debug, Default, Serialize)]
struct UnsubscribeOutcome {
    /// one_click, mail or link.
    method: &'static str,
    /// Done: the list's server took the request, the mail went, or the page opened.
    done: bool,
    /// The unsubscribe mail, once the SMTP server took it.
    sent: Vec<String>,
    /// Why the mail didn't go yet (it waits in the Outbox).
    errors: Vec<String>,
}

/// Leave the mailing list of a message, the best way it offers (`unsubscribe.rs`): one click
/// over https (the core posts it, to public addresses only), a mail through the account's
/// SMTP, or the list's page in the browser. Never from Spam. The window asks first for a
/// mail or a page. The message gets a note, and every message of the list says it was done.
#[tauri::command]
async fn unsubscribe(
    app: AppHandle,
    state: State<'_, AppState>,
    id: MessageId,
) -> CmdResult<UnsubscribeOutcome> {
    let plan = with_store(&state, |s| s.unsubscribe_plan(id))?;
    let mut outcome = UnsubscribeOutcome {
        method: plan.method.code(),
        ..UnsubscribeOutcome::default()
    };
    match &plan.method {
        Method::OneClick(url) => {
            let url = url.clone();
            tauri::async_runtime::spawn_blocking(move || {
                telex_core::unsubscribe::post(&url, false)
            })
            .await
            .map_err(failed)?
            .map_err(|e| capitalize(&e))?;
        }
        Method::Link(url) => app.opener().open_url(url, None::<&str>).map_err(|e| {
            tr!(
                "Couldn't open the link: {e}",
                "Nie udało się otworzyć linku: {e}"
            )
        })?,
        Method::Mail { .. } => {
            let (account, identity) = with_store(&state, |s| s.unsubscribe_sender(id))?;
            let draft = telex_core::unsubscribe::draft(&account, Some(identity), &plan.method)
                .unwrap_or_default();
            let (account, originals) = with_store(&state, |s| draft_context(s, &draft))?;
            let composed = smtp::compose(&account, &draft, &originals, Local::now())?;
            let queued = with_store(&state, |s| {
                s.queue_outgoing(
                    &account.id,
                    &composed.message,
                    &composed.raw,
                    &composed.env_from,
                    &composed.env_to,
                    None,
                    None,
                )
            })?;
            send_account_outbox(&state, &account, &mut outcome.sent, &mut outcome.errors).await;
            // Not sent yet: it waits in the Outbox, and only then is it done.
            if with_store(&state, |s| s.waiting_to_go(queued))? {
                return Ok(outcome);
            }
        }
    }
    with_store(&state, |s| {
        s.record_unsubscribe(&plan, Local::now().naive_local())
    })?;
    outcome.done = true;
    Ok(outcome)
}

/// What answering an invitation did: the sending, and the card as it is now.
#[derive(Debug, Default, Serialize)]
struct InviteAnswer {
    sent: Vec<String>,
    errors: Vec<String>,
    invite: Option<Invite>,
}

/// Accept, Maybe or Decline a calendar invitation: the reply to the organizer goes at once
/// (without the moment to undo; another answer can follow it).
#[tauri::command(async)]
async fn answer_invite(
    state: State<'_, AppState>,
    id: MessageId,
    answer: Answer,
    comment: String,
) -> CmdResult<InviteAnswer> {
    let replied = with_store(&state, |s| {
        Ok(s.answer_invite(id, answer, &comment, Local::now()))
    })??;
    let account = with_store(&state, |s| s.account(&replied.account).cloned())?;
    let mut outcome = InviteAnswer::default();
    send_account_outbox(&state, &account, &mut outcome.sent, &mut outcome.errors).await;
    outcome.invite = with_store(&state, |s| s.invite(id))?;
    Ok(outcome)
}

/// Send what is due in the account's Outbox, after any send already running.
async fn send_account_outbox(
    state: &AppState,
    account: &Account,
    sent: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    let _sending = state.sending.lock().await;
    let has_mail = with_store(state, |s| {
        s.has_outgoing(&account.id, Local::now().naive_local())
    })
    .unwrap_or(false);
    if !has_mail {
        return;
    }
    let mut problem = |e: &str| errors.push(format!("{}: {e}", account.name));
    let Some(settings) = &account.smtp else {
        problem(&tr!(
            "no SMTP server is set, so the Outbox waits. Add one in Settings › Accounts",
            "nie ustawiono serwera SMTP, więc wiadomości czekają w Skrzynce nadawczej. \
             Dodaj go w Ustawienia › Konta"
        ));
        return;
    };
    let username = account
        .imap
        .as_ref()
        .map_or(account.email.as_str(), |i| i.username.as_str());
    let password = match secrets::load_password(&account.email) {
        Ok(p) => p,
        Err(e) => {
            problem(&e);
            return;
        }
    };
    let transport = match smtp::transport(settings, username, &password) {
        Ok(t) => t,
        Err(SendError::Refused(e) | SendError::Failed(e)) => {
            problem(&e);
            return;
        }
    };
    match smtp::send_outbox(account, &transport, &state.store).await {
        Ok(report) => {
            sent.extend(report.sent);
            for p in &report.problems {
                problem(p);
            }
        }
        Err(e) => problem(&e.to_string()),
    }
}

/// Send queued changes (read state, flags, moves, deletes) to the servers now, without
/// downloading, every account at the same time. The UI calls this a few seconds after the
/// user changes something.
#[tauri::command]
async fn push_changes(state: State<'_, AppState>) -> CmdResult<SyncOutcome> {
    let Some(_busy) = BusyGuard::take(&state.busy) else {
        return Ok(SyncOutcome::busy());
    };
    let mut outcome = SyncOutcome::default();
    // Only accounts with queued changes: no password is read, and no keyring asked, for
    // the others.
    let accounts: Vec<Account> = server_accounts(&state)?
        .into_iter()
        .filter(|a| with_store(&state, |s| s.has_pending_changes(&a.id)).unwrap_or(true))
        .collect();
    let results = join_all(accounts.iter().map(|account| async {
        match secrets::load_password(&account.email) {
            Ok(password) => imap::push_account(account, &password, &state.store)
                .await
                .map_err(|e| {
                    tr!(
                        "{e}. Your changes are kept and sent at the next check",
                        "{e}. Zmiany są zachowane i zostaną wysłane przy następnym sprawdzaniu \
                         poczty"
                    )
                }),
            Err(e) => Err(e),
        }
    }))
    .await;
    for (account, result) in accounts.iter().zip(results) {
        match result {
            Ok(refused) => outcome
                .errors
                .extend(refused.iter().map(|r| format!("{}: {r}", account.name))),
            Err(e) => outcome.errors.push(format!("{}: {e}", account.name)),
        }
    }
    Ok(outcome)
}

pub fn run() {
    let mut context = tauri::generate_context!();
    let mut app = tauri::Builder::default()
        // One Telex at a time, registered first: two on one mail base would send the same
        // Outbox twice and check and sort new mail twice. Starting it again shows the window.
        // Also when the first one is hidden in the tray.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init());
    // Release builds only: the others can't update themselves and say so.
    if let Some(updater) = updates::plugin(&mut context) {
        app = app.plugin(updater);
    }
    app.manage(updates::Found::default())
        .manage(FilesState::default())
        .manage(migrate::Sources::default())
        .manage(watch::Watchers::default())
        .manage(keys::KeysState::default())
        .manage(tray::TrayState::default())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db_path = dir.join("telex.db");
            app.manage(settings::SettingsState::load(dir.join("settings.json")));
            app.manage(spell::SpellState::start(&dir));
            let (store, problem) = open_store(&db_path);
            if let Some(problem) = problem {
                app.dialog()
                    .message(problem)
                    .kind(MessageDialogKind::Error)
                    .title("Telex")
                    .show(|_| {});
            }
            app.manage(AppState {
                store: Mutex::new(store),
                db_path,
                busy: AtomicBool::new(false),
                sending: tokio::sync::Mutex::new(()),
                backing_up: AtomicBool::new(false),
                restorable: Mutex::new(None),
            });
            // Attachments opened last time; files still open elsewhere stay.
            telex_core::attachments::clear_opened(&std::env::temp_dir());
            watch::restart(app.handle());
            send_timer(app.handle().clone());
            // The tray icon, and the window shown (it starts hidden: tauri.conf.json), unless
            // Telex started hidden in the tray.
            tray::start(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::DragDrop(drop) => files::drag_drop(window, drop),
            // The main window gone (not hidden in the tray) closes the messages open in their
            // own, and Telex.
            WindowEvent::Destroyed if window.label() == "main" => {
                for (label, open) in window.app_handle().webview_windows() {
                    if label != "main" {
                        let _ = open.close();
                    }
                }
            }
            WindowEvent::Resized(_) => tray::minimized(window),
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            folder_tree,
            connections,
            wake_server,
            list_page,
            get_message,
            message_markdown,
            set_unread,
            set_flagged,
            set_unread_many,
            set_flagged_many,
            set_tag_many,
            save_tag,
            delete_tag,
            move_tag,
            move_messages,
            mark_spam,
            spam_status,
            delete_messages,
            mark_folder_read,
            choose_folder,
            set_sent_copy,
            list_ids,
            next_unread,
            recipients,
            contacts::contacts,
            contacts::contact_groups,
            contacts::save_contact,
            contacts::delete_contacts,
            contacts::restore_contacts,
            contacts::save_group,
            contacts::delete_group,
            contacts::restore_group,
            contacts::suggested_contacts,
            contacts::import_vcards,
            contacts::export_vcards,
            hold_message,
            send_now,
            send_later_choices,
            unsubscribe,
            answer_invite,
            download_older,
            files::open_attachment,
            files::save_attachment,
            files::save_attachments,
            files::pick_files,
            export::save_messages,
            export::message_source,
            export::print_document,
            export::save_pdf,
            export::drag_out,
            backup::backup_state,
            backup::backup_now,
            backup::auto_backup,
            backup::pick_backup_dir,
            backup::inspect_backup,
            backup::restore_backup,
            backup::compact_base,
            backup::show_backup_place,
            folders::create_local_folder,
            folders::rename_local_folder,
            folders::delete_local_folder,
            folders::export_folder,
            folders::import_mail,
            migrate::find_programs,
            migrate::open_source,
            migrate::move_in,
            spell::spell_languages,
            spell::spell_check,
            spell::spell_suggest,
            spell::spell_add,
            spell::spell_download,
            spell::spell_remove,
            away::get_away,
            away::save_away,
            keys::pgp_keys,
            keys::pgp_import,
            keys::pgp_import_text,
            keys::pgp_import_offered,
            keys::pgp_generate,
            keys::pgp_export,
            keys::pgp_public_text,
            keys::pgp_delete,
            keys::pgp_verify,
            keys::pgp_unlock,
            keys::pgp_forget,
            keys::pgp_compose,
            keys::smime_certs,
            keys::smime_pick,
            keys::smime_import_p12,
            keys::smime_cancel_p12,
            keys::smime_export,
            keys::smime_delete,
            keys::smime_verify,
            keys::smime_unlock,
            keys::smime_forget,
            save_outgoing,
            discard_draft,
            draft_html,
            html_to_text,
            mentions_attachment,
            text_to_html,
            clean_html,
            usage,
            trusted_senders,
            add_account,
            remove_account,
            known_provider,
            detect_settings,
            check_mail,
            push_changes,
            send_message,
            get_html,
            trust_sender,
            open_link,
            open_window,
            list_rules,
            save_rule,
            preview_rule,
            list_saved_searches,
            save_search,
            delete_search,
            delete_rule,
            move_rule,
            move_account,
            run_rules,
            run_scheduled_rules,
            start_draft,
            folder_templates,
            save_folder_template,
            files::editor_image,
            save_identity,
            delete_identity,
            settings::get_settings,
            settings::update_settings,
            settings::export_settings,
            settings::show_export,
            settings::open_import,
            settings::apply_import,
            tray::close_requested,
            tray::desktop_state,
            tray::open_taskbar_settings,
            updates::update_info,
            count::count_copy,
            updates::check_update,
            updates::install_update,
        ])
        .build(context)
        .expect("Telex failed to start")
        .run(|app, event| {
            // The process may end without dropping the store: tidy the statistics now.
            if let tauri::RunEvent::Exit = event {
                if let Some(state) = app.try_state::<AppState>() {
                    if let Ok(store) = state.store.lock() {
                        store.optimize();
                    }
                }
            }
        });
}
