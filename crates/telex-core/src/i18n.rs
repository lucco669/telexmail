//! The language of the words the core writes: errors, notes on messages, status lines.
//!
//! The UI says which language the user picked ([`set`]). Every sentence the core writes
//! comes from [`tr!`](crate::tr), which keeps the English and the Polish side by side in the
//! code, so neither can be forgotten; the other languages come from catalogs keyed by the
//! English, `crates/telex-core/i18n/<code>.json`, compiled in by the `telex-i18n` macros. A
//! sentence a catalog doesn't have yet is written in English. Text from servers (an IMAP
//! refusal, an SMTP error) stays as the server wrote it. Notes already written on messages
//! stay in the language they were written in.
//!
//! Adding a sentence: `tr!("English", "Polski")`; `node scripts/check-i18n.mjs` lists the
//! catalogs that lack it. Adding a language: a variant in [`Lang`] (with its code, plural
//! rule and place in [`Lang::ALL`]), its catalog, and its line in `telex-i18n`'s `CATALOGS`.

use std::cell::Cell;
use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

#[doc(hidden)]
pub use telex_i18n::{tr_in, trn_in};

// The catalogs are read by the macros; this makes Cargo build again when one changes.
const _: [&str; 7] = [
    include_str!("../i18n/de.json"),
    include_str!("../i18n/cs.json"),
    include_str!("../i18n/sk.json"),
    include_str!("../i18n/uk.json"),
    include_str!("../i18n/es.json"),
    include_str!("../i18n/fr.json"),
    include_str!("../i18n/it.json"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    En,
    Pl,
    De,
    Cs,
    Sk,
    Uk,
    Es,
    Fr,
    It,
}

/// Which form a word takes after a number (CLDR's categories, for whole numbers).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plural {
    One,
    Few,
    Many,
    Other,
}

impl Lang {
    /// Every language, in the order of the enum (its number in [`set`]).
    pub const ALL: [Lang; 9] = [
        Lang::En,
        Lang::Pl,
        Lang::De,
        Lang::Cs,
        Lang::Sk,
        Lang::Uk,
        Lang::Es,
        Lang::Fr,
        Lang::It,
    ];

    /// "en", "pl", "de", "cs", "sk", "uk", "es", "fr", "it": the settings' `language`.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Pl => "pl",
            Lang::De => "de",
            Lang::Cs => "cs",
            Lang::Sk => "sk",
            Lang::Uk => "uk",
            Lang::Es => "es",
            Lang::Fr => "fr",
            Lang::It => "it",
        }
    }

    /// "pl" → Polish, "de" → German…; anything Telex doesn't speak is English.
    pub fn from_code(code: &str) -> Self {
        let code = code.trim();
        Self::ALL
            .into_iter()
            .find(|l| l.code().eq_ignore_ascii_case(code))
            .unwrap_or(Lang::En)
    }

    /// The form a word takes after `n`: 1 plik, 2 pliki, 5 plików; 1 soubor, 3 soubory,
    /// 5 souborů; 1 файл, 2 файли, 5 файлів; French counts 0 like 1.
    pub fn plural(self, n: u64) -> Plural {
        let (ten, hundred) = (n % 10, n % 100);
        match self {
            Lang::En | Lang::De | Lang::Es | Lang::It => {
                if n == 1 {
                    Plural::One
                } else {
                    Plural::Other
                }
            }
            Lang::Fr => {
                if n <= 1 {
                    Plural::One
                } else {
                    Plural::Other
                }
            }
            Lang::Cs | Lang::Sk => match n {
                1 => Plural::One,
                2..=4 => Plural::Few,
                _ => Plural::Other,
            },
            Lang::Pl => {
                if n == 1 {
                    Plural::One
                } else if (2..=4).contains(&ten) && !(12..=14).contains(&hundred) {
                    Plural::Few
                } else {
                    Plural::Many
                }
            }
            Lang::Uk => {
                if ten == 1 && hundred != 11 {
                    Plural::One
                } else if (2..=4).contains(&ten) && !(12..=14).contains(&hundred) {
                    Plural::Few
                } else {
                    Plural::Many
                }
            }
        }
    }

    /// Whether 1.5 is written 1,5.
    pub fn decimal_comma(self) -> bool {
        self != Lang::En
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
        let n = usize::from(LANG.load(Ordering::Relaxed));
        Lang::ALL.get(n).copied().unwrap_or_default()
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
    match Lang::Pl.plural(n) {
        Plural::One => 0,
        Plural::Few => 1,
        _ => 2,
    }
}

/// A sentence in the current language: `tr!("Can't reach {host}.", "Brak połączenia z {host}.")`.
/// Both are format strings with the same arguments, named in place or passed after them.
/// The other languages come from the catalogs (see the module's notes).
#[macro_export]
macro_rules! tr {
    ($($t:tt)*) => {
        $crate::i18n::tr_in!($crate, $($t)*)
    };
}

