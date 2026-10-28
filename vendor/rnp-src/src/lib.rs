//! rnp-src: compile librnp and all dependencies from source.
//!
//! A pure build-time library, mirroring the `botan-src` crate pattern: it
//! has no build script of its own and declares no `links`. The dependent
//! crate's build script (`rnp-sys`) calls [`build()`], so the compilation
//! lands in the *caller's* `OUT_DIR` and the caller owns all `cargo:`
//! directive emission.
//!
//! [`build()`] downloads and compiles:
//!
//! - librnp 0.18.1 (OpenPGP implementation), or HEAD under `pqc`/`crypto-refresh`
//! - json-c 0.17 (JSON parsing, required by librnp)
//! - zlib 1.3.1 (compression)
//! - bzip2 1.0.8 (compression, with bz_internal_error fix)
//!
//! Botan is provided by the [`botan_src`] crate dependency.
//!
//! Pure-logic types and constants live in [`links`] so they are
//! unit-testable here without invoking the C/C++ toolchain.

pub mod config;
pub mod links;

pub use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use config::BuildConfig;
use links::{CmakeDep, Deps, JSON_C, ZLIB};

/// librnp version this crate compiles by default (release tarball).
pub const RNP_VERSION: &str = "0.18.1";

/// When `pqc` or `crypto-refresh` Cargo feature is on, rnp-src builds
/// librnp from upstream `main` instead of the 0.18.1 release tarball.
/// librnp 0.18.1 has EC_Group/EC_Point code paths (gated behind
/// ENABLE_PQC=ON / ENABLE_CRYPTO_REFRESH=ON) that are incompatible with
/// Botan 3.12's opaque (PIMPL) types; `main` has the fixes.
///
/// Pinned to a specific commit for reproducible builds: `main` moves, and
/// an unpinned ref means a fresh build gets whatever landed that day —
/// including regressions nobody validated against this crate. The pinned
/// clone is synced (fetch + hard checkout) on every build, and the flavor
/// cache directory embeds the pin's short SHA, so bumping this constant
/// automatically invalidates stale cached artifacts.
///
/// To bump: update the SHA to the new upstream tip, run the
/// vendored+pqc build (CI job "pqc + crypto-refresh" validates), ship.
const RNP_HEAD_REF: &str = "470695b98abe8a427fc47847acb387c089cb156d";

const BZIP2_VERSION: &str = "1.0.8";

/// bzip2 tarball candidates: sourceware first, Debian source pool as the
/// mirror. Verified byte-identical for 1.0.8 (sha256
/// ab5a03176ee106d3f0fa90e381da478ddae405918153cca248e682cd0c4a2269) and
/// extracting to the same top-level `bzip2-1.0.8/` directory. The 502
/// outage of the single origin took down every CI leg at once (#99).
fn bzip2_tarball_candidates() -> Vec<String> {
    vec![
        format!("https://sourceware.org/pub/bzip2/bzip2-{BZIP2_VERSION}.tar.gz"),
        format!(
            "https://deb.debian.org/debian/pool/main/b/bzip2/bzip2_{BZIP2_VERSION}.orig.tar.gz"
        ),
    ]
}

// ---------------------------------------------------------------------
// Flavor — the single source of truth for "which librnp is this build?".
//
// Historically the flavor lived in three hand-agreed places (cfg-gated
// source choice, hand-written install-prefix string, reported version
// string); when they disagreed, cached artifacts from one flavor were
// served to another (the stale-PQC-cache and unpatched-cache bugs). The
// enum makes the disagreement impossible: every downstream decision —
// cache directory, reported version, source preparation — is derived
// here, in one match.
// ---------------------------------------------------------------------

/// Which librnp source a vendored build compiles, plus its backport
/// revision. Derived once from the crate's Cargo features; every
/// flavor-dependent decision is derived from the value, never from the
/// features again.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flavor {
    /// The 0.18.1 release tarball with backport level 1 applied (the
    /// short-RSA-MPI padding fix for Botan 3.13,
    /// `patches/rsa-short-mpi-botan-3.13.patch`).
    Release0181B1,
    /// librnp HEAD (`RNP_HEAD_REF`) for the PQC / crypto-refresh flavors;
    /// its API surface drifts, so callers fall back to runtime bindgen.
    Head,
}

impl Flavor {
    /// Resolve the flavor from this crate's Cargo features. The one place
    /// `cfg!` is consulted for flavor purposes.
    pub fn from_features() -> Self {
        if cfg!(feature = "pqc") || cfg!(feature = "crypto-refresh") {
            Flavor::Head
        } else {
            Flavor::Release0181B1
        }
    }

    /// Install directory name under the build prefix. Keyed by source,
    /// backport level, and (for HEAD) the exact pinned commit — so two
    /// flavors, two patch levels, or two pins never share a cached
    /// librnp.a / header set.
    pub fn cache_dir(self) -> String {
        match self {
            Flavor::Release0181B1 => "rnp-0.18.1-b1".to_string(),
            Flavor::Head => format!("rnp-head-{}", &RNP_HEAD_REF[..8]),
        }
    }

    /// Whether this flavor's librnp needs json-c. librnp `main` vendored
    /// nlohmann/json as a single header (upstream 4f5c4e6e, "Remove
    /// json-c mentions from the codebase") and no longer links json-c;
    /// the 0.18.1 release tarball still requires it.
    pub fn needs_json_c(self) -> bool {
        matches!(self, Flavor::Release0181B1)
    }

    /// Version string reported in [`Installed::librnp_version`]. Drives
    /// pregenerated-bindings selection in dependents: `"head"` always
    /// means "API surface drifts, bind at runtime".
    pub fn librnp_version(self) -> &'static str {
        match self {
            Flavor::Release0181B1 => RNP_VERSION,
            Flavor::Head => "head",
        }
    }

    /// Whether this flavor builds from the moving HEAD ref.
    pub fn is_head(self) -> bool {
        matches!(self, Flavor::Head)
    }
}

/// Build librnp (+ deps) with the detected configuration (platform
/// defaults, cargo features, and the caller's `BOTAN_CONFIGURE_*` env).
pub fn build() -> Installed {
    build_with(BuildConfig::detect())
}

