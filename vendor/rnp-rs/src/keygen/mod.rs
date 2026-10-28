//! Key generation.
//!
//! Two builders over `rnp_op_generate_*`:
//!
//! - [`KeyBuilder`] for primary keys.
//! - [`SubkeyBuilder`] for subkeys.
//!
//! Plus the [`generate_key_json`] shortcut, the one-call `generate_key_*`
//! shorthands, and the deprecated `generate_test_key` shim.
//!
//! ## Module layout
//!
//! | Sub-module | Concern                                               |
//! |------------|-------------------------------------------------------|
//! | `primary`  | `KeyBuilder` + `apply_setters`                        |
//! | `subkey`   | `SubkeyBuilder` + `apply_subkey_setters`              |
//! | `shorthand`| one-call `generate_key_*` free functions              |
//!
//! The algorithm/curve/hash/cipher/compression/key-usage enums live in
//! [`crate::algorithm`] and are re-exported here for backward compatibility.

use crate::context::Context;
use crate::error::{self, Result, check};
use crate::ffi;
use crate::key::Key;
use std::ffi::CString;

// Backward compatibility: re-export the algorithm enums from this module.
pub use crate::algorithm::{Algorithm, Cipher, Compression, Curve, Hash, KeyUsage};
#[cfg(feature = "pqc")]
pub use crate::algorithm::{PqcAlgorithm, librnp_supports_pqc};

pub(crate) mod primary;
pub(crate) mod shorthand;
pub(crate) mod subkey;

pub use primary::KeyBuilder;
pub use shorthand::{
    generate_key_25519, generate_key_dsa_eg, generate_key_ec, generate_key_ex, generate_key_rsa,
    generate_key_sm2,
};
pub use subkey::SubkeyBuilder;

// ---------------------------------------------------------------------------
// Shared protection config (used by both primary and subkey builders).
// ---------------------------------------------------------------------------

/// Protection configuration shared between `Key::protect` (phase 03) and
/// `KeyBuilder::protection` (phase 12). One canonical type — adding a new
/// field here updates both call sites. OCP via composition.
#[derive(Default, Clone)]
pub(crate) struct ProtectConfig {
    pub password: Option<String>,
    pub cipher: Option<Cipher>,
    pub mode: Option<String>,
    pub hash: Option<Hash>,
    pub iterations: Option<usize>,
}

impl ProtectConfig {
    /// Build from a user-supplied `ProtectOptions`.
    pub(crate) fn from_options(opts: &crate::key::ProtectOptions) -> Self {
        ProtectConfig {
            password: opts.password.clone(),
            cipher: opts.cipher,
            mode: opts.mode.clone(),
            hash: opts.hash,
            iterations: opts.iterations,
        }
    }
}

/// Apply a `ProtectConfig` to a `rnp_op_generate_t`. Shared by primary-key
/// and subkey generation.
pub(crate) unsafe fn apply_protection(
    op: ffi::rnp_op_generate_t,
    cfg: &ProtectConfig,
) -> Result<()> {
    unsafe {
        if let Some(pw) = &cfg.password {
            let c = CString::new(pw.as_str()).map_err(|_| error::Error::NulByte)?;
            check(ffi::rnp_op_generate_set_protection_password(op, c.as_ptr()))?;
        }
        if let Some(c2) = cfg.cipher {
            let cs = CString::new(c2.as_str()).unwrap();
            check(ffi::rnp_op_generate_set_protection_cipher(op, cs.as_ptr()))?;
        }
        if let Some(h) = cfg.hash {
            let cs = CString::new(h.as_str()).unwrap();
            check(ffi::rnp_op_generate_set_protection_hash(op, cs.as_ptr()))?;
        }
        if let Some(m) = &cfg.mode {
            let cs = CString::new(m.as_str()).unwrap();
            check(ffi::rnp_op_generate_set_protection_mode(op, cs.as_ptr()))?;
        }
        if let Some(it) = cfg.iterations {
            check(ffi::rnp_op_generate_set_protection_iterations(
                op,
                it.try_into().unwrap_or(u32::MAX),
            ))?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Shared generate-op option replay
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub(crate) struct GenerateCommon<'a> {
    pub(crate) bits: Option<u32>,
    pub(crate) hash: Option<Hash>,
    pub(crate) dsa_qbits: Option<u32>,
    pub(crate) curve: Option<Curve>,
    pub(crate) expiration: Option<u32>,
    pub(crate) usages: &'a [KeyUsage],
    pub(crate) protection: Option<&'a ProtectConfig>,
    pub(crate) request_password: bool,
}

pub(crate) unsafe fn apply_generate_common(
    op: ffi::rnp_op_generate_t,
    common: GenerateCommon<'_>,
) -> Result<()> {
    unsafe {
        if let Some(n) = common.bits {
            check(ffi::rnp_op_generate_set_bits(op, n))?;
        }
        if let Some(h) = common.hash {
            let c = CString::new(h.as_str()).unwrap();
            check(ffi::rnp_op_generate_set_hash(op, c.as_ptr()))?;
        }
        if let Some(q) = common.dsa_qbits {
            check(ffi::rnp_op_generate_set_dsa_qbits(op, q))?;
        }
        if let Some(c) = common.curve {
            let cs = CString::new(c.as_str()).unwrap();
            check(ffi::rnp_op_generate_set_curve(op, cs.as_ptr()))?;
        }
        if let Some(exp) = common.expiration {
            check(ffi::rnp_op_generate_set_expiration(op, exp))?;
        }
        for u in common.usages {
            let c = CString::new(u.as_str()).unwrap();
            check(ffi::rnp_op_generate_add_usage(op, c.as_ptr()))?;
        }
        if let Some(cfg) = common.protection {
            apply_protection(op, cfg)?;
        }
        if common.request_password {
            check(ffi::rnp_op_generate_set_request_password(op, true))?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// JSON shortcut
// ---------------------------------------------------------------------------

/// Generate one or more keys from a JSON description. Returns the result
/// JSON (containing the generated key fingerprints).
///
/// See `rnp_generate_key_json` in librnp for the input JSON schema.
pub fn generate_key_json(ctx: &Context, json: &str) -> Result<String> {
    let c = CString::new(json).map_err(|_| error::Error::NulByte)?;
    crate::ffi_safe::call_for_string(|out| unsafe {
        ffi::rnp_generate_key_json(ctx.ffi, c.as_ptr(), out)
    })
}

// ---------------------------------------------------------------------------
// Deprecated test helper (kept as a pub(crate) shim over KeyBuilder for any
// in-crate callers; not exported beyond the crate boundary).
// ---------------------------------------------------------------------------

/// Generate an unprotected RSA-2048 keypair. New code should use
/// [`KeyBuilder`](crate::KeyBuilder) directly.
#[deprecated(note = "use KeyBuilder instead")]
#[allow(dead_code)]
pub(crate) fn generate_test_key<'a>(ctx: &'a Context, userid: &str) -> Result<Key<'a>> {
    KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid(userid)
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Sign)
        .add_usage(KeyUsage::Certify)
        .build(ctx)
}
