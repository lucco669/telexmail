//! Spelling: which words of a message the dictionaries of the user's languages don't know, and
//! what they might be instead.
//!
//! Two kinds of dictionaries: on Windows its own spell checker (the one Edge uses, a language
//! for each added to Windows), and Hunspell dictionaries: those LibreOffice or the Linux
//! distribution installed, and those Telex fetches when the user asks for a language the
//! system lacks (LibreOffice's, at a fixed version, each file checked against its SHA-256, kept
//! in a folder next to the settings). Nothing is bundled, and nothing is fetched unasked. A word
//! is right when any of the chosen languages knows it, so Polish mail with English words in it
//! is fine. The user's own words ("Add to dictionary") are kept in a file next to the settings
//! and count in every language.
//!
//! What isn't prose is left alone: quoted lines ("> …") and the line that introduces them, a
//! forwarded message, addresses, links, words with digits, ACRONYMS and words like iPhone.
//!
//! The checking runs on a thread of its own that owns the dictionaries: Windows' spell checker
//! is a COM object that stays on the thread that made it. Positions are in UTF-16 units, as
//! JavaScript counts them.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};

use serde::Serialize;

use crate::tr;

/// A language there is a dictionary for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Language {
    /// "pl-PL"
    pub tag: String,
    /// One Telex fetched, which it can remove again.
    pub own: bool,
}

/// Answers about words, a language at a time.
trait Engine {
    fn languages(&mut self) -> Vec<Language>;
    /// Whether `lang` knows `word`; `None` when there is no such dictionary.
    fn knows(&mut self, lang: &str, word: &str) -> Option<bool>;
    fn suggest(&mut self, lang: &str, word: &str) -> Vec<String>;
    /// Dictionaries were added or removed: look again.
    fn forget(&mut self) {}
}

enum Job {
    Languages(Sender<Vec<Language>>),
    Check {
        text: String,
        langs: Vec<String>,
        reply: Sender<Vec<[u32; 2]>>,
    },
    Suggest {
        word: String,
        langs: Vec<String>,
        reply: Sender<Vec<String>>,
    },
    Add {
        word: String,
        reply: Sender<Result<(), String>>,
    },
    Forget,
}

/// The spell checker: a handle to its thread.
pub struct Speller {
    jobs: Sender<Job>,
}

/// At most this many suggestions for a word.
const SUGGESTIONS: usize = 7;
/// Words remembered as right or wrong before the memory starts over.
const CACHE: usize = 50_000;

impl Speller {
    /// Start the checker: `words` is the file of the user's own words, `dictionaries` the
    /// folder of those Telex fetched.
    pub fn start(words: PathBuf, dictionaries: PathBuf) -> Self {
        let (jobs, rx) = mpsc::channel::<Job>();
        // Without a thread every answer is empty, as if there were no dictionaries.
        let _ = std::thread::Builder::new()
            .name("telex-spell".into())
            .spawn(move || {
                let mut worker = Worker::new(Box::new(Engines::new(dictionaries)), words);
                for job in rx {
                    worker.run(job);
                }
            });
        Self { jobs }
    }

    fn ask<T: Default>(&self, job: impl FnOnce(Sender<T>) -> Job) -> T {
        let (tx, rx) = mpsc::channel();
        if self.jobs.send(job(tx)).is_err() {
            return T::default();
        }
        rx.recv().unwrap_or_default()
    }

    /// The languages there are dictionaries for, A to Z.
    pub fn languages(&self) -> Vec<Language> {
        self.ask(Job::Languages)
    }

    /// The words of `text` none of `langs` knows, as [start, length] in UTF-16 units.
    pub fn check(&self, text: &str, langs: &[String]) -> Vec<[u32; 2]> {
        self.ask(|reply| Job::Check {
            text: text.to_owned(),
            langs: langs.to_vec(),
            reply,
        })
    }

    /// What `word` might be instead, from each language in turn.
    pub fn suggest(&self, word: &str, langs: &[String]) -> Vec<String> {
        self.ask(|reply| Job::Suggest {
            word: word.to_owned(),
            langs: langs.to_vec(),
            reply,
        })
    }

    /// Count `word` as right from now on, in every language.
    pub fn add(&self, word: &str) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        let job = Job::Add {
            word: word.to_owned(),
            reply: tx,
        };
        if self.jobs.send(job).is_err() {
            return Err(stopped());
        }
        rx.recv().unwrap_or_else(|_| Err(stopped()))
    }

    /// Dictionaries were fetched or removed: look again.
    pub fn refresh(&self) {
        let _ = self.jobs.send(Job::Forget);
    }
}

fn stopped() -> String {
    tr!(
        "The spell checker stopped. Restart Telex.",
        "Sprawdzanie pisowni przestało działać. Uruchom Telexa ponownie."
    )
}