/// Build librnp (+ deps) with an explicit [`BuildConfig`] — for
/// programmatic callers that want to name the Botan compiler and module
/// sets in code instead of relying on environment ordering. Resolution
/// semantics and precedence are documented on [`BuildConfig`].
pub fn build_with(config: BuildConfig) -> Installed {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let src_dir = out_dir.join("src");
    use sha2::{Digest, Sha256};
    let mut identity = Sha256::new();
    for source in [include_bytes!("lib.rs").as_slice(), include_bytes!("links.rs").as_slice(),
        include_bytes!("../botan/BotanConfig.cmake.in").as_slice()] { identity.update(source); }
    let identity = format!("{:x}", identity.finalize());
    let prefix = out_dir.join(format!("i-{}", &identity[..8]));
    fs::create_dir_all(&src_dir).ok();
    fs::create_dir_all(&prefix).ok();

    // Set macOS deployment target for consistent ABI compatibility.
    // env::set_var is unsafe in Rust 2024 edition.
    if cfg!(target_os = "macos") {
        unsafe {
            env::set_var("MACOSX_DEPLOYMENT_TARGET", "11.0");
        }
    }

    // Botan build configuration: one BuildConfig value, resolved with a
    // single precedence (defaults < env < feature-merge-as-union) and
    // applied at one write point. This replaces the scattered env
    // set_vars whose "default applied last" behavior stomped caller
    // intent (MSVC cross CC on Windows: #103; caller module lists under
    // the pqc feature). Policy and its table tests live in `config`.
    // Applied here — after the cache check — so cached builds never
    // mutate the process environment.
    config.apply();

    // --- 1. Botan (via botan-src crate) ---
    eprintln!("rnp-src: building Botan via botan-src crate...");
    let botan_prefix = build_botan(&prefix);

    // One flavor value decides everything below: which deps are built,
    // the cache directory (keyed by source + backport level + pin), the
    // reported version, and which source tree is prepared.
    let flavor = Flavor::from_features();

    // --- 2/3. cmake-based deps (json-c, zlib) ---
    // json-c is only needed by the 0.18.1 release tarball; librnp main
    // vendors nlohmann/json (see Flavor::needs_json_c).
    let jsonc_prefix = prefix.join("json-c");
    if flavor.needs_json_c()
        && !jsonc_prefix
            .join("lib")
            .join(staged_static_marker("libjson-c.a"))
            .exists()
    {
        eprintln!("rnp-src: building json-c {}...", JSON_C.version);
        cmake_dep_build(&JSON_C, &src_dir, &jsonc_prefix);
    }

    let zlib_prefix = prefix.join("zlib");
    if !zlib_prefix
        .join("lib")
        .join(staged_static_marker("libz.a"))
        .exists()
    {
        eprintln!("rnp-src: building zlib {}...", ZLIB.version);
        cmake_dep_build(&ZLIB, &src_dir, &zlib_prefix);
    }

    // --- 4. bzip2 (manual make + bz_internal_error shim) ---
    let bzip2_prefix = prefix.join("bzip2");
    if !bzip2_prefix
        .join("lib")
        .join(staged_static_marker("libbz2.a"))
        .exists()
    {
        eprintln!("rnp-src: building bzip2 {BZIP2_VERSION}...");
        build_bzip2(&src_dir, &bzip2_prefix);
    }

    // --- 5. librnp ---
    let rnp_prefix = prefix.join(flavor.cache_dir());
    if !rnp_prefix
        .join("lib")
        .join(staged_static_marker("librnp.a"))
        .exists()
    {
        let mut deps = Deps::new();
        deps.push("botan", botan_prefix.clone());
        if flavor.needs_json_c() {
            deps.push("jsonc", jsonc_prefix.clone());
        }
        deps.push("zlib", zlib_prefix.clone());
        deps.push("bzip2", bzip2_prefix.clone());

        eprintln!(
            "rnp-src: building librnp {} ({:?})...",
            flavor.librnp_version(),
            flavor
        );
        build_librnp(&src_dir, &rnp_prefix, &deps, &botan_prefix);
    }

    let librnp_version = flavor.librnp_version().to_string();

    let mut deps = Deps::new();
    deps.push("botan", botan_prefix.clone());
    if flavor.needs_json_c() {
        deps.push("jsonc", jsonc_prefix.clone());
    }
    deps.push("zlib", zlib_prefix.clone());
    deps.push("bzip2", bzip2_prefix.clone());

    Installed {
        lib_dir: rnp_prefix.join("lib"),
        include_dir: rnp_prefix.join("include"),
        librnp_version,
        flavor,
        dep_lib_dirs: deps.lib_dirs().collect(),
    }
}

/// Install layout produced by [`build`].
#[derive(Debug, Clone)]
pub struct Installed {
    /// Directory containing `librnp.a` + `libsexpp.a`.
    pub lib_dir: PathBuf,
    /// Directory containing `rnp/rnp.h` and friends.
    pub include_dir: PathBuf,
    /// `"0.18.1"` or `"head"` — which librnp source was built.
    pub librnp_version: String,
    /// Which flavor was built; lets dependents derive flavor-dependent
    /// decisions (e.g. whether json-c is linked) from the same value.
    pub flavor: Flavor,
    /// Per-dependency lib dirs (botan, json-c, zlib, bzip2) for the
    /// caller's `-L` link-search emissions.
    pub dep_lib_dirs: Vec<PathBuf>,
}

// ---------------------------------------------------------------------
// Botan — built via botan-src, then staged into our prefix via manual
// file copies (skipping the brittle `make install` step).
// ---------------------------------------------------------------------

fn build_botan(prefix: &Path) -> PathBuf {
    let botan_prefix = prefix.join("botan");
    fs::create_dir_all(botan_prefix.join("lib")).ok();
    fs::create_dir_all(botan_prefix.join("include")).ok();

    // botan-src's configure.py defaults to the DLL CRT (/MD) on MSVC.
    // When the final Rust link is crt-static, forward /MT through
    // botan-src's env passthrough (--msvc-runtime) so botan's objects
    // match the link. In-process env: botan_src::build() reads it in
    // this same build-script process.
    if target_is_msvc() && crt_static() {
        // SAFETY: a cargo build script is single-threaded at this point —
        // nothing else can be reading the environment concurrently.
        unsafe { env::set_var("BOTAN_CONFIGURE_MSVC_RUNTIME", "MT") };
    }

    // Share the one pinned Botan build with the S/MIME wrapper. No duplicate archive or
    // linker-search-order choice between separate cryptographic builds.
    let botan_build_dir = env::var("DEP_BOTAN_LIB_DIR").expect("shared Botan lib metadata");
    let botan_include_dir = PathBuf::from(env::var_os("DEP_BOTAN_INCLUDE_DIR").expect("shared Botan headers"));

    // Botan's static lib filename differs by platform: Unix uses the
    // libfoo.a convention; Windows (even with mingw/MSYS2) produces
    // botan-3.lib via `ar crs`. Pick the right name so we don't panic
    // on Windows looking for a file that doesn't exist.
    let lib_name = if cfg!(target_os = "windows") {
        "botan-3.lib"
    } else {
        "libbotan-3.a"
    };

    // Static library.
    let lib_src = PathBuf::from(&botan_build_dir).join(lib_name);
    if !lib_src.exists() {
        panic!(
            "rnp-src: expected Botan static library at {}, but it was not produced",
            lib_src.display()
        );
    }
    // Always copy as libbotan-3.a — cargo's rustc-link-lib=static=botan-3
    // searches for libbotan-3.a on Unix AND on Windows GNU target
    // (x86_64-pc-windows-gnu). Botan's Windows build produces botan-3.lib;
    // renaming to libbotan-3.a is safe because the archive format is the
    // same (GNU ar).
    fs::copy(&lib_src, botan_prefix.join("lib").join("libbotan-3.a"))
        .expect("rnp-src: failed to copy Botan static library into prefix");

    // Public headers — botan-src places them at
    // {build_dir}/build/include/public/ (note the double `build/`).
    let headers_src = botan_include_dir;
    let headers_dst = botan_prefix.join("include").join("botan-3");
    copy_dir_recursive(&headers_src, &headers_dst)
        .expect("rnp-src: failed to copy Botan public headers into prefix");

    // Generate BotanConfig.cmake from a template. See
    // rnp-src/botan/BotanConfig.cmake.in.
    write_botan_cmake_config(&botan_prefix);

    eprintln!("rnp-src: botan install prefix = {}", botan_prefix.display());
    botan_prefix
}

