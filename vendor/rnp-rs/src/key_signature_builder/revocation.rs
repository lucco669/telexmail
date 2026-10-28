//! [`RevocationSignatureBuilder`] — key-revocation signature.

use crate::error::{self, Result, check};
use crate::ffi;
use crate::key::Key;
use std::marker::PhantomData;
use std::ptr;

use super::configured::ConfiguredBuilder;
use super::inner::SignatureBuilderInner;

/// Build a key-revocation signature.
pub struct RevocationSignatureBuilder<'a> {
    pub(crate) inner: SignatureBuilderInner,
    pub(crate) _signer: &'a Key<'a>,
    pub(crate) _target: Option<&'a Key<'a>>,
}

impl<'a> RevocationSignatureBuilder<'a> {
    /// Begin a revocation signature. `target` of `None` means self-revocation.
    pub fn new(signer: &'a Key<'a>, target: Option<&'a Key<'a>>) -> Result<Self> {
        let target_handle = target.map(|k| k.handle).unwrap_or(ptr::null_mut());
        let mut handle: ffi::rnp_signature_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_key_revocation_signature_create(
                signer.handle,
                target_handle,
                &mut handle,
            ))?;
        }
        if handle.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(RevocationSignatureBuilder {
            inner: SignatureBuilderInner { handle },
            _signer: signer,
            _target: target,
        })
    }

    pub fn configure(self) -> ConfiguredBuilder<'a> {
        ConfiguredBuilder {
            inner: self.inner,
            _phantom: PhantomData,
        }
    }
}
