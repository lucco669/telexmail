//! Tauri shell: exposes `telex_core::MailStore` to the UI as commands.
//! Keep logic out of here; put it in `crates/telex-core` where it can be tested.

mod secrets;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use chrono::Local;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use telex_core::imap::{self, SyncProgress};
use telex_core::{
    sample::sample_store, Account, AccountSetup, DeleteOutcome, FolderKind, MailStore, Message,
    MessageId, OutgoingDraft, Snapshot, StoreError,
};

struct AppState {
    store: Mutex<MailStore>,
    /// The user's mail base: %APPDATA%\app.telexmail.desktop\telex.db.
    db_path: PathBuf,
    /// A mail check is running. A second one is refused instead of doubling the work.
    checking: AtomicBool,
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

/// `send: true` files the message in Sent, `false` in Drafts.
/// SMTP is not connected yet, so nothing actually leaves the computer.
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

/// Check the settings by logging in, then save the account and its password.
/// The first account replaces the sample mailbox with the user's own mail base.
/// Adding an address that already exists updates it (new server or password).
#[tauri::command]
async fn add_account(
    state: State<'_, AppState>,
    setup: AccountSetup,
    password: String,
) -> CmdResult<Account> {
    let setup = setup.cleaned()?;
    if password.is_empty() {
        return Err("Enter the password.".into());
    }
    let folders = imap::check_account(&setup.imap_settings(), &password)
        .await
        .map_err(|e| capitalize(&e.to_string()))?;
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
    if state.checking.load(Ordering::SeqCst) {
        return Err("Telex is checking mail. Try again when it has finished.".into());
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
struct CheckReport {
    /// Ids of messages that arrived.
    new: Vec<MessageId>,
    /// One line per account that failed, e.g. "Studio: can't reach mail.x.pl: …".
    errors: Vec<String>,
}

/// Clears the "checking" flag however the check ends, even on an early return.
struct CheckingGuard<'a>(&'a AtomicBool);

impl Drop for CheckingGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Download new mail for every account, one after another. Progress reaches the UI
/// as "sync-progress" events. One account failing doesn't stop the others.
#[tauri::command]
async fn check_mail(app: AppHandle, state: State<'_, AppState>) -> CmdResult<CheckReport> {
    if state.checking.swap(true, Ordering::SeqCst) {
        return Err("Already checking mail.".into());
    }
    let _guard = CheckingGuard(&state.checking);
    let accounts = with_store(&state, |s| {
        Ok((!s.is_sample()).then(|| s.accounts().to_vec()))
    })?;
    let Some(accounts) = accounts else {
        let new = with_store(&state, |s| s.fetch_new())?;
        return Ok(CheckReport {
            new,
            errors: Vec::new(),
        });
    };
    let progress = |p: SyncProgress| {
        let _ = app.emit("sync-progress", p);
    };
    let mut report = CheckReport::default();
    for account in accounts.iter().filter(|a| a.imap.is_some()) {
        let result = match secrets::load_password(&account.email) {
            Ok(password) => imap::sync_account(account, &password, &state.store, &progress)
                .await
                .map_err(|e| e.to_string()),
            Err(e) => Err(e),
        };
        match result {
            Ok(ids) => report.new.extend(ids),
            Err(e) => report.errors.push(format!("{}: {e}", account.name)),
        }
    }
    Ok(report)
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db_path = dir.join("telex.db");
            app.manage(AppState {
                store: Mutex::new(open_store(&db_path)),
                db_path,
                checking: AtomicBool::new(false),
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
        ])
        .run(tauri::generate_context!())
        .expect("Telex failed to start");
}