fn write_botan_cmake_config(botan_prefix: &Path) {
    let cmake_dir = botan_prefix
        .join("lib")
        .join("cmake")
        .join(format!("Botan-{}", botan_src::BOTAN_VERSION));
    fs::create_dir_all(&cmake_dir).ok();

    let template = include_str!("../botan/BotanConfig.cmake.in");
    let prefix_str = botan_prefix.display().to_string();
    let config = template
        .replace("@BOTAN_VERSION@", botan_src::BOTAN_VERSION)
        .replace("@BOTAN_PREFIX@", &prefix_str);

    fs::write(cmake_dir.join("BotanConfig.cmake"), config)
        .expect("rnp-src: failed to write BotanConfig.cmake");
    fs::write(
        cmake_dir.join("BotanConfigVersion.cmake"),
        format!("set(PACKAGE_VERSION \"{}\")\n", botan_src::BOTAN_VERSION),
    )
    .expect("rnp-src: failed to write BotanConfigVersion.cmake");
}

// ---------------------------------------------------------------------
// Cross-compile passthrough. Without a toolchain file, CMake assumes a
// build for the host OS and injects host-only assumptions (-rdynamic,
// extensionless ELF executables) that break mingw/OHOS cross builds.
// Users declare the target platform via:
//   RNP_CMAKE_TOOLCHAIN=/path/to/toolchain.cmake  → -DCMAKE_TOOLCHAIN_FILE
//   RNP_CMAKE_ARGS="-DCMAKE_SYSTEM_NAME=..."      → extra configure flags
// ---------------------------------------------------------------------

fn cross_toolchain_set() -> bool {
    env::var("RNP_CMAKE_TOOLCHAIN")
        .map(|v| !v.trim().is_empty())
        .is_ok_and(|v| v)
}

/// True when the cargo TARGET abi is MSVC: no POSIX toolchain
/// (make/gcc/ar) exists on that host, and compiler flag spellings
/// differ from gcc/clang.
fn target_is_msvc() -> bool {
    env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
}

/// True when the final Rust link requests a static CRT
/// (`-C target-feature=+crt-static`): every C/C++ object we stage must
/// then be compiled /MT — the static CRT (libucrt.lib) carries no
/// `__imp_*` DLL-import thunks, so any /MD object in the link fails
/// with LNK2019 on `__imp_strdup` and friends. Absence of the feature
/// means /MD, the toolchain default, and nothing changes.
fn crt_static() -> bool {
    env::var("CARGO_CFG_TARGET_FEATURE")
        .map(|features| crt_static_in_features(&features))
        .unwrap_or(false)
}

/// The parse half of crt_static(), split out for unit tests.
fn crt_static_in_features(features: &str) -> bool {
    features.split(',').any(|f| f == "crt-static")
}

/// Map a canonical Unix static-archive name to what the staged install
/// actually contains for the current target: MSVC drops the `lib` prefix
/// and the `.a` suffix (`librnp.a` → `rnp.lib`; `libz.a` → `z.lib` once
/// the alias table has run). Idempotency guards check for this name —
/// checking the Unix name on MSVC would rebuild every dep on every run.
fn staged_static_marker(unix_archive: &str) -> String {
    if !target_is_msvc() {
        return unix_archive.to_string();
    }
    let stem = unix_archive.strip_prefix("lib").unwrap_or(unix_archive);
    let stem = stem.strip_suffix(".a").unwrap_or(stem);
    format!("{stem}.lib")
}

/// Compiler pair for the librnp cmake build. One precedence, matching
/// `build_bzip2`'s existing CC contract: caller's `CC`/`CXX` env first
/// (the standard way to point a cross build at the target compiler —
/// clang-only environments like MSYS2 clangarm64 have no gcc to fall
/// back to, see #120), then platform defaults.
fn librnp_compilers() -> (String, String) {
    librnp_compilers_with(|key| std::env::var_os(key))
}

/// Pure resolution core of [`librnp_compilers`] with the environment
/// injected, so the precedence is table-testable.
fn librnp_compilers_with(get_env: impl Fn(&str) -> Option<OsString>) -> (String, String) {
    fn pick(get_env: &impl Fn(&str) -> Option<OsString>, key: &str, default: &str) -> String {
        get_env(key)
            .map(|v| v.to_string_lossy().into_owned())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| default.to_string())
    }
    let macos_target = get_env("CARGO_CFG_TARGET_OS").as_deref() == Some(OsStr::new("macos"));
    let (default_cc, default_cxx) = if macos_target {
        ("/usr/bin/clang", "/usr/bin/clang++")
    } else {
        ("gcc", "g++")
    };
    (
        pick(&get_env, "CC", default_cc),
        pick(&get_env, "CXX", default_cxx),
    )
}

