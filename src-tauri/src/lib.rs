//! Tauri shell: exposes `telex_core::MailStore` to the UI as commands.
//! Keep logic out of here; put it in `crates/telex-core` where it can be tested.

mod away;
mod backup;
mod files;
mod folders;
mod migrate;
mod secrets;
mod settings;
mod spell;
mod updates;
mod watch;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use chrono::{Local, TimeDelta};
use futures_util::future::join_all;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_opener::OpenerExt;
use telex_core::autoconfig::{self, Detected, Step};
use telex_core::drafts::{self, DraftKind, DraftStart};
use telex_core::filters::{self, Rule, RunReport, Trigger};
use telex_core::html::{self, FetchLimits, Palette, Paper};
use telex_core::imap::{self, Scope, SyncProgress};
use telex_core::recipients::Recipient;
use telex_core::smtp::{self, Originals, SendError};
use telex_core::tr;
use telex_core::usage::Usage;
use telex_core::{
    Account, AccountSetup, DeleteReport, FolderKind, FolderTree, Identity, ListView, MailStore,
    MessageId, OutgoingDraft, Page, PageAt, StoreError, UnreadAt,
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
fn folder_tree(state: State<'_, AppState>) -> CmdResult<FolderTree> {
    with_store(&state, |s| s.folder_tree())
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
    id: MessageId,
) -> CmdResult<telex_core::risk::InspectedMessage> {
    with_store(&state, |s| s.inspect_message(id))
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

/// The account a draft is from, and the raw messages it replies to and forwards.
fn draft_context(s: &MailStore, draft: &OutgoingDraft) -> Result<(Account, Originals), StoreError> {
    let account = s.account(&draft.account)?.clone();
    let raw = |id: Option<MessageId>| id.map(|id| s.raw_of(id)).transpose();
    let originals = Originals {
        replied: raw(draft.reply_to)?.flatten(),
        forwarded: raw(draft.forward_of)?.flatten(),
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
    draft: OutgoingDraft,
) -> CmdResult<MessageId> {
    files.check(&draft.attachments)?;
    let (account, originals) = with_store(&state, |s| draft_context(s, &draft))?;
    // The attached files are read with the store free: the window goes on meanwhile.
    let composing = draft.clone();
    let raw = tauri::async_runtime::spawn_blocking(move || {
        smtp::compose_draft(&account, &composing, &originals, Local::now()).ok()
    })
    .await
    .ok()
    .flatten();
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
    let raw = with_store(&state, |s| s.raw_of(id))?;
    Ok(raw
        .as_deref()
        .and_then(telex_core::mime::html_body)
        .map(|b| html::compose_clean(&b.html)))
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

/// HTML pasted into the editor, cleaned to what Telex sends: no styles, scripts or images.
#[tauri::command(async)]
fn clean_html(html: String) -> String {
    html::compose_clean(&html)
}

/// What the mail takes and how it moves, for the Usage page and the status-bar meter.
#[tauri::command(async)]
fn usage(state: State<'_, AppState>) -> CmdResult<Usage> {
    with_store(&state, |s| s.usage(Local::now().date_naive()))
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
    let (raw, trust) = with_store(&state, |s| Ok((s.raw_of(id)?, s.image_trust(id)?)))?;
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
) -> CmdResult<DraftStart> {
    let sign_replies = settings.get().sign_replies;
    with_store(&state, |s| {
        drafts::start(s, kind, of, &account, sign_replies)
    })
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
            tr!(
                "IMAP works, but sending doesn't: {e}.",
                "IMAP działa, ale wysyłanie nie: {e}."
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
    /// One line per message the SMTP server accepted, this one and any that waited.
    sent: Vec<String>,
    errors: Vec<String>,
}

/// Send a message: build it, put it in the Outbox and send it. With Settings › Writing's
/// moment to undo, it waits that long first (`hold_message` takes it back) and goes out in
/// the background; the result comes as an "outbox-sent" event. It only counts as sent once
/// the SMTP server accepts it; until then it waits in the Outbox and goes out at the next
/// check.
#[tauri::command]
async fn send_message(
    app: AppHandle,
    state: State<'_, AppState>,
    files: State<'_, FilesState>,
    settings: State<'_, SettingsState>,
    draft: OutgoingDraft,
) -> CmdResult<SendOutcome> {
    files.check(&draft.attachments)?;
    let wait = settings.get().undo_send;
    let (account, originals) = with_store(&state, |s| draft_context(s, &draft))?;
    let composed = smtp::compose(&account, &draft, &originals, Local::now())?;
    let send_after =
        (wait > 0).then(|| Local::now().naive_local() + TimeDelta::seconds(i64::from(wait)));
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
        ..SendOutcome::default()
    };
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
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(main) = app.get_webview_window("main") {
                let _ = main.unminimize();
                let _ = main.show();
                let _ = main.set_focus();
            }
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
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::DragDrop(drop) => files::drag_drop(window, drop),
            // Closing the main window closes the messages open in their own, and Telex.
            WindowEvent::CloseRequested { .. } if window.label() == "main" => {
                for (label, open) in window.app_handle().webview_windows() {
                    if label != "main" {
                        let _ = open.close();
                    }
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            folder_tree,
            list_page,
            get_message,
            set_unread,
            set_flagged,
            set_unread_many,
            set_flagged_many,
            move_messages,
            mark_spam,
            spam_status,
            delete_messages,
            mark_folder_read,
            choose_folder,
            list_ids,
            next_unread,
            recipients,
            hold_message,
            download_older,
            files::open_attachment,
            files::save_attachment,
            files::save_attachments,
            files::pick_files,
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
            start_draft,
            save_identity,
            delete_identity,
            settings::get_settings,
            settings::update_settings,
            settings::export_settings,
            settings::show_export,
            settings::open_import,
            settings::apply_import,
            updates::update_info,
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
