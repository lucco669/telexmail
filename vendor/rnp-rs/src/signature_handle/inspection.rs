//! Read-only inspection methods on [`Signature`].

use crate::error::{self, Result, check};
use crate::ffi;
use crate::ffi_safe::{
    call_for_bool, call_for_optional_string, call_for_string, call_for_u32, call_for_usize,
    cstr_to_optional_string,
};
use std::os::raw::c_char;
use std::ptr;

use super::{Signature, SignatureType, Subpacket, SubpacketType};

impl<'parent> Signature<'parent> {
    /// Signature type as a string (e.g. `"binary"`, `"text"`,
    /// `"certification generic"`). See RFC 4880 §5.2.1 for the meaning.
    pub fn sig_type(&self) -> Result<String> {
        call_for_string(|out| unsafe { ffi::rnp_signature_get_type(self.handle, out) })
    }

    /// Typed signature type. See [`SignatureType`].
    pub fn sig_type_enum(&self) -> Result<SignatureType> {
        Ok(SignatureType::parse(&self.sig_type()?))
    }

    /// Signing algorithm name (e.g. `"RSA"`, `"EDDSA"`).
    pub fn alg(&self) -> Result<String> {
        call_for_string(|out| unsafe { ffi::rnp_signature_get_alg(self.handle, out) })
    }

    /// Hash algorithm name (e.g. `"SHA256"`).
    pub fn hash_alg(&self) -> Result<String> {
        call_for_string(|out| unsafe { ffi::rnp_signature_get_hash_alg(self.handle, out) })
    }

    /// Creation time, seconds since the Unix epoch.
    pub fn creation(&self) -> Result<u32> {
        call_for_u32(|out| unsafe { ffi::rnp_signature_get_creation(self.handle, out) })
    }

    /// Expiration in seconds from creation. `0` means no expiration.
    pub fn expiration(&self) -> Result<u32> {
        call_for_u32(|out| unsafe { ffi::rnp_signature_get_expiration(self.handle, out) })
    }

    /// Key flags word (ORed `RNP_KEY_USAGE_*`).
    pub fn key_flags(&self) -> Result<u32> {
        call_for_u32(|out| unsafe { ffi::rnp_signature_get_key_flags(self.handle, out) })
    }

    /// Key expiration recorded in this signature, if any. `0` means no
    /// expiration.
    pub fn key_expiration(&self) -> Result<u32> {
        call_for_u32(|out| unsafe { ffi::rnp_signature_get_key_expiration(self.handle, out) })
    }

    /// Whether this signature marks its UID as the primary UID.
    pub fn primary_uid(&self) -> Result<bool> {
        call_for_bool(|out| unsafe { ffi::rnp_signature_get_primary_uid(self.handle, out) })
    }

    /// Features bitmask (MDC, AEAD, v5 keys). See `RNP_KEY_FEATURE_*`.
    pub fn features(&self) -> Result<u32> {
        call_for_u32(|out| unsafe { ffi::rnp_signature_get_features(self.handle, out) })
    }

    /// Hex keyid of the signer.
    pub fn keyid(&self) -> Result<String> {
        call_for_string(|out| unsafe { ffi::rnp_signature_get_keyid(self.handle, out) })
    }

    /// Hex fingerprint of the signer.
    pub fn key_fprint(&self) -> Result<String> {
        call_for_string(|out| unsafe { ffi::rnp_signature_get_key_fprint(self.handle, out) })
    }

    /// Hex fingerprint of the signer (alias).
    pub fn signer_keyid(&self) -> Result<String> {
        call_for_string(|out| unsafe { ffi::rnp_signature_get_keyid(self.handle, out) })
    }

    /// Preferred symmetric algorithms (cipher names).
    pub fn preferred_ciphers(&self) -> Result<Vec<String>> {
        super::preferred_list(self.handle, super::PreferredKind::Cipher)
    }

    /// Preferred hash algorithms (hash names).
    pub fn preferred_hashes(&self) -> Result<Vec<String>> {
        super::preferred_list(self.handle, super::PreferredKind::Hash)
    }

    /// Preferred compression algorithms (compression names).
    pub fn preferred_compressions(&self) -> Result<Vec<String>> {
        super::preferred_list(self.handle, super::PreferredKind::Compression)
    }

    /// Preferred key server URL, if any.
    pub fn key_server(&self) -> Result<Option<String>> {
        call_for_optional_string(|out| unsafe {
            ffi::rnp_signature_get_key_server(self.handle, out)
        })
    }

    /// The designated revoker recorded in this signature's revocation-key
    /// subpacket, if any. Wraps `rnp_signature_get_revoker`; upstream
    /// returns an empty string for signatures without a revoker, which is
    /// normalized to `None` here.
    pub fn revoker(&self) -> Result<Option<String>> {
        let raw = call_for_optional_string(|out| unsafe {
            ffi::rnp_signature_get_revoker(self.handle, out)
        })?;
        Ok(raw.filter(|s| !s.is_empty()))
    }

    /// Number of validation errors recorded for this signature.
    /// Wraps `rnp_signature_error_count`.
    pub fn error_count(&self) -> Result<usize> {
        call_for_usize(|out| unsafe { ffi::rnp_signature_error_count(self.handle, out) })
    }