/// Stable fingerprint of the librnp cmake configuration. Cache-busting
/// key for the rnp-build tree: any change to the args or the caller's
/// cmake passthroughs must invalidate a previously configured tree.
fn cmake_fingerprint(cmake_args: &[String], toolchain: &str, extra_args: &str) -> String {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for arg in cmake_args {
        arg.hash(&mut hasher);
    }
    toolchain.hash(&mut hasher);
    extra_args.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Guard the one invariant the ABI-mismatch crash taught us: librnp must
/// be compiled against the headers of the Botan we link. Panics at build
/// time instead of shipping a binary that corrupts C++ object layout.
fn assert_botan_headers_from_staged_prefix(build_dir: &Path, botan_prefix: &Path) {
    let Ok(cache) = fs::read_to_string(build_dir.join("CMakeCache.txt")) else {
        return;
    };
    let Some(include_dir) = cache
        .lines()
        .find_map(|line| line.strip_prefix("BOTAN_INCLUDE_DIR:PATH="))
    else {
        return;
    };
    if !Path::new(include_dir).starts_with(botan_prefix) {
        panic!(
            "rnp-src: librnp resolved Botan headers from {include_dir}, outside the staged \
             Botan prefix {}. Compiling librnp against foreign Botan headers while linking \
             ours corrupts the C++ ABI (rnpgp/rnp-rs#100). Remove that build tree and any \
             conflicting pkg-config entries, then rebuild.",
            botan_prefix.display()
        );
    }
}

fn append_cross_passthrough(cmd: &mut Command) {
    if let Ok(toolchain) = env::var("RNP_CMAKE_TOOLCHAIN")
        && !toolchain.trim().is_empty()
    {
        cmd.arg(format!("-DCMAKE_TOOLCHAIN_FILE={toolchain}"));
    }
    if let Ok(args) = env::var("RNP_CMAKE_ARGS") {
        cmd.args(args.split_whitespace());
    }
}

/// MSVC CRT selection for every cmake-built dep (json-c, zlib, librnp):
/// when the final Rust link is crt-static, force the static CRT.
/// CMAKE_MSVC_RUNTIME_LIBRARY only takes effect under policy CMP0091=NEW,
/// which projects with an old cmake_minimum_required pin to OLD — the
/// POLICY_DEFAULT override keeps the knob live for them. No-op elsewhere:
/// /MD stays the toolchain default, and mingw/OHOS never see the flags.
fn append_msvc_crt_args(cmd: &mut Command) {
    if target_is_msvc() && crt_static() {
        cmd.args([
            "-DCMAKE_POLICY_DEFAULT_CMP0091=NEW",
            "-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded",
        ]);
    }
}

// ---------------------------------------------------------------------
// Generic cmake dep builder. json-c and zlib are config-driven via
// `CmakeDep`; this is the single place that knows how to invoke cmake.
// Adding a new cmake-based dep = one `CmakeDep` const + a `Deps::push`
// call in main(); no new function.
// ---------------------------------------------------------------------

fn cmake_dep_build(dep: &CmakeDep, src_root: &Path, prefix: &Path) {
    let src = dep.source_dir(src_root);
    if !src.exists() {
        download_and_extract(&[dep.url()], src_root);
    }

    let build_dir = dep.build_dir(src_root);

    let mut configure = Command::new("cmake");
    configure
        .args([
            "-S",
            src.to_str().unwrap(),
            "-B",
            build_dir.to_str().unwrap(),
        ])
        .args(["-DCMAKE_BUILD_TYPE=Release", "-DBUILD_SHARED_LIBS=OFF"])
        .args(dep.extra_cmake_args)
        .arg(format!("-DCMAKE_INSTALL_PREFIX={}", prefix.display()));
    if let Some(min) = dep.cmake_policy_minimum {
        configure.arg(format!("-DCMAKE_POLICY_VERSION_MINIMUM={min}"));
    }
    append_msvc_crt_args(&mut configure);
    append_cross_passthrough(&mut configure);
    run(&mut configure, &format!("{} cmake", dep.name));

    run(
        Command::new("cmake").args([
            "--build",
            build_dir.to_str().unwrap(),
            "--parallel",
            &nproc(),
            // Multi-config generators (the Visual Studio default on a
            // windows host) build Debug without this while the generated
            // install script looks in Release/. A no-op on single-config
            // generators.
            "--config",
            "Release",
        ]),
        &format!("{} build", dep.name),
    );
    run(
        Command::new("cmake").args([
            "--install",
            build_dir.to_str().unwrap(),
            "--config",
            "Release",
        ]),
        &format!("{} install", dep.name),
    );

    // Apply platform-specific library filename aliases (e.g., zlib's
    // libzlibstatic.a on MinGW → libz.a). See CmakeDep::installed_lib_aliases.
    apply_lib_aliases(dep.installed_lib_aliases, &prefix.join("lib"), dep.name);
}

/// For each `(actual, expected)` pair: if `actual` exists under `lib_dir`
/// and `expected` doesn't, copy `actual` → `expected`. Idempotent and
/// silent on platforms where the source doesn't exist.
///
/// If after processing all aliases, any `expected` target is still missing,
/// dumps the directory listing to stderr for diagnosis (cargo hides build
/// script stderr on success, but on failure it appears in the error log).
fn apply_lib_aliases(aliases: &[(&str, &str)], lib_dir: &Path, dep_name: &str) {
    for (actual, expected) in aliases {
        let actual_path = lib_dir.join(actual);
        let expected_path = lib_dir.join(expected);
        if !actual_path.exists() || expected_path.exists() {
            continue;
        }
        fs::copy(&actual_path, &expected_path).unwrap_or_else(|e| {
            panic!(
                "rnp-src: {dep_name}: failed to alias {actual} → {expected} in {}: {e}",
                lib_dir.display()
            )
        });
        eprintln!("rnp-src: {dep_name}: aliased {actual} → {expected}");
    }

    // Verify all expected targets now exist; if not, list the lib dir for
    // diagnosis so the exact platform-specific filename is visible.
    let mut missing: Vec<&str> = aliases
        .iter()
        .filter(|(_, expected)| !lib_dir.join(expected).exists())
        .map(|(_, expected)| *expected)
        .collect();
    missing.sort_unstable();
    missing.dedup();
    if missing.is_empty() {
        return;
    }
    eprintln!(
        "rnp-src: {dep_name}: WARNING — expected lib(s) {} still missing after aliasing; listing {}:",
        missing.join(", "),
        lib_dir.display()
    );
    if let Ok(entries) = fs::read_dir(lib_dir) {
        for entry in entries.flatten() {
            eprintln!("  {}", entry.file_name().to_string_lossy());
        }
    }
}

// ---------------------------------------------------------------------
// bzip2 — hand-rolled because its Makefile is not cmake-compatible
// and it needs the bz_internal_error shim.
// ---------------------------------------------------------------------

fn build_bzip2(src_dir: &Path, prefix: &Path) {
    let bzip2_src = src_dir.join(format!("bzip2-{BZIP2_VERSION}"));
    if !bzip2_src.exists() {
        download_and_extract(&bzip2_tarball_candidates(), src_dir);
    }

    // MSVC targets: make/gcc/ar do not exist on a windows-msvc host, and
    // GNU-toolchain objects would carry the wrong CRT for the final link
    // anyway. Compile with cl and archive with lib instead.
    if target_is_msvc() {
        build_bzip2_msvc(&bzip2_src, prefix);
        return;
    }

    // Honor CC from the environment (cross builds point it at the target
    // compiler) before falling back to platform defaults.
    let cc = env::var("CC").unwrap_or_else(|_| {
        if cfg!(target_os = "macos") {
            "/usr/bin/clang".to_string()
        } else {
            "gcc".to_string()
        }
    });

    run(
        Command::new("make")
            .args(["libbz2.a"])
            .args(["-j", &nproc()])
            .args([format!("CC={cc}"), "CFLAGS=-O3 -fPIC".to_string()])
            .current_dir(&bzip2_src),
        "bzip2 make",
    );

    // Fix: bzip2's Makefile doesn't define bz_internal_error, leaving an
    // undefined symbol in libbz2.a. Write a small .c file, compile it,
    // and append the .o to the archive.
    let shim_src = bzip2_src.join("bz_internal_error_shim.c");
    fs::write(
        &shim_src,
        "#include <stdlib.h>\nvoid bz_internal_error(int errcode) { (void)errcode; abort(); }\n",
    )
    .unwrap();
    run(
        Command::new(cc)
            .args(["-c", "-O3", "-fPIC"])
            .arg(&shim_src)
            .arg("-o")
            .arg(bzip2_src.join("bz_internal_error_shim.o"))
            .current_dir(&bzip2_src),
        "bzip2 bz_internal_error shim compile",
    );
    run(
        Command::new("ar")
            .args(["rcs", "libbz2.a", "bz_internal_error_shim.o"])
            .current_dir(&bzip2_src),
        "bzip2 append shim to libbz2.a",
    );

    fs::create_dir_all(prefix.join("lib")).ok();
    fs::create_dir_all(prefix.join("include")).ok();
    fs::copy(
        bzip2_src.join("libbz2.a"),
        prefix.join("lib").join("libbz2.a"),
    )
    .unwrap();
    fs::copy(
        bzip2_src.join("bzlib.h"),
        prefix.join("include").join("bzlib.h"),
    )
    .unwrap();
}

/// bzip2 on MSVC: the Makefile path needs make/gcc/ar, none of which
/// exist on a windows-msvc host. bzip2 is seven C files — compile them
/// and the bz_internal_error shim with cl, archive with lib.
fn build_bzip2_msvc(bzip2_src: &Path, prefix: &Path) {
    const SRCS: &[&str] = &[
        "blocksort.c",
        "huffman.c",
        "crctable.c",
        "randtable.c",
        "compress.c",
        "decompress.c",
        "bzlib.c",
    ];

    // Match the final link's CRT: cargo's crt-static target feature is
    // the same signal every /MT consumer keys on; its absence means /MD.
    let crt = if crt_static() { "/MT" } else { "/MD" };

    let shim_src = bzip2_src.join("bz_internal_error_shim.c");
    fs::write(
        &shim_src,
        "#include <stdlib.h>\nvoid bz_internal_error(int errcode) { (void)errcode; abort(); }\n",
    )
    .unwrap();

    let mut objs: Vec<PathBuf> = Vec::new();
    for src in SRCS {
        let obj = bzip2_src.join(src.replace(".c", ".obj"));
        run(
            Command::new("cl")
                .args(["/nologo", "/c", "/O2", crt])
                .arg(bzip2_src.join(src))
                .arg(format!("/Fo{}", obj.display()))
                .current_dir(bzip2_src),
            "bzip2 cl",
        );
        objs.push(obj);
    }
    let shim_obj = bzip2_src.join("bz_internal_error_shim.obj");
    run(
        Command::new("cl")
            .args(["/nologo", "/c", "/O2", crt])
            .arg(&shim_src)
            .arg(format!("/Fo{}", shim_obj.display()))
            .current_dir(bzip2_src),
        "bzip2 bz_internal_error shim compile",
    );
    objs.push(shim_obj);

    let lib = bzip2_src.join("libbz2.lib");
    run(
        Command::new("lib")
            .arg("/nologo")
            .arg(format!("/OUT:{}", lib.display()))
            .args(&objs)
            .current_dir(bzip2_src),
        "bzip2 lib",
    );

    fs::create_dir_all(prefix.join("lib")).ok();
    fs::create_dir_all(prefix.join("include")).ok();
    fs::copy(&lib, prefix.join("lib").join("libbz2.lib")).unwrap();
    // FindBZip2's NAMES cover both spellings; stage both rather than
    // guess which cmake module version does the looking.
    fs::copy(&lib, prefix.join("lib").join("bz2.lib")).unwrap();
    fs::copy(
        bzip2_src.join("bzlib.h"),
        prefix.join("include").join("bzlib.h"),
    )
    .unwrap();
}

// ---------------------------------------------------------------------
// librnp — the final consumer of all deps above.
// ---------------------------------------------------------------------

/// Download + extract the librnp release tarball (default path), then
/// apply the backport patches it needs.
fn prepare_librnp_release(src_dir: &Path) -> PathBuf {
    let rnp_src = src_dir.join(format!("rnp-v{RNP_VERSION}"));
    if !rnp_src.exists() {
        let url = format!(
            "https://github.com/rnpgp/rnp/releases/download/v{RNP_VERSION}/rnp-v{RNP_VERSION}.tar.gz"
        );
        download_and_extract(&[url], src_dir);
    }
    apply_backports(&rnp_src);
    rnp_src
}

/// Apply `patches/*.patch` (format-patch style) to an extracted release
/// tree. Idempotent via a stamp file, so a cached extraction from before a
/// patch was added still gets patched (the paired install-prefix rename in
/// [`build`] then forces a rebuild of librnp itself).
fn apply_backports(rnp_src: &Path) {
    const STAMP: &str = ".rnp-backports-b1";
    if rnp_src.join(STAMP).exists() {
        return;
    }
    let patch = include_str!("../patches/rsa-short-mpi-botan-3.13.patch").replace("\r\n", "\n");
    let patch = &patch[patch.find("--- a/").expect("patch header")..];
    let patch = diffy::Patch::from_str(patch).expect("embedded upstream backport");
    let source = rnp_src.join("src/lib/crypto/rsa.cpp");
    let original = fs::read_to_string(&source).expect("RSA source");
    let updated = diffy::apply(&original, &patch).expect("exact upstream backport context");
    fs::write(source, updated).expect("write RSA backport");
    fs::write(rnp_src.join(STAMP), b"applied\n").expect("write backport stamp");
    eprintln!("rnp-src: applied backport: pad short RSA MPIs (Botan 3.13, rnpgp/rnp#2465)");
}

/// Clone librnp HEAD (or the pinned ref) for PQC/crypto-refresh builds.
/// librnp 0.18.1's PQC + crypto-refresh code paths don't compile against
/// Botan 3.12+; HEAD has the fix.
fn prepare_librnp_head(src_dir: &Path) -> PathBuf {
    let rnp_src = src_dir.join("rnp-head");
    if !rnp_src.exists() {
        eprintln!("rnp-src: cloning librnp (shallow)...");
        run(
            Command::new("git")
                .args(["clone", "--depth", "1", "--recurse-submodules"])
                .arg("https://github.com/rnpgp/rnp.git")
                .arg(&rnp_src),
            "git clone librnp",
        );
    }

    // Sync the clone to the pin on every build — a cached clone from an
    // earlier pin must not keep serving stale source. `--force` discards
    // the Botan-include patch modifications, which are re-applied below
    // (the patcher is idempotent, so a no-op on already-patched trees).
    run(
        Command::new("git").args(["-C"]).arg(&rnp_src).args([
            "fetch",
            "--depth",
            "1",
            "origin",
            RNP_HEAD_REF,
        ]),
        "git fetch pinned librnp commit",
    );
    run(
        Command::new("git").args(["-C"]).arg(&rnp_src).args([
            "checkout",
            "--force",
            "--detach",
            RNP_HEAD_REF,
        ]),
        "git checkout pinned librnp commit",
    );
    run(
        Command::new("git")
            .args(["-C"])
            .arg(&rnp_src)
            .args(["submodule", "sync", "--recursive"]),
        "git submodule sync",
    );
    run(
        Command::new("git").args(["-C"]).arg(&rnp_src).args([
            "submodule",
            "update",
            "--init",
            "--recursive",
        ]),
        "git submodule update",
    );

    // Local compatibility patches. Botan 3.11+ made several types opaque
    // (PIMPL): EC_Group, EC_Point, BigInt, EC_AffinePoint. Code that
    // references these types by name needs to #include the corresponding
    // header explicitly — older Botan headers transitively pulled them in
    // via ecdh.h, so librnp source doesn't always include them. Scan the
    // crypto source tree and inject any missing includes. Idempotent.
    patch_librnp_botan_includes(&rnp_src);
    eprintln!("rnp-src: librnp HEAD synced to {RNP_HEAD_REF}");

    rnp_src
}

/// For each `.cpp`/`.hpp` under librnp's `src/lib/crypto/`, check whether
/// it references one of the Botan types whose header became opaque in
/// Botan 3.11+ (and thus needs an explicit `#include`). If the file uses
/// the type but doesn't include the header, inject the include right
/// after the first existing `#include "botan/...` line.
///
/// Idempotent: re-running on an already-patched tree is a no-op.
fn patch_librnp_botan_includes(rnp_src: &Path) {
    /// (Botan type prefix, header to include)
    ///
    /// Botan 3.11+ made several types opaque (PIMPL). The headers below
    /// are correct for Botan 3.12; if a type moves between headers in a
    /// future release, this table needs updating.
    const TYPE_HEADER_PAIRS: &[(&str, &str)] = &[
        ("Botan::EC_Group", "botan/ec_group.h"),
        // EC_Point is declared inside ec_group.h, not its own header.
        ("Botan::EC_AffinePoint", "botan/ec_apoint.h"),
        ("Botan::BigInt", "botan/bigint.h"),
        ("Botan::ECDH_PrivateKey", "botan/ecdh.h"),
        ("Botan::ECDSA_PrivateKey", "botan/ecdsa.h"),
        ("Botan::Ed25519_PrivateKey", "botan/ed25519.h"),
        ("Botan::Ed448_PrivateKey", "botan/ed448.h"),
        ("Botan::X25519_PrivateKey", "botan/x25519.h"),
        ("Botan::X448_PrivateKey", "botan/x448.h"),
    ];

    let crypto_dir = rnp_src.join("src/lib/crypto");
    let mut files: Vec<PathBuf> = Vec::new();
    collect_files(&crypto_dir, &["cpp", "hpp", "h"], &mut files);

    for file in files {
        let Ok(content) = fs::read_to_string(&file) else {
            continue;
        };
        let mut patched = content.clone();
        let mut changed = false;
        for (type_prefix, header) in TYPE_HEADER_PAIRS {
            let include_line = format!("#include <{header}>");
            if patched.contains(include_line.as_str()) {
                continue;
            }
            if !patched.contains(type_prefix) {
                continue;
            }
            // Inject the include right after the first existing botan include,
            // or after the file's first #include if no botan include exists yet.
            let needle = "#include <botan/";
            if let Some(idx) = patched.find(needle) {
                let line_end = patched[idx..]
                    .find('\n')
                    .map(|n| idx + n + 1)
                    .unwrap_or(patched.len());
                patched.insert_str(line_end, &format!("{include_line}\n"));
            } else if let Some(idx) = patched.find("#include") {
                let line_end = patched[idx..]
                    .find('\n')
                    .map(|n| idx + n + 1)
                    .unwrap_or(patched.len());
                patched.insert_str(line_end, &format!("{include_line}\n"));
            }
            changed = true;
        }
        if changed {
            fs::write(&file, patched)
                .unwrap_or_else(|e| panic!("rnp-src: failed to patch {}: {e}", file.display()));
        }
    }
}

fn collect_files(dir: &Path, extensions: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, extensions, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str())
            && extensions.contains(&ext)
        {
            out.push(path);
        }
    }
}

