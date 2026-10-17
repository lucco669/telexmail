//! Backups of the mail base, putting one back, and compacting it: the commands, their dialogs,
//! and the moments they wait for. The logic is in `telex_core::backup`.
//!
//! A backup is read on a connection of its own, so Telex keeps working while it is written.
//! Restoring and compacting need the mail base to themselves: they wait until no check, push
//! or send is running and hold those off meanwhile, as installing an update does.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::mpsc;

use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use telex_core::backup::{self, BackupFile, BackupInfo};
use telex_core::{tr, MailStore};

use crate::settings::SettingsState;
use crate::{failed, open_store, watch, with_store, AppState, BusyGuard, CmdResult};

/// What Settings › Backups shows.
#[derive(Debug, Serialize)]
pub struct BackupState {
    /// The mail base: the file, how large it is, and how much of it is free space that
    /// compacting would give back. Empty `base` before the first account.
    base: String,
    base_bytes: u64,
    free_bytes: u64,
    /// Where backups go: the setting, or a suggestion while it is unset (`suggested`).
    dir: String,
    suggested: bool,
    /// The backups there, newest first.
    backups: Vec<BackupFile>,
    /// A backup is being written now.
    running: bool,
}

/// Where backups go: the chosen folder, else Documents\Telex backups.
fn backup_dir(app: &AppHandle) -> (PathBuf, bool) {
    let chosen = app.state::<SettingsState>().get().backup_dir;
    if !chosen.trim().is_empty() {
        return (PathBuf::from(chosen.trim()), false);
    }
    let documents = app
        .path()
        .document_dir()
        .unwrap_or_else(|_| std::env::temp_dir());
    (documents.join("Telex backups"), true)
}

fn busy_text() -> String {
    tr!(
        "Telex is talking to the mail server. Try again in a moment.",
        "Telex rozmawia z serwerem poczty. Spróbuj za chwilę."
    )
}

#[tauri::command(async)]
pub fn backup_state(app: AppHandle, state: State<'_, AppState>) -> CmdResult<BackupState> {
    let (base_bytes, free_bytes, on_disk) = with_store(&state, |s| {
        let (pages, free, size) = s.page_counts()?;
        Ok((pages * size, free * size, !s.is_in_memory()))
    })?;
    let (dir, suggested) = backup_dir(&app);
    Ok(BackupState {
        base: if on_disk {
            state.db_path.display().to_string()
        } else {
            String::new()
        },
        base_bytes,
        free_bytes,
        backups: backup::list(&dir),
        dir: dir.display().to_string(),
        suggested,
        running: state.backing_up.load(Ordering::SeqCst),
    })
}

/// Write a backup and keep only the newest ones the settings say.
fn back_up(app: &AppHandle) -> CmdResult<BackupFile> {
    let state = app.state::<AppState>();
    if with_store(&state, |s| Ok(s.is_in_memory()))? {
        return Err(tr!(
            "There is nothing to back up yet: add a mail account first.",
            "Nie ma jeszcze czego kopiować: najpierw dodaj konto pocztowe."
        ));
    }
    let Some(_running) = BusyGuard::take(&state.backing_up) else {
        return Err(tr!(
            "A backup is being made already.",
            "Kopia zapasowa właśnie powstaje."
        ));
    };
    let (dir, _) = backup_dir(app);
    let secret_keys = app.state::<SettingsState>().get().backup_secret_keys;
    let made = backup::write_keys(
        &state.db_path,
        &dir,
        Local::now().naive_local(),
        secret_keys,
    )
    .map_err(|e| {
        let shown = dir.display();
        tr!(
            "Couldn't back up to {shown}: {e}.",
            "Nie udało się zrobić kopii w {shown}: {e}."
        )
    })?;
    backup::prune(
        &dir,
        usize::from(app.state::<SettingsState>().get().backup_keep),
    );
    Ok(made)
}

/// Back up now, into the backup folder.
#[tauri::command]
pub async fn backup_now(app: AppHandle) -> CmdResult<BackupFile> {
    tauri::async_runtime::spawn_blocking(move || back_up(&app))
        .await
        .map_err(failed)?
}

/// Back up when the settings say one is due. `null` when none was.
#[tauri::command]
pub async fn auto_backup(app: AppHandle) -> CmdResult<Option<BackupFile>> {
    let settings = app.state::<SettingsState>().get();
    let (dir, _) = backup_dir(&app);
    let now = Local::now().naive_local();
    let empty = with_store(&app.state::<AppState>(), |s| Ok(s.is_in_memory()))?;
    if empty || !backup::due(&dir, settings.backup_every.period(), now) {
        return Ok(None);
    }
    tauri::async_runtime::spawn_blocking(move || back_up(&app).map(Some))
        .await
        .map_err(failed)?
}

/// The words of a dialog, in the interface's language.
#[derive(Debug, Deserialize)]
pub struct DialogWords {
    title: String,
    #[serde(default)]
    filter: String,
}

