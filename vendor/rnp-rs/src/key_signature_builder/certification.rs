//! [`CertificationBuilder`] — bind a UID to a key.

use crate::Uid;
use crate::error::{self, Result, check};
use crate::ffi;
use crate::key::Key;
use std::ffi::CString;
use std::marker::PhantomData;
use std::ptr;

use super::configured::ConfiguredBuilder;
use super::inner::SignatureBuilderInner;

/// Build a certification signature binding a UID to a key.
pub struct CertificationBuilder<'a> {
    pub(crate) inner: SignatureBuilderInner,
    pub(crate) _signer: &'a Key<'a>,
    pub(crate) _uid: &'a Uid<'a>,
}

impl<'a> CertificationBuilder<'a> {
    /// Begin a certification. `signer` is the key issuing the cert; `uid`
    /// is the UID being certified.
    pub fn new(signer: &'a Key<'a>, uid: &'a Uid<'a>) -> Result<Self> {
        Self::with_type(signer, uid, None)
    }

    /// Like [`Self::new`] but with an explicit certification type
    /// (`"generic"`, `"persona"`, `"casual"`, `"positive"`).
    pub fn with_type_str(signer: &'a Key<'a>, uid: &'a Uid<'a>, ty: &str) -> Result<Self> {
        Self::with_type(signer, uid, Some(ty))
    }

    fn with_type(signer: &'a Key<'a>, uid: &'a Uid<'a>, ty: Option<&str>) -> Result<Self> {
        let ty_c = ty.map(|s| CString::new(s).unwrap());
        let ty_ptr = ty_c.as_ref().map(|c| c.as_ptr()).unwrap_or(ptr::null());
        let mut handle: ffi::rnp_signature_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_key_certification_create(
                signer.handle,
                uid.handle,
                ty_ptr,
                &mut handle,
            ))?;
        }
        if handle.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(CertificationBuilder {
            inner: SignatureBuilderInner { handle },
            _signer: signer,
            _uid: uid,
        })
    }

    /// Access the shared setter chain.
    pub fn configure(self) -> ConfiguredBuilder<'a> {
        ConfiguredBuilder {
            inner: self.inner,
            _phantom: PhantomData,
        }
    }
}