fn build_librnp(src_dir: &Path, prefix: &Path, deps: &Deps, botan_prefix: &Path) {
    // Which source tree: the release tarball (+ backports) or HEAD —
    // derived from the flavor, not re-derived from cfg!.
    let rnp_src = if Flavor::from_features().is_head() {
        prepare_librnp_head(src_dir)
    } else {
        prepare_librnp_release(src_dir)
    };

    let (cc, cxx) = librnp_compilers();

    let build_dir = src_dir.join("rnp-build");

    // The full cmake configuration as one ordered value: fingerprinted
    // below to decide whether a cached rnp-build tree is still valid.
    let mut cmake_args: Vec<String> = vec![
        "-S".to_string(),
        rnp_src.display().to_string(),
        "-B".to_string(),
        build_dir.display().to_string(),
    ];
    // A user-supplied toolchain file owns compiler selection; our hardcoded
    // host defaults would override it and break cross builds.
    if !cross_toolchain_set() {
        cmake_args.push(format!("-DCMAKE_C_COMPILER={cc}"));
        cmake_args.push(format!("-DCMAKE_CXX_COMPILER={cxx}"));
    }
    // MSVC has no POSIX <dirent.h>/<getopt.h>; librnp's own sources
    // include both (uniwin.h -> getoptwin.h, file-utils.h,
    // librekey/rnp_key_store.cpp). Upstream's MSVC CI gets them from
    // vcpkg's MSBuild integration, which injects the port include dir
    // into EVERY target — find_path results alone only reach rnp-common
    // and the CLI targets, not librnp-obj. Our vendored compat headers
    // (compat/msvc/, MIT tronkko dirent + a declaration-only getopt shim)
    // replicate that global injection via /I on the flag vars.
    let msvc_compat = if target_is_msvc() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("compat")
            .join("msvc");
        Some(dir.display().to_string().replace('\\', "/"))
    } else {
        None
    };
    let cxx_force_include = if let Some(compat) = &msvc_compat {
        format!("-DCMAKE_CXX_FLAGS=/EHsc /DBOTAN_DLL= /FI cstring /I{compat}")
    } else {
        "-DCMAKE_CXX_FLAGS=-include cstring".to_string()
    };
    cmake_args.extend([
        "-DCRYPTO_BACKEND=botan3".to_string(),
        "-DBUILD_SHARED_LIBS=OFF".to_string(),
        "-DBUILD_TESTING=OFF".to_string(),
        "-DENABLE_DOC=OFF".to_string(),
        "-DCMAKE_BUILD_TYPE=Release".to_string(),
        // librnp has translation units missing #include <cstring>;
        // force-include it. The spelling is toolchain-specific:
        // -include for gcc/clang, /FI for MSVC — cl ignores "-include"
        // (D9002) and then treats "cstring" as a SOURCE file (C1083),
        // which breaks cmake's compiler smoke test on the VS generator.
        cxx_force_include.to_string(),
        format!("-DCMAKE_PREFIX_PATH={}", deps.cmake_prefix_path()),
        format!("-DCMAKE_INSTALL_PREFIX={}", prefix.display()),
        "-DCMAKE_POLICY_VERSION_MINIMUM=3.5".to_string(),
        // Botan discovery authority: our staged Botan is what the final
        // link uses, so its headers must be what librnp is compiled
        // against. FindBotan ranks BOTAN_ROOT_DIR above pkg-config hints;
        // without it, a host pkg-config entry (e.g. Homebrew's botan-3.pc)
        // hijacks discovery via NO_DEFAULT_PATH and librnp gets compiled
        // against foreign headers while we link our Botan — a silent C++
        // ABI mismatch that crashes with garbage virtual dispatch
        // (rnpgp/rnp-rs#100).
        format!("-DBOTAN_ROOT_DIR={}", botan_prefix.display()),
        "-DBOTAN_USE_PKGCONFIG=OFF".to_string(),
    ]);

    if let Some(compat) = &msvc_compat {
        // The CLI/example CMakeLists run find_path(GETOPT_INCLUDE_DIR /
        // DIRENT_INCLUDE_DIR) and find_library(GETOPT_LIBRARY) under
        // if(MSVC) and reference the results unconditionally; left
        // NOTFOUND they fail the whole cmake GENERATE step even though we
        // only ever build the librnp target. Presetting the vars skips
        // the searches. GETOPT_LIBRARY is only referenced by the CLI link
        // rules, which never execute — a placeholder path is enough; if
        // anyone ever does link a CLI against it, the empty archive fails
        // loudly instead of silently dropping getopt symbols.
        let placeholder = src_dir.join("rnp-msvc-getopt-placeholder.lib");
        fs::write(&placeholder, b"").ok();
        cmake_args.extend([
            format!("-DCMAKE_C_FLAGS=/I{compat}"),
            format!("-DDIRENT_INCLUDE_DIR={compat}"),
            format!("-DGETOPT_INCLUDE_DIR={compat}"),
            format!(
                "-DGETOPT_LIBRARY={}",
                placeholder.display().to_string().replace('\\', "/")
            ),
        ]);
    }

    // CRT pairing with the final Rust link (append_msvc_crt_args is the
    // Command-form twin used by the cmake-dep builder): crt-static means
    // every librnp object must be /MT or the exe link dies on missing
    // __imp_* CRT thunks (tamatebako/tebako#637's arm64 leg).
    if target_is_msvc() && crt_static() {
        cmake_args.extend([
            "-DCMAKE_POLICY_DEFAULT_CMP0091=NEW".to_string(),
            "-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded".to_string(),
        ]);
    }

    // Optional upstream features: surface as Cargo features on rnp-src so
    // rnp-rs can flip them without changing the build pipeline.
    if cfg!(feature = "pqc") {
        eprintln!("rnp-src: building librnp with ENABLE_PQC=ON");
        cmake_args.push("-DENABLE_PQC=ON".to_string());
    }
    if cfg!(feature = "crypto-refresh") {
        eprintln!("rnp-src: building librnp with ENABLE_CRYPTO_REFRESH=ON");
        cmake_args.push("-DENABLE_CRYPTO_REFRESH=ON".to_string());
    }

    if cfg!(target_os = "macos") {
        cmake_args.push("-DCMAKE_OSX_DEPLOYMENT_TARGET=11.0".to_string());
    }

    // Windows + MSYS2: Botan's static lib references Winsock (ws2_32) and
    // CryptoAPI (crypt32) symbols. librnp's CMakeLists.txt uses its own
    // FindBotan.cmake (module mode), so our BotanConfig.cmake's
    // INTERFACE_LINK_LIBRARIES is never read. CMAKE_CXX_STANDARD_LIBRARIES
    // is ALWAYS appended to every C++ link command by cmake regardless of
    // generator or find_package mode — the bulletproof way to inject these.
    if cfg!(target_os = "windows") {
        cmake_args.push("-DCMAKE_CXX_STANDARD_LIBRARIES=-lws2_32 -lcrypt32".to_string());
    }

    // A reused CMakeCache keeps find_path results (BOTAN_INCLUDE_DIR is
    // sticky), so any configuration change must discard the tree. The
    // caller's cmake passthroughs alter cmake's view too — they belong in
    // the key even though BuildConfig deliberately does not own them.
    let stamp = src_dir.join("rnp-build.cmake-stamp");
    let fingerprint = cmake_fingerprint(
        &cmake_args,
        &env::var("RNP_CMAKE_TOOLCHAIN").unwrap_or_default(),
        &env::var("RNP_CMAKE_ARGS").unwrap_or_default(),
    );
    if fs::read_to_string(&stamp).ok().as_deref() != Some(&fingerprint) {
        eprintln!("rnp-src: cmake configuration changed; discarding cached librnp build tree");
        let _ = fs::remove_dir_all(&build_dir);
        fs::write(&stamp, &fingerprint).ok();
    }

    let mut cmd = Command::new("cmake");
    cmd.args(&cmake_args);
    append_cross_passthrough(&mut cmd);

    run(&mut cmd, "librnp cmake");

    assert_botan_headers_from_staged_prefix(&build_dir, botan_prefix);
    // Build only the library target. The rnp/rnpkeys CLI executables are of
    // no use to a static-library consumer, and on some cross builds rnp's
    // CMakeLists excludes the CLI from the build while its install rule
    // still references it — a bare `cmake --install` then fails. Skipping
    // the CLI also cuts compile time.
    run(
        Command::new("cmake").args([
            "--build",
            build_dir.to_str().unwrap(),
            "--target",
            "librnp",
            "--parallel",
            &nproc(),
            // Same multi-config contract as the dep builds (Visual Studio
            // would otherwise build Debug); a no-op on single-config.
            "--config",
            "Release",
        ]),
        "librnp build",
    );
    // Install only the development component (librnp.a, libsexpp.a, and all
    // rnp headers install under COMPONENT development upstream; the CLIs are
    // COMPONENT cli). Component-scoped install never touches the CLI rules,
    // so it cannot fail on an unbuilt executable.
    run(
        Command::new("cmake").args([
            "--install",
            build_dir.to_str().unwrap(),
            "--component",
            "development",
            "--config",
            "Release",
        ]),
        "librnp install",
    );
}

