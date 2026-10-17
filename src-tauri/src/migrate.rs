//! Moving in from another mail program: finding Thunderbird, Outlook and the rest, and
//! bringing their accounts, mail and filters. The work is in `telex_core::migrate`; this is
//! the commands, the dialogs, the passwords and the first login of each new account.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{mpsc, Mutex};

use futures_util::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;
use telex_core::mailbox::{ImportProgress, ImportReport};
use telex_core::migrate::{self, Choice, Source};
use telex_core::settings::PlanItem;
use telex_core::{imap, tr, MailStore};

use crate::{capitalize, failed, secrets, watch, with_store, AppState, CmdResult};

/// New accounts log in this many at a time to read their folders.
const LOGINS_AT_ONCE: usize = 6;

/// The sources found or picked this session: the only ones `move_in` reads, as only files
/// the user chose can be attached.
#[derive(Default)]
pub struct Sources(Mutex<HashSet<String>>);

impl Sources {
    fn allow(&self, found: &[Source]) {
        if let Ok(mut set) = self.0.lock() {
            set.extend(found.iter().map(|s| s.path.clone()));
        }
    }

    fn allowed(&self, path: &str) -> bool {
        self.0.lock().is_ok_and(|set| set.contains(path))
    }
}

/// Mail programs on this PC and what each has. Nothing goes over the network.
#[tauri::command]
pub async fn find_programs(app: AppHandle) -> CmdResult<Vec<Source>> {
    let accounts = with_store(&app.state::<AppState>(), |s| Ok(s.accounts().to_vec()))?;
    let found = tauri::async_runtime::spawn_blocking(move || migrate::find(&accounts))
        .await
        .map_err(failed)?;
    app.state::<Sources>().allow(&found);
    Ok(found)
}

/// The words of the Open dialogs.
#[derive(Debug, Deserialize)]
pub struct OpenWords {
    title: String,
    filter: String,
}

/// What an Outlook data file or a folder of mail the user picks holds: `folder` picks a
/// folder (Maildir, MH, a Thunderbird profile), else a .pst file. `null` when cancelled.
#[tauri::command]
pub async fn open_source(
    app: AppHandle,
    window: WebviewWindow,
    folder: bool,
    words: OpenWords,
) -> CmdResult<Option<Source>> {
    let (tx, rx) = mpsc::channel();
    let dialog = app
        .dialog()
        .file()
        .set_parent(&window)
        .set_title(words.title);
    if folder {
        dialog.pick_folder(move |p| {
            let _ = tx.send(p);
        });
    } else {
        dialog
            .add_filter(words.filter, &["pst"])
            .pick_file(move |p| {
                let _ = tx.send(p);
            });
    }
    let Some(path) = crate::files::picked(rx).await else {
        return Ok(None);
    };
    let accounts = with_store(&app.state::<AppState>(), |s| Ok(s.accounts().to_vec()))?;
    let source = tauri::async_runtime::spawn_blocking(move || migrate::open(&path, &accounts))
        .await
        .map_err(failed)?
        .map_err(|e| e.to_string())?;
    app.state::<Sources>().allow(std::slice::from_ref(&source));
    Ok(Some(source))
}

/// Progress for the window: the step, how far it is, and what it is on.
#[derive(Debug, Clone, Serialize)]
struct MoveProgress {
    step: &'static str,
    done: u64,
    total: u64,
    name: String,
}

