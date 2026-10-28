//! Build configuration for the botan-src dependency — the one value that
//! owns every knob rnp-src writes to the `BOTAN_CONFIGURE_*` process
//! environment.
//!
//! ## Why this exists
//!
//! botan-src's only configuration surface is process env vars, so an
//! orchestrator must mutate global state to configure it. Done naively, a
//! "default" applied last beats caller intent — and every variant of that
//! bug shipped at least once:
//!
//! - `BOTAN_CONFIGURE_CC/BIN` were stomped on Windows (broke MSVC cross
//!   builds, patched in #103);
//! - `BOTAN_CONFIGURE_ENABLE_MODULES` was overwritten wholesale by the
//!   `pqc` feature set, silently discarding a caller's own modules;
//! - `BOTAN_CONFIGURE_DISABLE_MODULES` was replaced instead of appended.
//!
//! [`BuildConfig`] centralizes resolution behind **one** precedence —
//! *defaults < env < feature-merge* — where feature-merge is a **union**
//! (the pqc modules are added to the caller's set, never replace it).
//! Resolution is a pure function over (platform, features, env), so the
//! whole policy is unit-testable without a toolchain, and applying it is
//! a single write point instead of scattered `set_var`s.
//!
//! Read-only passthroughs (`RNP_CMAKE_TOOLCHAIN`, `RNP_CMAKE_ARGS`,
//! `BOTAN_SRC_TARBALL`/`BOTAN_SRC_DIR`) are caller-owned inputs forwarded
//! verbatim; they are deliberately *not* modeled here.

use std::collections::BTreeSet;
use std::ffi::OsString;

/// Modules the `pqc` Cargo feature adds to the Botan build. A *union* with
/// anything the caller requested — never a replacement.
pub const PQC_MODULES: &[&str] = &["ml_kem", "ml_dsa", "slh_dsa_sha2", "slh_dsa_shake"];

/// Windows-only module disables appended to the caller's set: the Windows
/// cert-store module pulls crypt32.lib symbols our static link does not
/// provide during the librnp build step.
pub const WINDOWS_DISABLED_MODULES: &[&str] = &["certstor_system_windows"];

/// The botan-src build configuration: everything rnp-src writes to the
/// `BOTAN_CONFIGURE_*` environment. Constructed via [`BuildConfig::detect`]
/// (platform + cargo features + caller env) or built explicitly for tests
/// and programmatic callers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildConfig {
    /// `BOTAN_CONFIGURE_CC` — C compiler for Botan's build.
    pub botan_cc: Option<String>,
    /// `BOTAN_CONFIGURE_CC_BIN` — C++ compiler for Botan's build.
    pub botan_cc_bin: Option<String>,
    /// `BOTAN_CONFIGURE_ENABLE_MODULES` — extra Botan modules to enable.
    pub botan_enable_modules: BTreeSet<String>,
    /// `BOTAN_CONFIGURE_DISABLE_MODULES` — Botan modules to leave out.
    pub botan_disable_modules: BTreeSet<String>,
}

impl BuildConfig {
    /// Resolve the configuration from the compiled target, this crate's
    /// Cargo features, and the caller's environment.
    pub fn detect() -> Self {
        Self::detect_with_env(|key| std::env::var_os(key))
    }

    /// Pure resolution core: same precedence as [`BuildConfig::detect`],
    /// with the environment injected so the policy is table-testable.
    ///
    /// Precedence, lowest to highest:
    /// 1. **defaults** — platform baseline (Windows MSYS2 `gcc`/`g++`,
    ///    cert-store module off);
    /// 2. **env** — caller-provided `BOTAN_CONFIGURE_*` values override
    ///    scalar defaults and seed the module sets;
    /// 3. **feature-merge** — the `pqc` feature's modules are *unioned*
    ///    into `botan_enable_modules`; Windows disables are *appended*.
    pub fn detect_with_env(get_env: impl Fn(&str) -> Option<OsString>) -> Self {
        let mut config = BuildConfig::default();

        // 1. Platform defaults (lowest precedence).
        if cfg!(target_os = "windows") {
            config.botan_cc = Some("gcc".to_string());
            config.botan_cc_bin = Some("g++".to_string());
        }

        // 2. Caller environment: scalars override, sets are seeded.
        if let Some(cc) = get_env("BOTAN_CONFIGURE_CC") {
            config.botan_cc = Some(cc.to_string_lossy().into_owned());
        }
        if let Some(cc_bin) = get_env("BOTAN_CONFIGURE_CC_BIN") {
            config.botan_cc_bin = Some(cc_bin.to_string_lossy().into_owned());
        }
        for module in csv_env(&get_env, "BOTAN_CONFIGURE_ENABLE_MODULES") {
            config.botan_enable_modules.insert(module);
        }
        for module in csv_env(&get_env, "BOTAN_CONFIGURE_DISABLE_MODULES") {
            config.botan_disable_modules.insert(module);
        }

        // 3. Feature merge: unions, never replacements.
        if cfg!(feature = "pqc") {
            for module in PQC_MODULES {
                config.botan_enable_modules.insert((*module).to_string());
            }
        }
        if cfg!(target_os = "windows") {
            for module in WINDOWS_DISABLED_MODULES {
                config.botan_disable_modules.insert((*module).to_string());
            }
        }

        config
    }

