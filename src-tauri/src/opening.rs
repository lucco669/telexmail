//! Opening the mail base at start without holding up the window, and the work an upgrade
//! leaves for later (telex-core's upkeep.rs).
//!
//! The store used to be opened in `setup`, on the window's thread before its event loop runs:
//! a long schema upgrade froze the window (10–15 s after 0.9.20's). Now a thread of its own
//! opens it while holding the store's lock, so every command that needs the mail waits for it
//! on the async pool, and the window asks `upgrade_state` how far an upgrade is.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use telex_core::Upgrade;

use crate::{open_store, send_timer, watch, with_store, AppState};

/// Where opening the mail base is: `Some` until it is open, with the upgrade once one runs.
#[derive(Default)]
pub struct Opening(Mutex<Option<Option<Upgrade>>>);

/// What the window shows while the mail base is being upgraded.
#[derive(Debug, Serialize)]
pub struct UpgradeState {
    /// The Telex doing the upgrade.
    version: String,
    done: usize,
    /// 0 until an upgrade is known to run (most starts have none).
    steps: usize,
}

/// Open the mail base on a thread of its own; returns once that thread holds the store, so no
/// command gets the empty store meanwhile. Then the watchers, the send timer and the upkeep.
pub fn start(app: &AppHandle) {
    *app.state::<Opening>()
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(None);
    let (holding, held) = mpsc::channel();
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let opening = app.state::<Opening>();
        let set = |now: Option<Option<Upgrade>>| {
            *opening.0.lock().unwrap_or_else(|e| e.into_inner()) = now;
        };
        let problem = {
            let Ok(mut store) = state.store.lock() else {
                let _ = holding.send(());
                set(None);
                return;
            };
            let _ = holding.send(());
            let (opened, problem) = open_store(&state.db_path, |u| set(Some(Some(u))));
            *store = opened;
            problem
        };
        set(None);
        if let Some(problem) = problem {
            app.dialog()
                .message(problem)
                .kind(MessageDialogKind::Error)
                .title("Telex")
                .show(|_| {});
        }
        watch::restart(&app);
        send_timer(app.clone());
        upkeep(&app);
    });
    let _ = held.recv();
}

/// How far opening the mail base is, or null once it is open. Not on the async pool, unlike
/// the other commands: it reads only `Opening`, never waits for the store, and so can't be
/// stuck behind commands that wait for the store there.
#[tauri::command]
pub fn upgrade_state(app: AppHandle, opening: State<'_, Opening>) -> Option<UpgradeState> {
    let now = *opening.0.lock().unwrap_or_else(|e| e.into_inner());
    now.map(|upgrade| UpgradeState {
        version: app.package_info().version.to_string(),
        done: upgrade.map_or(0, |u| u.done),
        steps: upgrade.map_or(0, |u| u.steps),
    })
}

/// One upkeep step's length: the window's commands wait at most about this long for the store.
const STEP: Duration = Duration::from_millis(40);
/// The pause between steps, so the window's commands get the store.
const PAUSE: Duration = Duration::from_millis(20);
/// How often the window hears how far it is ("upkeep-progress").
const TELL_EVERY: Duration = Duration::from_millis(500);

/// Run what upgrades left for later, a step at a time, until it is done. Started after the
/// mail base opens and after a restore (an older backup may bring work of its own); a second
/// start while one runs does nothing. An error stops it until the next start, where it goes on.
pub fn upkeep(app: &AppHandle) {
    static RUNNING: AtomicBool = AtomicBool::new(false);
    if RUNNING.swap(true, Ordering::AcqRel) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let mut told: Option<Instant> = None;
        loop {
            match with_store(&state, |s| s.upkeep_step(STEP)) {
                Ok(Some(left)) => {
                    if told.is_none_or(|t| t.elapsed() >= TELL_EVERY) {
                        let _ = app.emit("upkeep-progress", Some(left));
                        told = Some(Instant::now());
                    }
                }
                Ok(None) | Err(_) => {
                    if told.is_some() {
                        let _ = app.emit("upkeep-progress", None::<telex_core::upkeep::Progress>);
                    }
                    break;
                }
            }
            std::thread::sleep(PAUSE);
        }
        RUNNING.store(false, Ordering::Release);
    });
}
