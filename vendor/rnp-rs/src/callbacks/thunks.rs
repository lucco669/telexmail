//! C-side thunks + [`ContextBorrow`] for librnp callbacks.

use crate::context::Context;
use crate::ffi;
use crate::key::{Key, KeyIdentifier};
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::os::raw::c_void;

use super::Callbacks;
use super::registry::{KeyProviderHolder, PasswordHolder};
use super::types::{KeyRequestOutcome, RequestedKeyType};

// Bridges librnp's C callback to our `PasswordProvider` trait. We do not
// currently surface the `key` argument as a fully fledged `Key` because doing
// so safely within the callback's lifetime is fiddly; it is left as a TODO.
pub(crate) unsafe extern "C" fn password_thunk(
    _ffi: ffi::rnp_ffi_t,
    app_ctx: *mut c_void,
    key: ffi::rnp_key_handle_t,
    pgp_context: *const std::os::raw::c_char,
    buf: *mut std::os::raw::c_char,
    buf_len: usize,
) -> bool {
    if app_ctx.is_null() {
        return false;
    }
    unsafe {
        let provider = &*(app_ctx as *const PasswordHolder);

        let context_str = if pgp_context.is_null() {
            String::new()
        } else {
            std::ffi::CStr::from_ptr(pgp_context)
                .to_string_lossy()
                .into_owned()
        };

        // TODO: surface `key` as a borrowed crate::key::Key. For now pass None.
        let _ = key;
        let password = provider.inner.get_password(None, &context_str);
        let Some(password) = password else {
            return false;
        };

        let bytes = password.as_bytes();
        if bytes.len() + 1 > buf_len {
            return false;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf.cast::<u8>(), bytes.len());
        *buf.add(bytes.len()) = 0;
        true
    }
}

// Reconstructs a transient `Context` view of `ffi` for the duration of the
// callback. Wrapped in `ManuallyDrop` so its Drop never runs — the ffi is
// owned by another Context and must not be destroyed here.
pub(crate) unsafe extern "C" fn key_provider_thunk(
    ffi: ffi::rnp_ffi_t,
    app_ctx: *mut c_void,
    identifier_type: *const std::os::raw::c_char,
    identifier: *const std::os::raw::c_char,
    _hidden: bool,
) {
    unsafe {
        if app_ctx.is_null() || identifier.is_null() {
            return;
        }
        let holder = &*(app_ctx as *const KeyProviderHolder);
        let kind_str = if identifier_type.is_null() {
            String::new()
        } else {
            std::ffi::CStr::from_ptr(identifier_type)
                .to_string_lossy()
                .into_owned()
        };
        let id_str = std::ffi::CStr::from_ptr(identifier).to_string_lossy();
        let kind = RequestedKeyType::parse(&kind_str);
        let id = match kind {
            RequestedKeyType::Keyid => KeyIdentifier::Keyid(&id_str),
            RequestedKeyType::Fingerprint => KeyIdentifier::Fingerprint(&id_str),
            RequestedKeyType::Grip => KeyIdentifier::Grip(&id_str),
        };
        let ctx_ref = ContextBorrow::new(ffi);
        let _ = holder.inner.on_key_request(&ctx_ref, id, kind);
    }
}

/// Transient view of an `rnp_ffi_t` for use inside C callbacks.
///
/// Wraps a `Context` in `ManuallyDrop` so the underlying ffi is never
/// destroyed by this view. The lifetime tie (`'a`) prevents the view
/// from outliving anything that depends on the ffi still being valid.
pub(crate) struct ContextBorrow<'a> {
    inner: ManuallyDrop<Context>,
    _borrow: PhantomData<&'a Context>,
}

impl<'a> ContextBorrow<'a> {
    pub(crate) fn new(ffi: ffi::rnp_ffi_t) -> Self {
        ContextBorrow {
            inner: ManuallyDrop::new(Context {
                ffi,
                callbacks: Callbacks::new(),
            }),
            _borrow: PhantomData,
        }
    }
}

impl<'a> std::ops::Deref for ContextBorrow<'a> {
    type Target = Context;
    fn deref(&self) -> &Context {
        &self.inner
    }
}

// Suppress unused-parameter warnings.
#[allow(dead_code)]
fn _silence_warnings() {
    let _ = std::any::TypeId::of::<Key>();
    let _ = std::any::TypeId::of::<KeyRequestOutcome>();
}