struct Worker {
    engine: Box<dyn Engine>,
    file: PathBuf,
    /// The user's own words, lowercase.
    own: HashSet<String>,
    /// "word\u{1}pl-PL,en-US" → right or not.
    cache: HashMap<String, bool>,
    languages: Option<Vec<Language>>,
}

impl Worker {
    fn new(engine: Box<dyn Engine>, file: PathBuf) -> Self {
        let own = std::fs::read_to_string(&file)
            .unwrap_or_default()
            .lines()
            .map(|w| w.trim().to_lowercase())
            .filter(|w| !w.is_empty())
            .collect();
        Self {
            engine,
            file,
            own,
            cache: HashMap::new(),
            languages: None,
        }
    }

    fn run(&mut self, job: Job) {
        match job {
            Job::Languages(reply) => {
                let _ = reply.send(self.available().to_vec());
            }
            Job::Check { text, langs, reply } => {
                let _ = reply.send(self.mistakes(&text, &langs));
            }
            Job::Suggest { word, langs, reply } => {
                let _ = reply.send(self.suggestions(&word, &langs));
            }
            Job::Add { word, reply } => {
                let _ = reply.send(self.add(&word));
            }
            Job::Forget => {
                self.engine.forget();
                self.languages = None;
                self.cache.clear();
            }
        }
    }

    fn available(&mut self) -> &[Language] {
        if self.languages.is_none() {
            let mut list = self.engine.languages();
            list.sort_by(|a, b| a.tag.cmp(&b.tag));
            list.dedup_by(|a, b| a.tag == b.tag);
            self.languages = Some(list);
        }
        self.languages.as_deref().unwrap_or_default()
    }

    /// The chosen languages there are dictionaries for.
    fn usable(&mut self, langs: &[String]) -> Vec<String> {
        let available = self.available();
        langs
            .iter()
            .filter(|l| available.iter().any(|a| &a.tag == *l))
            .cloned()
            .collect()
    }

    fn is_right(&mut self, word: &str, langs: &[String]) -> bool {
        if self.own.contains(&word.to_lowercase()) {
            return true;
        }
        let key = format!("{word}\u{1}{}", langs.join(","));
        if let Some(&right) = self.cache.get(&key) {
            return right;
        }
        // "Dzień" at the start of a sentence is "dzień"; "Kraków" stays as it is.
        let lower = lowercase_first(word);
        let right = langs.iter().any(|l| {
            self.engine.knows(l, word) == Some(true)
                || (lower != word && self.engine.knows(l, &lower) == Some(true))
        });
        if self.cache.len() > CACHE {
            self.cache.clear();
        }
        self.cache.insert(key, right);
        right
    }

    fn mistakes(&mut self, text: &str, langs: &[String]) -> Vec<[u32; 2]> {
        let langs = self.usable(langs);
        if langs.is_empty() {
            return Vec::new();
        }
        words(text)
            .into_iter()
            .filter(|w| !self.is_right(w.text, &langs))
            .map(|w| [w.start, w.len])
            .collect()
    }

    fn suggestions(&mut self, word: &str, langs: &[String]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for lang in self.usable(langs) {
            for s in self.engine.suggest(&lang, word) {
                if !out.contains(&s) && s != word {
                    out.push(s);
                }
            }
        }
        out.truncate(SUGGESTIONS);
        out
    }

    fn add(&mut self, word: &str) -> Result<(), String> {
        let word = word.trim();
        if word.is_empty() || word.chars().any(char::is_whitespace) {
            return Ok(());
        }
        if self.own.insert(word.to_lowercase()) {
            self.cache.clear();
            let mut list: Vec<&String> = self.own.iter().collect();
            list.sort();
            let text: String = list.iter().map(|w| format!("{w}\n")).collect();
            let temp = self.file.with_extension("txt.tmp");
            std::fs::write(&temp, text)
                .and_then(|()| std::fs::rename(&temp, &self.file))
                .map_err(|e| {
                    tr!(
                        "Telex couldn't save the word: {e}.",
                        "Telex nie mógł zapisać słowa: {e}."
                    )
                })?;
        }
        Ok(())
    }
}

fn lowercase_first(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// A word to check, where it is in UTF-16 units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Word<'a> {
    text: &'a str,
    start: u32,
    len: u32,
}

fn utf16_len(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}

/// A forwarded message starts with a line like "-------- Forwarded message --------".
fn is_forward_line(line: &str) -> bool {
    let line = line.trim();
    line.starts_with("-----") && line.ends_with("-----") && line.len() > 12
}

/// The words of `text` worth checking.
fn words(text: &str) -> Vec<Word<'_>> {
    let mut out = Vec::new();
    let lines: Vec<&str> = text.split('\n').collect();
    let mut offset = 0u32;
    for (i, line) in lines.iter().enumerate() {
        let quoted = |l: &str| l.trim_start().starts_with('>');
        let introduces_quote =
            line.trim_end().ends_with(':') && lines.get(i + 1).is_some_and(|next| quoted(next));
        if is_forward_line(line) {
            break;
        }
        if !quoted(line) && !introduces_quote {
            line_words(line, offset, &mut out);
        }
        offset += utf16_len(line) + 1;
    }
    out
}

