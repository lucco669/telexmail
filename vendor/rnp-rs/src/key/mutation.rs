//! Mutation methods on [`Key`] plus the option structs they consume.
//!
//! Methods here change FFI state (locked/unlocked, protected/unprotected,
//! revoked, expiration changed, removed, etc.). Each method takes shared
//! ownership of the underlying key state — `Key` borrows the `Context` and
//! mutation is performed in place through the FFI handle.

use crate::error::{self, Result, check};
use crate::ffi;
use crate::keygen::{Cipher, Hash};
use crate::ops::Output;
use std::ffi::CString;
use std::os::raw::c_char;
use std::ptr;

use super::{ExportFlags, Key, RemoveFlags, RemoveSignaturesFlags};

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

/// Reason for revoking a key or UID.
#[derive(Clone, Debug, Default)]
pub struct RevocationReason {
    pub code: RevocationCode,
    pub reason: Option<String>,
}

impl RevocationReason {
    pub fn new(code: RevocationCode) -> Self {
        RevocationReason { code, reason: None }
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    pub(crate) fn code_str(&self) -> &'static str {
        self.code.as_str()
    }
}

/// Revocation reason code. Values 0..=3 per RFC 4880 §5.2.3.23.
///
/// Wire names are the librnp C-API identifiers (not the numeric values),
/// because `rnp_key_revoke` / `rnp_key_export_revocation` use
/// `str_to_revocation_type` to map by name.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RevocationCode {
    /// `"no"` — no specific reason (default).
    #[default]
    NoReason,
    /// `"superseded"` — key superseded.
    Superseded,
    /// `"compromised"` — key material compromised.
    Compromised,
    /// `"retired"` — key retired.
    Retired,
}

impl RevocationCode {
    pub fn as_str(self) -> &'static str {
        match self {
            RevocationCode::NoReason => "no",
            RevocationCode::Superseded => "superseded",
            RevocationCode::Compromised => "compromised",
            RevocationCode::Retired => "retired",
        }
    }
}

/// Options for [`Key::protect`]. Defaults match librnp's internal defaults
/// (AES256 / SHA256 / iterated-and-salted).
#[derive(Default)]
pub struct ProtectOptions {
    pub(crate) password: Option<String>,
    pub(crate) cipher: Option<Cipher>,
    pub(crate) mode: Option<String>,
    pub(crate) hash: Option<Hash>,
    pub(crate) iterations: Option<usize>,
}

impl ProtectOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn password(mut self, p: impl Into<String>) -> Self {
        self.password = Some(p.into());
        self
    }

    pub fn cipher(mut self, c: Cipher) -> Self {
        self.cipher = Some(c);
        self
    }

    pub fn mode(mut self, m: impl Into<String>) -> Self {
        self.mode = Some(m.into());
        self
    }

    pub fn hash(mut self, h: Hash) -> Self {
        self.hash = Some(h);
        self
    }

    pub fn iterations(mut self, n: usize) -> Self {
        self.iterations = Some(n);
        self
    }
}

/// Options for [`Key::add_uid`].
#[derive(Default)]
pub struct AddUidOptions {
    pub(crate) hash: Option<Hash>,
    pub(crate) key_flags: Option<u32>,
    pub(crate) key_expiration: Option<u32>,
    pub(crate) primary: bool,
}

impl AddUidOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn hash(mut self, h: Hash) -> Self {
        self.hash = Some(h);
        self
    }

    pub fn key_flags(mut self, flags: u32) -> Self {
        self.key_flags = Some(flags);
        self
    }

    pub fn key_expiration(mut self, seconds: u32) -> Self {
        self.key_expiration = Some(seconds);
        self
    }

    pub fn primary(mut self, p: bool) -> Self {
        self.primary = p;
        self
    }
}

// ---------------------------------------------------------------------------
// Mutation methods
// ---------------------------------------------------------------------------

impl<'ctx> Key<'ctx> {
    /// Protect (encrypt) the secret material with `options.password`.
    pub fn protect(&self, options: &ProtectOptions) -> Result<()> {
        let pw = options.password.as_ref().ok_or(error::Error::NullPointer)?;
        let pw_c = CString::new(pw.as_str()).map_err(|_| error::Error::NulByte)?;
        let cipher_c = options
            .cipher
            .map(|c| CString::new(c.as_str()).unwrap())
            .unwrap_or_else(|| CString::new("AES256").unwrap());
        // Cipher mode for secret-key protection: CFB (default), CBC, or OCB.
        // Distinct from the S2K type.
        let mode_c = options
            .mode
            .as_ref()
            .map(|m| CString::new(m.as_str()).unwrap())
            .unwrap_or_else(|| CString::new("CFB").unwrap());
        let hash_c = options
            .hash
            .map(|h| CString::new(h.as_str()).unwrap())
            .unwrap_or_else(|| CString::new("SHA256").unwrap());
        unsafe {
            check(ffi::rnp_key_protect(
                self.handle,
                pw_c.as_ptr(),
                cipher_c.as_ptr(),
                mode_c.as_ptr(),
                hash_c.as_ptr(),
                options.iterations.unwrap_or(0),
            ))?;
        }
        Ok(())
    }

