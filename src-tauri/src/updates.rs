//! Updates from GitHub Releases, through tauri-plugin-updater.
//!
//! Only release builds can update. They are compiled with the public half of the release
//! signing key in `TELEX_UPDATER_PUBKEY`, and the updater installs nothing that key didn't
//! sign. Every other build (`npm run dev`, the CI installers) leaves the plugin out and says
//! it can't update itself. On Linux only the AppImage replaces itself; a .deb belongs to the
//! package manager and says the same. Telex never checks on its own initiative: the UI asks
//! the user first, then checks as they chose (at each start and once a day, or once a day) or
//! when the user clicks the version number.

use std::sync::Mutex;
use std::time::Duration;

use chrono::{DateTime, Local};
use serde::Serialize;
use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Context, Emitter, State, Url, Wry};
use tauri_plugin_updater::{Error, Update, Updater, UpdaterExt};
use telex_core::tr;

use crate::{capitalize, AppState, BusyGuard, CmdResult};

/// The one address a check asks: a small JSON file GitHub serves from the newest published
/// release, with its version, its notes and where its installers are. The same file for
/// everyone, so the request says nothing about who asks or which version they run.
const ENDPOINT: &str = "https://github.com/lucco669/telexmail/releases/latest/download/latest.json";

/// Sent as the User-Agent instead of the plugin's own name and version.
const USER_AGENT: &str = "Telex";

fn cant_update() -> String {
    tr!(
        "This build of Telex can't update itself. Release builds from telexmail.app can.",
        "Ta kompilacja Telexa nie aktualizuje się sama. Wydania z telexmail.app to potrafią."
    )
}

/// The public half of the release signing key, compiled into release builds only.
fn pubkey() -> Option<&'static str> {
    option_env!("TELEX_UPDATER_PUBKEY")
        .map(str::trim)
        .filter(|key| !key.is_empty())
}

/// Whether this copy can replace itself with a release's file: a release build, and on Linux
/// one running as an AppImage (Tauri reads `APPIMAGE`, which the AppImage sets).
fn updatable(app: &AppHandle) -> bool {
    #[cfg(target_os = "linux")]
    let replaceable = {
        use tauri::Manager;
        app.env().appimage.is_some()
    };
    #[cfg(not(target_os = "linux"))]
    let replaceable = {
        let _ = app;
        true
    };
    replaceable && pubkey().is_some()
}

/// The updater plugin for a release build, or `None` when this build can't update itself.
///
/// The plugin reads its settings from `plugins.updater` in the app config and refuses to
/// start without one, so they are put there here, at start, instead of in tauri.conf.json:
/// the key isn't kept in the repository.
pub fn plugin(
    context: &mut Context<Wry>,
) -> Option<TauriPlugin<Wry, tauri_plugin_updater::Config>> {
    let key = pubkey()?;
    context.config_mut().plugins.0.insert(
        "updater".into(),
        serde_json::json!({
            "pubkey": key,
            // The signature must name the version the release says it is, so an old,
            // validly signed installer can't be passed off as a new one.
            "requireSignedVersion": true,
            // A progress bar while the installer runs and no questions; then Telex starts again.
            // Only Windows reads this; an AppImage is replaced without an installer.
            "windows": { "installMode": "passive" },
        }),
    );
    Some(tauri_plugin_updater::Builder::new().build())
}

/// The update the last check found, kept for `install_update`. `Update` is the plugin's
/// type; the Mutex lets the commands, which run on any thread, take turns with it.
#[derive(Default)]
pub struct Found(Mutex<Option<Update>>);

#[derive(Debug, Serialize)]
pub struct BuildInfo {
    version: String,
    /// Built with the release key (and on Linux running as an AppImage), so it can check for
    /// updates and install them.
    updatable: bool,
}

/// This build's version and whether it can update itself.
#[tauri::command(async)]
pub fn update_info(app: AppHandle) -> BuildInfo {
    BuildInfo {
        version: app.package_info().version.to_string(),
        updatable: updatable(&app),
    }
}

#[derive(Debug, Serialize)]
pub struct Available {
    version: String,
    /// What's new, from the release's tag message.
    notes: String,
    /// When the release was built, as local `YYYY-MM-DDTHH:MM:SS` like every date on the bridge.
    date: Option<String>,
}

impl Available {
    fn of(update: &Update) -> Self {
        let date = update
            .date
            .and_then(|d| DateTime::from_timestamp(d.unix_timestamp(), 0))
            .map(|d| {
                d.with_timezone(&Local)
                    .format("%Y-%m-%dT%H:%M:%S")
                    .to_string()
            });
        Self {
            version: update.version.clone(),
            notes: update.body.clone().unwrap_or_default().trim().to_string(),
            date,
        }
    }
}

