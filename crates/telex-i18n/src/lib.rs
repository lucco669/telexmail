//! The macros behind `telex_core::tr!` and `telex_core::trn!`.
//!
//! A sentence is written in the code in English and Polish, side by side:
//! `tr!("Can't reach {host}.", "Brak połączenia z {host}.")`. The other languages come from
//! catalogs keyed by the English, `crates/telex-core/i18n/<code>.json`. At compile time each
//! `tr!` becomes a `match` on the current language with one `format!` per language, so every
//! translation is a format string checked by the compiler like the English one: a placeholder
//! that isn't there fails the build. A language whose catalog lacks the sentence shares the
//! English arm.
//!
//! Use the macros through telex-core (`telex_core::tr!`), never this crate directly: they pass
//! in the path to telex-core (`$crate`), which a procedural macro can't know by itself.

use std::collections::HashMap;
use std::sync::OnceLock;

use proc_macro::{Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree};

/// The languages with a catalog: its file name and `telex_core::i18n::Lang`'s variant.
/// English and Polish are written in the code. Keep in step with `Lang` in telex-core.
const CATALOGS: &[(&str, &str)] = &[
    ("de", "De"),
    ("cs", "Cs"),
    ("sk", "Sk"),
    ("uk", "Uk"),
    ("es", "Es"),
    ("fr", "Fr"),
    ("it", "It"),
];

/// The plural forms a catalog entry of `trn!` may have, and `Plural`'s variants.
const FORMS: &[(&str, &str)] = &[("one", "One"), ("few", "Few"), ("many", "Many")];

type Catalog = HashMap<String, serde_json::Value>;

/// Every catalog, read once per compilation. A file that isn't there is an empty catalog.
fn catalogs() -> &'static Result<Vec<Catalog>, String> {
    static ALL: OnceLock<Result<Vec<Catalog>, String>> = OnceLock::new();
    ALL.get_or_init(|| {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../telex-core/i18n");
        CATALOGS
            .iter()
            .map(|(code, _)| {
                let path = format!("{dir}/{code}.json");
                match std::fs::read_to_string(&path) {
                    Ok(text) => serde_json::from_str(&text).map_err(|e| format!("{path}: {e}")),
                    Err(_) => Ok(Catalog::new()),
                }
            })
            .collect()
    })
}

/// `tr_in!($crate, "English", "Polski", args…)`: a sentence in the current language.
#[proc_macro]
pub fn tr_in(input: TokenStream) -> TokenStream {
    expand(input, false).unwrap_or_else(|e| error(&e))
}

/// `trn_in!($crate, count, "{n} file", "{n} files", "{n} plik", "{n} pliki", "{n} plików",
/// args…)`: the form for this count in the current language. The catalogs key it by the
/// first English form and hold `{"one": …, "few": …, "many": …, "other": …}`.
#[proc_macro]
pub fn trn_in(input: TokenStream) -> TokenStream {
    expand(input, true).unwrap_or_else(|e| error(&e))
}