/// The words of one line: runs of letters with an apostrophe between letters ("don't"), in
/// pieces of text between spaces that are prose.
fn line_words<'a>(line: &'a str, offset: u32, out: &mut Vec<Word<'a>>) {
    let mut at = offset;
    for piece in line.split_inclusive(char::is_whitespace) {
        let run = piece.trim_end();
        let skip = run.contains("://")
            || run.starts_with("www.")
            || run.contains('@')
            || run.contains(['/', '\\', '_', '='])
            || run.chars().any(|c| c.is_ascii_digit());
        if !skip {
            letters(run, at, out);
        }
        at += utf16_len(piece);
    }
}

fn letters<'a>(run: &'a str, offset: u32, out: &mut Vec<Word<'a>>) {
    let chars: Vec<(usize, char)> = run.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].1.is_alphabetic() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() {
            let c = chars[i].1;
            let joins = matches!(c, '\'' | '’')
                && i > start
                && chars.get(i + 1).is_some_and(|(_, n)| n.is_alphabetic());
            if c.is_alphabetic() || joins {
                i += 1;
            } else {
                break;
            }
        }
        let (from, to) = (
            chars[start].0,
            chars.get(i).map_or(run.len(), |(at, _)| *at),
        );
        let text = &run[from..to];
        // "e.g.", "i.e.": letters with dots between them are abbreviations.
        let dotted = chars.get(i).is_some_and(|(_, c)| *c == '.')
            && chars.get(i + 1).is_some_and(|(_, c)| c.is_alphabetic());
        if worth_checking(text) && !dotted {
            out.push(Word {
                text,
                start: offset + utf16_len(&run[..from]),
                len: utf16_len(text),
            });
        }
    }
}

/// One letter, ACRONYMS and mixed case (iPhone, McKinsey) aren't checked.
fn worth_checking(word: &str) -> bool {
    let mut chars = word.chars().filter(|c| c.is_alphabetic());
    let Some(first) = chars.next() else {
        return false;
    };
    let _ = first;
    let rest: Vec<char> = chars.collect();
    !rest.is_empty() && !rest.iter().any(|c| c.is_uppercase())
}

/// The languages to check in when the user hasn't chosen: Windows' display languages and its
/// regional format, then the interface's, as far as there are dictionaries for them.
pub fn default_languages(available: &[String], wanted: &[String]) -> Vec<String> {
    let base = |tag: &str| {
        tag.split(['-', '_'])
            .next()
            .unwrap_or(tag)
            .to_ascii_lowercase()
    };
    let mut out: Vec<String> = Vec::new();
    // Exactly the languages asked for first.
    for tag in wanted {
        if let Some(found) = available.iter().find(|a| a.eq_ignore_ascii_case(tag)) {
            if !out.contains(found) {
                out.push(found.clone());
            }
        }
    }
    // Then, for a language none of those covers, another country's: its own country's first
    // (pl-PL, de-DE), then the United States' and Britain's, then any.
    for tag in wanted {
        let lang = base(tag);
        if out.iter().any(|o| base(o) == lang) {
            continue;
        }
        let same: Vec<&String> = available.iter().filter(|a| base(a) == lang).collect();
        let preferred = [
            format!("{lang}-{}", lang.to_ascii_uppercase()),
            "en-US".into(),
            "en-GB".into(),
        ];
        let found = preferred
            .iter()
            .find_map(|p| same.iter().find(|a| a.eq_ignore_ascii_case(p)))
            .or_else(|| same.first());
        if let Some(found) = found {
            out.push((*found).clone());
        }
    }
    out
}

// ---- The dictionaries ----

/// Windows' spell checker first, for the languages it has; Hunspell dictionaries for the rest.
struct Engines {
    system: Option<Box<dyn Engine>>,
    hunspell: hunspell::Hunspell,
}

impl Engines {
    fn new(own: PathBuf) -> Self {
        Self {
            system: system_engine(),
            hunspell: hunspell::Hunspell::new(own, hunspell::system_dirs()),
        }
    }

    fn in_system(&mut self, lang: &str) -> bool {
        self.system
            .as_mut()
            .is_some_and(|s| s.languages().iter().any(|l| l.tag == lang))
    }
}

impl Engine for Engines {
    fn languages(&mut self) -> Vec<Language> {
        let mut all = self
            .system
            .as_mut()
            .map(|s| s.languages())
            .unwrap_or_default();
        for l in self.hunspell.languages() {
            if !all.iter().any(|a| a.tag == l.tag) {
                all.push(l);
            }
        }
        all
    }

    fn knows(&mut self, lang: &str, word: &str) -> Option<bool> {
        if let Some(answer) = self.system.as_mut().and_then(|s| s.knows(lang, word)) {
            return Some(answer);
        }
        self.hunspell.knows(lang, word)
    }