/// An account added, and why its first login failed, if it did.
#[derive(Debug, Serialize)]
pub struct Added {
    email: String,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MoveReport {
    accounts: Vec<Added>,
    mail: ImportReport,
    tags: Vec<PlanItem>,
    rules: Vec<PlanItem>,
    contacts: telex_core::contacts::ImportReport,
}

/// Bring in what the user ticked from the source at `path`: with the filters, the tags first
/// (the mail's keywords become tags only when they are tags here); the accounts (with the
/// passwords typed for them; each logs in once to read its folders), the mail kept on the PC
/// into folders on this PC, then the filters as rules, which need all of them, and the address
/// books. Progress comes as "move-progress" events.
#[tauri::command]
pub async fn move_in(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    path: String,
    choice: Choice,
) -> CmdResult<MoveReport> {
    if !app.state::<Sources>().allowed(&path) {
        return Err(failed(tr!(
            "Telex reads only what it found or what you chose",
            "Telex czyta tylko to, co znalazł albo co wybrano"
        )));
    }
    let accounts = with_store(&state, |s| Ok(s.accounts().to_vec()))?;
    let source = {
        let path = PathBuf::from(&path);
        tauri::async_runtime::spawn_blocking(move || migrate::open(&path, &accounts))
            .await
            .map_err(failed)?
            .map_err(|e| e.to_string())?
    };
    let emit = |step, done, total, name: &str| {
        let _ = app.emit_to(
            window.label(),
            "move-progress",
            MoveProgress {
                step,
                done,
                total,
                name: name.to_owned(),
            },
        );
    };
    // The mail base goes to disk with the first thing that comes in.
    with_store(&state, |s| {
        if s.is_in_memory() {
            *s = MailStore::open(&state.db_path)?;
        }
        s.ensure_this_pc()
    })?;
    let tags = if choice.filters {
        with_store(&state, |s| Ok(migrate::tags(&source, s)))?
    } else {
        Vec::new()
    };

    let wanted: Vec<_> = source
        .accounts
        .iter()
        .filter(|a| choice.accounts.contains(&a.key) && !a.present && a.blocked.is_none())
        .collect();
    let mut added = Vec::new();
    let mut logins = Vec::new();
    for (i, found) in wanted.iter().enumerate() {
        emit(
            "accounts",
            i as u64,
            wanted.len() as u64,
            &found.setup.email,
        );
        let setup = match found.setup.clone().cleaned() {
            Ok(s) => s,
            Err(e) => {
                added.push(Added {
                    email: found.setup.email.clone(),
                    error: Some(e),
                });
                continue;
            }
        };
        let password = choice
            .passwords
            .get(&found.key)
            .filter(|p| !p.is_empty())
            .cloned();
        if let Some(password) = &password {
            if let Err(e) = secrets::save_password(&setup.email, password) {
                added.push(Added {
                    email: setup.email.clone(),
                    error: Some(capitalize(&e)),
                });
                continue;
            }
        }
        let account = with_store(&state, |s| s.add_found_account(&setup, &found.identities))?;
        match password {
            Some(password) => logins.push((account, password)),
            None => added.push(Added {
                email: account.email.clone(),
                error: Some(tr!(
                    "no password yet: enter it in Settings › Accounts › Edit",
                    "nie ma jeszcze hasła: wpisz je w Ustawieniach › Konta › Edytuj"
                )),
            }),
        }
    }
    // Each logs in once, a few at a time, to read its folders, so filters find them.
    let total = wanted.len() as u64;
    let mut done = (wanted.len() - logins.len()) as u64;
    let mut checks = stream::iter(logins.into_iter().map(|(account, password)| async move {
        let settings = account.imap.clone();
        let result = match settings {
            Some(settings) => imap::check_account(&settings, &password)
                .await
                .map_err(|e| capitalize(&e.to_string())),
            None => Ok(Vec::new()),
        };
        (account, result)
    }))
    .buffer_unordered(LOGINS_AT_ONCE);
    while let Some((account, result)) = checks.next().await {
        done += 1;
        emit("accounts", done, total, &account.email);
        let error = match result {
            Ok(folders) => with_store(&state, |s| {
                let plan = imap::plan_folders(&folders, &s.folder_hints(&account.id)?);
                s.set_folders(&account.id, &plan)
            })
            .err(),
            Err(e) => {
                let _ = with_store(&state, |s| {
                    s.record_sync(&account.id, chrono::Local::now().naive_local(), Some(&e))
                });
                Some(e)
            }
        };
        added.push(Added {
            email: account.email.clone(),
            error,
        });
    }
    drop(checks);
    if !added.is_empty() {
        watch::restart(&app);
    }

    let mail = if choice.folders.is_empty() {
        ImportReport::default()
    } else {
        let total: u64 = source
            .folders
            .iter()
            .filter(|f| choice.folders.contains(&f.key))
            .map(|f| f.bytes)
            .sum();
        let handle = app.clone();
        let (source, keys, label) = (
            source.clone(),
            choice.folders.clone(),
            window.label().to_owned(),
        );
        tauri::async_runtime::spawn_blocking(move || {
            let state = handle.state::<AppState>();
            let progress = |p: ImportProgress| {
                let _ = handle.emit_to(
                    &label,
                    "move-progress",
                    MoveProgress {
                        step: "mail",
                        done: p.bytes.min(total),
                        total,
                        name: p.file,
                    },
                );
            };
            migrate::import_mail(&state.store, &source, &keys, &progress)
        })
        .await
        .map_err(failed)?
        .map_err(|e| e.to_string())?
    };

    let rules = if choice.filters {
        with_store(&state, |s| Ok(migrate::rules(&source, s)))?
    } else {
        Vec::new()
    };
    let contacts = if choice.books.is_empty() {
        telex_core::contacts::ImportReport::default()
    } else {
        emit("contacts", 0, 1, "");
        with_store(&state, |s| migrate::import_books(s, &source, &choice.books))?
    };
    Ok(MoveReport {
        accounts: added,
        mail,
        tags,
        rules,
        contacts,
    })
}
