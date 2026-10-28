// rnp-sys build script: generate raw FFI bindings (bindgen or the
// pregenerated file) and link against librnp.
//
// Three linking modes, dispatched at compile time via cfg(feature):
//
//   1. --features vendored — call rnp_src::build(), which compiles librnp
//      + Botan + json-c + zlib + bzip2 from source into our OUT_DIR and
//      returns the install layout (botan-rs `botan-src` pattern).
//   2. RNP_INCLUDE_DIR / RNP_LIB_DIR env vars (explicit path).
//   3. Default — pkg-config discovers the system librnp; falls back to
//      hardcoded candidate paths on minimal images.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    prepare_msvc();
    // docs.rs sandbox: no librnp, no libclang, no C toolchain for a vendored
    // build. Documentation only needs the pregenerated bindings — rustdoc
    // never links the native library. docs.rs sets DOCS_RS=1 for builds it
    // invokes, so take the fast path and skip link discovery entirely.
    if env::var_os("DOCS_RS").is_some() {
        let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
        let src = pregenerated_bindings_path();
        fs::copy(&src, out_dir.join("bindings.rs")).unwrap_or_else(|e| {
            panic!("Couldn't copy pregenerated bindings for docs.rs build: {e}")
        });
        eprintln!("rnp-sys: docs.rs build — pregenerated bindings, no link");
        return;
    }

    // Mixed-graph detection (requires the `botan-sys-detect` feature, which
    // makes botan-sys's links metadata visible here). If another crate in
    // this build graph enabled botan-sys's vendored feature, Botan is being
    // compiled twice — once by botan-sys, once by our vendored path — and
    // the final link binds whichever archive comes first in search-path
    // order. Warn loudly rather than leave that to fail (or link the wrong
    // Botan) at link time.
    if env::var("DEP_BOTAN_VENDORED").as_deref() == Ok("1")
        && env::var_os("DEP_BOTAN_LIB_DIR").is_none() {
        println!(
            "cargo:warning=rnp-sys: botan-sys/vendored is also active in this build graph. \
             Botan is being compiled TWICE (botan-sys's build and rnp-src's build). \
             The linker will pick one libbotan-3 by search-path order. Prefer system Botan \
             for both crates, or align Botan versions, to avoid duplicate builds and \
             order-dependent linking."
        );
    }

    let loc = locate_librnp();

    let rnp_header = loc.include_dir.join("rnp").join("rnp.h");
    if !rnp_header.exists() {
        panic!(
            "Could not find <rnp/rnp.h> under {}. Expected the header at {}.",
            loc.include_dir.display(),
            rnp_header.display()
        );
    }

    if let Some(dir) = &loc.lib_dir {
        println!("cargo:rustc-link-search=native={}", dir.display());
    }
    for dir in &loc.extra_lib_dirs {
        println!("cargo:rustc-link-search=native={}", dir.display());
    }
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed={}", rnp_header.display());
    println!("cargo:rerun-if-env-changed=RNP_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=RNP_LIB_DIR");
    println!("cargo:rerun-if-env-changed=RNP_BINDINGS_RUNTIME");
    println!("cargo:rerun-if-env-changed=RNP_BINDINGS_PREGENERATED");
    println!("cargo:rerun-if-env-changed=RNP_BINDINGS_EXPERIMENTAL");
    println!("cargo:rerun-if-env-changed=RNP_BINDINGS_REGENERATE");

    let out_path = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let bindings_path = out_path.join("bindings.rs");
    match bindings_source(&loc) {
        BindingsSource::Pregenerated(src) => {
            // Skip bindgen entirely — no libclang needed on the build host.
            // This is what makes cross builds work in minimal containers
            // where libclang can't even find its own stdbool.h.
            eprintln!("rnp-rs: using pregenerated bindings: {}", src.display());
            println!("cargo:rerun-if-changed={}", src.display());
            fs::copy(&src, &bindings_path)
                .unwrap_or_else(|e| panic!("Couldn't copy pregenerated bindings: {e}"));
        }
        BindingsSource::Runtime => {
            let bindings = generate_bindings(&loc.include_dir);
            bindings
                .write_to_file(&bindings_path)
                .expect("Couldn't write bindings");
            maybe_regenerate_bindings(&bindings_path, loc.librnp_version.as_deref());
        }
    }

    emit_link_directives(&loc);
}

