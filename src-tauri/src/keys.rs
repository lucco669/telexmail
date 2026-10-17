//! OpenPGP keys (Settings › Keys) and passphrases (the line above encrypted mail, compose's
//! Sign): imported from files, pasted, taken from a message, made, exported and removed, all
//! in `telex_core::openpgp`. A passphrase is kept in the store's memory for the session, and
//! in the system's password store only when the user ticks Remember (secrets.rs).

use std::collections::HashSet;
use std::sync::{mpsc, Mutex};

use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;
use telex_core::openpgp::{
    Choice, ComposeState, ImportReport, KeyInfo, KeyKind, KeyRef, Protect, Source, WriteProblem,
};
use telex_core::smtp;
use telex_core::{tr, Account, MessageId, OutgoingDraft};

use crate::secrets;
use crate::settings::SettingsState;
use crate::{with_store, AppState, CmdResult};

/// The largest key file read.
const MAX_FILE: u64 = 8 * 1024 * 1024;

#[derive(Default)]
pub struct KeysState {
    /// Keys whose remembered passphrase was tried this session (one try each, so a wrong
    /// one in the password store doesn't ask the store at every message).
    tried: Mutex<HashSet<String>>,
}

impl KeysState {
    fn first_try(&self, fingerprint: &str) -> bool {
        self.tried
            .lock()
            .map(|mut t| t.insert(fingerprint.to_owned()))
            .unwrap_or(false)
    }
}

/// Give the store a key's remembered passphrase, once a session: true when it opened it.
pub fn unlock_remembered(state: &AppState, keys: &KeysState, fingerprint: &str) -> bool {
    if !keys.first_try(fingerprint) {
        return false;
    }
    let Some(passphrase) = secrets::load_passphrase(fingerprint) else {
        return false;
    };
    with_store(state, |s| s.pgp_unlock(fingerprint, &passphrase)).unwrap_or(false)
}

/// A key on the keys page: what the core knows, and for own keys whether the passphrase is
/// known this session and remembered.
#[derive(Debug, Serialize)]
pub struct KeyRow {
    #[serde(flatten)]
    info: KeyInfo,
    unlocked: bool,
    remembered: bool,
}

#[tauri::command(async)]
pub fn pgp_keys(state: State<'_, AppState>) -> CmdResult<Vec<KeyRow>> {
    let (keys, unlocked) = with_store(&state, |s| Ok((s.pgp_keys()?, s.pgp_unlocked())))?;
    Ok(keys
        .into_iter()
        .map(|info| {
            let remembered = info.secret
                && info.protected
                && secrets::load_passphrase(&info.fingerprint).is_some();
            KeyRow {
                unlocked: !info.protected || unlocked.contains(&info.fingerprint),
                remembered,
                info,
            }
        })
        .collect())
}

#[derive(Debug, Deserialize)]
pub struct DialogWords {
    title: String,
    filter: String,
    #[serde(default)]
    file_name: String,
}

/// Import the keys in files the user picks (.asc, .gpg, .pgp, .key). `null`: cancelled.
#[tauri::command]
pub async fn pgp_import(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    words: DialogWords,
) -> CmdResult<Option<ImportReport>> {
    let (tx, rx) = mpsc::channel();
    app.dialog()
        .file()
        .set_parent(&window)
        .set_title(words.title)
        .add_filter(words.filter, &["asc", "gpg", "pgp", "key", "pub", "txt"])
        .pick_file(move |path| {
            let _ = tx.send(path);
        });
    let Some(path) = crate::files::picked(rx).await else {
        return Ok(None);
    };
    let shown = path.display().to_string();
    let too_big = std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_FILE);
    if too_big {
        return Err(tr!(
            "{shown} is too large to be a key file.",
            "{shown} jest za duży jak na plik z kluczem."
        ));
    }
    let data = std::fs::read(&path).map_err(|e| {
        tr!(
            "Couldn't read {shown}: {e}.",
            "Nie udało się odczytać {shown}: {e}."
        )
    })?;
    let now = Local::now().naive_local();
    with_store(&state, |s| s.import_keys(&data, Source::File, now)).map(Some)
}

/// Import keys pasted as text (an armored block).
#[tauri::command(async)]
pub fn pgp_import_text(state: State<'_, AppState>, text: String) -> CmdResult<ImportReport> {
    let now = Local::now().naive_local();
    with_store(&state, |s| {
        s.import_keys(text.as_bytes(), Source::File, now)
    })
}

/// Import the keys the open message carries (its Autocrypt header, an attached key).
#[tauri::command(async)]
pub fn pgp_import_offered(state: State<'_, AppState>, id: MessageId) -> CmdResult<ImportReport> {
    let now = Local::now().naive_local();
    with_store(&state, |s| s.import_offered_keys(id, now))
}

/// Make a key for an address; with `remember`, its passphrase goes to the system store.
#[tauri::command(async)]
pub fn pgp_generate(
    state: State<'_, AppState>,
    name: String,
    email: String,
    passphrase: String,
    kind: KeyKind,
    remember: bool,
) -> CmdResult<KeyRef> {
    let now = Local::now().naive_local();
    let made = with_store(&state, |s| {
        s.generate_key(&name, &email, &passphrase, kind, now)
    })?;
    if remember {
        secrets::save_passphrase(&made.fingerprint, &passphrase)?;
    }
    Ok(made)
}

