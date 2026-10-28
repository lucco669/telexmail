//! [`VerifySignature`] + [`SignatureStatus`] — per-signature verification result.

use crate::error::{self, Result, check};
use crate::ffi;
use crate::ffi_safe::call_for_string;
use std::ptr;

/// Per-signature verification status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureStatus {
    Valid,
    Invalid,
    Unknown,
}

/// Per-signature result extracted from a [`super::VerifyResult`].
pub struct VerifySignature<'op> {
    _owner: std::marker::PhantomData<&'op ()>,
    pub(crate) handle: ffi::rnp_op_verify_signature_t,
}

impl<'op> VerifySignature<'op> {
    pub(crate) fn new(handle: ffi::rnp_op_verify_signature_t) -> Self {
        VerifySignature { handle, _owner: std::marker::PhantomData }
    }

    /// True iff the signature was valid.
    pub fn status_is_valid(&self) -> bool {
        let code = unsafe { ffi::rnp_op_verify_signature_get_status(self.handle) };
        code == error::SUCCESS
    }

    pub fn status_code(&self) -> u32 {
        unsafe { ffi::rnp_op_verify_signature_get_status(self.handle) }
    }

    /// Categorize the verification outcome.
    pub fn status(&self) -> SignatureStatus {
        let code = unsafe { ffi::rnp_op_verify_signature_get_status(self.handle) };
        if code == error::SUCCESS {
            SignatureStatus::Valid
        } else {
            // Map by category: Sig-validation range (0x14…) → Invalid;
            // Crypto range with no-signatures / unknown-signature → Unknown;
            // everything else → Invalid.
            match error::ErrorKind::from_code(code) {
                error::ErrorKind::NoSignaturesFound
                | error::ErrorKind::SignatureUnknown
                | error::ErrorKind::SigNoSignerKey
                | error::ErrorKind::SigNoSignerId => SignatureStatus::Unknown,
                _ => SignatureStatus::Invalid,
            }
        }
    }

    /// Hash algorithm used (e.g. `"SHA256"`).
    pub fn hash(&self) -> Result<String> {
        call_for_string(|raw| unsafe { ffi::rnp_op_verify_signature_get_hash(self.handle, raw) })
    }

    /// Creation and expiration times of the signature.
    pub fn times(&self) -> Result<(u32, u32)> {
        let mut create: u32 = 0;
        let mut expires: u32 = 0;
        unsafe {
            check(ffi::rnp_op_verify_signature_get_times(
                self.handle,
                &mut create,
                &mut expires,
            ))?;
        }
        Ok((create, expires))
    }

    /// The full [`Signature`](crate::Signature) handle, for inspection of subpackets, key
    /// flags, and other metadata. The returned handle borrows `self`.
    pub fn handle(&self) -> Result<crate::Signature<'_>> {
        let mut raw: ffi::rnp_signature_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_op_verify_signature_get_handle(
                self.handle,
                &mut raw,
            ))?;
        }
        if raw.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(crate::Signature::from_handle(raw))
    }

    /// The signing key, when present in the keyring. Returns `None` if the
    /// signer's key wasn't loaded. The returned [`Key`](crate::Key) borrows `self`.
    pub fn key(&self) -> Result<Option<crate::Key<'_>>> {
        let mut raw: ffi::rnp_key_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_op_verify_signature_get_key(self.handle, &mut raw))?;
        }
        if raw.is_null() {
            Ok(None)
        } else {
            Ok(Some(crate::Key::from_handle(raw)))
        }
    }

    /// Hex keyid of the signer, drawn from the underlying signature
    /// handle. Empty when the signer's key is unavailable and the
    /// signature itself doesn't carry a keyid.
    pub fn keyid(&self) -> Result<String> {
        let sig = self.handle()?;
        sig.keyid()
    }
}
