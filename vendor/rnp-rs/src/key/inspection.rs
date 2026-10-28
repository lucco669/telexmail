//! Read-only inspection methods on [`Key`].
//!
//! These methods query key state without mutating the FFI handle. They form
//! the bulk of the inspection surface and are safe to call concurrently with
//! each other (librnp itself is not thread-safe across contexts, but the
//! underlying getters are pure reads).
//!
//! All FFI calls go through the [`crate::ffi_safe`] seam — this file is
//! intentionally free of inline `unsafe` blocks outside that module.

use crate::ffi_safe::{
    call_for_bool, call_for_optional_string, call_for_string, call_for_u32, call_for_u64,
    call_for_usize,
};
use crate::keygen;
use crate::{dump, ffi};

use super::{ExportFlags, Key, RevocationReason};
use std::ffi::CString;
use std::ptr;

impl<'ctx> Key<'ctx> {
    // --- export (logically inspection — they read state and produce bytes) --

    /// Export this key as raw bytes. Pass `ExportFlags::ARMORED` for ASCII
    /// armor, otherwise binary OpenPGP packets are produced.
    pub fn export(&self, flags: ExportFlags) -> crate::error::Result<Vec<u8>> {
        use crate::error::check;
        use crate::ops::Output;
        let output = Output::to_memory()?;
        unsafe {
            check(ffi::rnp_key_export(
                self.handle,
                output.as_ptr(),
                flags.bits(),
            ))?;
        }
        output.into_bytes()
    }

    pub fn raw_public_data(&self) -> crate::error::Result<Vec<u8>> {
        use crate::ffi_safe::call_for_owned_bytes;
        call_for_owned_bytes(|ptr, len| unsafe {
            ffi::rnp_get_public_key_data(self.handle, ptr, len)
        })
    }

    pub fn raw_secret_data(&self) -> crate::error::Result<Vec<u8>> {
        use crate::ffi_safe::call_for_owned_bytes;
        call_for_owned_bytes(|ptr, len| unsafe {
            ffi::rnp_get_secret_key_data(self.handle, ptr, len)
        })
    }

    /// Export an Autocrypt-compatible form of this key (a single-UID
    /// public key suitable for inclusion in email headers). `subkey` may
    /// be `None` for the primary-only case.
    pub fn export_autocrypt(
        &self,
        subkey: Option<&Key<'_>>,
        uid: Option<&str>,
        flags: ExportFlags,
    ) -> crate::error::Result<Vec<u8>> {
        use crate::error::{Error, check};
        use crate::ops::Output;
        let subkey_handle = subkey.map(|k| k.handle).unwrap_or(self.handle);
        let uid_c = uid
            .map(|s| CString::new(s).map_err(|_| Error::NulByte))
            .transpose()?;
        let uid_ptr = uid_c.as_ref().map(|c| c.as_ptr()).unwrap_or(ptr::null());
        let output = Output::to_memory()?;
        unsafe {
            check(ffi::rnp_key_export_autocrypt(
                self.handle,
                subkey_handle,
                uid_ptr,
                output.as_ptr(),
                flags.bits(),
            ))?;
        }
        output.into_bytes()
    }

    /// Export a revocation certificate for this key. The hash algorithm
    /// and revocation reason are configured by `hash` and `reason`; the
    /// password (if needed) is obtained from the configured password
    /// provider.
    pub fn export_revocation(
        &self,
        flags: ExportFlags,
        reason: RevocationReason,
        hash: keygen::Hash,
    ) -> crate::error::Result<Vec<u8>> {
        super::export_revocation_impl(self.handle, flags, reason, hash)
    }

    // --- scalar identifiers ----------------------------------------------

    /// The key's primary user id, if any.
    pub fn primary_uid(&self) -> crate::error::Result<Option<String>> {
        call_for_optional_string(|out| unsafe { ffi::rnp_key_get_primary_uid(self.handle, out) })
    }

    /// Algorithm name (e.g. `"RSA"`, `"EDDSA"`).
    pub fn alg(&self) -> crate::error::Result<String> {
        call_for_string(|out| unsafe { ffi::rnp_key_get_alg(self.handle, out) })
    }

    /// Key size in bits. Returns 0 for curves where the concept doesn't apply.
    pub fn bits(&self) -> crate::error::Result<u32> {
        call_for_u32(|out| unsafe { ffi::rnp_key_get_bits(self.handle, out) })
    }

    /// DSA `q` size in bits. Only meaningful for DSA keys.
    pub fn dsa_qbits(&self) -> crate::error::Result<u32> {
        call_for_u32(|out| unsafe { ffi::rnp_key_get_dsa_qbits(self.handle, out) })
    }

