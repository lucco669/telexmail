//! Counting this copy of Telex once a day, after the user said yes (`telex_core::count`).
//! Only release builds count: dev and CI builds would only add noise.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Manager, State};
use telex_core::count::{self, Sent};
use telex_core::settings::UsageCount;

use crate::settings::SettingsState;
use crate::{updates, CmdResult};

/// A count on its way, so two calls don't send the same day twice.
static SENDING: AtomicBool = AtomicBool::new(false);

/// Send today's count when the user said yes and it isn't sent yet. True when one went.
/// A failure is quiet: the next call (the window asks every hour) tries again.
#[tauri::command(async)]
pub fn count_copy(app: AppHandle, settings: State<'_, SettingsState>) -> CmdResult<bool> {
    let chosen = settings.get();
    if chosen.usage_count != UsageCount::On || !updates::release_build() {
        return Ok(false);
    }
    if SENDING.swap(true, Ordering::SeqCst) {
        return Ok(false);
    }
    let sent = (|| {
        let path = app
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?
            .join("count.json");
        let mut sent = Sent::load(&path);
        let today = chrono::Utc::now().date_naive();
        let version = app.package_info().version.to_string();
        let lang = chosen.effective_language(settings.locale());
        let Some(ping) = count::due(&sent, today, &version, count::os(), lang) else {
            return Ok(false);
        };
        count::send(count::ENDPOINT, &ping)?;
        sent.record(&ping, today);
        sent.save(&path).map_err(|e| e.to_string())?;
        Ok(true)
    })();
    SENDING.store(false, Ordering::SeqCst);
    sent
}
