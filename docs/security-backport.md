# GLib 0.18 security backport

Tauri's GTK 0.18 dependency family cannot use GLib 0.20 without an API migration. Telex
therefore vendors the published **glib 0.18.5** crate and applies the upstream fix for
[RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html).

The source archive comes from `https://static.crates.io/crates/glib/glib-0.18.5.crate`, with
SHA-256 `233daaf6e83ae6a12a52055f568f9d7cf4671dabb78ff9560ab6da230ce00ee5`, matching the
registry's recorded checksum. Its MIT license and copyright notices are retained.
The only source change is in `src/variant_iter.rs`: make the C out-pointer mutable and
pass `&mut p` instead of `&p`, as in [upstream PR #1343](https://github.com/gtk-rs/gtk-rs-core/pull/1343).
The API and version number remain 0.18.5; the version is not relabelled as 0.20.

`[patch.crates-io]` selects this source for the entire dependency graph. `scripts/security/verify-glib.mjs`
checks the complete file inventory and every file's SHA-256 against `glib-backport.json`, verifies
the mutable out-pointer, and verifies that Cargo.lock resolves GLib to the local backport.
`.gitattributes` preserves the published crate bytes on Windows and Linux.

RustSec's registry matching does not audit local path dependencies. The passing registry scan
alone therefore says nothing about this backport: the additional source guard and Linux
regression are required. No advisory is added to an ignore list. The unmaintained
`proc-macro-error` advisory remains visible.

Run the regression with compiler optimizations, where the original bug caused crashes:

```
node scripts/security/verify-glib.mjs
cargo test -p telex-security --release --locked --test glib_regression
```

The Linux test exercises `next`, `next_back`, `nth`, `nth_back`, `last`, collection,
exhaustion and extreme indices, including non-ASCII strings, 1,000 times. Release publication
depends on this test, the source guard, dependency and secret scans, native webview regressions,
and bounded parser fuzzing. Windows does not link GLib.

Remove the backport only when Tauri can resolve to an upstream patched GLib API family; then
remove the patch, vendor files and backport-specific hash guard together and rerun both platforms.
