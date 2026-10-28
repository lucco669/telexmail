//! Security profile, feature queries, and related utilities.
//!
//! Wraps `rnp_add_security_rule`, `rnp_get_security_rule`,
//! `rnp_remove_security_rule`, `rnp_supported_features`,
//! `rnp_supports_feature`, `rnp_calculate_iterations`, `rnp_request_password`,
//! `rnp_ffi_set_key_provider`, and `rnp_set_timestamp`.

use crate::context::Context;
use crate::error::{self, Result, check};
use crate::ffi;
use crate::ffi_safe::{call_for_bool, call_for_optional_string, call_for_string, call_for_usize};
use std::ffi::CString;
use std::ptr;

/// Prohibition level for an algorithm or operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecurityLevel {
    /// The action may not be performed at all.
    Prohibited,
    /// The action is allowed but flagged as insecure.
    Insecure,
    /// The action is allowed by default.
    Default,
}

impl SecurityLevel {
    pub fn as_u32(self) -> u32 {
        match self {
            SecurityLevel::Prohibited => ffi::RNP_SECURITY_PROHIBITED as u32,
            SecurityLevel::Insecure => ffi::RNP_SECURITY_INSECURE as u32,
            SecurityLevel::Default => ffi::RNP_SECURITY_DEFAULT as u32,
        }
    }
}

/// Flags refining a security rule.
#[derive(Clone, Copy, Debug, Default)]
pub struct SecurityFlags(pub u32);

impl SecurityFlags {
    /// Override the rule's level for this action.
    pub const OVERRIDE: Self = Self(ffi::RNP_SECURITY_OVERRIDE as u32);
    /// Apply when verifying keys.
    pub const VERIFY_KEY: Self = Self(ffi::RNP_SECURITY_VERIFY_KEY as u32);
    /// Apply when verifying data (messages, signatures).
    pub const VERIFY_DATA: Self = Self(ffi::RNP_SECURITY_VERIFY_DATA as u32);
    /// Remove all matching rules.
    pub const REMOVE_ALL: Self = Self(ffi::RNP_SECURITY_REMOVE_ALL as u32);

    pub fn bits(self) -> u32 {
        self.0
    }
}

impl std::ops::BitOr for SecurityFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Feature category — corresponds to the `RNP_FEATURE_*` string constants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeatureType {
    SymmetricAlgorithm,
    AeadAlgorithm,
    ProtectionMode,
    PublicKeyAlgorithm,
    HashAlgorithm,
    CompressionAlgorithm,
    Curve,
}

impl FeatureType {
    pub fn as_str(self) -> &'static str {
        // The RNP_FEATURE_* constants are `[u8; N]` byte arrays that include
        // a trailing NUL (they're C string literals). Strip it.
        macro_rules! b2s {
            ($b:expr) => {{
                let bytes: &[u8] = $b;
                let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
                std::str::from_utf8(&bytes[..end]).expect("RNP_FEATURE_* is ASCII")
            }};
        }
        match self {
            FeatureType::SymmetricAlgorithm => b2s!(ffi::RNP_FEATURE_SYMM_ALG),
            FeatureType::AeadAlgorithm => b2s!(ffi::RNP_FEATURE_AEAD_ALG),
            FeatureType::ProtectionMode => b2s!(ffi::RNP_FEATURE_PROT_MODE),
            FeatureType::PublicKeyAlgorithm => b2s!(ffi::RNP_FEATURE_PK_ALG),
            FeatureType::HashAlgorithm => b2s!(ffi::RNP_FEATURE_HASH_ALG),
            FeatureType::CompressionAlgorithm => b2s!(ffi::RNP_FEATURE_COMP_ALG),
            FeatureType::Curve => b2s!(ffi::RNP_FEATURE_CURVE),
        }
    }
}

/// A row in librnp's security-rule table.
#[derive(Clone, Debug)]
pub struct SecurityRule {
    pub level: SecurityLevel,
    pub flags: SecurityFlags,
    pub typ: FeatureType,
    pub name: String,
    pub from: u64,
}

impl Context {
    /// Add a rule to the security profile.
    pub fn add_security_rule(&self, rule: &SecurityRule) -> Result<()> {
        let type_c = CString::new(rule.typ.as_str()).unwrap();
        let name_c = CString::new(rule.name.as_str()).map_err(|_| error::Error::NulByte)?;
        unsafe {
            check(ffi::rnp_add_security_rule(
                self.ffi,
                type_c.as_ptr(),
                name_c.as_ptr(),
                rule.flags.bits(),
                rule.from,
                rule.level.as_u32(),
            ))?;
        }
        Ok(())
    }