fn prepare_msvc() {
    let target = env::var("TARGET").expect("TARGET");
    if !target.ends_with("windows-msvc") { return; }
    unsafe { env::set_var("CMAKE_GENERATOR", "Ninja"); }
    unsafe { env::set_var("CC", "cl"); env::set_var("CXX", "cl"); }
    let compiler = cc::windows_registry::find_tool(&target, "cl.exe").expect("MSVC C++ build tools");
    for (key, value) in compiler.get_envs() { unsafe { env::set_var(key, value); } }
    for ancestor in compiler.path().ancestors() {
        let tools = ancestor.join("Common7/IDE/CommonExtensions/Microsoft/CMake");
        if tools.is_dir() {
            let mut paths = vec![tools.join("CMake/bin"), tools.join("Ninja")];
            paths.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()));
            unsafe { env::set_var("PATH", env::join_paths(paths).expect("tool paths")); }
            break;
        }
    }
}

// -----------------------------------------------------------------------
// Link-mode resolution.
// -----------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // not every variant is constructed under every feature combo
enum LinkMode {
    /// `cargo:rustc-link-lib=dylib=rnp` — system-installed shared library.
    System,
    /// Same as System but driven by explicit RNP_INCLUDE_DIR/RNP_LIB_DIR.
    Explicit,
    /// rnp-src compiled librnp + all deps as static libraries.
    Vendored,
}

#[derive(Debug, Clone)]
struct LibrnpLocation {
    include_dir: PathBuf,
    lib_dir: Option<PathBuf>,
    link_mode: LinkMode,
    /// Vendored mode only: which librnp source was built ("0.18.1" / "head").
    /// Drives pregenerated-bindings selection; None for system/explicit.
    librnp_version: Option<String>,
    /// Additional `-L` paths emitted via cargo:rustc-link-search. Empty
    /// for System/Explicit; populated by Vendored to surface per-dep
    /// install prefixes (Botan, json-c, zlib, bzip2).
    extra_lib_dirs: Vec<PathBuf>,
    /// Vendored mode only: whether json-c was built and must be linked.
    /// False for the HEAD flavor — librnp main vendors nlohmann/json
    /// (upstream 4f5c4e6e) and no longer links json-c.
    link_json_c: bool,
    /// Library names to link, with mode-appropriate prefix. When non-empty,
    /// `emit_link_directives` iterates these instead of falling back to
    /// the single hardcoded `dylib=rnp`. Populated by pkg-config so
    /// transitive deps (libz, libbz2, libbotan-3) get linked even when
    /// the distro's static `librnp.a` lacks recorded dependencies.
    extra_link_libs: Vec<LinkLib>,
}