    /// Curve name (e.g. `"NIST P-256"`, `"Ed25519"`). Returns `None` if the
    /// algorithm is not curve-based.
    pub fn curve(&self) -> crate::error::Result<Option<String>> {
        call_for_optional_string(|out| unsafe { ffi::rnp_key_get_curve(self.handle, out) })
    }

    /// Key version (4 or 6 in current OpenPGP; legacy v3 possible).
    pub fn version(&self) -> crate::error::Result<u32> {
        call_for_u32(|out| unsafe { ffi::rnp_key_get_version(self.handle, out) })
    }

    /// Creation time, seconds since the Unix epoch.
    pub fn creation(&self) -> crate::error::Result<u32> {
        call_for_u32(|out| unsafe { ffi::rnp_key_get_creation(self.handle, out) })
    }

    /// Expiration in seconds from creation. `0` means no expiration.
    pub fn expiration(&self) -> crate::error::Result<u32> {
        call_for_u32(|out| unsafe { ffi::rnp_key_get_expiration(self.handle, out) })
    }

    /// Hex keyid (e.g. `"014F7B24CD14F2A5"`).
    pub fn keyid(&self) -> crate::error::Result<String> {
        call_for_string(|out| unsafe { ffi::rnp_key_get_keyid(self.handle, out) })
    }

    /// Hex fingerprint (40 chars for v4 SHA-1, 64 for v6 SHA-256).
    pub fn fingerprint(&self) -> crate::error::Result<String> {
        call_for_string(|out| unsafe { ffi::rnp_key_get_fprint(self.handle, out) })
    }

    /// Hex key grip (librnp's SHA-1-based identifier used for keyring lookups).
    pub fn grip(&self) -> crate::error::Result<String> {
        call_for_string(|out| unsafe { ffi::rnp_key_get_grip(self.handle, out) })
    }

    /// Fingerprint of the primary key that this subkey belongs to. `None`
    /// if this is itself a primary.
    pub fn primary_fprint(&self) -> crate::error::Result<Option<String>> {
        call_for_optional_string(|out| unsafe { ffi::rnp_key_get_primary_fprint(self.handle, out) })
    }

    /// Grip of the primary key that this subkey belongs to. `None` if this
    /// is itself a primary.
    pub fn primary_grip(&self) -> crate::error::Result<Option<String>> {
        call_for_optional_string(|out| unsafe { ffi::rnp_key_get_primary_grip(self.handle, out) })
    }

    /// Key usage flags word (ORed `RNP_KEY_USAGE_*` constants). Use
    /// [`Self::allows_usage`] to test individual usages.
    pub fn allows_usage(&self, usage: keygen::KeyUsage) -> crate::error::Result<bool> {
        let usage_c = CString::new(usage.as_str()).unwrap();
        call_for_bool(|out| unsafe {
            ffi::rnp_key_allows_usage(self.handle, usage_c.as_ptr(), out)
        })
    }

    // --- booleans --------------------------------------------------------

    pub fn have_secret(&self) -> crate::error::Result<bool> {
        call_for_bool(|out| unsafe { ffi::rnp_key_have_secret(self.handle, out) })
    }

    pub fn have_public(&self) -> crate::error::Result<bool> {
        call_for_bool(|out| unsafe { ffi::rnp_key_have_public(self.handle, out) })
    }

    pub fn is_primary(&self) -> crate::error::Result<bool> {
        call_for_bool(|out| unsafe { ffi::rnp_key_is_primary(self.handle, out) })
    }

    pub fn is_sub(&self) -> crate::error::Result<bool> {
        call_for_bool(|out| unsafe { ffi::rnp_key_is_sub(self.handle, out) })
    }

    pub fn is_valid(&self) -> crate::error::Result<bool> {
        call_for_bool(|out| unsafe { ffi::rnp_key_is_valid(self.handle, out) })
    }

    pub fn is_revoked(&self) -> crate::error::Result<bool> {
        call_for_bool(|out| unsafe { ffi::rnp_key_is_revoked(self.handle, out) })
    }

    pub fn is_locked(&self) -> crate::error::Result<bool> {
        call_for_bool(|out| unsafe { ffi::rnp_key_is_locked(self.handle, out) })
    }

    /// True if the key was revoked with a "key compromised" reason code.
    /// Wraps `rnp_key_is_compromised`. Upstream reports
    /// `RNP_ERROR_BAD_PARAMETERS` for keys that are not revoked at all;
    /// that case is reported as `Ok(false)` — the answer to "was it
    /// compromised?" is simply no.
    pub fn is_compromised(&self) -> crate::error::Result<bool> {
        revoked_reason_flag(|out| unsafe { ffi::rnp_key_is_compromised(self.handle, out) })
    }

