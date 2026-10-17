//! The address book's commands: contacts and groups, people to offer, vCard files in and out
//! (Open and Save dialogs). The logic is in `telex_core::{contacts, vcard}`.

use std::sync::mpsc;

use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;
use telex_core::contacts::{Contact, Group, ImportReport};
use telex_core::recipients::Recipient;
use telex_core::{MailStore, StoreError};

use crate::folders::FileWords;
use crate::{failed, with_store, AppState, CmdResult};

/// How many people written to the address book offers to add.
const OFFERED: usize = 50;

/// A change to the address book: before the first account the store is in memory, so the
/// mail base on disk is made first (as moving in does) and the contact isn't lost.
fn on_disk<T>(
    state: &AppState,
    f: impl FnOnce(&mut MailStore) -> Result<T, StoreError>,
) -> CmdResult<T> {
    with_store(state, |s| {
        if s.is_in_memory() {
            *s = MailStore::open(&state.db_path)?;
        }
        f(s)
    })
}

#[tauri::command(async)]
pub fn contacts(state: State<'_, AppState>) -> CmdResult<Vec<Contact>> {
    with_store(&state, |s| s.contacts())
}

#[tauri::command(async)]
pub fn contact_groups(state: State<'_, AppState>) -> CmdResult<Vec<Group>> {
    with_store(&state, |s| s.contact_groups())
}

#[tauri::command(async)]
pub fn save_contact(state: State<'_, AppState>, contact: Contact) -> CmdResult<Contact> {
    on_disk(&state, |s| s.save_contact(&contact))
}

/// Remove contacts; returns them, for Z.
#[tauri::command(async)]
pub fn delete_contacts(state: State<'_, AppState>, ids: Vec<i64>) -> CmdResult<Vec<Contact>> {
    with_store(&state, |s| s.delete_contacts(&ids))
}

#[tauri::command(async)]
pub fn restore_contacts(state: State<'_, AppState>, contacts: Vec<Contact>) -> CmdResult<()> {
    on_disk(&state, |s| s.restore_contacts(&contacts))
}

#[tauri::command(async)]
pub fn save_group(state: State<'_, AppState>, group: Group) -> CmdResult<Group> {
    on_disk(&state, |s| s.save_group(&group))
}

/// Remove a group (not its contacts); returns it, for Z.
#[tauri::command(async)]
pub fn delete_group(state: State<'_, AppState>, id: i64) -> CmdResult<Option<Group>> {
    with_store(&state, |s| s.delete_group(id))
}

#[tauri::command(async)]
pub fn restore_group(state: State<'_, AppState>, group: Group) -> CmdResult<Group> {
    on_disk(&state, |s| s.restore_group(&group))
}

/// People written to who aren't contacts yet, to offer.
#[tauri::command(async)]
pub fn suggested_contacts(state: State<'_, AppState>) -> CmdResult<Vec<Recipient>> {
    with_store(&state, |s| s.suggested_contacts(OFFERED))
}

/// Bring in .vcf files chosen in Open. What happened, or `null` when cancelled.
#[tauri::command]
pub async fn import_vcards(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    words: FileWords,
) -> CmdResult<Option<ImportReport>> {
    let (tx, rx) = mpsc::channel();
    app.dialog()
        .file()
        .set_parent(&window)
        .set_title(words.title)
        .add_filter(words.filter, &["vcf", "vcard"])
        .pick_files(move |paths| {
            let _ = tx.send(paths);
        });
    let picked = tauri::async_runtime::spawn_blocking(move || rx.recv().ok().flatten())
        .await
        .map_err(failed)?
        .unwrap_or_default();
    let paths: Vec<_> = picked
        .into_iter()
        .filter_map(|p| p.into_path().ok())
        .collect();
    if paths.is_empty() {
        return Ok(None);
    }
    let files = tauri::async_runtime::spawn_blocking(move || {
        paths
            .iter()
            .map(std::fs::read)
            .collect::<std::io::Result<Vec<Vec<u8>>>>()
    })
    .await
    .map_err(failed)?
    .map_err(failed)?;
    let mut report = ImportReport::default();
    for bytes in files {
        report.add(&on_disk(&state, |s| s.import_vcards(&bytes))?);
    }
    Ok(Some(report))
}

/// Save the contacts `ids` (all of them when empty) as one .vcf where the user picks. The
/// path written, or `null` when cancelled.
#[tauri::command]
pub async fn export_vcards(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    ids: Vec<i64>,
    words: FileWords,
) -> CmdResult<Option<String>> {
    let text = with_store(&state, |s| s.export_vcards(&ids))?;
    let (tx, rx) = mpsc::channel();
    app.dialog()
        .file()
        .set_parent(&window)
        .set_title(words.title)
        .set_file_name(words.file_name)
        .add_filter(words.filter, &["vcf"])
        .save_file(move |path| {
            let _ = tx.send(path);
        });
    let Some(dest) = crate::files::picked(rx).await else {
        return Ok(None);
    };
    let to = dest.clone();
    tauri::async_runtime::spawn_blocking(move || std::fs::write(&to, text))
        .await
        .map_err(failed)?
        .map_err(failed)?;
    Ok(Some(dest.display().to_string()))
}
