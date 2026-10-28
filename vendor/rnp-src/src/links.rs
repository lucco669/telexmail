//! Shared types and constants for rnp-src's build pipeline.
//!
//! Path-includeable from the crate root (see `lib.rs`) so the pure logic is
//! unit-testable without invoking the C/C++ toolchain.

use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------
// Dependency registry — single source of truth for what rnp-src builds.
// Adding a dep here automatically flows through to the cmake prefix path
// and the per-dep lib dirs returned by `build()`.
// ---------------------------------------------------------------------

/// All dependency names rnp-src installs, in cmake-prefix order.
pub const DEPS: &[&str] = &["botan", "jsonc", "zlib", "bzip2"];

// ---------------------------------------------------------------------
// Deps — collection of per-dep install prefixes.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct Deps {
    items: Vec<Dep>,
}

#[derive(Debug, Clone)]
pub struct Dep {
    pub name: &'static str,
    pub prefix: PathBuf,
}

impl Deps {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn push(&mut self, name: &'static str, prefix: PathBuf) {
        assert!(
            DEPS.contains(&name),
            "rnp-src: Deps::push({name}) — name not in DEPS; update src/links.rs first"
        );
        self.items.push(Dep { name, prefix });
    }

    pub fn by_name(&self, name: &str) -> Option<&Dep> {
        self.items.iter().find(|d| d.name == name)
    }

    /// Semicolon-joined list of dep prefixes, as cmake's CMAKE_PREFIX_PATH expects.
    pub fn cmake_prefix_path(&self) -> String {
        self.items
            .iter()
            .map(|d| d.prefix.display().to_string())
            .collect::<Vec<_>>()
            .join(";")
    }

    /// Per-dep lib dirs in registration order — the caller emits these as
    /// linker search paths.
    pub fn lib_dirs(&self) -> impl Iterator<Item = PathBuf> + '_ {
        self.items.iter().map(|d| d.prefix.join("lib"))
    }

    pub fn iter(&self) -> impl Iterator<Item = &Dep> {
        self.items.iter()
    }
}

// ---------------------------------------------------------------------
// CmakeDep — config-driven description of a cmake-built tarball dep.
// ---------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CmakeDep {
    pub name: &'static str,
    pub version: &'static str,
    /// `format!`-style template; `{version}` is substituted at runtime.
    pub url_template: &'static str,
    /// Extra `-D` flags beyond the standard CMAKE_INSTALL_PREFIX /
    /// BUILD_SHARED_LIBS / CMAKE_BUILD_TYPE.
    pub extra_cmake_args: &'static [&'static str],
    /// cmake policy version floor; some old libs need <3.5.
    pub cmake_policy_minimum: Option<&'static str>,
    /// Library filename aliases to ensure exist under `<prefix>/lib/`
    /// after `cmake --install`. Each entry is `(actual, expected)`: if
    /// `actual` exists and `expected` doesn't, copy `actual` → `expected`.
    ///
    /// Handles platform-specific output-name quirks where a dep's cmake
    /// target name doesn't match what rustc's `-l` flag expects. Example:
    /// zlib on MinGW produces `libzlibstatic.a`, but rnp-rs links with
    /// `static=z` which searches for `libz.a`.
    pub installed_lib_aliases: &'static [(&'static str, &'static str)],
}

impl CmakeDep {
    pub fn source_dir(&self, src_root: &Path) -> PathBuf {
        src_root.join(format!("{}-{}", self.name, self.version))
    }

    pub fn build_dir(&self, src_root: &Path) -> PathBuf {
        src_root.join(format!("{}-build", self.name))
    }

    pub fn url(&self) -> String {
        self.url_template.replace("{version}", self.version)
    }
}

/// Bundled cmake deps rnp-src knows how to build.
pub const JSON_C: CmakeDep = CmakeDep {
    name: "json-c",
    version: "0.18",
    url_template: "https://s3.amazonaws.com/json-c_releases/releases/json-c-{version}.tar.gz",
    // JSONC_BUILD_APPS: json-c's sample apps (apps/json_parse, ...) are not
    // gated by BUILD_TESTING. On cross builds, linking executables drags in
    // the host's executable-linking assumptions (-rdynamic etc.), which is a
    // separate failure class from the library itself.
    extra_cmake_args: &["-DBUILD_TESTING=OFF", "-DJSONC_BUILD_APPS=OFF"],
    cmake_policy_minimum: Some("3.5"),
    installed_lib_aliases: &[],
};

