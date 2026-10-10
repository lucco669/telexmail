//! Tauri shell: exposes `telex_core::MailStore` to the UI as commands.
//! Keep logic out of here; put it in `crates/telex-core` where it can be tested.

mod secrets;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use chrono::Local;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;
use telex_core::filters::{self, Rule, RunReport, Trigger};
use telex_core::html::{self, FetchLimits};
use telex_core::imap::{self, SyncProgress};
use telex_core::smtp::{self, Originals, SendError};
use telex_core::{
    sample::sample_store, Account, AccountSetup, DeleteOutcome, FolderKind, MailStore, Message,
    MessageId, OutgoingDraft, Snapshot, StoreError,
};

struct AppState {
    store: Mutex<MailStore>,
    /// The user's mail base: %APPDATA%\app.telexmail.desktop\telex.db.
    db_path: PathBuf,
    /// Telex is talking to the IMAP servers (a check or a push). Another one waits its
    /// turn instead of sending the same changes twice.
    busy: AtomicBool,
    /// An Outbox is being sent. A second sender would send the same mail twice.
    sending: AtomicBool,
}

/// Errors cross the IPC bridge as plain, user-readable strings.
type CmdResult<T> = Result<T, String>;

fn with_store<T>(
    state: &AppState,
    f: impl FnOnce(&mut MailStore) -> Result<T, StoreError>,
) -> CmdResult<T> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "The mail store stopped responding. Restart Telex.".to_string())?;
    f(&mut store).map_err(|e| e.to_string())
}

/// The user's mail base once they have added an account, the sample mailbox before that.
fn open_store(db_path: &Path) -> MailStore {
    if db_path.exists() {
        match MailStore::open(db_path) {
            Ok(store) if !store.accounts().is_empty() => return store,
            Ok(_) => {}
            Err(e) => eprintln!("Telex: can't open {}: {e}", db_path.display()),
        }
    }
    sample_store()
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Accounts, folders and every message summary, for the first paint and after bulk changes.
#[tauri::command]
fn snapshot(state: State<'_, AppState>) -> CmdResult<Snapshot> {
    with_store(&state, |s| s.snapshot())
}

#[tauri::command]
fn get_message(state: State<'_, AppState>, id: MessageId) -> CmdResult<Message> {
    with_store(&state, |s| s.get(id))
}

#[tauri::command]
fn set_unread(state: State<'_, AppState>, id: MessageId, unread: bool) -> CmdResult<()> {
    with_store(&state, |s| s.set_unread(id, unread))
}

#[tauri::command]
fn set_flagged(state: State<'_, AppState>, id: MessageId, flagged: bool) -> CmdResult<()> {
    with_store(&state, |s| s.set_flagged(id, flagged))
}

#[tauri::command]
fn move_message(state: State<'_, AppState>, id: MessageId, folder: String) -> CmdResult<()> {
    with_store(&state, |s| s.move_to(id, &folder))
}

#[tauri::command]
fn delete_message(state: State<'_, AppState>, id: MessageId) -> CmdResult<DeleteOutcome> {
    with_store(&state, |s| s.delete(id))
}

/// `send: false` saves a draft. `send: true` is for the sample mailbox only, which can't
/// really send: it files the message in Sent. Real mail goes through `send_message`.
#[tauri::command]
fn save_outgoing(
    state: State<'_, AppState>,
    draft: OutgoingDraft,
    send: bool,
) -> CmdResult<MessageId> {
    let kind = if send {
        FolderKind::Sent
    } else {
        FolderKind::Drafts
    };
    with_store(&state, |s| {
        s.save_outgoing(draft, kind, Local::now().naive_local())
    })
}

/// Full-text search over every stored message, bodies included.
#[tauri::command]
fn search(state: State<'_, AppState>, query: String) -> CmdResult<Vec<MessageId>> {
    with_store(&state, |s| s.search(&query))
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
/// the message isn't spam.
#[tauri::command]
async fn get_html(
    state: State<'_, AppState>,
    id: MessageId,
    load_images: Option<bool>,
) -> CmdResult<Option<HtmlView>> {
    let (raw, sender, trusted, auto) = with_store(&state, |s| {
        let sender = s.get(id)?.from_email;
        let trusted = s.is_sender_trusted(&sender)?;
        Ok((s.raw_of(id)?, sender, trusted, s.auto_load_images(id)?))
    })?;
    let Some(raw) = raw else { return Ok(None) };
    let Some(first) = html::render(&raw, &HashMap::new()) else {
        return Ok(None);
    };
    let (rendered, loaded) = if load_images.unwrap_or(auto) && !first.remote.is_empty() {
        let urls = first.remote.clone();
        let images = tauri::async_runtime::spawn_blocking(move || {
            html::fetch_images(&urls, &FetchLimits::default())
        })
        .await
        .map_err(|e| e.to_string())?;
        let loaded = images.len();
        (html::render(&raw, &images).unwrap_or(first), loaded)
    } else {
        (first, 0)
    };
    Ok(Some(HtmlView {
        document: rendered.document,
        blocked: rendered.blocked,
        loaded,
        sender,
        trusted,
    }))
}

/// Load a sender's remote images without asking from now on, or stop that.
#[tauri::command]
fn trust_sender(state: State<'_, AppState>, email: String, trusted: bool) -> CmdResult<()> {
    with_store(&state, |s| s.set_sender_trusted(&email, trusted))
}

/// Every rule, in the order they run.
#[tauri::command]
fn list_rules(state: State<'_, AppState>) -> CmdResult<Vec<Rule>> {
    with_store(&state, |s| s.rules())
}

/// Add (id 0) or update a rule. Mistakes come back as sentences for the editor.
#[tauri::command]
fn save_rule(state: State<'_, AppState>, rule: Rule) -> CmdResult<Rule> {
    with_store(&state, |s| s.save_rule(&rule))
}

#[tauri::command]
fn delete_rule(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    with_store(&state, |s| s.delete_rule(id))
}

/// Move a rule up (-1) or down (1).
#[tauri::command]
fn move_rule(state: State<'_, AppState>, id: i64, delta: i64) -> CmdResult<()> {
    with_store(&state, |s| s.move_rule(id, delta))
}

/// Run the rules now on every message in a folder. Auto-replies don't run here: they
/// answer new mail only.
#[tauri::command]
fn run_rules(state: State<'_, AppState>, folder: String) -> CmdResult<RunReport> {
    with_store(&state, |s| {
        let ids: Vec<MessageId> = s.list(&folder)?.iter().map(|m| m.id).collect();
        filters::run(s, &ids, Trigger::OnDemand, Local::now())
    })
}

/// Open a web link from a message in the default browser.
#[tauri::command]
fn open_link(app: AppHandle, url: String) -> CmdResult<()> {
    let url = url.trim();
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return Err("Telex opens only web links (http and https).".into());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| format!("Couldn't open the link: {e}"))
}

/// Check the settings by logging in to IMAP and SMTP, then save the account and its
/// password. The first account replaces the sample mailbox with the user's own mail base.
/// Adding an address that already exists updates it; an empty password then keeps the
/// saved one.
#[tauri::command]
async fn add_account(
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
        (true, false) => return Err("Enter the password.".into()),
    };
    let folders = imap::check_account(&setup.imap_settings(), &password)
        .await
        .map_err(|e| capitalize(&e.to_string()))?;
    smtp::check(&setup.smtp_settings(), &setup.username, &password)
        .await
        .map_err(|e| format!("IMAP works, but sending doesn't: {e}."))?;
    secrets::save_password(&setup.email, &password)?;
    with_store(&state, |s| {
        if s.is_sample() {
            *s = MailStore::open(&state.db_path)?;
        }
        let account = s.upsert_account(&setup)?;
        s.set_folders(&account.id, &imap::plan_folders(&folders))?;
        s.account(&account.id).cloned()
    })
}

