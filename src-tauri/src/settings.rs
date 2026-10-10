//! Settings commands: `settings.json` next to the mail base, and exporting and importing the
//! settings file through Windows' own Save and Open dialogs. The logic is in
//! `telex_core::settings`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Mutex, MutexGuard};

use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::{DialogExt, FilePath};
use tauri_plugin_opener::OpenerExt;
use telex_core::i18n::{self, Lang};
use telex_core::settings::{
    self as core, ExportChoice, Import, ImportChoice, ImportError, ImportPlan, ImportReport,
    Settings, SystemLocale,
};
use telex_core::tr;

use crate::{with_store, AppState, CmdResult};

pub struct SettingsState {
    settings: Mutex<Settings>,
    path: PathBuf,
    /// There was no settings.json: the UI brings over what older versions kept in the window.
    fresh: AtomicBool,
    /// Windows' languages, read once at start (changing them means signing out anyway).
    locale: SystemLocale,
    /// The text of the file being imported, between its preview and Import.
    pending: Mutex<Option<String>>,
    /// The last exported file, for "Show in folder".
    exported: Mutex<Option<PathBuf>>,
}

impl SettingsState {
    /// Read `settings.json` (or start from the defaults when there is none).
    pub fn load(path: PathBuf) -> Self {
        let loaded = Settings::load(&path);
        let locale = core::system_locale();
        i18n::set(Lang::from_code(loaded.settings.effective_language(&locale)));
        Self {
            settings: Mutex::new(loaded.settings),
            path,
            fresh: AtomicBool::new(loaded.fresh),
            locale,
            pending: Mutex::new(None),
            exported: Mutex::new(None),
        }
    }

    /// What the window gets. The core writes its errors and notes in the same language.
    fn view(&self, settings: Settings) -> SettingsView {
        let language = settings.effective_language(&self.locale);
        i18n::set(Lang::from_code(language));
        SettingsView {
            language,
            offer: settings.language_to_offer(&self.locale),
            fresh: self.fresh.load(Ordering::SeqCst),
            settings,
        }
    }

    fn current(&self) -> CmdResult<Settings> {
        Ok(lock(&self.settings)?.clone())
    }

    /// The settings now, for the rest of the shell (the defaults if the lock broke).
    pub fn get(&self) -> Settings {
        self.current().unwrap_or_default()
    }

    /// Save the settings to disk, then make them current.
    fn replace(&self, next: Settings) -> CmdResult<SettingsView> {
        let mut current = lock(&self.settings)?;
        next.save(&self.path).map_err(|e| {
            tr!(
                "Couldn't save the settings: {e}.",
                "Nie udało się zapisać ustawień: {e}."
            )
        })?;
        *current = next.clone();
        self.fresh.store(false, Ordering::SeqCst);
        Ok(self.view(next))
    }
}

fn lock<T>(m: &Mutex<T>) -> CmdResult<MutexGuard<'_, T>> {
    m.lock().map_err(|_| {
        tr!(
            "The settings stopped responding. Restart Telex.",
            "Ustawienia przestały odpowiadać. Uruchom Telexa ponownie."
        )
    })
}

#[derive(Debug, Serialize)]
pub struct SettingsView {
    settings: Settings,
    /// The language on screen: the chosen one, or Windows' for "auto".
    language: &'static str,
    /// A language to offer once, on the first start (English Windows in a Polish region).
    offer: Option<&'static str>,
    fresh: bool,
}

#[tauri::command]
pub fn get_settings(settings: State<'_, SettingsState>) -> CmdResult<SettingsView> {
    Ok(settings.view(settings.current()?))
}

/// Change some settings and save them. `changes` holds only the ones that change; a value
/// this version doesn't know leaves that setting as it was.
#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    settings: State<'_, SettingsState>,
    changes: Value,
) -> CmdResult<SettingsView> {
    let before = settings.current()?;
    let (next, _) = before.merged(&changes);
    let push = next.push_mail != before.push_mail;
    let view = settings.replace(next)?;
    if push {
        crate::watch::restart(&app);
    }
    Ok(view)
}

/// The words of the Save and Open dialogs, in the interface's language.
#[derive(Debug, Deserialize)]
pub struct DialogWords {
    title: String,
    /// The file type's name, e.g. "Telex settings".
    filter: String,
    #[serde(default)]
    file_name: String,
}

