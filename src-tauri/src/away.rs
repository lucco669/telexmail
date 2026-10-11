//! The away reply's commands: one account's, read and saved. Saving puts it on the server
//! (ManageSieve) where the server takes it, else Telex answers. The logic is in
//! `telex_core::{away, sieve}`.

use serde::Serialize;
use tauri::State;
use telex_core::away::{self, Away, Place};

use crate::{secrets, with_store, AppState, CmdResult};

/// Why the server didn't take the reply (or didn't let go of it), as a code for the UI and the
/// server's own words.
#[derive(Debug, Serialize)]
pub struct Why {
    code: &'static str,
    detail: String,
}

#[derive(Debug, Serialize)]
pub struct AwayView {
    away: Away,
    why: Option<Why>,
}

#[tauri::command]
pub fn get_away(state: State<'_, AppState>, account: String) -> CmdResult<Away> {
    with_store(&state, |s| s.away(&account))
}

/// Save an account's away reply. The server's part is Telex's business, so what the window
/// sends about it is replaced by what the mail base knows.
#[tauri::command]
pub async fn save_away(state: State<'_, AppState>, away: Away) -> CmdResult<AwayView> {
    away.check()?;
    let (account, before) = with_store(&state, |s| {
        Ok((s.account(&away.account)?.clone(), s.away(&away.account)?))
    })?;
    let wanted = Away {
        place: before.place,
        script: before.script,
        created: before.created,
        ..away
    };
    let (saved, why) = match &account.imap {
        Some(imap) => match secrets::load_password(&account.email) {
            Ok(password) => {
                let (saved, why) = away::apply(&account, imap, &password, wanted).await;
                (
                    saved,
                    why.map(|e| Why {
                        code: e.code(),
                        detail: e.to_string(),
                    }),
                )
            }
            // Without the password the server can't be reached: it stays as it was.
            Err(e) => (
                wanted,
                Some(Why {
                    code: "login",
                    detail: e,
                }),
            ),
        },
        None => (
            Away {
                place: Place::Telex,
                ..wanted
            },
            None,
        ),
    };
    with_store(&state, |s| s.save_away(&saved))?;
    Ok(AwayView { away: saved, why })
}
