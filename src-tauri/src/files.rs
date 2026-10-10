//! Attachments and files: opening and saving the attachments of stored mail, and choosing
//! files to attach (Telex's Open dialog, or dropping them on the window). The logic is in
//! `telex_core::attachments`; this is the dialogs and the system's default apps.
//!
//! The window only ever names files to attach that came from here: a path from the dialog
//! or a drop is remembered, and sending refuses any other.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tauri_plugin_dialog::{DialogExt, FilePath};
use tauri_plugin_opener::OpenerExt;
use telex_core::attachments::{self, is_risky};
use telex_core::{tr, Attachment, FolderKind, MessageId};

use crate::{with_store, AppState, CmdResult};

/// Files the user chose this session, and the last place attachments were saved to.
#[derive(Default)]
pub struct FilesState {
    picked: Mutex<HashSet<PathBuf>>,
    saved: Mutex<Option<PathBuf>>,
}

impl FilesState {
    /// A file to attach, now that the user chose it: its name and size for the compose pane.
    fn remember(&self, path: &Path) -> Option<Attachment> {
        let meta = std::fs::metadata(path).ok().filter(|m| m.is_file())?;
        let name = path.file_name()?.to_string_lossy().into_owned();
        if let Ok(mut picked) = self.picked.lock() {
            picked.insert(path.to_path_buf());
        }
        Some(Attachment {
            name,
            size_kb: u32::try_from(meta.len().div_ceil(1024))
                .unwrap_or(u32::MAX)
                .max(1),
            path: Some(path.display().to_string()),
        })
    }

    /// Every file a draft attaches from this PC was chosen in Telex.
    pub fn check(&self, attachments: &[Attachment]) -> CmdResult<()> {
        let picked = self.picked.lock().map_err(|_| poisoned())?;
        for a in attachments {
            if let Some(path) = &a.path {
                if !picked.contains(Path::new(path)) {
                    let name = &a.name;
                    return Err(tr!(
                        "Attach \"{name}\" again: Telex attaches only files chosen in it.",
                        "Dołącz plik „{name}” jeszcze raz: Telex dołącza tylko pliki wybrane w nim."
                    ));
                }
            }
        }
        Ok(())
    }
}

fn poisoned() -> String {
    tr!(
        "Telex stopped responding. Restart it.",
        "Telex przestał odpowiadać. Uruchom go ponownie."
    )
}

/// What trying to open an attachment did.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Opening {
    /// It opened in the system's app for its kind.
    Opened,
    /// It can run programs: the window asks first, then calls again with `sure`.
    Risky,
    /// It came in Spam: the window asks first.
    Spam,
}

/// Open an attachment in the app Windows (or Linux) uses for its kind. It is written to a
/// folder of Telex's under the system's temporary folder first, marked as coming from the
/// internet. Files that can run code, and anything in Spam, wait for `sure`.
#[tauri::command]
pub fn open_attachment(
    app: AppHandle,
    state: State<'_, AppState>,
    id: MessageId,
    index: usize,
    sure: bool,
) -> CmdResult<Opening> {
    let (file, spam) = with_store(&state, |s| {
        let spam = s
            .folder_of(id)?
            .and_then(|f| s.kind_of(&f))
            .is_some_and(|k| k == FolderKind::Spam);
        Ok((s.attachment(id, index)?, spam))
    })?;
    if !sure && is_risky(&file.name) {
        return Ok(Opening::Risky);
    }
    if !sure && spam {
        return Ok(Opening::Spam);
    }
    let dir = attachments::opened_dir(&std::env::temp_dir(), id);
    let path = attachments::write_file(&dir, &file, true).map_err(|e| {
        tr!(
            "Couldn't open the attachment: {e}.",
            "Nie udało się otworzyć załącznika: {e}."
        )
    })?;
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(|e| {
            tr!(
                "Couldn't open the attachment: {e}.",
                "Nie udało się otworzyć załącznika: {e}."
            )
        })?;
    Ok(Opening::Opened)
}

/// The words of a Save or Open dialog, in the interface's language.
#[derive(Debug, Deserialize)]
pub struct DialogWords {
    title: String,
}