#[derive(Debug, Clone)]
struct LinkLib {
    name: String,
    kind: LinkLibKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // only constructed in the non-vendored build path
enum LinkLibKind {
    Dylib,
    Static,
}

/// Per-feature dispatch: under `--features vendored` we delegate to
/// `locate_vendored`; otherwise to `locate_system_or_explicit`. Splitting
/// by cfg avoids the early-return + `#[allow(unreachable_code)]` pattern
/// and lets the dead-code analyzer prove every variant is constructed.
#[cfg(feature = "vendored")]
fn locate_librnp() -> LibrnpLocation {
    locate_vendored()
}

#[cfg(not(feature = "vendored"))]
fn locate_librnp() -> LibrnpLocation {
    locate_system_or_explicit()
}

#[cfg(feature = "vendored")]
fn locate_vendored() -> LibrnpLocation {
    // Compiles into OUR OUT_DIR (the caller's, per the botan-src pattern).
    let installed = rnp_src::build();
    // Links metadata for any downstream build script that wants to know
    // which librnp flavor was linked (DEP_RNP_LIBRNP_VERSION).
    println!("cargo:librnp_version={}", installed.librnp_version);
    LibrnpLocation {
        librnp_version: Some(installed.librnp_version),
        include_dir: installed.include_dir,
        lib_dir: Some(installed.lib_dir),
        link_mode: LinkMode::Vendored,
        link_json_c: installed.flavor.needs_json_c(),
        extra_lib_dirs: installed.dep_lib_dirs,
        extra_link_libs: Vec::new(),
    }
}

#[cfg(not(feature = "vendored"))]
fn locate_system_or_explicit() -> LibrnpLocation {
    // Explicit path via env vars wins over pkg-config / hardcoded search.
    if let Some(loc) = locate_explicit() {
        return loc;
    }
    if let Some(loc) = locate_via_pkg_config() {
        return loc;
    }
    locate_via_hardcoded_paths()
}

#[cfg(not(feature = "vendored"))]
fn locate_explicit() -> Option<LibrnpLocation> {
    let dir = env::var("RNP_INCLUDE_DIR").ok()?;
    let include_dir = PathBuf::from(dir);
    let lib_dir = env::var("RNP_LIB_DIR").ok().map(PathBuf::from).or_else(|| {
        // Sibling-OF include dir: if include is /foo/include, try /foo/lib.
        let mut candidate = include_dir.clone();
        candidate.pop();
        candidate.push("lib");
        if candidate.exists() {
            Some(candidate)
        } else {
            None
        }
    });
    Some(LibrnpLocation {
        include_dir,
        lib_dir,
        link_mode: LinkMode::Explicit,
        librnp_version: None,
        link_json_c: false,
        extra_lib_dirs: Vec::new(),
        extra_link_libs: Vec::new(),
    })
}

/// Ask pkg-config where librnp + its transitive deps live. Falls back
/// gracefully on images without librnp.pc (returns None → caller falls
/// through to hardcoded path search).
#[cfg(not(feature = "vendored"))]
fn locate_via_pkg_config() -> Option<LibrnpLocation> {
    // Use `--static` so Requires.private deps (libz, libbz2, …) come
    // back too. This matters on macOS where brew's static `librnp.a`
    // has unresolved symbols for its transitive deps — the .dylib
    // records them via install_name but .a does not.
    let lib = pkg_config::Config::new().statik(true).probe("rnp").ok()?;
    let include_dir = lib.include_paths.first()?.clone();
    let lib_dir = lib.link_paths.first().cloned();
    // pkg-config's `libs` field carries bare library names (without
    // the `-l` prefix). Convert each into our LinkLib shape so
    // emit_link_directives can iterate uniformly.
    let extra_link_libs: Vec<LinkLib> = lib
        .libs
        .iter()
        .filter_map(|s| parse_pkg_config_lib(s))
        .collect();
    Some(LibrnpLocation {
        include_dir,
        lib_dir,
        link_mode: LinkMode::System,
        librnp_version: None,
        link_json_c: false,
        extra_lib_dirs: lib.link_paths.clone(),
        extra_link_libs,
    })
}

/// Parse a bare library name from pkg-config's `libs` field into a
/// (name, kind) pair. Returns None for things we can't represent
/// (e.g. framework references, full paths).
///
/// pkg-config's `libs` is a Vec<String> of names like `rnp`, `z`,
/// `bz2` — no `-l` prefix. The static variant is encoded as
/// `lib<name>.a` which we strip back to `<name>`.
#[cfg(not(feature = "vendored"))]
fn parse_pkg_config_lib(s: &str) -> Option<LinkLib> {
    let (kind, name) = match s.strip_prefix("lib").and_then(|n| n.strip_suffix(".a")) {
        Some(basename) => (LinkLibKind::Static, basename.to_string()),
        None => (LinkLibKind::Dylib, s.to_string()),
    };
    Some(LinkLib { name, kind })
}

#[cfg(not(feature = "vendored"))]
fn locate_via_hardcoded_paths() -> LibrnpLocation {
    let candidate_dirs: Vec<PathBuf> = if cfg!(target_os = "macos") {
        vec![
            PathBuf::from("/opt/homebrew/include"),
            PathBuf::from("/usr/local/include"),
            PathBuf::from("/usr/include"),
        ]
    } else {
        vec![PathBuf::from("/usr/include")]
    };
    let include_dir = candidate_dirs
        .iter()
        .find(|d| d.join("rnp").join("rnp.h").exists())
        .cloned()
        .unwrap_or_else(|| PathBuf::from("/usr/include"));
    let lib_dir = if cfg!(target_os = "macos") {
        let mut candidate = include_dir.clone();
        candidate.pop();
        candidate.push("lib");
        if candidate.exists() {
            Some(candidate)
        } else {
            None
        }
    } else {
        None
    };
    // Homebrew's librnp.a (when brew ships the static archive) has
    // unresolved symbols for its transitive deps — the .dylib records
    // them via install_name but .a does not. Emit them explicitly as a
    // fallback when pkg-config didn't surface them. Linux package
    // managers (.deb, .rpm) record transitive deps in the .so, so this
    // list is macOS-only.
    //
    // `c++` provides ___gxx_personality_v0 (C++ exception personality
    // routine) that the static archive references but doesn't carry.
    let extra_link_libs: Vec<LinkLib> = if cfg!(target_os = "macos") {
        ["z", "bz2", "json-c", "botan-3", "sexpp", "c++"]
            .iter()
            .map(|&name| LinkLib {
                name: name.to_string(),
                kind: LinkLibKind::Dylib,
            })
            .collect()
    } else {
        Vec::new()
    };
    LibrnpLocation {
        include_dir,
        lib_dir,
        link_mode: LinkMode::System,
        librnp_version: None,
        link_json_c: false,
        extra_lib_dirs: Vec::new(),
        extra_link_libs,
    }
}

// -----------------------------------------------------------------------
// Bindings: pregenerated vs runtime bindgen.
//
// The crate ships a pregenerated bindings/bindings-<librnp-version>.rs so
// cross builds don't need a working host-side libclang (minimal cross
// containers routinely ship a libclang that can't even find stdbool.h).
// The file is target-independent: rnp.h is all opaque handles + primitives,
// and C types render as std::os::raw aliases (c_char etc.) that resolve
// per-target at compile time. It is generated with the experimental PQC +
// crypto-refresh defines so one file serves all feature combos — unused
// symbols are absorbed by ffi's dead_code allow.
//
// Selection order:
//   1. RNP_BINDINGS_RUNTIME=1      → always runtime bindgen
//   2. RNP_BINDINGS_PREGENERATED=1  → always the shipped file (escape
//      hatch for cross builds in Explicit/System mode)
//   3. auto: vendored build against the exact librnp version the file was
//      generated from (HEAD always falls back — its API surface drifts)
// -----------------------------------------------------------------------

/// librnp version the shipped pregenerated bindings were generated against.
const PREGENERATED_LIBRNP_VERSION: &str = "0.18.1";

fn pregenerated_bindings_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("bindings")
        .join(format!("bindings-{PREGENERATED_LIBRNP_VERSION}.rs"))
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .map(|v| !v.is_empty() && v != "0")
        .unwrap_or(false)
}

