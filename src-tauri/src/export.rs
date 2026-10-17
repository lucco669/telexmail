//! A whole message out of Telex: saved as .eml (Save as…, or dragged out of the list onto
//! Explorer or the desktop), printed or saved as PDF, and its source shown. The logic is in
//! `telex_core::export`; this is the dialogs and the platform parts (`native`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;
use telex_core::attachments;
use telex_core::export::{self, Eml, Source};
use telex_core::html::{self, FetchLimits};
use telex_core::{tr, MessageId};

use crate::files::{picked, FilesState};
use crate::{failed, native, with_store, AppState, CmdResult};

/// The words of the Save dialogs, in the interface's language: the title and the name of the
/// kind of file.
#[derive(Debug, Deserialize)]
pub struct SaveWords {
    title: String,
    filter: String,
}

/// At most this many messages go out at once, saved or dragged.
const MANY: usize = 1000;

fn too_many(ids: &[MessageId]) -> CmdResult<()> {
    if ids.len() > MANY {
        let n = MANY;
        return Err(tr!(
            "Save up to {n} messages at a time.",
            "Zapisuj najwyżej {n} wiadomości naraz."
        ));
    }
    Ok(())
}

fn emls(state: &AppState, ids: &[MessageId]) -> CmdResult<Vec<Eml>> {
    with_store(state, |s| ids.iter().map(|&id| s.eml(id)).collect())
}

/// Save messages as .eml files, byte for byte as they came: one where the user picks in Save,
/// several into a folder the user picks, one file each under names made from their subjects.
/// The file or folder written, or `null` when cancelled.
#[tauri::command]
pub async fn save_messages(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    ids: Vec<MessageId>,
    words: SaveWords,
) -> CmdResult<Option<String>> {
    too_many(&ids)?;
    let all = emls(&state, &ids)?;
    let (tx, rx) = mpsc::channel();
    let dialog = app
        .dialog()
        .file()
        .set_parent(&window)
        .set_title(words.title);
    let send = move |path| {
        let _ = tx.send(path);
    };
    match all.as_slice() {
        [] => return Ok(None),
        [one] => dialog
            .set_file_name(&one.name)
            .add_filter(words.filter, &["eml"])
            .save_file(send),
        _ => dialog.pick_folder(send),
    }
    let Some(path) = picked(rx).await else {
        return Ok(None);
    };
    let written = match all.as_slice() {
        [one] => attachments::save_bytes(&path, &one.raw),
        _ => export::write_emls(&path, &all).map(|_| ()),
    };
    written.map_err(|e| {
        let shown = path.display();
        tr!(
            "Couldn't save {shown}: {e}.",
            "Nie udało się zapisać {shown}: {e}."
        )
    })?;
    Ok(Some(path.display().to_string()))
}

/// The message as text and its header fields, for View source and All headers.
#[tauri::command(async)]
pub fn message_source(state: State<'_, AppState>, id: MessageId) -> CmdResult<Source> {
    with_store(&state, |s| s.source(id))
}

/// A document of the message to print or save as PDF: its HTML (`html`, as the window shows
/// it) or its text, under a header. Remote images load as for reading (`load_images`, as in
/// `get_html`), never otherwise.
#[tauri::command]
pub async fn print_document(
    state: State<'_, AppState>,
    id: MessageId,
    html: bool,
    load_images: Option<bool>,
) -> CmdResult<String> {
    let (message, raw, trust) = with_store(&state, |s| {
        Ok((s.get(id)?, s.raw_of(id)?, s.image_trust(id)?))
    })?;
    let load = load_images.unwrap_or(trust.auto);
    tauri::async_runtime::spawn_blocking(move || {
        let first = export::print_document(&message, raw.as_deref(), html, &HashMap::new());
        if !load || first.remote.is_empty() {
            return first.document;
        }
        let images = html::fetch_images(&first.remote, &FetchLimits::default());
        export::print_document(&message, raw.as_deref(), html, &images).document
    })
    .await
    .map_err(failed)
}

/// What Save as PDF did.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PdfSaved {
    /// Written to this file.
    Saved(String),
    Cancelled,
    /// This system can't write a PDF straight away: the window opens the print dialog, whose
    /// printers include one that saves a PDF.
    PrintDialog,
}

/// Save the message the window has made ready to print (its print layout) as a PDF where
/// the user picks. On Windows WebView2 writes it; elsewhere the print dialog does.
#[tauri::command]
pub async fn save_pdf(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    id: MessageId,
    words: SaveWords,
) -> CmdResult<PdfSaved> {
    if !native::CAN_PRINT_TO_PDF {
        return Ok(PdfSaved::PrintDialog);
    }
    let subject = with_store(&state, |s| Ok(s.get(id)?.subject))?;
    let (tx, rx) = mpsc::channel();
    app.dialog()
        .file()
        .set_parent(&window)
        .set_title(words.title)
        .set_file_name(export::file_name(&subject, "pdf"))
        .add_filter(words.filter, &["pdf"])
        .save_file(move |path| {
            let _ = tx.send(path);
        });
    let Some(path) = picked(rx).await else {
        return Ok(PdfSaved::Cancelled);
    };
    native::print_to_pdf(&window, path.clone())
        .await
        .map_err(|e| {
            let shown = path.display();
            tr!(
                "Couldn't save {shown}: {e}.",
                "Nie udało się zapisać {shown}: {e}."
            )
        })?;
    Ok(PdfSaved::Saved(path.display().to_string()))
}

/// Messages dragged out of the list, past the window's edge: written as .eml files to a
/// folder of Telex's under the system's temporary folder (emptied when Telex starts), then
/// handed to the system's own drag, so Explorer, the desktop or another program can take
/// them. True when they were dropped somewhere, false when the drag was cancelled.
#[tauri::command]
pub async fn drag_out(
    window: WebviewWindow,
    state: State<'_, AppState>,
    files: State<'_, FilesState>,
    ids: Vec<MessageId>,
) -> CmdResult<bool> {
    too_many(&ids)?;
    let all = emls(&state, &ids)?;
    if all.is_empty() {
        return Ok(false);
    }
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S%3f").to_string();
    let dir: PathBuf = std::env::temp_dir().join("Telex").join("drag").join(stamp);
    let paths = export::write_emls(&dir, &all).map_err(|e| {
        tr!(
            "Couldn't make the files: {e}.",
            "Nie udało się utworzyć plików: {e}."
        )
    })?;
    // Telex's own window would take the files as dropped on it (attachments, mail for a
    // folder on this PC): not while they are being dragged out.
    files.dragging_out(true);
    let dropped = native::drag_files(&window, paths).await;
    files.dragging_out(false);
    Ok(dropped? && !files.dropped_home())
}
