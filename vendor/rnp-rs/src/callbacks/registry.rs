//! `Callbacks` registry — owns the password/key-provider holders and
//! their installation lifetime.

use crate::ffi;
use std::os::raw::c_void;

use super::thunks::{key_provider_thunk, password_thunk};
use super::traits::{KeyProvider, PasswordProvider};

/// Holder for a boxed `PasswordProvider`. The C-side thunk derefs the
/// raw pointer to this struct.
pub(crate) struct PasswordHolder {
    pub(crate) inner: Box<dyn PasswordProvider>,
}

/// Holder for a boxed `KeyProvider`. The C-side thunk derefs the raw
/// pointer to this struct.
pub(crate) struct KeyProviderHolder {
    pub(crate) inner: Box<dyn KeyProvider>,
}

/// Owns the password + key-provider holders. Field-replaced via the
/// `set_*` methods; `take()` drops both before `Context::drop` calls
/// `rnp_ffi_destroy`.
pub(crate) struct Callbacks {
    password: Option<Box<PasswordHolder>>,
    key_provider: Option<Box<KeyProviderHolder>>,
}

impl Callbacks {
    pub(crate) fn new() -> Self {
        Callbacks {
            password: None,
            key_provider: None,
        }
    }

    pub(crate) fn set_password(
        &mut self,
        ffi: ffi::rnp_ffi_t,
        provider: Box<dyn PasswordProvider>,
    ) {
        let boxed = Box::new(PasswordHolder { inner: provider });
        let raw: *mut PasswordHolder = Box::into_raw(boxed);
        // SAFETY: the thunk derefs `raw` only while `self` is alive — we
        // re-box it in `take()` before drop.
        unsafe {
            let _ = ffi::rnp_ffi_set_pass_provider(ffi, Some(password_thunk), raw as *mut c_void);
        }
        self.password = Some(unsafe { Box::from_raw(raw) });
    }

    pub(crate) fn set_key_provider(&mut self, ffi: ffi::rnp_ffi_t, provider: Box<dyn KeyProvider>) {
        let boxed = Box::new(KeyProviderHolder { inner: provider });
        let raw: *mut KeyProviderHolder = Box::into_raw(boxed);
        unsafe {
            let _ =
                ffi::rnp_ffi_set_key_provider(ffi, Some(key_provider_thunk), raw as *mut c_void);
        }
        self.key_provider = Some(unsafe { Box::from_raw(raw) });
    }

    /// Drop the holders. Called from `Context::drop` before destroying the
    /// ffi so any final callbacks librnp fires during teardown see freed
    /// memory first.
    pub(crate) fn take(&mut self) {
        self.password.take();
        self.key_provider.take();
    }
}
