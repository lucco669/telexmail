//! Spelling while writing. The checker is the core's (`telex_core::spell`), on a thread of its
//! own; these commands ask it in the languages the settings choose, and fetch or remove the
//! dictionaries Telex keeps (only when the user asks).

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use telex_core::spell::{self, Language, Speller};

use crate::settings::SettingsState;
use crate::CmdResult;

pub struct SpellState {
    speller: Speller,
    /// The dictionaries Telex fetched: "dictionaries" next to the settings.
    dir: PathBuf,
}

impl SpellState {
    /// `dir` is where the settings are: the user's words and Telex's dictionaries go there.
    pub fn start(dir: &std::path::Path) -> Self {
        let own = dir.join("dictionaries");
        Self {
            speller: Speller::start(dir.join("words.txt"), own.clone()),
            dir: own,
        }
    }
}

/// A dictionary Telex can fetch, and how big it is.
#[derive(Debug, Serialize)]
pub struct Offer {
    tag: &'static str,
    bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct SpellLanguages {
    /// Every language there is a dictionary for.
    languages: Vec<Language>,
    /// The ones checked now: the settings' choice, or the automatic one.
    chosen: Vec<String>,
    /// The choice is automatic (the settings name none).
    auto: bool,
    /// Dictionaries Telex can fetch, for languages the system lacks.
    downloads: Vec<Offer>,
    /// Of those, the ones for languages the user writes in (Windows' language list, its
    /// display and regional languages, the interface's): worth offering unasked.
    missing: Vec<&'static str>,
}

/// The languages to check in: the user's, or Windows' display languages, the languages the
/// user writes in (Windows' language list), its regional format and the interface's language,
/// as far as there are dictionaries.
fn chosen(app: &AppHandle, languages: &[Language]) -> (Vec<String>, bool) {
    let current = app.state::<SettingsState>().get();
    if !current.spell_languages.is_empty() {
        return (current.spell_languages, false);
    }
    let available: Vec<String> = languages.iter().map(|l| l.tag.clone()).collect();
    (spell::default_languages(&available, &wanted(app)), true)
}

/// The languages the user is likely to write in, from Windows and the interface.
fn wanted(app: &AppHandle) -> Vec<String> {
    let settings = app.state::<SettingsState>();
    let locale = settings.locale();
    let mut wanted = locale.languages.clone();
    wanted.extend(locale.writing.iter().cloned());
    wanted.extend(locale.region.clone());
    wanted.push(settings.get().effective_language(locale).to_owned());
    wanted
}

fn view(app: &AppHandle) -> SpellLanguages {
    let state = app.state::<SpellState>();
    let languages = state.speller.languages();
    let (chosen, auto) = chosen(app, &languages);
    let base = |tag: &str| tag.split('-').next().unwrap_or(tag).to_ascii_lowercase();
    let offers: Vec<&spell::Download> = spell::DOWNLOADS
        .iter()
        .filter(|d| !languages.iter().any(|l| base(&l.tag) == base(d.tag)))
        .collect();
    let wanted: Vec<String> = wanted(app).iter().map(|w| base(w)).collect();
    let missing = offers
        .iter()
        .filter(|d| wanted.contains(&base(d.tag)))
        .map(|d| d.tag)
        .collect();
    SpellLanguages {
        languages,
        chosen,
        auto,
        downloads: offers
            .iter()
            .map(|d| Offer {
                tag: d.tag,
                bytes: d.bytes,
            })
            .collect(),
        missing,
    }
}

/// Run `f` off the async runtime: the checker answers through a channel.
async fn blocking<T: Send + 'static>(
    app: &AppHandle,
    f: impl FnOnce(&AppHandle) -> T + Send + 'static,
) -> CmdResult<T> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || f(&app))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn spell_languages(app: AppHandle) -> CmdResult<SpellLanguages> {
    blocking(&app, view).await
}

/// The misspelt words of `text`, as [start, length] in UTF-16 units.
#[tauri::command]
pub async fn spell_check(app: AppHandle, text: String) -> CmdResult<Vec<[u32; 2]>> {
    blocking(&app, move |app| {
        let state = app.state::<SpellState>();
        let (langs, _) = chosen(app, &state.speller.languages());
        state.speller.check(&text, &langs)
    })
    .await
}

#[tauri::command]
pub async fn spell_suggest(app: AppHandle, word: String) -> CmdResult<Vec<String>> {
    blocking(&app, move |app| {
        let state = app.state::<SpellState>();
        let (langs, _) = chosen(app, &state.speller.languages());
        state.speller.suggest(&word, &langs)
    })
    .await
}

/// Count a word as right from now on, in every language.
#[tauri::command]
pub async fn spell_add(app: AppHandle, word: String) -> CmdResult<()> {
    blocking(&app, move |app| {
        app.state::<SpellState>().speller.add(&word)
    })
    .await?
}

/// Fetch the dictionary for a language the system lacks. The new list comes back.
#[tauri::command]
pub async fn spell_download(app: AppHandle, tag: String) -> CmdResult<SpellLanguages> {
    blocking(&app, move |app| {
        let state = app.state::<SpellState>();
        spell::download(&state.dir, &tag)?;
        state.speller.refresh();
        Ok(view(app))
    })
    .await?
}

/// Remove a dictionary Telex fetched. The new list comes back.
#[tauri::command]
pub async fn spell_remove(app: AppHandle, tag: String) -> CmdResult<SpellLanguages> {
    blocking(&app, move |app| {
        let state = app.state::<SpellState>();
        spell::remove(&state.dir, &tag)?;
        state.speller.refresh();
        Ok(view(app))
    })
    .await?
}
