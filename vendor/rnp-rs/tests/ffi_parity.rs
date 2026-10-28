//! FFI surface-parity enforcement.
//!
//! The safe crate's promise is full parity with librnp's public C API: every
//! function declared in `rnp.h` (as bound in `rnp-sys`) is exercised by a
//! safe wrapper in `src/`. This test keeps that promise mechanical — when
//! `rnp-sys` bindings regenerate against a newer librnp and new `rnp_*`
//! functions appear, CI fails here until each one is either wrapped or
//! consciously excluded (with a reason) in [`EXCLUDED`].
//!
//! The audit runs only inside the repository: it reads the bindings file
//! from the `rnp-sys` sibling, which is not part of the published package.
//! `scripts/parity-audit.sh` prints the same audit for humans.

use std::fs;
use std::path::{Path, PathBuf};

/// Functions consciously not wrapped, each with a reason.
///
/// Coverage is currently complete, so the list is empty. Add an entry only
/// with a justification that will still make sense a year from now
/// (e.g. "deprecated upstream since 0.17", "superseded by the JSON API").
/// An entry becomes stale — and the hygiene test below fails — once the
/// function is wrapped.
const EXCLUDED: &[(&str, &str)] = &[
    // KeyBuilder defers every setter to build time and replays its own
    // vectors, so clearing a vector is exactly the C-side clear; calling
    // these on the builder's op would be a no-op by construction.
    (
        "rnp_op_generate_clear_pref_ciphers",
        "equivalent: KeyBuilder::clear_pref_cipher clears the local vector",
    ),
    (
        "rnp_op_generate_clear_pref_compression",
        "equivalent: KeyBuilder::clear_pref_compression clears the local vector",
    ),
    (
        "rnp_op_generate_clear_pref_hashes",
        "equivalent: KeyBuilder::clear_pref_hash clears the local vector",
    ),
    (
        "rnp_op_generate_clear_usage",
        "equivalent: KeyBuilder::clear_usage clears the local vector",
    ),
    // Present in the 0.18.1 headers, removed on main. The pqc/crypto-refresh
    // features compile librnp from HEAD, where these symbols no longer
    // exist, so wrappers for them cannot be compiled for that flavor.
    // SLH-DSA parameter sets are selected via the SLH-DSA-SHA2-* algorithm
    // names instead.
    (
        "rnp_key_sphincsplus_get_param",
        "removed on librnp main; 0.18.1-only experimental surface",
    ),
    (
        "rnp_op_generate_set_sphincsplus_param",
        "removed on librnp main; 0.18.1-only experimental surface",
    ),
];

/// First `bindings-*.rs` under `rnp-sys/bindings/` (the filename carries
/// the librnp version, so don't hardcode it). `None` outside the repo.
fn bindings_file() -> Option<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rnp-sys/bindings");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("bindings-") && n.ends_with(".rs"))
                .unwrap_or(false)
        })
        .collect();
    files.sort();
    files.into_iter().next()
}

fn declared_functions(bindings: &Path) -> Vec<String> {
    let content = fs::read_to_string(bindings).expect("read bindings");
    let mut names: Vec<String> = content
        .lines()
        .filter_map(|l| l.trim().strip_prefix("pub fn rnp_"))
        .map(|rest| {
            let end = rest.find('(').expect("bindgen puts '(' after the name");
            format!("rnp_{}", &rest[..end])
        })
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Concatenate every `.rs` file under `src/`.
fn crate_source() -> String {
    fn walk(dir: &Path, out: &mut String) {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs")
                && let Ok(content) = fs::read_to_string(&path)
            {
                out.push_str(&content);
            }
        }
    }
    let mut src = String::new();
    walk(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src").as_path(),
        &mut src,
    );
    src
}

#[test]
fn every_bound_ffi_function_is_exercised() {
    let Some(bindings) = bindings_file() else {
        // Published package: the rnp-sys sibling isn't shipped. See the
        // module docs — the audit runs in-repo and in CI.
        eprintln!("skipping: no rnp-sys bindings sibling found");
        return;
    };

    let declared = declared_functions(&bindings);
    assert!(
        declared.len() > 200,
        "suspiciously few declared functions ({}); bindings parsing broke?",
        declared.len()
    );

    let source = crate_source();
    let missing: Vec<&String> = declared
        .iter()
        .filter(|name| !EXCLUDED.iter().any(|(n, _)| n == *name))
        .filter(|name| !source.contains(&format!("ffi::{name}(")))
        .collect();

    assert!(
        missing.is_empty(),
        "{} bound FFI function(s) with no safe wrapper call site: {missing:?}\n\
         wrap them, or add them to the EXCLUDED list in tests/ffi_parity.rs \
         with a reason",
        missing.len()
    );
}

#[test]
fn exclusion_list_stays_hygienic() {
    let Some(bindings) = bindings_file() else {
        eprintln!("skipping: no rnp-sys bindings sibling found");
        return;
    };
    let declared = declared_functions(&bindings);
    let source = crate_source();

    for (name, _reason) in EXCLUDED {
        assert!(
            declared.iter().any(|d| d == name),
            "EXCLUDED entry {name} is not a bound function; remove it"
        );
        assert!(
            !source.contains(&format!("ffi::{name}(")),
            "EXCLUDED entry {name} is now wrapped; remove the exclusion"
        );
    }

    let mut sorted: Vec<&str> = EXCLUDED.iter().map(|(n, _)| *n).collect();
    sorted.sort();
    let names: Vec<&str> = EXCLUDED.iter().map(|(n, _)| *n).collect();
    assert_eq!(sorted, names, "EXCLUDED must stay sorted");
}