enum BindingsSource {
    Pregenerated(PathBuf),
    Runtime,
}

fn bindings_source(loc: &LibrnpLocation) -> BindingsSource {
    if env_flag("RNP_BINDINGS_RUNTIME") {
        return BindingsSource::Runtime;
    }
    let prebuilt = pregenerated_bindings_path();
    if env_flag("RNP_BINDINGS_PREGENERATED") {
        assert!(
            prebuilt.exists(),
            "RNP_BINDINGS_PREGENERATED=1 but {} does not exist",
            prebuilt.display()
        );
        return BindingsSource::Pregenerated(prebuilt);
    }
    let version_matches = loc.librnp_version.as_deref() == Some(PREGENERATED_LIBRNP_VERSION);
    if loc.link_mode == LinkMode::Vendored && version_matches && prebuilt.exists() {
        return BindingsSource::Pregenerated(prebuilt);
    }
    BindingsSource::Runtime
}

/// Under RNP_BINDINGS_REGENERATE=1, copy the just-generated bindings back
/// into bindings/ so they can be committed. Requires the vendored feature
/// (that's how we know which librnp version the headers are from). Pair
/// with RNP_BINDINGS_EXPERIMENTAL=1 to include the PQC/crypto-refresh
/// surface in the shipped file.
fn maybe_regenerate_bindings(out_bindings: &Path, librnp_version: Option<&str>) {
    if !env_flag("RNP_BINDINGS_REGENERATE") {
        return;
    }
    let version = librnp_version.expect(
        "RNP_BINDINGS_REGENERATE requires --features vendored to identify the librnp version",
    );
    let dest_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("bindings");
    fs::create_dir_all(&dest_dir).expect("Couldn't create bindings/ directory");
    let dest = dest_dir.join(format!("bindings-{version}.rs"));
    let generated = fs::read_to_string(out_bindings).expect("Couldn't read generated bindings");
    let banner = format!(
        "// Generated by bindgen against librnp {version} headers, with\n\
         // RNP_EXPERIMENTAL_PQC and RNP_EXPERIMENTAL_CRYPTO_REFRESH defined.\n\
         // Regenerate via scripts/regenerate-bindings.sh. Do not edit.\n\n"
    );
    fs::write(&dest, banner + &generated).expect("Couldn't write pregenerated bindings");
    eprintln!("rnp-rs: regenerated {} — commit it", dest.display());
}