    fn suggest(&mut self, lang: &str, word: &str) -> Vec<String> {
        if self.in_system(lang) {
            if let Some(s) = self.system.as_mut() {
                return s.suggest(lang, word);
            }
        }
        self.hunspell.suggest(lang, word)
    }

    fn forget(&mut self) {
        self.hunspell.forget();
    }
}

#[cfg(windows)]
fn system_engine() -> Option<Box<dyn Engine>> {
    windows_engine::WindowsEngine::new().map(|e| Box::new(e) as Box<dyn Engine>)
}

#[cfg(not(windows))]
fn system_engine() -> Option<Box<dyn Engine>> {
    None
}

#[cfg(windows)]
mod windows_engine {
    use std::collections::HashMap;

    use windows::core::{HSTRING, PWSTR};
    use windows::Win32::Globalization::{
        ISpellChecker, ISpellCheckerFactory, ISpellingError, SpellCheckerFactory,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, IEnumString, CLSCTX_INPROC_SERVER,
        COINIT_MULTITHREADED,
    };

    use super::{Engine, Language, SUGGESTIONS};

    pub struct WindowsEngine {
        factory: ISpellCheckerFactory,
        checkers: HashMap<String, Option<ISpellChecker>>,
        languages: Option<Vec<Language>>,
    }

    impl WindowsEngine {
        pub fn new() -> Option<Self> {
            // SAFETY: this thread is the checker's own and joins the multithreaded apartment
            // once; the factory and its checkers are used on it only.
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                let factory: ISpellCheckerFactory =
                    CoCreateInstance(&SpellCheckerFactory, None, CLSCTX_INPROC_SERVER).ok()?;
                Some(Self {
                    factory,
                    checkers: HashMap::new(),
                    languages: None,
                })
            }
        }

        fn checker(&mut self, lang: &str) -> Option<&ISpellChecker> {
            let factory = &self.factory;
            self.checkers
                .entry(lang.to_owned())
                .or_insert_with(|| {
                    let tag = HSTRING::from(lang);
                    // SAFETY: plain calls on the factory with a valid string.
                    unsafe {
                        if !factory.IsSupported(&tag).ok()?.as_bool() {
                            return None;
                        }
                        factory.CreateSpellChecker(&tag).ok()
                    }
                })
                .as_ref()
        }
    }

    /// The strings of an IEnumString, each freed once read.
    fn strings(list: &IEnumString, limit: usize) -> Vec<String> {
        let mut out = Vec::new();
        while out.len() < limit {
            let mut item = [PWSTR::null()];
            let mut fetched = 0u32;
            // SAFETY: room for one string; Windows allocates it with CoTaskMemAlloc, so it is
            // freed here after being copied.
            unsafe {
                if list.Next(&mut item, Some(&mut fetched)).is_err() || fetched == 0 {
                    break;
                }
                if let Ok(s) = item[0].to_string() {
                    out.push(s);
                }
                CoTaskMemFree(Some(item[0].0 as *const _));
            }
        }
        out
    }

    impl Engine for WindowsEngine {
        fn languages(&mut self) -> Vec<Language> {
            let factory = &self.factory;
            self.languages
                .get_or_insert_with(|| {
                    // SAFETY: a plain call on the factory.
                    let list = unsafe { factory.SupportedLanguages() };
                    list.map(|l| strings(&l, 200))
                        .unwrap_or_default()
                        .into_iter()
                        .map(|tag| Language { tag, own: false })
                        .collect()
                })
                .clone()
        }

        fn knows(&mut self, lang: &str, word: &str) -> Option<bool> {
            let checker = self.checker(lang)?;
            // SAFETY: a plain call with a valid string; the enumerator is read once.
            unsafe {
                let errors = checker.Check(&HSTRING::from(word)).ok()?;
                let mut first: Option<ISpellingError> = None;
                let found = errors.Next(&mut first).is_ok() && first.is_some();
                Some(!found)
            }
        }

        fn suggest(&mut self, lang: &str, word: &str) -> Vec<String> {
            let Some(checker) = self.checker(lang) else {
                return Vec::new();
            };
            // SAFETY: a plain call with a valid string.
            match unsafe { checker.Suggest(&HSTRING::from(word)) } {
                Ok(list) => strings(&list, SUGGESTIONS),
                Err(_) => Vec::new(),
            }
        }
    }
}

mod hunspell {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    use super::{Engine, Language, SUGGESTIONS};

    pub struct Hunspell {
        /// Telex's own folder first, then the system's.
        own: PathBuf,
        dirs: Vec<PathBuf>,
        files: Option<Vec<(Language, PathBuf)>>,
        loaded: HashMap<String, Option<spellbook::Dictionary>>,
    }