/// Save one attachment where the user picks. `null` when they cancel.
#[tauri::command]
pub async fn save_attachment(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    files: State<'_, FilesState>,
    id: MessageId,
    index: usize,
    words: DialogWords,
) -> CmdResult<Option<String>> {
    let file = with_store(&state, |s| s.attachment(id, index))?;
    let (tx, rx) = mpsc::channel();
    app.dialog()
        .file()
        .set_parent(&window)
        .set_title(words.title)
        .set_file_name(attachments::safe_file_name(&file.name))
        .save_file(move |path| {
            let _ = tx.send(path);
        });
    let Some(path) = picked(rx).await else {
        return Ok(None);
    };
    attachments::save_as(&path, &file).map_err(|e| {
        let shown = path.display();
        tr!(
            "Couldn't save {shown}: {e}.",
            "Nie udało się zapisać {shown}: {e}."
        )
    })?;
    *files.saved.lock().map_err(|_| poisoned())? = Some(path.clone());
    Ok(Some(path.display().to_string()))
}

/// Save every attachment of a message into a folder the user picks; a file of the same name
/// already there is kept and the new one numbered. The folder, or `null` when cancelled.
#[tauri::command]
pub async fn save_attachments(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    files: State<'_, FilesState>,
    id: MessageId,
    words: DialogWords,
) -> CmdResult<Option<String>> {
    let all = with_store(&state, |s| s.attachments(id))?;
    let (tx, rx) = mpsc::channel();
    app.dialog()
        .file()
        .set_parent(&window)
        .set_title(words.title)
        .pick_folder(move |path| {
            let _ = tx.send(path);
        });
    let Some(dir) = picked(rx).await else {
        return Ok(None);
    };
    let mut last = None;
    for file in &all {
        last = Some(attachments::write_file(&dir, file, false).map_err(|e| {
            let name = &file.name;
            tr!(
                "Couldn't save \"{name}\": {e}.",
                "Nie udało się zapisać pliku „{name}”: {e}."
            )
        })?);
    }
    *files.saved.lock().map_err(|_| poisoned())? = last;
    Ok(Some(dir.display().to_string()))
}

/// Show the last saved attachment in Explorer (or the file manager).
#[tauri::command]
pub fn show_saved(app: AppHandle, files: State<'_, FilesState>) -> CmdResult<()> {
    let Some(path) = files.saved.lock().map_err(|_| poisoned())?.clone() else {
        return Ok(());
    };
    app.opener().reveal_item_in_dir(&path).map_err(|e| {
        tr!(
            "Couldn't open the folder: {e}",
            "Nie udało się otworzyć folderu: {e}"
        )
    })
}

/// Choose files to attach. Empty when cancelled.
#[tauri::command]
pub async fn pick_files(
    app: AppHandle,
    window: WebviewWindow,
    files: State<'_, FilesState>,
    words: DialogWords,
) -> CmdResult<Vec<Attachment>> {
    let (tx, rx) = mpsc::channel();
    app.dialog()
        .file()
        .set_parent(&window)
        .set_title(words.title)
        .pick_files(move |paths| {
            let _ = tx.send(paths);
        });
    let paths = tauri::async_runtime::spawn_blocking(move || rx.recv().ok().flatten())
        .await
        .ok()
        .flatten()
        .unwrap_or_default();
    Ok(paths
        .into_iter()
        .filter_map(|p| p.into_path().ok())
        .filter_map(|p| files.remember(&p))
        .collect())
}

/// Files dropped on the window: remembered, and handed to the window as attachments in a
/// "files-dropped" event. Folders are left out.
pub fn dropped(app: &AppHandle, paths: &[PathBuf]) {
    let files = app.state::<FilesState>();
    let attachments: Vec<Attachment> = paths.iter().filter_map(|p| files.remember(p)).collect();
    if !attachments.is_empty() {
        let _ = app.emit("files-dropped", attachments);
    }
}

/// Waits for a dialog without holding up the async runtime.
async fn picked(rx: mpsc::Receiver<Option<FilePath>>) -> Option<PathBuf> {
    tauri::async_runtime::spawn_blocking(move || rx.recv().ok().flatten())
        .await
        .ok()
        .flatten()
        .and_then(|p| p.into_path().ok())
}
