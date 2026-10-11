//! Folders on this PC, and mail in and out of Telex as files: a folder saved as an mbox file,
//! .eml and mbox files read into a folder on this PC (chosen in Open, or dropped on it). The
//! logic is in `telex_core::{store, mailbox}`; this is the commands and their dialogs.

use std::path::PathBuf;
use std::sync::mpsc;

use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;
use telex_core::mailbox::{self, ImportProgress, ImportReport};
use telex_core::{tr, StoreError};

use crate::files::FilesState;
use crate::{with_store, AppState, CmdResult};

/// Make a folder on this PC. Returns its key.
#[tauri::command]
pub fn create_local_folder(state: State<'_, AppState>, name: String) -> CmdResult<String> {
    with_store(&state, |s| s.create_local_folder(&name))
}

/// Give a folder on this PC another name.
#[tauri::command]
pub fn rename_local_folder(
    state: State<'_, AppState>,
    folder: String,
    name: String,
) -> CmdResult<()> {
    with_store(&state, |s| s.rename_local_folder(&folder, &name))
}

/// Remove an empty folder on this PC. Returns how many rules lost their move to it.
#[tauri::command]
pub fn delete_local_folder(state: State<'_, AppState>, folder: String) -> CmdResult<usize> {
    with_store(&state, |s| s.delete_local_folder(&folder))
}

/// The words of the dialogs, in the interface's language: the title, the name of the kind of
/// file, and for Save the file name to suggest.
#[derive(Debug, Deserialize)]
pub struct FileWords {
    title: String,
    filter: String,
    #[serde(default)]
    all: String,
    #[serde(default)]
    file_name: String,
}

fn failed(e: impl std::fmt::Display) -> String {
    tr!("It didn't work: {e}", "Nie udało się: {e}")
}

/// Save a folder as an mbox file where the user picks. It is read on a connection of its
/// own, so Telex keeps working meanwhile. The path written, or `null` when cancelled.
#[tauri::command]
pub async fn export_folder(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    files: State<'_, FilesState>,
    folder: String,
    words: FileWords,
) -> CmdResult<Option<String>> {
    let on_disk = with_store(&state, |s| {
        s.folder_name(&folder)
            .ok_or_else(|| StoreError::UnknownFolder(folder.clone()))?;
        Ok(!s.is_in_memory())
    })?;
    if !on_disk {
        return Err(tr!(
            "There is no mail to save yet.",
            "Nie ma jeszcze poczty do zapisania."
        ));
    }
    let (tx, rx) = mpsc::channel();
    app.dialog()
        .file()
        .set_parent(&window)
        .set_title(words.title)
        .set_file_name(mailbox::file_name_for(&words.file_name))
        .add_filter(words.filter, &["mbox"])
        .save_file(move |path| {
            let _ = tx.send(path);
        });
    let Some(dest) = crate::files::picked(rx).await else {
        return Ok(None);
    };
    let db = state.db_path.clone();
    let to = dest.clone();
    tauri::async_runtime::spawn_blocking(move || mailbox::export_folder(&db, &folder, &to))
        .await
        .map_err(failed)?
        .map_err(|e| e.to_string())?;
    files.remember_saved(&dest);
    Ok(Some(dest.display().to_string()))
}

/// Read .eml and mbox files into a folder on this PC: `paths` dropped on it this session, or
/// else chosen in Open. Progress comes as "import-progress" events. What happened, or `null`
/// when cancelled.
#[tauri::command]
pub async fn import_mail(
    app: AppHandle,
    window: WebviewWindow,
    files: State<'_, FilesState>,
    folder: String,
    paths: Option<Vec<String>>,
    words: FileWords,
) -> CmdResult<Option<ImportReport>> {
    let paths: Vec<PathBuf> = match paths {
        Some(paths) => {
            let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
            files.check_paths(&paths)?;
            paths
        }
        None => {
            let (tx, rx) = mpsc::channel();
            app.dialog()
                .file()
                .set_parent(&window)
                .set_title(words.title)
                .add_filter(words.filter, &["eml", "mbox", "mbx", "msg"])
                .add_filter(words.all, &["*"])
                .pick_files(move |paths| {
                    let _ = tx.send(paths);
                });
            let picked = tauri::async_runtime::spawn_blocking(move || rx.recv().ok().flatten())
                .await
                .ok()
                .flatten()
                .unwrap_or_default();
            picked
                .into_iter()
                .filter_map(|p| p.into_path().ok())
                .collect()
        }
    };
    if paths.is_empty() {
        return Ok(None);
    }
    let handle = app.clone();
    let report = tauri::async_runtime::spawn_blocking(move || {
        let state = handle.state::<AppState>();
        let progress = |p: ImportProgress| {
            let _ = handle.emit_to(window.label(), "import-progress", p);
        };
        mailbox::import_files(&state.store, &folder, &paths, &progress)
    })
    .await
    .map_err(failed)?
    .map_err(|e| e.to_string())?;
    Ok(Some(report))
}
