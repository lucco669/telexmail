//! Tauri shell: exposes `telex_core::MailStore` to the UI as commands.
//! Keep logic out of here; put it in `crates/telex-core` where it can be tested.

use std::sync::Mutex;

use chrono::Local;
use tauri::State;
use telex_core::{
    sample::sample_store, DeleteOutcome, FolderKind, MailStore, Message, MessageId, OutgoingDraft,
    Snapshot, StoreError,
};

struct AppState(Mutex<MailStore>);

/// Errors cross the IPC bridge as plain, user-readable strings.
type CmdResult<T> = Result<T, String>;

fn with_store<T>(
    state: &State<'_, AppState>,
    f: impl FnOnce(&mut MailStore) -> Result<T, StoreError>,
) -> CmdResult<T> {
    let mut store = state
        .0
        .lock()
        .map_err(|_| "The mail store stopped responding. Restart Telex.".to_string())?;
    f(&mut store).map_err(|e| e.to_string())
}

/// Accounts, folders and every message summary, for the first paint and after bulk changes.
#[tauri::command]
fn snapshot(state: State<'_, AppState>) -> CmdResult<Snapshot> {
    with_store(&state, |s| Ok(s.snapshot()))
}

#[tauri::command]
fn get_message(state: State<'_, AppState>, id: MessageId) -> CmdResult<Message> {
    with_store(&state, |s| s.get(id).cloned())
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

/// Stand-in for an IMAP check: returns the ids of messages that just arrived.
#[tauri::command]
fn check_mail(state: State<'_, AppState>) -> CmdResult<Vec<MessageId>> {
    with_store(&state, |s| Ok(s.fetch_new()))
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState(Mutex::new(sample_store())))
        .invoke_handler(tauri::generate_handler![
            snapshot,
            get_message,
            set_unread,
            set_flagged,
            move_message,
            delete_message,
            save_outgoing,
            check_mail,
        ])
        .run(tauri::generate_context!())
        .expect("Telex failed to start");
}