    /// Apply the resolved configuration to the process environment — the
    /// single write point before `botan_src::build()` runs.
    pub fn apply(&self) {
        // SAFETY: single-threaded build-script context, before any botan
        // build reads these variables. Edition-2024 requires the unsafe
        // block for env mutation.
        unsafe {
            if let Some(cc) = &self.botan_cc {
                std::env::set_var("BOTAN_CONFIGURE_CC", cc);
            }
            if let Some(cc_bin) = &self.botan_cc_bin {
                std::env::set_var("BOTAN_CONFIGURE_CC_BIN", cc_bin);
            }
            if !self.botan_enable_modules.is_empty() {
                std::env::set_var(
                    "BOTAN_CONFIGURE_ENABLE_MODULES",
                    self.botan_enable_modules
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(","),
                );
            }
            if !self.botan_disable_modules.is_empty() {
                std::env::set_var(
                    "BOTAN_CONFIGURE_DISABLE_MODULES",
                    self.botan_disable_modules
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(","),
                );
            }
        }
    }
}

/// Split a comma-separated env value into trimmed, non-empty entries.
fn csv_env(get_env: &impl Fn(&str) -> Option<OsString>, key: &str) -> Vec<String> {
    get_env(key)
        .map(|value| {
            value
                .to_string_lossy()
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Test env from &str pairs.
    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + use<'a> {
        let map: HashMap<String, OsString> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), OsString::from(*v)))
            .collect();
        move |key: &str| map.get(key).cloned()
    }

    // The pqc arms below are cfg-dependent: on a default-feature build the
    // union cases degrade to no-ops. Assert both ways so the table stays
    // honest under either feature set.
    fn pqc_on() -> bool {
        cfg!(feature = "pqc")
    }

    #[test]
    fn caller_cc_is_respected_everywhere() {
        // The #103 regression: a caller-named compiler must never be
        // stomped by platform defaults, on any platform.
        let cfg = BuildConfig::detect_with_env(env(&[("BOTAN_CONFIGURE_CC", "cl")]));
        assert_eq!(cfg.botan_cc.as_deref(), Some("cl"));
    }

    #[test]
    fn caller_cc_bin_is_respected() {
        let cfg = BuildConfig::detect_with_env(env(&[("BOTAN_CONFIGURE_CC_BIN", "clang++-19")]));
        assert_eq!(cfg.botan_cc_bin.as_deref(), Some("clang++-19"));
    }

    #[test]
    fn pqc_feature_unions_with_caller_modules() {
        let cfg =
            BuildConfig::detect_with_env(env(&[("BOTAN_CONFIGURE_ENABLE_MODULES", "zlib,foo")]));
        assert!(cfg.botan_enable_modules.contains("zlib"));
        assert!(cfg.botan_enable_modules.contains("foo"));
        if pqc_on() {
            for m in PQC_MODULES {
                assert!(
                    cfg.botan_enable_modules.contains(*m),
                    "pqc module {m} must be unioned in, not replace the caller's set"
                );
            }
        }
    }

    #[test]
    fn disable_modules_union_with_caller_set() {
        let cfg = BuildConfig::detect_with_env(env(&[(
            "BOTAN_CONFIGURE_DISABLE_MODULES",
            "something_else",
        )]));
        assert!(cfg.botan_disable_modules.contains("something_else"));
        if cfg!(target_os = "windows") {
            assert!(
                cfg.botan_disable_modules
                    .contains("certstor_system_windows"),
                "windows disable must append, not replace"
            );
        }
    }

    #[test]
    fn env_csv_is_trimmed_and_empties_dropped() {
        // Exact parsing is a property of csv_env itself.
        let parsed = csv_env(
            &env(&[("BOTAN_CONFIGURE_ENABLE_MODULES", " a , ,b,, ")]),
            "BOTAN_CONFIGURE_ENABLE_MODULES",
        );
        assert_eq!(parsed, vec!["a".to_string(), "b".to_string()]);
        // And detect_with_env seeds the set with exactly those entries
        // (containment, so feature unions don't invalidate the test).
        let cfg =
            BuildConfig::detect_with_env(env(&[("BOTAN_CONFIGURE_ENABLE_MODULES", " a , ,b,, ")]));
        assert!(cfg.botan_enable_modules.contains("a"));
        assert!(cfg.botan_enable_modules.contains("b"));
        assert!(!cfg.botan_enable_modules.contains(""));
    }

    #[test]
    fn windows_default_cc_only_when_caller_silent() {
        let cfg = BuildConfig::detect_with_env(env(&[]));
        if cfg!(target_os = "windows") {
            assert_eq!(cfg.botan_cc.as_deref(), Some("gcc"));
            assert_eq!(cfg.botan_cc_bin.as_deref(), Some("g++"));
        } else {
            assert_eq!(cfg.botan_cc, None);
        }
    }

    #[test]
    fn empty_config_applies_nothing() {
        // apply() on a default config must not create env entries.
        BuildConfig::default().apply();
        assert!(std::env::var_os("BOTAN_CONFIGURE_ENABLE_MODULES").is_none());
        assert!(std::env::var_os("BOTAN_CONFIGURE_DISABLE_MODULES").is_none());
    }

    #[test]
    fn detect_without_env_matches_platform_default() {
        let cfg = BuildConfig::detect_with_env(env(&[]));
        if cfg!(target_os = "windows") {
            assert!(
                cfg.botan_disable_modules
                    .contains("certstor_system_windows")
            );
        }
    }
}