/// A sentence with a count, in the form the current language takes for it:
/// `trn!(n, "{n} new message", "{n} new messages", "{n} nowa wiadomość",
/// "{n} nowe wiadomości", "{n} nowych wiadomości")`. `n` is any integer; the sentences take
/// their arguments like [`tr!`](crate::tr)'s. The catalogs key it by the first English form.
#[macro_export]
macro_rules! trn {
    ($($t:tt)*) => {
        $crate::i18n::trn_in!($crate, $($t)*)
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
    fn a_sentence_no_catalog_has_is_english() {
        let n = 3;
        for lang in Lang::ALL.into_iter().filter(|l| *l != Lang::Pl) {
            assert_eq!(
                with(lang, || tr!(
                    "Nobody translated {n} of these.",
                    "Nikt nie przetłumaczył {n} z nich."
                )),
                "Nobody translated 3 of these.",
                "{lang:?}"
            );
        }
    }

    #[test]
    fn counts_take_their_form() {
        let say = |n: usize| {
            trn!(
                n,
                "{n} test file",
                "{n} test files",
                "{n} plik testowy",
                "{n} pliki testowe",
                "{n} plików testowych"
            )
        };
        assert_eq!(with(Lang::En, || say(1)), "1 test file");
        assert_eq!(with(Lang::En, || say(0)), "0 test files");
        assert_eq!(with(Lang::Pl, || say(22)), "22 pliki testowe");
        assert_eq!(with(Lang::Pl, || say(25)), "25 plików testowych");
        assert_eq!(with(Lang::De, || say(2)), "2 test files"); // not in the catalog
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
    fn plural_rules() {
        use Plural::*;
        let of = |lang: Lang| [0, 1, 2, 4, 5, 11, 21, 22, 25, 111].map(|n| lang.plural(n));
        assert_eq!(
            of(Lang::Cs),
            [Other, One, Few, Few, Other, Other, Other, Other, Other, Other]
        );
        assert_eq!(
            of(Lang::Uk),
            [Many, One, Few, Few, Many, Many, One, Few, Many, Many]
        );
        assert_eq!(
            of(Lang::Fr),
            [One, One, Other, Other, Other, Other, Other, Other, Other, Other]
        );
        assert_eq!(
            of(Lang::De),
            [Other, One, Other, Other, Other, Other, Other, Other, Other, Other]
        );
    }

    #[test]
    fn codes() {
        assert_eq!(Lang::from_code("PL"), Lang::Pl);
        assert_eq!(Lang::from_code("en"), Lang::En);
        assert_eq!(Lang::from_code("de"), Lang::De);
        assert_eq!(Lang::from_code("uk"), Lang::Uk);
        assert_eq!(Lang::from_code("nl"), Lang::En);
        for (n, lang) in Lang::ALL.into_iter().enumerate() {
            assert_eq!(lang as usize, n);
            assert_eq!(Lang::from_code(lang.code()), lang);
            assert_eq!(with(lang, current), lang);
        }
        assert_eq!(
            crate::settings::LANGUAGES,
            Lang::ALL.map(Lang::code).as_slice()
        );
    }

    /// The catalogs, as the macros read them.
    fn catalogs() -> Vec<(Lang, serde_json::Map<String, serde_json::Value>)> {
        [
            (Lang::De, include_str!("../i18n/de.json")),
            (Lang::Cs, include_str!("../i18n/cs.json")),
            (Lang::Sk, include_str!("../i18n/sk.json")),
            (Lang::Uk, include_str!("../i18n/uk.json")),
            (Lang::Es, include_str!("../i18n/es.json")),
            (Lang::Fr, include_str!("../i18n/fr.json")),
            (Lang::It, include_str!("../i18n/it.json")),
        ]
        .into_iter()
        .map(|(lang, text)| (lang, serde_json::from_str(text).expect("a catalog is JSON")))
        .collect()
    }

    /// What a format string takes: its named arguments, how many positional ones, and the
    /// format specs ({:.1}), in order. `{{` and `}}` are plain braces.
    fn holes(s: &str) -> (Vec<String>, usize, Vec<String>) {
        let plain = s.replace("{{", "").replace("}}", "");
        let (mut named, mut positional, mut specs) = (Vec::new(), 0, Vec::new());
        for piece in plain.split('{').skip(1) {
            let inside = piece.split('}').next().unwrap_or("");
            let (arg, spec) = inside.split_once(':').unwrap_or((inside, ""));
            if arg.is_empty() || arg.chars().all(|c| c.is_ascii_digit()) {
                positional += 1;
            } else if !named.contains(&arg.to_owned()) {
                named.push(arg.to_owned());
            }
            if !spec.is_empty() {
                specs.push(spec.to_owned());
            }
        }
        named.sort();
        specs.sort();
        (named, positional, specs)
    }

    #[test]
    fn translations_keep_every_placeholder() {
        let mut wrong = Vec::new();
        for (lang, catalog) in catalogs() {
            for (english, value) in &catalog {
                let texts: Vec<&str> = match value {
                    serde_json::Value::String(s) => vec![s.as_str()],
                    serde_json::Value::Object(forms) => {
                        forms.values().filter_map(|v| v.as_str()).collect()
                    }
                    _ => vec![],
                };
                assert!(!texts.is_empty(), "{lang:?}: {english:?} has no text");
                for text in texts {
                    let (en, de) = (holes(english), holes(text));
                    // A count's "one" form may say "a minute" instead of "{n} minute".
                    let drop_n = |mut h: (Vec<String>, usize, Vec<String>)| {
                        h.0.retain(|x| x != "n");
                        h
                    };
                    if en != de && drop_n(en.clone()) != drop_n(de) {
                        wrong.push(format!("{lang:?}: {english:?} → {text:?}"));
                    }
                }
            }
        }
        assert!(
            wrong.is_empty(),
            "placeholders differ:\n{}",
            wrong.join("\n")
        );
    }

    #[test]
    fn catalogs_translate() {
        // A sentence of the core, in a language from a catalog.
        let catalog = catalogs();
        let german = &catalog.iter().find(|(l, _)| *l == Lang::De).unwrap().1;
        let english = "Choose how often one person may get it: 1 to 365 days.";
        if let Some(serde_json::Value::String(text)) = german.get(english) {
            let said = with(Lang::De, || {
                tr!(
                    "Choose how often one person may get it: 1 to 365 days.",
                    "Wybierz, jak często jedna osoba może ją dostać: od 1 do 365 dni."
                )
            });
            assert_eq!(&said, text);
            assert_ne!(said, english);
        }
    }
}