    /// Look up the rule in effect at time `time` for `(typ, name)` with
    /// `flags` scope.
    pub fn get_security_rule(
        &self,
        typ: FeatureType,
        name: &str,
        flags: SecurityFlags,
        time: u64,
    ) -> Result<SecurityRule> {
        let type_c = CString::new(typ.as_str()).unwrap();
        let name_c = CString::new(name).map_err(|_| error::Error::NulByte)?;
        let mut level: u32 = 0;
        let mut from: u64 = 0;
        let mut flags_bits = flags.bits();
        unsafe {
            check(ffi::rnp_get_security_rule(
                self.ffi,
                type_c.as_ptr(),
                name_c.as_ptr(),
                time,
                &mut flags_bits,
                &mut from,
                &mut level,
            ))?;
        }
        Ok(SecurityRule {
            level: match level {
                x if x == ffi::RNP_SECURITY_PROHIBITED as u32 => SecurityLevel::Prohibited,
                x if x == ffi::RNP_SECURITY_INSECURE as u32 => SecurityLevel::Insecure,
                _ => SecurityLevel::Default,
            },
            flags: SecurityFlags(flags_bits),
            typ,
            name: name.to_string(),
            from,
        })
    }

    /// Remove rules matching `(typ, name)` with `flags` scope. Pass
    /// [`SecurityFlags::REMOVE_ALL`] to wipe all matching rules.
    pub fn remove_security_rule(
        &self,
        typ: FeatureType,
        name: &str,
        flags: SecurityFlags,
    ) -> Result<usize> {
        let type_c = CString::new(typ.as_str()).unwrap();
        let name_c = CString::new(name).map_err(|_| error::Error::NulByte)?;
        call_for_usize(|out| unsafe {
            ffi::rnp_remove_security_rule(
                self.ffi,
                type_c.as_ptr(),
                name_c.as_ptr(),
                0,
                flags.bits(),
                0,
                out,
            )
        })
    }

    /// Override the wall-clock used by librnp (mostly for tests).
    pub fn set_timestamp(&self, time: u64) -> Result<()> {
        unsafe { check(ffi::rnp_set_timestamp(self.ffi, time)) }
    }
}

// Top-level (not bound to a Context) feature queries and helpers.

/// Check whether the linked librnp supports `name` under category `typ`.
pub fn supports_feature(typ: FeatureType, name: &str) -> Result<bool> {
    let type_c = CString::new(typ.as_str()).unwrap();
    let name_c = CString::new(name).map_err(|_| error::Error::NulByte)?;
    call_for_bool(|out| unsafe { ffi::rnp_supports_feature(type_c.as_ptr(), name_c.as_ptr(), out) })
}

/// List all feature names supported under `typ`. Returned as a JSON array
/// string (librnp's format).
pub fn supported_features(typ: FeatureType) -> Result<String> {
    let type_c = CString::new(typ.as_str()).unwrap();
    call_for_string(|out| unsafe { ffi::rnp_supported_features(type_c.as_ptr(), out) })
}

/// Recommended S2K iteration count for `hash` and target `memory` bytes.
pub fn calculate_iterations(hash: crate::keygen::Hash, memory: usize) -> Result<usize> {
    let hash_c = CString::new(hash.as_str()).unwrap();
    call_for_usize(|out| unsafe { ffi::rnp_calculate_iterations(hash_c.as_ptr(), memory, out) })
}

/// Manually request a password from the configured password provider on
/// `ctx`. `key` is the key the password is for (or `None`). `context_str`
/// is the why (e.g. `"sign"`, `"decrypt"`).
///
/// The returned [`SecretString`](crate::SecretString) zeroises its bytes on drop — callers who
/// need a plain `String` can use `SecretString::into_string`, but should
/// prefer keeping the secret scoped.
pub fn request_password(
    ctx: &Context,
    key: Option<&crate::Key<'_>>,
    context_str: &str,
) -> Result<Option<crate::SecretString>> {
    let key_handle = key.map(|k| k.handle).unwrap_or(ptr::null_mut());
    let ctx_c = CString::new(context_str).map_err(|_| error::Error::NulByte)?;
    let s = call_for_optional_string(|out| unsafe {
        ffi::rnp_request_password(ctx.ffi, key_handle, ctx_c.as_ptr(), out)
    })?;
    Ok(s.map(crate::SecretString::new))
}