fn expand(input: TokenStream, plural: bool) -> Result<TokenStream, String> {
    let mut parts = split_commas(input);
    let strings = if plural { 5 } else { 2 };
    let fixed = 1 + usize::from(plural) + strings;
    if parts.len() < fixed {
        return Err(if plural {
            "trn! takes a count, two English forms and three Polish ones".into()
        } else {
            "tr! takes the English and the Polish".into()
        });
    }
    let args: Vec<TokenStream> = parts.split_off(fixed);
    let krate = parts.remove(0);
    let count = plural.then(|| parts.remove(0));
    let literals = parts
        .iter()
        .map(|p| string_literal(p.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    let (english_key, span) = (&literals[0].0, literals[0].1.span());
    let catalogs = catalogs().as_ref().map_err(Clone::clone)?;

    let lang = |variant: &str| path(&krate, &["i18n", "Lang", variant]);
    // format!(text, args…), the text spanned as the English so that names in it ({host})
    // are the caller's variables.
    let format = |text: &Literal| {
        let mut inner = vec![TokenTree::Literal(text.clone())];
        for arg in &args {
            inner.push(punct(','));
            inner.extend(arg.clone());
        }
        let mut out = path_from_root(&["std", "format"]);
        out.push(punct('!'));
        out.push(TokenTree::Group(Group::new(
            Delimiter::Parenthesis,
            inner.into_iter().collect(),
        )));
        out
    };
    let translated = |value: &str| {
        let mut lit = Literal::string(value);
        lit.set_span(span);
        lit
    };

    let mut arms: Vec<TokenTree> = Vec::new();
    let mut with_english = vec!["En"];
    let count_var = || TokenTree::Ident(Ident::new("__telex_count", Span::call_site()));
    // match Lang::X.plural(count) { Plural::One => …, Plural::Few => …, _ => … }
    let by_form = |variant: &str, forms: &[(&str, &Literal)], other: &Literal| {
        let mut out = lang(variant);
        out.push(punct('.'));
        out.push(TokenTree::Ident(Ident::new("plural", Span::call_site())));
        out.push(TokenTree::Group(Group::new(
            Delimiter::Parenthesis,
            TokenStream::from(count_var()),
        )));
        let mut inner = Vec::new();
        for (form, text) in forms {
            inner.extend(path(&krate, &["i18n", "Plural", form]));
            inner.extend(fat_arrow());
            inner.extend(format(text));
            inner.push(punct(','));
        }
        inner.push(TokenTree::Ident(Ident::new("_", Span::call_site())));
        inner.extend(fat_arrow());
        inner.extend(format(other));
        let mut m = vec![TokenTree::Ident(Ident::new("match", Span::call_site()))];
        m.extend(out);
        m.push(TokenTree::Group(Group::new(
            Delimiter::Brace,
            inner.into_iter().collect(),
        )));
        m
    };

    // Polish, from the code.
    arms.extend(lang("Pl"));
    arms.extend(fat_arrow());
    if plural {
        let pl = &literals[2..5];
        arms.extend(by_form(
            "Pl",
            &[("One", &pl[0].1), ("Few", &pl[1].1)],
            &pl[2].1,
        ));
    } else {
        arms.extend(format(&literals[1].1));
    }
    arms.push(punct(','));

    // The catalogs' languages.
    for ((_, variant), catalog) in CATALOGS.iter().zip(catalogs) {
        let entry = catalog.get(english_key);
        let body = match (plural, entry) {
            (false, Some(serde_json::Value::String(text))) => format(&translated(text)),
            (true, Some(serde_json::Value::String(text))) => format(&translated(text)),
            (true, Some(serde_json::Value::Object(forms))) => {
                let text = |name: &str| forms.get(name).and_then(|v| v.as_str());
                let Some(other) = text("other").or_else(|| text("many")) else {
                    return Err(format!(
                        "the {variant} catalog's \"{english_key}\" has no \"other\" form"
                    ));
                };
                let forms: Vec<(&str, Literal)> = FORMS
                    .iter()
                    .filter_map(|(name, form)| text(name).map(|t| (*form, translated(t))))
                    .collect();
                let refs: Vec<(&str, &Literal)> = forms.iter().map(|(f, l)| (*f, l)).collect();
                by_form(variant, &refs, &translated(other))
            }
            _ => {
                with_english.push(variant);
                continue;
            }
        };
        arms.extend(lang(variant));
        arms.extend(fat_arrow());
        arms.extend(body);
        arms.push(punct(','));
    }

    // English, and every language without this sentence.
    for (i, variant) in with_english.iter().enumerate() {
        if i > 0 {
            arms.push(punct('|'));
        }
        arms.extend(lang(variant));
    }
    arms.extend(fat_arrow());
    if plural {
        arms.extend(by_form("En", &[("One", &literals[0].1)], &literals[1].1));
    } else {
        arms.extend(format(&literals[0].1));
    }
    arms.push(punct(','));

    let mut out: Vec<TokenTree> = Vec::new();
    if let Some(count) = count {
        // let __telex_count: u64 = TryFrom::try_from(count).unwrap_or(u64::MAX);
        out.push(TokenTree::Ident(Ident::new("let", Span::call_site())));
        out.push(count_var());
        out.push(punct(':'));
        out.push(TokenTree::Ident(Ident::new("u64", Span::call_site())));
        out.push(punct('='));
        out.extend(path_from_root(&["core", "convert", "TryFrom", "try_from"]));
        out.push(TokenTree::Group(Group::new(Delimiter::Parenthesis, count)));
        out.push(punct('.'));
        out.push(TokenTree::Ident(Ident::new("unwrap_or", Span::call_site())));
        let max: TokenStream = "u64::MAX".parse().expect("u64::MAX");
        out.push(TokenTree::Group(Group::new(Delimiter::Parenthesis, max)));
        out.push(punct(';'));
    }
    out.push(TokenTree::Ident(Ident::new("match", Span::call_site())));
    out.extend(path(&krate, &["i18n", "current"]));
    out.push(TokenTree::Group(Group::new(
        Delimiter::Parenthesis,
        TokenStream::new(),
    )));
    out.push(TokenTree::Group(Group::new(
        Delimiter::Brace,
        arms.into_iter().collect(),
    )));
    Ok(TokenStream::from(TokenTree::Group(Group::new(
        Delimiter::Brace,
        out.into_iter().collect(),
    ))))
}

/// The input's parts between top-level commas (commas inside brackets stay in their part).
fn split_commas(input: TokenStream) -> Vec<TokenStream> {
    let mut parts = vec![Vec::new()];
    for tt in input {
        match &tt {
            TokenTree::Punct(p) if p.as_char() == ',' => parts.push(Vec::new()),
            _ => parts.last_mut().expect("one part").push(tt),
        }
    }
    if parts.last().is_some_and(Vec::is_empty) {
        parts.pop(); // a trailing comma
    }
    parts.into_iter().map(|p| p.into_iter().collect()).collect()
}

/// A string literal's value and the literal itself.
fn string_literal(part: TokenStream) -> Result<(String, Literal), String> {
    let mut tokens: Vec<TokenTree> = part.into_iter().collect();
    // A literal passed on by macro_rules may come wrapped in an invisible group.
    while let [TokenTree::Group(g)] = tokens.as_slice() {
        if g.delimiter() != Delimiter::None {
            break;
        }
        tokens = g.stream().into_iter().collect();
    }
    let [TokenTree::Literal(lit)] = tokens.as_slice() else {
        return Err("expected a string literal".into());
    };
    let source = lit.to_string();
    let value = if let Some(raw) = source.strip_prefix('r') {
        let hashes = raw.len() - raw.trim_start_matches('#').len();
        raw.get(hashes + 1..raw.len() - hashes - 1)
            .ok_or("expected a string literal")?
            .to_owned()
    } else if source.starts_with('"') && source.ends_with('"') && source.len() >= 2 {
        unescape(&source[1..source.len() - 1])?
    } else {
        return Err("expected a string literal".into());
    };
    Ok((value, lit.clone()))
}

/// The text a Rust string literal stands for: `\n`, `\"`, `\u{…}`, a line continuation…
fn unescape(s: &str) -> Result<String, String> {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('0') => out.push('\0'),
            Some(c @ ('\\' | '"' | '\'')) => out.push(c),
            Some('x') => {
                let hex: String = chars.by_ref().take(2).collect();
                let n = u8::from_str_radix(&hex, 16).map_err(|e| e.to_string())?;
                out.push(char::from(n));
            }
            Some('u') => {
                let code: String = chars
                    .by_ref()
                    .skip_while(|c| *c == '{')
                    .take_while(|c| *c != '}')
                    .collect();
                let n = u32::from_str_radix(&code, 16).map_err(|e| e.to_string())?;
                out.push(char::from_u32(n).ok_or("not a character")?);
            }
            Some('\n' | '\r') => {
                while chars.peek().is_some_and(|c| c.is_whitespace()) {
                    chars.next();
                }
            }
            other => return Err(format!("unknown escape \\{}", other.unwrap_or(' '))),
        }
    }
    Ok(out)
}