/// Write the chosen parts to a file the user picks. `null` when they cancel.
#[tauri::command]
pub async fn export_settings(
    app: AppHandle,
    window: WebviewWindow,
    store: State<'_, AppState>,
    settings: State<'_, SettingsState>,
    choice: ExportChoice,
    words: DialogWords,
) -> CmdResult<Option<String>> {
    let current = settings.current()?;
    let version = app.package_info().version.to_string();
    let now = Local::now().naive_local();
    let text = with_store(&store, |s| core::export(s, &current, choice, &version, now))?;
    let (tx, rx) = mpsc::channel();
    app.dialog()
        .file()
        .set_parent(&window)
        .set_title(words.title)
        .add_filter(words.filter, &["json"])
        .set_file_name(words.file_name)
        .save_file(move |path| {
            let _ = tx.send(path);
        });
    let Some(path) = picked(rx).await else {
        return Ok(None);
    };
    let shown = path.display().to_string();
    std::fs::write(&path, text).map_err(|e| {
        tr!(
            "Couldn't save {shown}: {e}.",
            "Nie udało się zapisać {shown}: {e}."
        )
    })?;
    *lock(&settings.exported)? = Some(path);
    Ok(Some(shown))
}

/// Waits for the dialog without holding up the async runtime.
async fn picked(rx: mpsc::Receiver<Option<FilePath>>) -> Option<PathBuf> {
    tauri::async_runtime::spawn_blocking(move || rx.recv().ok().flatten())
        .await
        .ok()
        .flatten()
        .and_then(|p| p.into_path().ok())
}

/// Show the last exported file in Explorer.
#[tauri::command]
pub fn show_export(app: AppHandle, settings: State<'_, SettingsState>) -> CmdResult<()> {
    let Some(path) = lock(&settings.exported)?.clone() else {
        return Ok(());
    };
    app.opener().reveal_item_in_dir(&path).map_err(|e| {
        tr!(
            "Couldn't open the folder: {e}",
            "Nie udało się otworzyć folderu: {e}"
        )
    })
}

/// Pick a settings file and say what importing it would do. `null` when the user cancels.
/// Nothing changes until `apply_import`.
#[tauri::command]
pub async fn open_import(
    app: AppHandle,
    window: WebviewWindow,
    store: State<'_, AppState>,
    settings: State<'_, SettingsState>,
    words: DialogWords,
) -> Result<Option<ImportPlan>, ImportError> {
    let (tx, rx) = mpsc::channel();
    app.dialog()
        .file()
        .set_parent(&window)
        .set_title(words.title)
        .add_filter(words.filter, &["json"])
        .pick_file(move |path| {
            let _ = tx.send(path);
        });
    let Some(path) = picked(rx).await else {
        return Ok(None);
    };
    let text =
        std::fs::read_to_string(&path).map_err(|e| ImportError::Unreadable(e.to_string()))?;
    let unreadable = |e: String| ImportError::Unreadable(e);
    let current = settings.current().map_err(unreadable)?;
    let plan = with_store(&store, |s| Ok(Import::read(&text, s, &current)))
        .map_err(unreadable)??
        .plan;
    *lock(&settings.pending).map_err(unreadable)? = Some(text);
    Ok(Some(plan))
}

#[derive(Debug, Serialize)]
pub struct ImportOutcome {
    report: ImportReport,
    settings: SettingsView,
}

/// Import the ticked parts of the file from `open_import`. It is matched again first, in case
/// rules or accounts changed meanwhile.
#[tauri::command]
pub fn apply_import(
    app: AppHandle,
    store: State<'_, AppState>,
    settings: State<'_, SettingsState>,
    choice: ImportChoice,
) -> CmdResult<ImportOutcome> {
    let Some(text) = lock(&settings.pending)?.take() else {
        return Err(tr!(
            "Choose the file to import again.",
            "Wybierz jeszcze raz plik do importu."
        ));
    };
    let current = settings.current()?;
    let (next, report) = with_store(&store, |s| {
        let import = Import::read(&text, s, &current)
            .map_err(|e| telex_core::StoreError::Invalid(e.to_string()))?;
        Ok(import.apply(s, choice))
    })?;
    let view = match next {
        Some(next) => {
            let push = next.push_mail != current.push_mail;
            let view = settings.replace(next)?;
            if push {
                crate::watch::restart(&app);
            }
            view
        }
        None => settings.view(current),
    };
    Ok(ImportOutcome {
        report,
        settings: view,
    })
}
