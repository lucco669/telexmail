//! [`Symenc`] — symmetric-only (password-based) recipient extracted from a
//! [`super::VerifyResult`].

use crate::error::Result;
use crate::ffi;
use crate::ffi_safe::{call_for_optional_string, call_for_string, call_for_u32};

/// Symmetric-only (password-based) recipient extracted from a [`super::VerifyResult`].
pub struct Symenc<'op> {
    _owner: std::marker::PhantomData<&'op ()>,
    pub(crate) handle: ffi::rnp_symenc_handle_t,
}

impl<'op> Symenc<'op> {
    pub(crate) fn new(handle: ffi::rnp_symenc_handle_t) -> Self {
        Symenc { handle, _owner: std::marker::PhantomData }
    }

    /// Symmetric cipher name used for this symenc.
    pub fn cipher(&self) -> Result<String> {
        call_for_string(|raw| unsafe { ffi::rnp_symenc_get_cipher(self.handle, raw) })
    }

    /// AEAD algorithm name, if AEAD was used.
    pub fn aead_alg(&self) -> Result<Option<String>> {
        call_for_optional_string(|out| unsafe { ffi::rnp_symenc_get_aead_alg(self.handle, out) })
    }

    /// Hash algorithm used in the S2K derivation.
    pub fn hash_alg(&self) -> Result<String> {
        call_for_string(|raw| unsafe { ffi::rnp_symenc_get_hash_alg(self.handle, raw) })
    }

    /// S2K type string (`"simple"`, `"salted"`, `"iterated and salted"`).
    pub fn s2k_type(&self) -> Result<String> {
        call_for_string(|raw| unsafe { ffi::rnp_symenc_get_s2k_type(self.handle, raw) })
    }

    /// Number of S2K iterations.
    pub fn s2k_iterations(&self) -> Result<u32> {
        call_for_u32(|out| unsafe { ffi::rnp_symenc_get_s2k_iterations(self.handle, out) })
    }
}
