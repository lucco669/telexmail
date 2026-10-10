//! The language of the words the core writes: errors, notes on messages, status lines.
//!
//! The UI says which language the user picked ([`set`]). Every sentence the core writes
//! comes from [`tr!`](crate::tr), which keeps the English and the Polish side by side, so
//! neither can be forgotten. Text from servers (an IMAP refusal, an SMTP error) stays as
//! the server wrote it. Notes already written on messages stay in the language they were
//! written in.

use std::cell::Cell;
use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    En,
    Pl,
}

impl Lang {
    /// "pl" → Polish; anything else is English.
    pub fn from_code(code: &str) -> Self {
        if code.trim().eq_ignore_ascii_case("pl") {
            Self::Pl
        } else {
            Self::En
        }
    }
}

/// The app's language, for every thread.
static LANG: AtomicU8 = AtomicU8::new(0);

thread_local! {
    /// A language for this thread only (see [`with`]).
    static OVERRIDE: Cell<Option<Lang>> = const { Cell::new(None) };
}

/// Use this language from now on.
pub fn set(lang: Lang) {
    LANG.store(lang as u8, Ordering::Relaxed);
}

/// The language words are written in now.
pub fn current() -> Lang {
    OVERRIDE.with(Cell::get).unwrap_or_else(|| {
        if LANG.load(Ordering::Relaxed) == Lang::Pl as u8 {
            Lang::Pl
        } else {
            Lang::En
        }
    })
}

/// Run `f` with another language on this thread only. Tests use it: they run side by
/// side, so they must not change the language of the others.
pub fn with<T>(lang: Lang, f: impl FnOnce() -> T) -> T {
    let before = OVERRIDE.with(|o| o.replace(Some(lang)));
    let out = f();
    OVERRIDE.with(|o| o.set(before));
    out
}

/// Which of the three Polish plural forms a count takes: 1 plik, 2–4 pliki, 5 plików
/// (and 12–14 plików, 22 pliki).
pub fn pl_form(n: u64) -> usize {
    let (ten, hundred) = (n % 10, n % 100);
    if n == 1 {
        0
    } else if (2..=4).contains(&ten) && !(12..=14).contains(&hundred) {
        1
    } else {
        2
    }
}

/// A sentence in the current language: `tr!("Can't reach {host}.", "Brak połączenia z {host}.")`.
/// Both are format strings with the same arguments, named in place or passed after them.
#[macro_export]
macro_rules! tr {
    ($en:literal, $pl:literal $(, $($arg:tt)*)?) => {
        match $crate::i18n::current() {
            $crate::i18n::Lang::Pl => format!($pl $(, $($arg)*)?),
            $crate::i18n::Lang::En => format!($en $(, $($arg)*)?),
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentences_follow_the_language() {
        let host = "mail.x.pl";
        let say = || tr!("Can't reach {host}.", "Brak połączenia z {host}.");
        assert_eq!(with(Lang::En, say), "Can't reach mail.x.pl.");
        assert_eq!(with(Lang::Pl, say), "Brak połączenia z mail.x.pl.");
        assert_eq!(with(Lang::Pl, || tr!("{} of {}", "{} z {}", 2, 5)), "2 z 5");
    }

    #[test]
    fn polish_plurals() {
        let forms = ["plik", "pliki", "plików"];
        let word = |n| forms[pl_form(n)];
        assert_eq!(
            [1, 2, 4, 5, 11, 12, 14, 21, 22, 25, 102, 112].map(word),
            [
                "plik", "pliki", "pliki", "plików", "plików", "plików", "plików", "plików",
                "pliki", "plików", "pliki", "plików"
            ]
        );
        assert_eq!(word(0), "plików");
    }

    #[test]
    fn codes() {
        assert_eq!(Lang::from_code("PL"), Lang::Pl);
        assert_eq!(Lang::from_code("en"), Lang::En);
        assert_eq!(Lang::from_code("de"), Lang::En);
    }
}