/// Save a key to a file the user picks: the public key, or (`secret`) the whole key,
/// protected by its passphrase. `null`: cancelled.
#[tauri::command]
pub async fn pgp_export(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    fingerprint: String,
    secret: bool,
    words: DialogWords,
) -> CmdResult<Option<String>> {
    let text = with_store(&state, |s| {
        if secret {
            s.export_secret_key(&fingerprint)
        } else {
            s.export_public_key(&fingerprint)
        }
    })?;
    let (tx, rx) = mpsc::channel();
    app.dialog()
        .file()
        .set_parent(&window)
        .set_title(words.title)
        .add_filter(words.filter, &["asc"])
        .set_file_name(words.file_name)
        .save_file(move |path| {
            let _ = tx.send(path);
        });
    let Some(path) = crate::files::picked(rx).await else {
        return Ok(None);
    };
    let shown = path.display().to_string();
    std::fs::write(&path, text).map_err(|e| {
        tr!(
            "Couldn't save {shown}: {e}.",
            "Nie udało się zapisać {shown}: {e}."
        )
    })?;
    Ok(Some(shown))
}

/// A key's public part as text, for the clipboard.
#[tauri::command(async)]
pub fn pgp_public_text(state: State<'_, AppState>, fingerprint: String) -> CmdResult<String> {
    with_store(&state, |s| s.export_public_key(&fingerprint))
}

/// Remove a key, and its remembered passphrase.
#[tauri::command(async)]
pub fn pgp_delete(state: State<'_, AppState>, fingerprint: String) -> CmdResult<()> {
    with_store(&state, |s| s.delete_key(&fingerprint))?;
    secrets::delete_passphrase(&fingerprint);
    Ok(())
}

/// The user compared a key's fingerprint with its owner (or takes that back).
#[tauri::command(async)]
pub fn pgp_verify(
    state: State<'_, AppState>,
    fingerprint: String,
    verified: bool,
) -> CmdResult<()> {
    with_store(&state, |s| s.set_key_verified(&fingerprint, verified))
}

/// A passphrase typed for an own key: true when it opens the key; with `remember` it goes
/// to the system store too.
#[tauri::command(async)]
pub fn pgp_unlock(
    state: State<'_, AppState>,
    fingerprint: String,
    passphrase: String,
    remember: bool,
) -> CmdResult<bool> {
    let opened = with_store(&state, |s| s.pgp_unlock(&fingerprint, &passphrase))?;
    if opened && remember {
        secrets::save_passphrase(&fingerprint, &passphrase)?;
    }
    Ok(opened)
}

/// Forget a key's passphrase: for this session and in the system store.
#[tauri::command(async)]
pub fn pgp_forget(state: State<'_, AppState>, fingerprint: String) -> CmdResult<()> {
    with_store(&state, |s| {
        s.pgp_lock(&fingerprint);
        Ok(())
    })?;
    secrets::delete_passphrase(&fingerprint);
    Ok(())
}

/// The addresses a draft goes to (To and Cc) and its blind copies, as far as they parse.
fn recipients(draft: &OutgoingDraft) -> (Vec<String>, Vec<String>) {
    let mut to = smtp::lenient_emails(&draft.to);
    to.extend(smtp::lenient_emails(&draft.cc));
    (to, smtp::lenient_emails(&draft.bcc))
}

/// What a message (or, `for_draft`, its draft) needs from OpenPGP. When signing finds the key
/// locked, its remembered passphrase is tried once; the error is a sentence for the window.
pub fn protection_for(
    state: &AppState,
    keys: &KeysState,
    account: &Account,
    draft: &OutgoingDraft,
    choice: Choice,
    autocrypt: bool,
    for_draft: bool,
) -> CmdResult<Protect> {
    let from = account.identity(draft.identity).email.clone();
    let (to, bcc) = recipients(draft);
    let attempt = || {
        with_store(state, |s| {
            Ok(s.protection(&from, &to, &bcc, choice, autocrypt, for_draft))
        })
    };
    match attempt()? {
        Err(WriteProblem::Locked { key }) if unlock_remembered(state, keys, &key.fingerprint) => {
            attempt()?.map_err(|e| e.to_string())
        }
        other => other.map_err(|e| e.to_string()),
    }
}

/// After sending: what was chosen, for the next message to the same people.
pub fn remember(state: &AppState, draft: &OutgoingDraft, choice: Choice) {
    let (mut to, bcc) = recipients(draft);
    to.extend(bcc);
    let _ = with_store(state, |s| s.remember_choice(&to, choice));
}

/// Where compose's Sign and Encrypt start, and what is missing, for the message as typed
/// (the same draft Save would send).
#[tauri::command(async)]
pub fn pgp_compose(
    state: State<'_, AppState>,
    settings: State<'_, SettingsState>,
    draft: OutgoingDraft,
) -> CmdResult<ComposeState> {
    let sign = settings.get().pgp_sign;
    let (mut all, bcc) = recipients(&draft);
    all.extend(bcc);
    with_store(&state, |s| {
        let from = s
            .account(&draft.account)?
            .identity(draft.identity)
            .email
            .clone();
        Ok(s.compose_state(&from, &all, draft.reply_to, sign))
    })
}