    /// Where the system keeps Hunspell dictionaries: LibreOffice's extensions on Windows, the
    /// distribution's folders elsewhere.
    pub fn system_dirs() -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        if cfg!(windows) {
            for var in ["ProgramFiles", "ProgramFiles(x86)"] {
                let Some(base) = std::env::var_os(var) else {
                    continue;
                };
                let extensions = Path::new(&base).join(r"LibreOffice\share\extensions");
                for entry in std::fs::read_dir(extensions)
                    .into_iter()
                    .flatten()
                    .flatten()
                {
                    if entry.file_name().to_string_lossy().starts_with("dict-") {
                        dirs.push(entry.path());
                    }
                }
            }
        } else {
            dirs.extend(
                [
                    "/usr/share/hunspell",
                    "/usr/share/myspell",
                    "/usr/share/myspell/dicts",
                    "/usr/local/share/hunspell",
                ]
                .iter()
                .map(PathBuf::from),
            );
            if let Some(home) = std::env::var_os("HOME") {
                dirs.push(Path::new(&home).join(".local/share/hunspell"));
            }
        }
        dirs
    }

    /// "de_DE_frami" → "de-DE": the language and the country; variants count as the language.
    pub fn tag_of(stem: &str) -> Option<String> {
        let mut parts = stem.split(['_', '-']);
        let lang = parts.next().filter(|l| (2..=3).contains(&l.len()))?;
        Some(match parts.next() {
            Some(c) if c.len() == 2 && c.chars().all(|c| c.is_ascii_uppercase()) => {
                format!("{}-{c}", lang.to_ascii_lowercase())
            }
            _ => lang.to_ascii_lowercase(),
        })
    }

    impl Hunspell {
        pub fn new(own: PathBuf, system: Vec<PathBuf>) -> Self {
            let mut dirs = vec![own.clone()];
            dirs.extend(system);
            Self {
                own,
                dirs,
                files: None,
                loaded: HashMap::new(),
            }
        }

        /// "pl_PL.dic" with its "pl_PL.aff", by language; the first found for each.
        fn files(&mut self) -> &[(Language, PathBuf)] {
            if self.files.is_none() {
                let mut out: Vec<(Language, PathBuf)> = Vec::new();
                for dir in &self.dirs {
                    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
                        .into_iter()
                        .flatten()
                        .flatten()
                        .map(|e| e.path())
                        .filter(|p| {
                            p.extension().is_some_and(|e| e == "dic")
                                && p.with_extension("aff").is_file()
                        })
                        .collect();
                    found.sort();
                    for path in found {
                        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                        let Some(tag) = tag_of(stem) else { continue };
                        if !out.iter().any(|(l, _)| l.tag == tag) {
                            let own = *dir == self.own;
                            out.push((Language { tag, own }, path));
                        }
                    }
                }
                self.files = Some(out);
            }
            self.files.as_deref().unwrap_or_default()
        }

        fn dictionary(&mut self, lang: &str) -> Option<&spellbook::Dictionary> {
            if !self.loaded.contains_key(lang) {
                let file = self
                    .files()
                    .iter()
                    .find(|(l, _)| l.tag == lang)
                    .map(|(_, p)| p.clone());
                let dict = file.and_then(|dic| load(&dic));
                self.loaded.insert(lang.to_owned(), dict);
            }
            self.loaded.get(lang)?.as_ref()
        }
    }

    fn load(dic: &Path) -> Option<spellbook::Dictionary> {
        let aff = std::fs::read(dic.with_extension("aff")).ok()?;
        // The charset line is ASCII whatever the charset is.
        let charset = String::from_utf8_lossy(&aff)
            .lines()
            .find_map(|l| l.strip_prefix("SET "))
            .map(|c| c.trim().to_owned());
        let mut aff = utf8(&aff, charset.as_deref());
        if let Some(c) = &charset {
            aff = aff.replacen(&format!("SET {c}"), "SET UTF-8", 1);
        }
        let dic = utf8(&std::fs::read(dic).ok()?, charset.as_deref());
        spellbook::Dictionary::new(&aff, &dic).ok()
    }

    /// Dictionary files in their own charset ("SET ISO8859-2") as UTF-8.
    fn utf8(bytes: &[u8], charset: Option<&str>) -> String {
        match charset.map(str::to_ascii_lowercase).as_deref() {
            None | Some("utf-8") | Some("utf8") => String::from_utf8_lossy(bytes).into_owned(),
            Some(name) => {
                let name = name.replace("iso8859", "iso-8859");
                match mail_parser::decoders::charsets::map::charset_decoder(name.as_bytes()) {
                    Some(decode) => decode(bytes),
                    None => String::from_utf8_lossy(bytes).into_owned(),
                }
            }
        }
    }

    impl Engine for Hunspell {
        fn languages(&mut self) -> Vec<Language> {
            self.files().iter().map(|(l, _)| l.clone()).collect()
        }

        fn knows(&mut self, lang: &str, word: &str) -> Option<bool> {
            Some(self.dictionary(lang)?.check(word))
        }

        fn suggest(&mut self, lang: &str, word: &str) -> Vec<String> {
            let mut out = Vec::new();
            if let Some(dict) = self.dictionary(lang) {
                dict.suggest(word, &mut out);
            }
            out.truncate(SUGGESTIONS);
            out
        }

        fn forget(&mut self) {
            self.files = None;
            self.loaded.clear();
        }
    }
}

