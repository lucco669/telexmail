//! [`SignatureBuilderInner`] + [`SignatureSetterOps`] trait.

use crate::error::{Result, check};
use crate::ffi;
use crate::key::{Key, RevocationReason};
use crate::keygen::{Cipher, Compression, Hash};
use crate::signature_handle::Signature;
use std::ffi::CString;
use std::ptr;

/// Shared setter surface for all signature-creation builders.
///
/// Each method takes and returns `Self` so builders chain cleanly. The
/// blanket impl on the internal `SignatureBuilderInner` does the actual FFI work;
/// concrete builders just delegate.
pub trait SignatureSetterOps: Sized {
    fn hash(self, h: Hash) -> Self;
    fn creation(self, t: u32) -> Self;
    fn key_flags(self, flags: u32) -> Self;
    fn key_expiration(self, seconds: u32) -> Self;
    fn features(self, bits: u32) -> Self;
    fn add_preferred_cipher(self, c: Cipher) -> Self;
    fn add_preferred_hash(self, h: Hash) -> Self;
    fn add_preferred_compression(self, c: Compression) -> Self;
    fn primary_uid(self, primary: bool) -> Self;
    fn key_server(self, url: impl Into<String>) -> Self;
    fn key_server_prefs(self, prefs: u8) -> Self;
    fn revocation_reason(self, reason: RevocationReason) -> Self;
    fn revoker(self, revoker: &Key<'_>) -> Self;
    fn trust_level(self, level: u8, amount: u8) -> Self;

    /// Finalize the signature: compute it and attach it to the target.
    fn sign(self) -> Result<Signature<'static>>;
}

/// Concrete backing struct. Holds the in-progress `rnp_signature_handle_t`.
/// Not public — callers use the typed builders.
pub(crate) struct SignatureBuilderInner {
    pub(crate) handle: ffi::rnp_signature_handle_t,
}

impl SignatureSetterOps for SignatureBuilderInner {
    fn hash(self, h: Hash) -> Self {
        let c = CString::new(h.as_str()).unwrap();
        let _ = unsafe { ffi::rnp_key_signature_set_hash(self.handle, c.as_ptr()) };
        self
    }

    fn creation(self, t: u32) -> Self {
        let _ = unsafe { ffi::rnp_key_signature_set_creation(self.handle, t) };
        self
    }

    fn key_flags(self, flags: u32) -> Self {
        let _ = unsafe { ffi::rnp_key_signature_set_key_flags(self.handle, flags) };
        self
    }

    fn key_expiration(self, seconds: u32) -> Self {
        let _ = unsafe { ffi::rnp_key_signature_set_key_expiration(self.handle, seconds) };
        self
    }

    fn features(self, bits: u32) -> Self {
        let _ = unsafe { ffi::rnp_key_signature_set_features(self.handle, bits) };
        self
    }

    fn add_preferred_cipher(self, c: Cipher) -> Self {
        let cs = CString::new(c.as_str()).unwrap();
        let _ = unsafe { ffi::rnp_key_signature_add_preferred_alg(self.handle, cs.as_ptr()) };
        self
    }

    fn add_preferred_hash(self, h: Hash) -> Self {
        let cs = CString::new(h.as_str()).unwrap();
        let _ = unsafe { ffi::rnp_key_signature_add_preferred_hash(self.handle, cs.as_ptr()) };
        self
    }

    fn add_preferred_compression(self, c: Compression) -> Self {
        let cs = CString::new(c.as_str()).unwrap();
        let _ = unsafe { ffi::rnp_key_signature_add_preferred_zalg(self.handle, cs.as_ptr()) };
        self
    }

    fn primary_uid(self, primary: bool) -> Self {
        let _ = unsafe { ffi::rnp_key_signature_set_primary_uid(self.handle, primary) };
        self
    }

    fn key_server(self, url: impl Into<String>) -> Self {
        if let Ok(c) = CString::new(url.into()) {
            let _ = unsafe { ffi::rnp_key_signature_set_key_server(self.handle, c.as_ptr()) };
        }
        self
    }

    fn key_server_prefs(self, prefs: u8) -> Self {
        let _ = unsafe { ffi::rnp_key_signature_set_key_server_prefs(self.handle, prefs as u32) };
        self
    }

    fn revocation_reason(self, reason: RevocationReason) -> Self {
        let code_c = CString::new(reason.code_str()).unwrap();
        let text_c = reason
            .reason
            .as_ref()
            .and_then(|s| CString::new(s.as_str()).ok());
        let text_ptr = text_c.as_ref().map(|c| c.as_ptr()).unwrap_or(ptr::null());
        let _ = unsafe {
            ffi::rnp_key_signature_set_revocation_reason(self.handle, code_c.as_ptr(), text_ptr)
        };
        self
    }

    fn revoker(self, revoker: &Key<'_>) -> Self {
        let _ = unsafe { ffi::rnp_key_signature_set_revoker(self.handle, revoker.handle, 0) };
        self
    }

    fn trust_level(self, level: u8, amount: u8) -> Self {
        let _ = unsafe { ffi::rnp_key_signature_set_trust_level(self.handle, level, amount) };
        self
    }

    fn sign(self) -> Result<Signature<'static>> {
        unsafe {
            check(ffi::rnp_key_signature_sign(self.handle))?;
            Ok(Signature::from_handle(self.handle))
        }
    }
}