    /// True if the key was retired (revocation reason "no longer used").
    /// Wraps `rnp_key_is_retired`; see [`Self::is_compromised`] for the
    /// not-revoked mapping.
    pub fn is_retired(&self) -> crate::error::Result<bool> {
        revoked_reason_flag(|out| unsafe { ffi::rnp_key_is_retired(self.handle, out) })
    }

    /// True if the key was superseded (revocation reason "superseded by
    /// another key"). Wraps `rnp_key_is_superseded`; see
    /// [`Self::is_compromised`] for the not-revoked mapping.
    pub fn is_superseded(&self) -> crate::error::Result<bool> {
        revoked_reason_flag(|out| unsafe { ffi::rnp_key_is_superseded(self.handle, out) })
    }

    /// The userid string at `idx`, without going through a [`Uid`] handle
    /// (the handle-returning variant is [`Self::uid_at`]). `None` when
    /// `idx` is out of range. Wraps `rnp_key_get_uid_at`.
    ///
    /// [`Uid`]: crate::Uid
    pub fn uid_string_at(&self, idx: usize) -> crate::error::Result<Option<String>> {
        call_for_optional_string(|out| unsafe { ffi::rnp_key_get_uid_at(self.handle, idx, out) })
    }

    /// The key's direct revocation signature, if the key is revoked.
    /// Wraps `rnp_key_get_revocation_signature`.
    pub fn revocation_signature(&self) -> crate::error::Result<Option<crate::Signature<'_>>> {
        use crate::error::check;
        let mut handle: ffi::rnp_signature_handle_t = std::ptr::null_mut();
        unsafe {
            check(ffi::rnp_key_get_revocation_signature(
                self.handle,
                &mut handle,
            ))?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(crate::Signature::from_handle(handle)))
        }
    }

    pub fn is_protected(&self) -> crate::error::Result<bool> {
        call_for_bool(|out| unsafe { ffi::rnp_key_is_protected(self.handle, out) })
    }

    /// Last time the key is valid, as a 32-bit seconds-since-epoch. Returns
    /// `u32::MAX` if the key never expires.
    pub fn valid_till(&self) -> crate::error::Result<u32> {
        call_for_u32(|out| unsafe { ffi::rnp_key_valid_till(self.handle, out) })
    }

    /// Same as [`Self::valid_till`] but 64-bit (y2038-safe).
    pub fn valid_till64(&self) -> crate::error::Result<u64> {
        call_for_u64(|out| unsafe { ffi::rnp_key_valid_till64(self.handle, out) })
    }

    /// Whether the key's validity window has passed, as computed by
    /// librnp itself (accounts for self-signature expiry, not just the
    /// expiration field). Wraps `rnp_key_is_expired`.
    pub fn is_expired(&self) -> crate::error::Result<bool> {
        call_for_bool(|out| unsafe { ffi::rnp_key_is_expired(self.handle, out) })
    }

    // --- protection ------------------------------------------------------

    pub fn protection_type(&self) -> crate::error::Result<String> {
        call_for_string(|out| unsafe { ffi::rnp_key_get_protection_type(self.handle, out) })
    }

    pub fn protection_mode(&self) -> crate::error::Result<String> {
        call_for_string(|out| unsafe { ffi::rnp_key_get_protection_mode(self.handle, out) })
    }

    pub fn protection_cipher(&self) -> crate::error::Result<String> {
        call_for_string(|out| unsafe { ffi::rnp_key_get_protection_cipher(self.handle, out) })
    }

    pub fn protection_hash(&self) -> crate::error::Result<String> {
        call_for_string(|out| unsafe { ffi::rnp_key_get_protection_hash(self.handle, out) })
    }

    pub fn protection_iterations(&self) -> crate::error::Result<usize> {
        call_for_usize(|out| unsafe { ffi::rnp_key_get_protection_iterations(self.handle, out) })
    }

    // --- revocation ------------------------------------------------------

    /// Textual reason for the key's revocation, if any.
    pub fn revocation_reason(&self) -> crate::error::Result<Option<String>> {
        call_for_optional_string(|out| unsafe {
            ffi::rnp_key_get_revocation_reason(self.handle, out)
        })
    }

    /// Number of revokers (third-party keys authorized to revoke this key).
    pub fn revoker_count(&self) -> crate::error::Result<usize> {
        call_for_usize(|out| unsafe { ffi::rnp_key_get_revoker_count(self.handle, out) })
    }

    /// Revocation key specification at `idx` (a string of the form
    /// `"1:FINGERPRINT"`).
    pub fn revoker_at(&self, idx: usize) -> crate::error::Result<String> {
        call_for_string(|out| unsafe { ffi::rnp_key_get_revoker_at(self.handle, idx, out) })
    }

    // --- 25519 bit-tweak (v6 HKP fingerprint masking) --------------------

    pub fn is_25519_bits_tweaked(&self) -> crate::error::Result<bool> {
        call_for_bool(|out| unsafe { ffi::rnp_key_25519_bits_tweaked(self.handle, out) })
    }

    // --- child-handle enumeration ----------------------------------------

    pub fn uid_count(&self) -> crate::error::Result<usize> {
        call_for_usize(|out| unsafe { ffi::rnp_key_get_uid_count(self.handle, out) })
    }

    pub fn uid_at(&self, idx: usize) -> crate::error::Result<Option<crate::Uid<'_>>> {
        use crate::error::check;
        let mut handle: ffi::rnp_uid_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_key_get_uid_handle_at(
                self.handle,
                idx,
                &mut handle,
            ))?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(crate::Uid::from_handle(handle)))
        }
    }

    pub fn uids(&self) -> crate::error::Result<Vec<crate::Uid<'_>>> {
        let n = self.uid_count()?;
        (0..n)
            .map(|i| self.uid_at(i)?.ok_or(crate::error::Error::NullPointer))
            .collect()
    }

    pub fn subkey_count(&self) -> crate::error::Result<usize> {
        call_for_usize(|out| unsafe { ffi::rnp_key_get_subkey_count(self.handle, out) })
    }

    pub fn subkey_at(&self, idx: usize) -> crate::error::Result<Option<crate::Subkey<'_>>> {
        let mut handle: ffi::rnp_key_handle_t = ptr::null_mut();
        unsafe {
            crate::error::check(ffi::rnp_key_get_subkey_at(self.handle, idx, &mut handle))?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(crate::Subkey::from_handle(handle)))
        }
    }

    pub fn subkeys(&self) -> crate::error::Result<Vec<crate::Subkey<'_>>> {
        let n = self.subkey_count()?;
        (0..n)
            .map(|i| self.subkey_at(i)?.ok_or(crate::error::Error::NullPointer))
            .collect()
    }

    pub fn signature_count(&self) -> crate::error::Result<usize> {
        call_for_usize(|out| unsafe { ffi::rnp_key_get_signature_count(self.handle, out) })
    }

    pub fn signature_at(&self, idx: usize) -> crate::error::Result<Option<crate::Signature<'_>>> {
        let mut handle: ffi::rnp_signature_handle_t = ptr::null_mut();
        unsafe {
            crate::error::check(ffi::rnp_key_get_signature_at(self.handle, idx, &mut handle))?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(crate::Signature::from_handle(handle)))
        }
    }

    pub fn signatures(&self) -> crate::error::Result<Vec<crate::Signature<'_>>> {
        let n = self.signature_count()?;
        (0..n)
            .map(|i| {
                self.signature_at(i)?
                    .ok_or(crate::error::Error::NullPointer)
            })
            .collect()
    }

    // --- JSON ------------------------------------------------------------

    /// Find the default subkey for a given usage. Returns `None` if no
    /// subkey is suitable. Wraps `rnp_key_get_default_key`.
    pub fn default_key_for(
        &self,
        usage: keygen::KeyUsage,
    ) -> crate::error::Result<Option<Key<'_>>> {
        use crate::error::check;
        let usage_c = CString::new(usage.as_str()).unwrap();
        let mut handle: ffi::rnp_key_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_key_get_default_key(
                self.handle,
                usage_c.as_ptr(),
                0,
                &mut handle,
            ))?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(Key::from_handle(handle)))
        }
    }

    pub fn to_json(&self, flags: dump::JsonFlags) -> crate::error::Result<String> {
        call_for_string(|out| unsafe { ffi::rnp_key_to_json(self.handle, flags.bits(), out) })
    }

    pub fn packets_to_json(
        &self,
        secret: bool,
        flags: dump::JsonDumpFlags,
    ) -> crate::error::Result<String> {
        call_for_string(|out| unsafe {
            ffi::rnp_key_packets_to_json(self.handle, secret, flags.bits(), out)
        })
    }
}

/// Drives an `rnp_key_is_revoked_with_code`-family getter. Upstream returns
/// `RNP_ERROR_BAD_PARAMETERS` when the key is not revoked at all — the
/// answer to "was it revoked with this reason?" is then simply false.
fn revoked_reason_flag<F>(mut f: F) -> crate::error::Result<bool>
where
    F: FnMut(*mut bool) -> u32,
{
    let mut out = false;
    let code = f(&mut out);
    match crate::error::check(code) {
        Ok(()) => Ok(out),
        Err(crate::error::Error::Rnp { code: c, .. })
            if c == crate::error::codes::RNP_ERROR_BAD_PARAMETERS =>
        {
            Ok(false)
        }
        Err(e) => Err(e),
    }
}