// ---- Dictionaries Telex fetches ----

/// LibreOffice's dictionaries, at a fixed version: the same files every time, so each can be
/// checked against the SHA-256 written here before it is used.
const DICTIONARIES_AT: &str =
    "https://raw.githubusercontent.com/LibreOffice/dictionaries/32b006a2c22a4ac7e8ed3f03346f7b3d85a970a4/";

/// A dictionary Telex can fetch: where it is in LibreOffice's collection, and its files' hashes.
pub struct Download {
    pub tag: &'static str,
    path: &'static str,
    aff: &'static str,
    dic: &'static str,
    /// Both files together.
    pub bytes: u64,
}

pub const DOWNLOADS: &[Download] = &[
    Download {
        tag: "pl-PL",
        path: "pl_PL/pl_PL",
        aff: "82973651651aa930335c865b339b98db376ca3dbf3a661b70b9eeb71fdf41dca",
        dic: "c0848440599eb88e5aca500418d5f389e562ec2c157b63dbe39d354658ffba49",
        bytes: 257_542 + 5_252_096,
    },
    Download {
        tag: "en-US",
        path: "en/en_US",
        aff: "e746c882dd6f303c2c46e7452804b9201115a6942cfeb15f18f8edf774d2e24e",
        dic: "f0b1a234bd178bdd01875b2a392a9647f888b8fe879f79c52aae62c2759b3647",
        bytes: 3_205 + 551_762,
    },
    Download {
        tag: "en-GB",
        path: "en/en_GB",
        aff: "0fd6ed120ef28957847d98ba5149b117e27116cf81b5aa36208453f6755a36fd",
        dic: "04e90f34f5263bf26780e9c4a442e9ad16584e227af49ddd1b3b21b01df5b29c",
        bytes: 44_158 + 1_230_571,
    },
    Download {
        tag: "de-DE",
        path: "de/de_DE_frami",
        aff: "646bf3333ac69c23e9d794533ee5241d6f755c359e8fe10a648f87613743d594",
        dic: "4ca3c958b0e5545910999bc246f668840bf8ede3df8e5e6790d05edd5a586c38",
        bytes: 19_067 + 4_356_903,
    },
    Download {
        tag: "uk-UA",
        path: "uk_UA/uk_UA",
        aff: "2219dd15e9802adebc45722c60943b1472640260491af38dd3e43b07e75585e6",
        dic: "2e5a9e67be63bdb089b3459addb5d71113319d13768e277bcae20f3cc1ad5a93",
        bytes: 211_759 + 8_919_324,
    },
    Download {
        tag: "cs-CZ",
        path: "cs_CZ/cs_CZ",
        aff: "7ecb20620ecd46ebd9c36f3f33e69dd4eda385cba5b2bb4e6bc396d910e297f7",
        dic: "d8e8c88c006fdae72dac8c85df11b0c99a773e05a4ab0fcbe92244876668ca74",
        bytes: 111_575 + 3_656_362,
    },
    Download {
        tag: "es-ES",
        path: "es/es_ES",
        aff: "e73a9bf8e1383f4986a5dc9e2fbed49371c0c61f511c626d15586bd433c1cad9",
        dic: "6975dddec3d5d2c676069537bc67b4b5f786c65c5d4cf6703a82acf779ac9ec1",
        bytes: 169_202 + 715_989,
    },
];

/// The largest file Telex takes, well above the biggest dictionary.
const MAX_FILE: u64 = 32 * 1024 * 1024;

fn sha256_hex(data: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, data)
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Fetch the dictionary for `tag` into `dir`. Each file must match its hash, and is written
/// whole or not at all. Only GitHub is asked, and only for these files; nothing about the user
/// goes with the request.
pub fn download(dir: &Path, tag: &str) -> Result<(), String> {
    let wanted = DOWNLOADS.iter().find(|d| d.tag == tag).ok_or_else(|| {
        tr!(
            "Telex has no dictionary to fetch for {tag}.",
            "Telex nie ma słownika do pobrania dla {tag}."
        )
    })?;
    let failed = |e: String| {
        tr!(
            "The dictionary didn't download: {e}.",
            "Słownik się nie pobrał: {e}."
        )
    };
    std::fs::create_dir_all(dir).map_err(|e| failed(e.to_string()))?;
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(180)))
        .max_redirects(0)
        .user_agent("Telex")
        .build();
    let agent = ureq::Agent::new_with_config(config);
    let stem = tag.replace('-', "_");
    let mut fetched = Vec::new();
    for (ext, sha) in [("aff", wanted.aff), ("dic", wanted.dic)] {
        let url = format!("{DICTIONARIES_AT}{}.{ext}", wanted.path);
        let data = agent
            .get(&url)
            .call()
            .map_err(|e| failed(e.to_string()))?
            .body_mut()
            .with_config()
            .limit(MAX_FILE)
            .read_to_vec()
            .map_err(|e| failed(e.to_string()))?;
        if sha256_hex(&data) != sha {
            return Err(failed(tr!(
                "the file isn't the one Telex expects",
                "plik nie jest tym, którego Telex oczekuje"
            )));
        }
        fetched.push((dir.join(format!("{stem}.{ext}")), data));
    }
    for (path, data) in fetched {
        let temp = path.with_extension("part");
        std::fs::write(&temp, data)
            .and_then(|()| std::fs::rename(&temp, &path))
            .map_err(|e| failed(e.to_string()))?;
    }
    Ok(())
}