// ---------------------------------------------------------------------
// Process utilities.
// ---------------------------------------------------------------------

fn nproc() -> String {
    Command::new("nproc")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "4".to_string())
}

fn download_and_extract(candidates: &[String], dest: &Path) {
    let url = candidates.first().expect("native source identity");
    let body: &[u8] = if url.contains("json-c-0.18.tar.gz") {
        include_bytes!("../vendor/json-c-0.18.tar.gz")
    } else if url.contains("zlib-1.3.2.tar.gz") {
        include_bytes!("../vendor/zlib-1.3.2.tar.gz")
    } else if url.contains("rnp-v0.18.1.tar.gz") {
        include_bytes!("../vendor/rnp-v0.18.1.tar.gz")
    } else if url.contains("bzip2-1.0.8.tar.gz") {
        include_bytes!("../vendor/bzip2-1.0.8.tar.gz")
    } else { panic!("unbundled native source: {url}"); };
    extract_tarball_from(body, dest, url);
}

/// Extract a downloaded gzip tarball into `dest`.
fn extract_tarball_from(body: &[u8], dest: &Path, url: &str) {
    use sha2::{Digest, Sha256};
    let expected = if url.contains("json-c-0.18.tar.gz") {
        "876ab046479166b869afc6896d288183bbc0e5843f141200c677b3e8dfb11724"
    } else if url.contains("zlib-1.3.2.tar.gz") {
        "bb329a0a2cd0274d05519d61c667c062e06990d72e125ee2dfa8de64f0119d16"
    } else if url.contains("rnp-v0.18.1.tar.gz") {
        "423c8e32e1e591462f759adf8441b1c44bca96d9f5daff13b82e81a79f18ecfd"
    } else if url.contains("bzip2-1.0.8.tar.gz") || url.contains("bzip2_1.0.8.orig.tar.gz") {
        "ab5a03176ee106d3f0fa90e381da478ddae405918153cca248e682cd0c4a2269"
    } else { panic!("unpinned native source: {url}"); };
    assert_eq!(format!("{:x}", Sha256::digest(body)), expected, "native source checksum: {url}");
    // Verify gzip magic bytes before attempting to decompress.
    if body.len() < 2 || body[0] != 0x1f || body[1] != 0x8b {
        let snippet = String::from_utf8_lossy(&body[..body.len().min(200)]);
        panic!(
            "rnp-src: {url} did not return a gzip tarball (first {} bytes): {snippet}",
            body.len()
        );
    }

    let decoder = flate2::read::GzDecoder::new(std::io::Cursor::new(body));
    let mut archive = tar::Archive::new(decoder);
    for entry in archive.entries().expect("native archive entries") {
        let mut entry = entry.expect("native archive entry");
        let path = entry.path().expect("native archive path").into_owned();
        // json-c's disabled shell tests contain symlinks; none are build inputs.
        if entry.header().entry_type().is_symlink()
            && path.starts_with("json-c-0.18/tests") { continue; }
        assert!(entry.unpack_in(dest).expect("extract native source"), "native source escaped destination");
    }
}