fn updater(app: &AppHandle) -> CmdResult<Updater> {
    if !updatable(app) {
        return Err(cant_update());
    }
    let endpoint = Url::parse(ENDPOINT).map_err(|e| e.to_string())?;
    app.updater_builder()
        .endpoints(vec![endpoint])
        .map_err(describe)?
        // Applies to the check and to the download. No overall time limit for the download,
        // which takes longer on a slow line, but it gives up if the data stops coming.
        .configure_client(|client| {
            client
                .user_agent(USER_AGENT)
                .connect_timeout(Duration::from_secs(15))
                .read_timeout(Duration::from_secs(30))
        })
        .build()
        .map_err(describe)
}

/// Ask GitHub for the newest release. `null` when this is the newest version.
#[tauri::command]
pub async fn check_update(app: AppHandle, found: State<'_, Found>) -> CmdResult<Option<Available>> {
    let update = updater(&app)?.check().await.map_err(describe)?;
    let available = update.as_ref().map(Available::of);
    *found.0.lock().map_err(|_| stuck())? = update;
    Ok(available)
}

fn stuck() -> String {
    tr!(
        "The updater stopped responding. Restart Telex.",
        "Aktualizator przestał odpowiadać. Uruchom Telexa ponownie."
    )
}

#[derive(Debug, Default, Serialize)]
pub struct InstallOutcome {
    /// A check, push or send was running, so nothing started. Try again shortly.
    busy: bool,
}

#[derive(Debug, Clone, Serialize)]
struct Progress {
    done: usize,
    total: Option<u64>,
}

/// Download the update the last check found, verify its signature and start its installer.
/// On Windows the installer closes Telex, updates it and starts it again; on Linux the new
/// AppImage takes the old one's place and Telex restarts into it. Either way, when this works
/// it doesn't return. Progress reaches the UI as "update-progress" events.
#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    state: State<'_, AppState>,
    found: State<'_, Found>,
) -> CmdResult<InstallOutcome> {
    // Telex must not close halfway through talking to a mail server: a message the SMTP
    // server took but Telex hadn't yet filed as sent would go out again. Holding both flags
    // also keeps a new check or send from starting before the installer closes Telex.
    let Some(_busy) = BusyGuard::take(&state.busy) else {
        return Ok(InstallOutcome { busy: true });
    };
    let Ok(_sending) = state.sending.try_lock() else {
        return Ok(InstallOutcome { busy: true });
    };
    let known = found.0.lock().map_err(|_| stuck())?.clone();
    let update = match known {
        Some(update) => update,
        None => updater(&app)?
            .check()
            .await
            .map_err(describe)?
            .ok_or_else(|| {
                tr!(
                    "This is already the newest version.",
                    "To już najnowsza wersja."
                )
            })?,
    };
    let (mut done, mut shown) = (0usize, None);
    let bytes = update
        .download(
            |chunk, total| {
                done += chunk;
                // One event per percent (or per 256 KB when the size is unknown), not per chunk.
                let step = total.map_or(done as u64 >> 18, |t| done as u64 * 100 / t.max(1));
                if shown != Some(step) {
                    shown = Some(step);
                    let _ = app.emit("update-progress", Progress { done, total });
                }
            },
            || {},
        )
        .await
        .map_err(describe)?;
    update.install(bytes).map_err(describe)?;
    // Windows' installer has closed Telex by now. An AppImage was only replaced on disk, so
    // Telex closes its window and starts the new file itself.
    if cfg!(not(windows)) {
        app.restart();
    }
    Ok(InstallOutcome::default())
}

/// The updater's errors as sentences for the status bar.
fn describe(e: Error) -> String {
    match e {
        // GitHub answered, but not with a release: none published yet, or the repository is private.
        Error::ReleaseNotFound => tr!(
            "GitHub has no published Telex release to update from.",
            "Na GitHubie nie ma opublikowanego wydania Telexa, z którego można się zaktualizować."
        ),
        Error::Reqwest(_) => tr!(
            "Couldn't reach GitHub. Check the internet connection and try again.",
            "Brak połączenia z GitHubem. Sprawdź połączenie z internetem i spróbuj ponownie."
        ),
        // "Download request failed with status: 403 Forbidden"
        Error::Network(e) => {
            let status = e.trim_start_matches("Download request failed with status: ");
            tr!(
                "The download from GitHub failed ({status}). Try again later.",
                "Pobieranie z GitHuba się nie udało ({status}). Spróbuj później."
            )
        }
        Error::Minisign(_)
        | Error::Base64(_)
        | Error::SignatureUtf8(_)
        | Error::SignedVersionMismatch { .. }
        | Error::MissingSignedVersion => tr!(
            "The update isn't signed with Telex's release key, so Telex didn't install it.",
            "Aktualizacja nie ma podpisu Telexa, więc Telex jej nie zainstalował."
        ),
        Error::TargetNotFound(_) | Error::TargetsNotFound(_) => tr!(
            "The newest release has no installer for this kind of Telex installation.",
            "Najnowsze wydanie nie ma instalatora dla tego rodzaju instalacji Telexa."
        ),
        Error::Serialization(_) | Error::Semver(_) => tr!(
            "GitHub's answer about the newest version couldn't be read.",
            "Nie udało się odczytać odpowiedzi GitHuba o najnowszej wersji."
        ),
        e => capitalize(&e.to_string()),
    }
}