/// Remove a dictionary Telex fetched.
pub fn remove(dir: &Path, tag: &str) -> Result<(), String> {
    let stem = tag.replace('-', "_");
    for ext in ["aff", "dic"] {
        match std::fs::remove_file(dir.join(format!("{stem}.{ext}"))) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                return Err(tr!(
                    "Telex couldn't remove the dictionary: {e}.",
                    "Telex nie mógł usunąć słownika: {e}."
                ))
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A dictionary from a list of words, for testing the rest.
    struct Known(HashMap<String, HashSet<&'static str>>);

    impl Engine for Known {
        fn languages(&mut self) -> Vec<Language> {
            self.0
                .keys()
                .map(|tag| Language {
                    tag: tag.clone(),
                    own: false,
                })
                .collect()
        }
        fn knows(&mut self, lang: &str, word: &str) -> Option<bool> {
            Some(self.0.get(lang)?.contains(word))
        }
        fn suggest(&mut self, lang: &str, word: &str) -> Vec<String> {
            let mut s: Vec<String> = self
                .0
                .get(lang)
                .into_iter()
                .flatten()
                .filter(|k| k.len() == word.len() && k.chars().next() == word.chars().next())
                .map(|k| (*k).to_owned())
                .collect();
            s.sort();
            s
        }
    }

    fn worker(file: PathBuf) -> Worker {
        let pl: HashSet<&str> = [
            "dzień",
            "dobry",
            "w",
            "załączniku",
            "wycena",
            "pozdrawiam",
            "jest",
            "ok",
            "pani",
            "anna",
            "kraków",
        ]
        .into();
        let en: HashSet<&str> = ["meeting", "deadline", "the", "is", "don't"].into();
        Worker::new(
            Box::new(Known(HashMap::from([
                ("pl-PL".to_owned(), pl),
                ("en-US".to_owned(), en),
            ]))),
            file,
        )
    }

    fn marked(text: &str, found: &[[u32; 2]]) -> Vec<String> {
        let units: Vec<u16> = text.encode_utf16().collect();
        found
            .iter()
            .map(|[s, l]| String::from_utf16_lossy(&units[*s as usize..(*s + *l) as usize]))
            .collect()
    }

    #[test]
    fn misspelt_words_are_found_where_javascript_counts_them() {
        let file = std::env::temp_dir().join(format!("telex-words-{}.txt", std::process::id()));
        let _ = std::fs::remove_file(&file);
        let mut w = worker(file.clone());
        let both = ["pl-PL".to_owned(), "en-US".to_owned()];
        let text = "Dzień dobry 😀 Pani Anno,\nw załącznikku wycena, deadline is Monday.\n\nPozdrawiam\nAnna";
        let found = w.mistakes(text, &both);
        assert_eq!(marked(text, &found), ["Anno", "załącznikku", "Monday"]);
        // English alone: the Polish words are wrong.
        assert_eq!(w.mistakes("Dzień dobry", &["en-US".to_owned()]).len(), 2);
        // A language without a dictionary: nothing is checked.
        assert!(w.mistakes("Dzień dobri", &["de-DE".to_owned()]).is_empty());
        // Suggestions from each language, and the user's own words.
        assert_eq!(w.suggestions("wycana", &both), ["wycena"]);
        w.add("Monday").unwrap();
        assert_eq!(
            marked(text, &w.mistakes(text, &both)),
            ["Anno", "załącznikku"]
        );
        let again = worker(file.clone());
        assert!(again.own.contains("monday"), "kept in the file");
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn what_isnt_prose_is_left_alone() {
        let text = "Ok, see www.lindenstudio.com and https://x.pl/a, anna@x.pl, C:\\Temp, v2 10:30 ABC iPhone e.g. snake_case a=b.\n\
            Dnia 10.10.2026 o 06:12, Anna Kowalska napisał(a):\n> Gdzie jest wycenna?\n> \n\
            kotek\n-------- Przekazana wiadomość --------\nOd: Ktośtam\nBłendy";
        let found: Vec<&str> = words(text).iter().map(|w| w.text).collect();
        assert_eq!(found, ["Ok", "see", "and", "kotek"]);
        // An apostrophe between letters belongs to the word; quotes around one don't.
        let found: Vec<&str> = words("don't 'quoted' rock’n’roll")
            .iter()
            .map(|w| w.text)
            .collect();
        assert_eq!(found, ["don't", "quoted", "rock’n’roll"]);
    }

    #[test]
    fn languages_follow_the_system_when_not_chosen() {
        let available: Vec<String> = ["de-DE", "en-GB", "en-US", "pl-PL"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let wanted: Vec<String> = ["en-US", "pl-PL", "pl"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        assert_eq!(default_languages(&available, &wanted), ["en-US", "pl-PL"]);
        let wanted = vec!["en-AU".to_owned(), "de".to_owned()];
        assert_eq!(
            default_languages(&available, &wanted),
            ["en-US", "de-DE"],
            "the same language, another country"
        );
        // Windows in US English, the interface in English: one English, not two.
        let windows: Vec<String> = ["en-CA", "en-LR", "en-PH", "en-US"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let wanted = vec!["en-US".to_owned(), "en".to_owned()];
        assert_eq!(default_languages(&windows, &wanted), ["en-US"]);
    }

    #[test]
    fn hunspell_dictionaries_are_read_in_their_charset() {
        let dir = std::env::temp_dir().join(format!("telex-hunspell-{}", std::process::id()));
        let own = dir.join("own");
        std::fs::create_dir_all(&own).unwrap();
        // Polish dictionaries came in ISO-8859-2.
        std::fs::write(
            dir.join("pl_PL.aff"),
            b"SET ISO8859-2\nTRY aeiouwz\xb3\n\nSFX A Y 1\nSFX A 0 y .\n",
        )
        .unwrap();
        std::fs::write(dir.join("pl_PL.dic"), b"3\nkot/A\n\xbf\xf3\xb3w\nma\n").unwrap();
        // Telex names what it fetches by the language alone.
        std::fs::write(own.join("de_DE.aff"), "SET UTF-8\n").unwrap();
        std::fs::write(own.join("de_DE.dic"), "1\nKatze\n").unwrap();
        let mut h = hunspell::Hunspell::new(own.clone(), vec![dir.clone()]);
        let mut langs = h.languages();
        langs.sort_by(|a, b| a.tag.cmp(&b.tag));
        assert_eq!(
            langs,
            [
                Language {
                    tag: "de-DE".into(),
                    own: true
                },
                Language {
                    tag: "pl-PL".into(),
                    own: false
                }
            ]
        );
        assert_eq!(h.knows("pl-PL", "żółw"), Some(true));
        assert_eq!(h.knows("pl-PL", "koty"), Some(true), "affix rules work");
        assert_eq!(h.knows("pl-PL", "kotx"), Some(false));
        assert!(h.suggest("pl-PL", "żółv").contains(&"żółw".to_owned()));
        assert_eq!(h.knows("de-DE", "Katze"), Some(true));
        assert_eq!(h.knows("fr-FR", "chat"), None);
        // Removing one Telex fetched, and looking again.
        remove(&own, "de-DE").unwrap();
        assert_eq!(h.knows("de-DE", "Katze"), Some(true), "still loaded");
        h.forget();
        assert_eq!(h.knows("de-DE", "Katze"), None);
        assert_eq!(hunspell::tag_of("en_US"), Some("en-US".into()));
        assert_eq!(hunspell::tag_of("hyph_pl_PL"), None);
        assert_eq!(hunspell::tag_of("de_DE_frami"), Some("de-DE".into()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_dictionary_is_checked_before_it_is_kept() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(DOWNLOADS
            .iter()
            .all(|d| d.aff.len() == 64 && d.dic.len() == 64));
        let dir = std::env::temp_dir().join(format!("telex-nodict-{}", std::process::id()));
        assert!(download(&dir, "xx-XX").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Fetches the US English dictionary from GitHub (needs the internet).
    #[test]
    #[ignore]
    fn a_dictionary_downloads_and_loads() {
        let dir = std::env::temp_dir().join(format!("telex-dicts-{}", std::process::id()));
        download(&dir, "en-US").unwrap();
        let mut h = hunspell::Hunspell::new(dir.clone(), Vec::new());
        assert_eq!(h.knows("en-US", "meeting"), Some(true));
        assert_eq!(h.knows("en-US", "meetting"), Some(false));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Windows' own checker, where this PC has English.
    #[cfg(windows)]
    #[test]
    fn windows_spell_checker_answers() {
        let Some(mut engine) = windows_engine::WindowsEngine::new() else {
            return;
        };
        if !engine.languages().iter().any(|l| l.tag == "en-US") {
            return;
        }
        assert_eq!(engine.knows("en-US", "meeting"), Some(true));
        assert_eq!(engine.knows("en-US", "meetting"), Some(false));
        assert!(engine
            .suggest("en-US", "meetting")
            .contains(&"meeting".to_owned()));
    }
}