fn generate_bindings(include_dir: &std::path::Path) -> bindgen::Bindings {
    let pqc_on = cfg!(feature = "pqc") || env_flag("RNP_BINDINGS_EXPERIMENTAL");
    let crypto_refresh_on =
        cfg!(feature = "crypto-refresh") || env_flag("RNP_BINDINGS_EXPERIMENTAL");

    let mut builder = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", include_dir.display()))
        .clang_arg("-DRNP_USE_64BIT_STRICT")
        .clang_arg("-Wno-deprecated-declarations");

    if cfg!(feature = "pqc") {
        println!(
            "cargo:warning=rnp-rs: building with RNP_EXPERIMENTAL_PQC — requires \
             librnp built with ENABLE_PQC=ON"
        );
    }
    if pqc_on {
        builder = builder.clang_arg("-DRNP_EXPERIMENTAL_PQC");
    }
    if crypto_refresh_on {
        builder = builder.clang_arg("-DRNP_EXPERIMENTAL_CRYPTO_REFRESH");
    }

    builder
        .allowlist_function("rnp_.*")
        .allowlist_type("rnp_.*")
        .allowlist_var("RNP_.*")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .layout_tests(false)
        .default_macro_constant_type(bindgen::MacroTypeVariation::Signed)
        .generate()
        .expect("Unable to generate rnp bindings")
}

// -----------------------------------------------------------------------
// Link directives.
// -----------------------------------------------------------------------

/// Target predicates for build scripts: cfg! sees the HOST; these see the
/// crate's actual TARGET via CARGO_CFG_*.
fn cfg_target_os(name: &str) -> bool {
    env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok(name)
}

fn cfg_target_env(name: &str) -> bool {
    env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok(name)
}

fn emit_link_directives(loc: &LibrnpLocation) {
    match loc.link_mode {
        LinkMode::System | LinkMode::Explicit => {
            // rnp itself is always required.
            println!("cargo:rustc-link-lib=dylib=rnp");
            // When pkg-config or our fallback surfaced transitive deps
            // (libz, libbz2, libbotan-3, …), emit them after rnp so the
            // linker resolves them in dependency order.
            for lib in &loc.extra_link_libs {
                if lib.name == "rnp" {
                    continue; // already emitted above
                }
                let kind = match lib.kind {
                    LinkLibKind::Dylib => "dylib",
                    LinkLibKind::Static => "static",
                };
                println!("cargo:rustc-link-lib={kind}={}", lib.name);
            }
        }
        LinkMode::Vendored => {
            // rnp-src compiled librnp + all deps as static libraries.
            println!("cargo:rustc-link-lib=static=rnp");
            println!("cargo:rustc-link-lib=static=sexpp");
            println!("cargo:rustc-link-lib=static=botan-3");
            if loc.link_json_c {
                println!("cargo:rustc-link-lib=static=json-c");
            }
            println!("cargo:rustc-link-lib=static=z");
            println!("cargo:rustc-link-lib=static=bz2");

            // C++ standard library — librnp, sexpp, and Botan are all C++.
            // macOS uses libc++; Linux/MinGW link libstdc++ explicitly.
            // MSVC needs nothing here: cl-compiled objects carry embedded
            // /DEFAULTLIB directives for the CRT/C++ runtime (libcmt +
            // libcpmt, or msvcrt + msvcprt), and there is no stdc++.lib to
            // point at — emitting the line would fail the final link.
            // NOTE: build scripts must consult CARGO_CFG_TARGET_* — cfg!
            // evaluates against the HOST. The MSYS2 CI job runs cargo on a
            // windows-msvc host while targeting windows-gnu; a cfg!-based
            // msvc guard matched the host there and silently dropped
            // -lstdc++ for a gnu target (undefined std:: refs at link).
            if cfg_target_os("macos") {
                println!("cargo:rustc-link-lib=dylib=c++");
            } else if cfg_target_os("windows") && cfg_target_env("msvc") {
                // covered by /DEFAULTLIB in the objects themselves
            } else {
                println!("cargo:rustc-link-lib=dylib=stdc++");
            }

            // Windows system libs needed transitively by the vendored deps'
            // static archives:
            //   - advapi32: json-c's random_seed.c (CryptAcquireContext,
            //     CryptGenRandom, CryptReleaseContext) — only when json-c
            //     is linked (0.18.1 flavor)
            //   - ws2_32 / crypt32: Botan's Winsock + CryptoAPI usage
            if cfg_target_os("windows") {
                if loc.link_json_c {
                    println!("cargo:rustc-link-lib=dylib=advapi32");
                }
                println!("cargo:rustc-link-lib=dylib=ws2_32");
                println!("cargo:rustc-link-lib=dylib=crypt32");
            }
        }
    }
}