/// Capture stderr/stdout into the panic message so future failures
/// are diagnosable. Trade-off: callers lose live streaming; for the
/// long Botan compile, that's already handled by botan-src's own
/// stream-to-cargo. Use this for short configure/install steps where
/// error context matters more than progress visibility.
fn run(cmd: &mut Command, label: &str) {
    let output = cmd
        .output()
        .unwrap_or_else(|e| panic!("rnp-src: failed to spawn {label}: {e}"));
    if !output.status.success() {
        panic!(
            "rnp-src: {label} failed with status {}\n\
             --- command ---\n\
             {cmd:?}\n\
             --- stdout ---\n\
             {}\n\
             --- stderr ---\n\
             {}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

/// Recursively copy `src` into `dst`. Errors on IO failure but tolerates
/// destination-already-exists.
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            let _ = fs::remove_file(&to);
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Flavor mapping tests — the flavor → {cache dir, version, source} table
// is pure, so it is testable without compiling any C++.
// ---------------------------------------------------------------------

#[cfg(test)]
mod flavor_tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn release_flavor_names_its_cache_and_version() {
        let f = Flavor::Release0181B1;
        assert_eq!(f.cache_dir(), "rnp-0.18.1-b1");
        assert_eq!(f.librnp_version(), RNP_VERSION);
        assert!(!f.is_head());
        assert!(f.needs_json_c(), "0.18.1 tarball links json-c");
    }

    #[test]
    fn head_flavor_reports_head_and_pins_its_cache() {
        let f = Flavor::Head;
        assert_eq!(f.librnp_version(), "head");
        assert!(f.is_head());
        // The cache dir embeds the pin's short SHA: bumping RNP_HEAD_REF
        // automatically invalidates cached artifacts from the old pin.
        assert_eq!(f.cache_dir(), format!("rnp-head-{}", &RNP_HEAD_REF[..8]));
        assert!(
            !f.needs_json_c(),
            "librnp main vendors nlohmann/json; no json-c"
        );
    }

    fn os_env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + use<'a> {
        let map: HashMap<String, OsString> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), OsString::from(*v)))
            .collect();
        move |key: &str| map.get(key).cloned()
    }

    #[test]
    fn cc_cxx_env_steers_the_librnp_compilers() {
        // The #120 case: MSYS2 clangarm64 has no gcc — the caller's
        // CC/CXX must win over the gcc/g++ platform default.
        let (cc, cxx) = librnp_compilers_with(os_env(&[("CC", "clang"), ("CXX", "clang++")]));
        assert_eq!((cc.as_str(), cxx.as_str()), ("clang", "clang++"));
    }

    #[test]
    fn empty_cc_cxx_falls_back_to_platform_default() {
        let (cc, cxx) = librnp_compilers_with(os_env(&[("CC", "  ")]));
        assert_eq!((cc.as_str(), cxx.as_str()), ("gcc", "g++"));
    }

    #[test]
    fn macos_target_defaults_to_apple_clang() {
        let (cc, cxx) = librnp_compilers_with(os_env(&[("CARGO_CFG_TARGET_OS", "macos")]));
        assert_eq!(
            (cc.as_str(), cxx.as_str()),
            ("/usr/bin/clang", "/usr/bin/clang++")
        );
    }

    #[test]
    fn cmake_fingerprint_is_stable_for_identical_configuration() {
        let args = vec![
            "-S".to_string(),
            "/src/rnp".to_string(),
            "-DBOTAN_ROOT_DIR=/prefix/botan".to_string(),
        ];
        let first = cmake_fingerprint(&args, "", "");
        let second = cmake_fingerprint(&args, "", "");
        assert_eq!(first, second);
    }

    #[test]
    fn cmake_fingerprint_changes_with_any_configuration_input() {
        let args = vec!["-DBOTAN_ROOT_DIR=/a".to_string()];
        let base = cmake_fingerprint(&args, "", "");
        // A changed cmake arg (e.g. a new BOTAN_ROOT_DIR) must invalidate.
        assert_ne!(
            base,
            cmake_fingerprint(&["-DBOTAN_ROOT_DIR=/b".to_string()], "", "")
        );
        // So must the caller's toolchain / passthrough overrides.
        assert_ne!(base, cmake_fingerprint(&args, "/tc.cmake", ""));
        assert_ne!(base, cmake_fingerprint(&args, "", "-DENABLE_FOO=ON"));
    }

    #[test]
    fn cache_dirs_never_collide_across_flavors() {
        // Two flavors sharing a cache dir is the stale-artifact bug class
        // this enum exists to prevent.
        let dirs = [Flavor::Release0181B1.cache_dir(), Flavor::Head.cache_dir()];
        assert_ne!(dirs[0], dirs[1]);
    }

    #[test]
    fn from_features_matches_the_compiled_flavor() {
        // Under a default build this must be the release flavor; under
        // pqc/crypto-refresh it must be HEAD. The cfg! consult and the
        // enum are the same decision made once.
        let expect_head = cfg!(feature = "pqc") || cfg!(feature = "crypto-refresh");
        assert_eq!(Flavor::from_features().is_head(), expect_head);
    }

    #[test]
    fn head_pin_is_a_full_sha() {
        // A short ref would make the fetch/checkout in
        // prepare_librnp_head ambiguous and break the 8-char cache key.
        assert_eq!(RNP_HEAD_REF.len(), 40);
        assert!(RNP_HEAD_REF.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn crt_static_parsing_matches_only_the_exact_feature() {
        assert!(crt_static_in_features("crt-static"));
        assert!(crt_static_in_features("crt-static,panic=abort"));
        assert!(crt_static_in_features("panic=abort,crt-static"));
        assert!(!crt_static_in_features(""));
        assert!(!crt_static_in_features("panic=abort"));
        // Substring tricks must not trip it.
        assert!(!crt_static_in_features("crt-static-x"));
        assert!(!crt_static_in_features("xcrt-static"));
    }
}