/// Remove an account, its local mail and its saved password. The server is not touched.
#[tauri::command]
fn remove_account(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    if state.busy.load(Ordering::SeqCst) {
        return Err("Telex is talking to the mail server. Try again in a moment.".into());
    }
    let email = with_store(&state, |s| {
        let email = s.account(&id)?.email.clone();
        s.remove_account(&id)?;
        if s.accounts().is_empty() {
            *s = sample_store();
        }
        Ok(email)
    })?;
    secrets::delete_password(&email);
    Ok(())
}

#[derive(Debug, Default, Serialize)]
struct SyncOutcome {
    /// Ids of messages that arrived.
    new: Vec<MessageId>,
    /// One line per message sent from an Outbox: `"Re: Wycena" to Anna Kowalska`.
    sent: Vec<String>,
    /// New messages the rules changed.
    sorted: usize,
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

/// The accounts that live on a server; none while the sample mailbox is showing.
fn server_accounts(state: &AppState) -> CmdResult<Vec<Account>> {
    with_store(state, |s| {
        Ok(if s.is_sample() {
            Vec::new()
        } else {
            s.accounts()
                .iter()
                .filter(|a| a.imap.is_some())
                .cloned()
                .collect()
        })
    })
}

/// Full sync of every account, one after another: send queued changes, download new
/// mail and flag changes, send what changed meanwhile. Progress reaches the UI as
/// "sync-progress" events. One account failing doesn't stop the others.
#[tauri::command]
async fn check_mail(app: AppHandle, state: State<'_, AppState>) -> CmdResult<SyncOutcome> {
    let Some(_busy) = BusyGuard::take(&state.busy) else {
        return Ok(SyncOutcome::busy());
    };
    if with_store(&state, |s| Ok(s.is_sample()))? {
        let new = with_store(&state, |s| s.fetch_new())?;
        return Ok(SyncOutcome {
            new,
            ..SyncOutcome::default()
        });
    }
    let progress = |p: SyncProgress| {
        let _ = app.emit("sync-progress", p);
    };
    let mut outcome = SyncOutcome::default();
    for account in server_accounts(&state)? {
        // Send first, so the copies for Sent go to the server in this same sync.
        send_account_outbox(&state, &account, &mut outcome.sent, &mut outcome.errors).await;
        let result = match secrets::load_password(&account.email) {
            Ok(password) => imap::sync_account(&account, &password, &state.store, &progress)
                .await
                .map_err(|e| e.to_string()),
            Err(e) => Err(e),
        };
        match result {
            Ok(report) => {
                outcome.new.extend(report.new);
                outcome.sorted += report.sorted;
                let refused = report.refused.iter();
                outcome
                    .errors
                    .extend(refused.map(|r| format!("{}: {r}", account.name)));
                // Rules may have queued automatic replies: send them now, not next time.
                if report.answered > 0 {
                    send_account_outbox(&state, &account, &mut outcome.sent, &mut outcome.errors)
                        .await;
                }
            }
            Err(e) => outcome.errors.push(format!("{}: {e}", account.name)),
        }
    }
    Ok(outcome)
}

#[derive(Debug, Default, Serialize)]
struct SendOutcome {
    /// The message's id in the Outbox (or, for the sample mailbox, in Sent).
    queued: MessageId,
    /// One line per message the SMTP server accepted, this one and any that waited.
    sent: Vec<String>,
    errors: Vec<String>,
    /// The sample mailbox can't send; the message was only filed in Sent.
    sample: bool,
}

/// Send a message: build it, put it in the Outbox and send the Outbox right away. It
/// only counts as sent once the SMTP server accepts it; until then it waits in the
/// Outbox and goes out at the next check.
#[tauri::command]
async fn send_message(state: State<'_, AppState>, draft: OutgoingDraft) -> CmdResult<SendOutcome> {
    if with_store(&state, |s| Ok(s.is_sample()))? {
        let queued = with_store(&state, |s| {
            s.save_outgoing(draft, FolderKind::Sent, Local::now().naive_local())
        })?;
        return Ok(SendOutcome {
            queued,
            sample: true,
            ..SendOutcome::default()
        });
    }
    let (account, originals) = with_store(&state, |s| {
        let account = s.account(&draft.account)?.clone();
        let raw = |id: Option<MessageId>| id.map(|id| s.raw_of(id)).transpose();
        let originals = Originals {
            replied: raw(draft.reply_to)?.flatten(),
            forwarded: raw(draft.forward_of)?.flatten(),
        };
        Ok((account, originals))
    })?;
    let composed = smtp::compose(&account, &draft, &originals, Local::now())?;
    let queued = with_store(&state, |s| {
        s.queue_outgoing(
            &account.id,
            &composed.message,
            &composed.raw,
            &composed.env_from,
            &composed.env_to,
            draft.replaces,
        )
    })?;
    let mut outcome = SendOutcome {
        queued,
        ..SendOutcome::default()
    };
    send_account_outbox(&state, &account, &mut outcome.sent, &mut outcome.errors).await;
    Ok(outcome)
}

/// Send what waits in the account's Outbox. If another send is running, it picks up
/// new mail itself, so this one does nothing.
async fn send_account_outbox(
    state: &AppState,
    account: &Account,
    sent: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    let Some(_sending) = BusyGuard::take(&state.sending) else {
        return;
    };
    let has_mail = with_store(state, |s| s.has_outgoing(&account.id)).unwrap_or(false);
    if !has_mail {
        return;
    }
    let mut problem = |e: &str| errors.push(format!("{}: {e}", account.name));
    let Some(settings) = &account.smtp else {
        problem("no SMTP server is set, so the Outbox waits. Edit the account (under Add account) to add one");
        return;
    };
    let username = account
        .imap
        .as_ref()
        .map_or(account.email.as_str(), |i| i.username.as_str());
    let password = match secrets::load_password(&account.email) {
        Ok(p) => p,
        Err(e) => return problem(&e),
    };
    let transport = match smtp::transport(settings, username, &password) {
        Ok(t) => t,
        Err(SendError::Refused(e) | SendError::Failed(e)) => return problem(&e),
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
/// downloading. The UI calls this a few seconds after the user changes something.
#[tauri::command]
async fn push_changes(state: State<'_, AppState>) -> CmdResult<SyncOutcome> {
    let Some(_busy) = BusyGuard::take(&state.busy) else {
        return Ok(SyncOutcome::busy());
    };
    let mut outcome = SyncOutcome::default();
    for account in server_accounts(&state)? {
        let result = match secrets::load_password(&account.email) {
            Ok(password) => imap::push_account(&account, &password, &state.store)
                .await
                .map_err(|e| format!("{e}. Your changes are kept and sent at the next check")),
            Err(e) => Err(e),
        };
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
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db_path = dir.join("telex.db");
            app.manage(AppState {
                store: Mutex::new(open_store(&db_path)),
                db_path,
                busy: AtomicBool::new(false),
                sending: AtomicBool::new(false),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            get_message,
            set_unread,
            set_flagged,
            move_message,
            delete_message,
            save_outgoing,
            search,
            add_account,
            remove_account,
            check_mail,
            push_changes,
            send_message,
            get_html,
            trust_sender,
            open_link,
            list_rules,
            save_rule,
            delete_rule,
            move_rule,
            run_rules,
        ])
        .run(tauri::generate_context!())
        .expect("Telex failed to start");
}