    /// Remove the secret-key protection. `password` is required if the key
    /// is protected; pass `None` to use the configured password provider.
    pub fn unprotect(&self, password: Option<&str>) -> Result<()> {
        let pw_c = password
            .map(|p| CString::new(p).map_err(|_| error::Error::NulByte))
            .transpose()?;
        let pw_ptr = pw_c.as_ref().map(|c| c.as_ptr()).unwrap_or(ptr::null());
        unsafe { check(ffi::rnp_key_unprotect(self.handle, pw_ptr)) }
    }

    /// Lock the key — discard the in-memory decrypted secret material.
    pub fn lock(&self) -> Result<()> {
        unsafe { check(ffi::rnp_key_lock(self.handle)) }
    }

    /// Unlock the key (decrypt secret material in memory).
    pub fn unlock(&self, password: Option<&str>) -> Result<()> {
        let pw_c = password
            .map(|p| CString::new(p).map_err(|_| error::Error::NulByte))
            .transpose()?;
        let pw_ptr = pw_c.as_ref().map(|c| c.as_ptr()).unwrap_or(ptr::null());
        unsafe { check(ffi::rnp_key_unlock(self.handle, pw_ptr)) }
    }

    /// Add a user ID to this key. The key must be unlocked if protected.
    pub fn add_uid(&self, uid: &str, options: &AddUidOptions) -> Result<()> {
        let uid_c = CString::new(uid).map_err(|_| error::Error::NulByte)?;
        let hash_c = options
            .hash
            .map(|h| CString::new(h.as_str()).unwrap())
            .unwrap_or_else(|| CString::new("SHA256").unwrap());
        unsafe {
            check(ffi::rnp_key_add_uid(
                self.handle,
                uid_c.as_ptr(),
                hash_c.as_ptr(),
                options.key_expiration.unwrap_or(0),
                options.key_flags.unwrap_or(0) as u8,
                options.primary,
            ))?;
        }
        Ok(())
    }

    /// Revoke this key. The hash algorithm and reason are configured; if
    /// the secret key is locked, the password is obtained from the password
    /// provider.
    pub fn revoke(&self, reason: RevocationReason, hash: Hash) -> Result<()> {
        let reason_c = CString::new(reason.code_str()).unwrap();
        let hash_c = CString::new(hash.as_str()).unwrap();
        let reason_text_c = match &reason.reason {
            Some(s) => Some(CString::new(s.as_str()).map_err(|_| error::Error::NulByte)?),
            None => None,
        };
        let reason_text_ptr = reason_text_c
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(ptr::null());
        unsafe {
            check(ffi::rnp_key_revoke(
                self.handle,
                0,
                hash_c.as_ptr(),
                reason_c.as_ptr(),
                reason_text_ptr,
            ))?;
        }
        Ok(())
    }

    /// Update the key's expiration. `seconds` is from creation. Requires
    /// an unlocked key.
    pub fn set_expiration(&self, seconds: u32) -> Result<()> {
        unsafe { check(ffi::rnp_key_set_expiration(self.handle, seconds)) }
    }

    /// Remove the key from its keyring.
    pub fn remove(&self, flags: RemoveFlags) -> Result<()> {
        unsafe { check(ffi::rnp_key_remove(self.handle, flags.bits())) }
    }

    /// Remove signatures matching `flags`. The optional callback is not
    /// surfaced — callers can re-inspect remaining signatures after the call.
    pub fn remove_signatures(&self, flags: RemoveSignaturesFlags) -> Result<()> {
        unsafe {
            check(ffi::rnp_key_remove_signatures(
                self.handle,
                flags.bits(),
                None,
                ptr::null_mut(),
            ))
        }
    }

    /// Apply the v6 25519 bit-tweak (HKP fingerprint masking).
    pub fn set_25519_bits_tweak(&self) -> Result<()> {
        unsafe { check(ffi::rnp_key_25519_bits_tweak(self.handle)) }
    }
}

// ---------------------------------------------------------------------------
// Shared FFI helper — used by `Key::export_revocation` (inspection) and the
// free-standing `generate_revocation_certificate` in `signature.rs`.
// ---------------------------------------------------------------------------

pub(crate) fn export_revocation_impl(
    handle: ffi::rnp_key_handle_t,
    flags: ExportFlags,
    reason: RevocationReason,
    hash: Hash,
) -> Result<Vec<u8>> {
    let reason_c = CString::new(reason.code_str()).unwrap();
    let hash_c = CString::new(hash.as_str()).unwrap();
    let reason_text_c = match &reason.reason {
        Some(s) => Some(CString::new(s.as_str()).map_err(|_| error::Error::NulByte)?),
        None => None,
    };
    let reason_text_ptr = reason_text_c
        .as_ref()
        .map(|c| c.as_ptr())
        .unwrap_or(ptr::null());
    let output = Output::to_memory()?;
    unsafe {
        check(ffi::rnp_key_export_revocation(
            handle,
            output.as_ptr(),
            flags.bits(),
            hash_c.as_ptr(),
            reason_c.as_ptr(),
            reason_text_ptr,
        ))?;
    }
    output.into_bytes()
}

// Kept for future callbacks that need a stable pointer beyond a single FFI
// call. Currently unused.
#[allow(dead_code)]
fn leak_cstring(c: CString) -> *const c_char {
    let ptr = c.as_ptr();
    std::mem::forget(c);
    ptr
}