pub const ZLIB: CmakeDep = CmakeDep {
    name: "zlib",
    version: "1.3.2",
    url_template: "https://github.com/madler/zlib/releases/download/v{version}/zlib-{version}.tar.gz",
    extra_cmake_args: &["-DZLIB_BUILD_SHARED=OFF", "-DZLIB_BUILD_STATIC=ON", "-DZLIB_BUILD_TESTING=OFF"],
    cmake_policy_minimum: None,
    // On MinGW (Windows + MSYS2 UCRT64), zlib's cmake produces libzlibstatic.a
    // (target name `zlibstatic`) instead of libz.a. rnp-rs links with
    // `static=z` which searches for libz.a — alias both possible MinGW
    // outputs to the canonical Unix name. On Unix this is a no-op: the
    // sources don't exist there (libz.a is produced directly).
    // On MSVC the static archive installs as zlibstatic.lib (zlib.lib, when
    // present, is the DLL import library — linking it statically would
    // silently create a runtime dependency on a zlib1.dll nobody ships).
    // rustc resolves `static=z` to z.lib.
    installed_lib_aliases: &[
        ("zs.lib", "z.lib"),
        ("libzs.a", "libz.a"),
        ("libzlib.a", "libz.a"),
        ("libzlibstatic.a", "libz.a"),
        ("zlibstatic.lib", "z.lib"),
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deps_cmake_prefix_path_joins_with_semicolon() {
        let mut deps = Deps::new();
        deps.push("botan", PathBuf::from("/a/botan"));
        deps.push("jsonc", PathBuf::from("/b/json-c"));
        assert_eq!(deps.cmake_prefix_path(), "/a/botan;/b/json-c");
    }

    #[test]
    fn empty_deps_yields_empty_prefix_path() {
        let deps = Deps::new();
        assert_eq!(deps.cmake_prefix_path(), "");
    }

    #[test]
    fn deps_by_name_finds_pushed_entry() {
        let mut deps = Deps::new();
        deps.push("botan", PathBuf::from("/x/botan"));
        assert_eq!(
            deps.by_name("botan").map(|d| d.prefix.clone()),
            Some(PathBuf::from("/x/botan"))
        );
        assert!(deps.by_name("jsonc").is_none());
    }

    #[test]
    fn deps_push_rejects_unknown_name() {
        let mut deps = Deps::new();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            deps.push("nonexistent", PathBuf::from("/x"));
        }));
        assert!(result.is_err(), "push should panic for unknown dep name");
    }

    #[test]
    fn cmake_dep_source_and_build_dirs() {
        let dep = CmakeDep {
            name: "json-c",
            version: "0.17",
            url_template: "",
            extra_cmake_args: &[],
            cmake_policy_minimum: None,
            installed_lib_aliases: &[],
        };
        let root = Path::new("/tmp/src");
        assert_eq!(dep.source_dir(root), PathBuf::from("/tmp/src/json-c-0.17"));
        assert_eq!(dep.build_dir(root), PathBuf::from("/tmp/src/json-c-build"));
    }

    #[test]
    fn zlib_aliases_cover_mingw_and_msvc_static_names() {
        // rnp-rs links `static=z` on every platform: rustc resolves that to
        // libz.a on Unix/MinGW and z.lib on MSVC, so the alias table must
        // produce both from whatever zlib's cmake installs per toolchain.
        let aliases = ZLIB.installed_lib_aliases;
        assert!(aliases.contains(&("libzlibstatic.a", "libz.a")));
        assert!(aliases.contains(&("zlibstatic.lib", "z.lib")));
    }

    #[test]
    fn cmake_dep_url_substitutes_version() {
        assert_eq!(
            JSON_C.url(),
            "https://s3.amazonaws.com/json-c_releases/releases/json-c-0.17.tar.gz"
        );
        assert_eq!(
            ZLIB.url(),
            "https://github.com/madler/zlib/releases/download/v1.3.1/zlib-1.3.1.tar.gz"
        );
    }
}
