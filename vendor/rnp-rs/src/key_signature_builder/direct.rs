//! [`DirectSignatureBuilder`] — direct signature on a key (no UID binding).

use crate::error::{self, Result, check};
use crate::ffi;
use crate::key::Key;
use std::marker::PhantomData;
use std::ptr;

use super::configured::ConfiguredBuilder;
use super::inner::SignatureBuilderInner;

/// Build a direct signature on a key (no UID binding).
pub struct DirectSignatureBuilder<'a> {
    pub(crate) inner: SignatureBuilderInner,
    pub(crate) _signer: &'a Key<'a>,
    pub(crate) _target: Option<&'a Key<'a>>,
}

impl<'a> DirectSignatureBuilder<'a> {
    /// Begin a direct signature. `target` of `None` means self-signature.
    pub fn new(signer: &'a Key<'a>, target: Option<&'a Key<'a>>) -> Result<Self> {
        let target_handle = target.map(|k| k.handle).unwrap_or(ptr::null_mut());
        let mut handle: ffi::rnp_signature_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_key_direct_signature_create(
                signer.handle,
                target_handle,
                &mut handle,
            ))?;
        }
        if handle.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(DirectSignatureBuilder {
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