    /// The raw `rnp_result_t` validation error at `idx`. `None` when `idx`
    /// is out of range. Pair with [`from_rnp_code`](crate::from_rnp_code)
    /// for a readable form. Wraps `rnp_signature_error_at`.
    pub fn error_at(&self, idx: usize) -> Result<Option<u32>> {
        // Upstream signals an out-of-range index with BAD_PARAMETERS;
        // bounds-check against the count first so `None` is the answer.
        if idx >= self.error_count()? {
            return Ok(None);
        }
        let mut code: ffi::rnp_result_t = 0;
        unsafe { check(ffi::rnp_signature_error_at(self.handle, idx, &mut code))? };
        Ok(Some(code as u32))
    }

    /// Key server preferences bitmask.
    pub fn key_server_prefs(&self) -> Result<u32> {
        call_for_u32(|out| unsafe { ffi::rnp_signature_get_key_server_prefs(self.handle, out) })
    }

    /// Trust level (0..=255 per RFC 4880 §5.2.3.13).
    pub fn trust_level(&self) -> Result<u8> {
        let mut level: u8 = 0;
        let mut amount: u8 = 0;
        unsafe {
            check(ffi::rnp_signature_get_trust_level(
                self.handle,
                &mut level,
                &mut amount,
            ))?
        };
        Ok(level)
    }

    /// Trust amount (0..=255).
    pub fn trust_amount(&self) -> Result<u8> {
        let mut level: u8 = 0;
        let mut amount: u8 = 0;
        unsafe {
            check(ffi::rnp_signature_get_trust_level(
                self.handle,
                &mut level,
                &mut amount,
            ))?
        };
        Ok(amount)
    }

    /// Revocation reason `(code, text)`, if this is a revocation signature.
    /// Returns `None` if the signature is not a revocation.
    pub fn revocation_reason(&self) -> Result<Option<(String, String)>> {
        let mut code_raw: *mut c_char = ptr::null_mut();
        let mut reason_raw: *mut c_char = ptr::null_mut();
        unsafe {
            let code = ffi::rnp_signature_get_revocation_reason(
                self.handle,
                &mut code_raw,
                &mut reason_raw,
            );
            if code == error::NOT_FOUND {
                return Ok(None);
            }
            check(code)?;
            let c = cstr_to_optional_string(code_raw).unwrap_or_default();
            let r = cstr_to_optional_string(reason_raw).unwrap_or_default();
            Ok(Some((c, r)))
        }
    }

    /// Whether this signature is currently valid. Returns `Ok(true)` if so,
    /// else `Ok(false)` (or an `Err` for a hard parse failure).
    pub fn is_valid(&self) -> Result<bool> {
        let code = unsafe { ffi::rnp_signature_is_valid(self.handle, 0) };
        Ok(code == error::SUCCESS)
    }

    /// Number of subpackets on this signature.
    pub fn subpacket_count(&self) -> Result<usize> {
        call_for_usize(|out| unsafe { ffi::rnp_signature_subpacket_count(self.handle, out) })
    }

    /// Borrow the subpacket at index `idx`.
    pub fn subpacket_at(&self, idx: usize) -> Result<Option<Subpacket>> {
        let mut handle: ffi::rnp_sig_subpacket_t = ptr::null_mut();
        unsafe {
            let code = ffi::rnp_signature_subpacket_at(self.handle, idx, &mut handle);
            if code == error::NOT_FOUND {
                return Ok(None);
            }
            check(code)?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(Subpacket::from_handle(handle)))
        }
    }

    /// All subpackets.
    pub fn subpackets(&self) -> Result<Vec<Subpacket>> {
        let n = self.subpacket_count()?;
        (0..n)
            .map(|i| self.subpacket_at(i)?.ok_or(error::Error::NullPointer))
            .collect()
    }

    /// Dump this signature packet as JSON. See
    /// [`crate::dump::JsonDumpFlags`].
    pub fn packet_to_json(&self, flags: crate::dump::JsonDumpFlags) -> Result<String> {
        call_for_string(|raw| unsafe {
            ffi::rnp_signature_packet_to_json(self.handle, flags.bits(), raw)
        })
    }

    /// The signing key, when present in the keyring. Returns `None` if
    /// the signer isn't loaded. Wraps `rnp_signature_get_signer`.
    pub fn signer_key(&self) -> Result<Option<crate::Key<'_>>> {
        let mut handle: ffi::rnp_key_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_signature_get_signer(self.handle, &mut handle))?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(crate::Key::from_handle(handle)))
        }
    }

    /// Find a subpacket of the given type. `hashed = None` means "either
    /// hashed or unhashed", `Some(true)` restricts to hashed,
    /// `Some(false)` to unhashed. `skip` skips the first N matches.
    pub fn find_subpacket(
        &self,
        typ: SubpacketType,
        hashed: Option<bool>,
        skip: usize,
    ) -> Result<Option<Subpacket>> {
        let mut handle: ffi::rnp_sig_subpacket_t = ptr::null_mut();
        let code = unsafe {
            ffi::rnp_signature_subpacket_find(
                self.handle,
                typ.as_u8(),
                hashed.unwrap_or(false),
                skip,
                &mut handle,
            )
        };
        if code == error::NOT_FOUND {
            return Ok(None);
        }
        check(code)?;
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(Subpacket::from_handle(handle)))
        }
    }
}