/// `$crate::a::b`.
fn path(krate: &TokenStream, segments: &[&str]) -> Vec<TokenTree> {
    let mut out: Vec<TokenTree> = krate.clone().into_iter().collect();
    for s in segments {
        out.extend(colons());
        out.push(TokenTree::Ident(Ident::new(s, Span::call_site())));
    }
    out
}

/// `::a::b`.
fn path_from_root(segments: &[&str]) -> Vec<TokenTree> {
    let mut out = Vec::new();
    for s in segments {
        out.extend(colons());
        out.push(TokenTree::Ident(Ident::new(s, Span::call_site())));
    }
    out
}

fn colons() -> [TokenTree; 2] {
    [
        TokenTree::Punct(Punct::new(':', Spacing::Joint)),
        TokenTree::Punct(Punct::new(':', Spacing::Alone)),
    ]
}

fn fat_arrow() -> [TokenTree; 2] {
    [
        TokenTree::Punct(Punct::new('=', Spacing::Joint)),
        TokenTree::Punct(Punct::new('>', Spacing::Alone)),
    ]
}

fn punct(c: char) -> TokenTree {
    TokenTree::Punct(Punct::new(c, Spacing::Alone))
}

fn error(message: &str) -> TokenStream {
    format!("::core::compile_error!({message:?})")
        .parse()
        .expect("compile_error!")
}