/// Choose the folder backups go to. The folder, or `null` when cancelled; the window saves
/// it as a setting.
#[tauri::command]
pub async fn pick_backup_dir(
    app: AppHandle,
    window: WebviewWindow,
    words: DialogWords,
) -> CmdResult<Option<String>> {
    let (dir, _) = backup_dir(&app);
    let (tx, rx) = mpsc::channel();
    let start = if dir.exists() {
        dir
    } else {
        dir.parent().map(Path::to_path_buf).unwrap_or(dir)
    };
    app.dialog()
        .file()
        .set_parent(&window)
        .set_title(words.title)
        .set_directory(start)
        .pick_folder(move |path| {
            let _ = tx.send(path);
        });
    Ok(crate::files::picked(rx)
        .await
        .map(|p| p.display().to_string()))
}

/// What a backup holds: one of those in the backup folder (`path`), or one chosen in Open.
/// `null` when cancelled.
#[tauri::command]
pub async fn inspect_backup(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    path: Option<String>,
    words: DialogWords,
) -> CmdResult<Option<BackupInfo>> {
    let path = match path {
        Some(path) => {
            let (dir, _) = backup_dir(&app);
            let listed = backup::list(&dir).into_iter().any(|b| b.path == path);
            if !listed {
                return Err(tr!(
                    "That backup is no longer there.",
                    "Tej kopii już tam nie ma."
                ));
            }
            PathBuf::from(path)
        }
        None => {
            let (tx, rx) = mpsc::channel();
            let (dir, _) = backup_dir(&app);
            let mut dialog = app
                .dialog()
                .file()
                .set_parent(&window)
                .set_title(words.title)
                .add_filter(words.filter, &["db"]);
            if dir.exists() {
                dialog = dialog.set_directory(dir);
            }
            dialog.pick_file(move |path| {
                let _ = tx.send(path);
            });
            match crate::files::picked(rx).await {
                Some(path) => path,
                None => return Ok(None),
            }
        }
    };
    if path == state.db_path {
        return Err(tr!(
            "That is the mail base itself, not a backup.",
            "To sama baza poczty, nie kopia zapasowa."
        ));
    }
    let info = backup::inspect(&path).map_err(|e| e.to_string())?;
    *state.restorable.lock().map_err(|_| failed("lock"))? = Some(path);
    Ok(Some(info))
}

/// Put the backup last inspected in place of the mail base. The mail base as it was is kept
/// next to it; its name comes back. The window then starts again on the restored mail.
#[tauri::command]
pub async fn restore_backup(app: AppHandle, path: String) -> CmdResult<String> {
    let handle = app.clone();
    let aside = tauri::async_runtime::spawn_blocking(move || {
        let state = handle.state::<AppState>();
        let from = state
            .restorable
            .lock()
            .map_err(|_| failed("lock"))?
            .clone()
            .filter(|p| p.display().to_string() == path)
            .ok_or_else(|| failed("choose the backup again"))?;
        // Nothing may talk to the servers or write a backup meanwhile.
        let Some(_busy) = BusyGuard::take(&state.busy) else {
            return Err(busy_text());
        };
        let Ok(_sending) = state.sending.try_lock() else {
            return Err(busy_text());
        };
        let Some(_backing) = BusyGuard::take(&state.backing_up) else {
            return Err(busy_text());
        };
        let mut store = state.store.lock().map_err(|_| failed("lock"))?;
        // Let go of the file: an empty store in memory meanwhile.
        *store = MailStore::in_memory().map_err(|e| e.to_string())?;
        let result = backup::restore(&state.db_path, &from, Local::now().naive_local());
        // Whatever happened, open what is there now: the restored mail, or the old one.
        let (opened, problem) = open_store(&state.db_path);
        *store = opened;
        let aside = result.map_err(|e| e.to_string())?;
        problem.map_or(Ok(aside), Err)
    })
    .await
    .map_err(failed)??;
    watch::restart(&app);
    Ok(aside.display().to_string())
}

/// Give the space of removed mail back to the disk. The sizes before and after, in bytes.
#[tauri::command]
pub async fn compact_base(app: AppHandle) -> CmdResult<(u64, u64)> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let Some(_busy) = BusyGuard::take(&state.busy) else {
            return Err(busy_text());
        };
        let Some(_backing) = BusyGuard::take(&state.backing_up) else {
            return Err(busy_text());
        };
        with_store(&state, |s| s.compact())
    })
    .await
    .map_err(failed)?
}

/// Show the backup folder, or the mail base, in Explorer (or the file manager).
#[tauri::command(async)]
pub fn show_backup_place(app: AppHandle, state: State<'_, AppState>, base: bool) -> CmdResult<()> {
    let opened = if base {
        app.opener().reveal_item_in_dir(&state.db_path)
    } else {
        let (dir, _) = backup_dir(&app);
        match backup::list(&dir).first() {
            Some(newest) => app.opener().reveal_item_in_dir(&newest.path),
            None => app
                .opener()
                .open_path(dir.display().to_string(), None::<&str>),
        }
    };
    opened.map_err(|e| {
        tr!(
            "Couldn't open the folder: {e}",
            "Nie udało się otworzyć folderu: {e}"
        )
    })
}
