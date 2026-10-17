//! New mail at once: one connection per server account waits in IMAP IDLE on the Inbox
//! (`telex_core::imap::watch_inbox`) and tells the window when something changes there with
//! an "inbox-changed" event; the window then checks that Inbox. A broken connection is tried
//! again after a pause that grows to ten minutes; a server without IDLE is left to the
//! regular checks. Watchers on one server take turns (`telex_core::servers`): a few watch at
//! once, the rest wait for a place, and a server that stopped answering rests before anyone
//! connects again. And a notification for new mail while the window isn't the active one.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use telex_core::i18n::{self, Lang};
use telex_core::imap::{self, SyncError, Watched};
use telex_core::{Account, FolderKind, MessageId};

use crate::settings::SettingsState;
use crate::{secrets, server_accounts, with_store, AppState};

/// The running watchers, one per account.
#[derive(Default)]
pub struct Watchers(Mutex<Vec<JoinHandle<()>>>);

/// Stop every watcher, then start one per server account if the settings want new mail at
/// once. Called at start, after an account is added or removed, and when the setting changes.
pub fn restart(app: &AppHandle) {
    let watchers = app.state::<Watchers>();
    let Ok(mut running) = watchers.0.lock() else {
        return;
    };
    for watcher in running.drain(..) {
        watcher.abort();
    }
    if !app.state::<SettingsState>().get().push_mail {
        return;
    }
    let accounts = server_accounts(&app.state::<AppState>()).unwrap_or_default();
    for account in accounts {
        let app = app.clone();
        running.push(tauri::async_runtime::spawn(watch(app, account)));
    }
}

async fn watch(app: AppHandle, account: Account) {
    let Some(settings) = account.imap.clone() else {
        return;
    };
    let first_pause = Duration::from_secs(30);
    let mut pause = first_pause;
    loop {
        // No password (yet): the regular checks say so; watching waits.
        if let Ok(password) = secrets::load_password(&account.email) {
            let id = account.id.clone();
            let notice = app.clone();
            let changed = move || {
                let _ = notice.emit("inbox-changed", &id);
            };
            let started = Instant::now();
            match imap::watch_inbox(&settings, &password, &changed).await {
                Ok(Watched::NoIdle) => return,
                // A rejected password stays rejected: logging in again and again can get the
                // address banned. Saving the account again restarts the watcher.
                Err(SyncError::Login(_)) => return,
                // The server is resting: try once its rest is over, not before.
                Err(SyncError::Resting(until)) => {
                    let left = (until - chrono::Local::now()).to_std().unwrap_or_default();
                    pause = pause.max(left + Duration::from_secs(5));
                }
                // A connection that held for a while was fine: start over with short pauses.
                Err(_) if started.elapsed() > Duration::from_secs(300) => pause = first_pause,
                Err(_) => {}
            }
        }
        tokio::time::sleep(pause).await;
        pause = (pause * 2).min(Duration::from_secs(600));
    }
}

/// A notification for new mail in an Inbox, when the window isn't the active one and the
/// settings want it: the sender and subject of one message, or how many and from whom.
pub fn announce(app: &AppHandle, arrived: &[MessageId]) {
    if arrived.is_empty() || !app.state::<SettingsState>().get().notify {
        return;
    }
    let focused = app
        .get_webview_window("main")
        .and_then(|w| w.is_focused().ok())
        .unwrap_or(false);
    if focused {
        return;
    }
    let state = app.state::<AppState>();
    let found = with_store(&state, |s| {
        let mut found = Vec::new();
        for &id in arrived.iter().rev().take(50) {
            let Ok(m) = s.get(id) else { continue };
            if s.kind_of(&m.folder) == Some(FolderKind::Inbox) {
                found.push((m.from_name, m.subject));
            }
        }
        Ok(found)
    })
    .unwrap_or_default();
    let (title, body) = match found.as_slice() {
        [] => return,
        [(from, subject)] => (from.clone(), subject.clone()),
        more => {
            let n = more.len();
            let title = match (i18n::current(), i18n::pl_form(n as u64)) {
                (Lang::Pl, 1) => format!("{n} nowe wiadomości"),
                (Lang::Pl, _) => format!("{n} nowych wiadomości"),
                (Lang::En, _) => format!("{n} new messages"),
            };
            let mut senders: Vec<&str> = Vec::new();
            for (from, _) in more {
                if !senders.contains(&from.as_str()) {
                    senders.push(from);
                }
            }
            let mut body = senders
                .iter()
                .take(3)
                .copied()
                .collect::<Vec<_>>()
                .join(", ");
            if senders.len() > 3 {
                body.push('…');
            }
            (title, body)
        }
    };
    let _ = app.notification().builder().title(title).body(body).show();
}
