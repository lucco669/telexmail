//! Top-level librnp FFI handle.
//!
//! [`Context`] is the entry point to the library: it wraps an `rnp_ffi_t` and
//! owns its lifetime. Key rings, signing operations and verification operations
//! are all performed through a `Context`.
//!
//! Callback installation lives on [`Context`] as a thin delegate to the
//! callbacks registry; the trait definitions,
//! holder types, and C-side thunks are in [`crate::callbacks`].
//!
//! ## Where the `Context` verbs live
//!
//! The `Context` surface is intentionally implemented near the domain it
//! touches, not all in this file. Start here, then follow the concern:
//!
//! | Concern | Module | Examples |
//! |---------|--------|----------|
//! | key lookup/loading | `crate::key::lookup` | `find_key`, `load_keys` |
//! | keyring management | [`crate::keyring`] | `save_keys`, `import_keys`, `unload_keys`, counts, identifiers |
//! | callbacks | [`crate::callbacks`] | `set_password_provider`, `set_key_provider`, logging |
//! | security profile | [`crate::security`] | security rules, features, password requests |
//! | operation builders | [`crate::signature`], [`crate::encrypt`], `crate::verify` | sign, encrypt, decrypt, verify |
//!
//! That split is the implementation locality; this table is the front door.

use crate::callbacks::Callbacks;
use crate::error::{self, Result, check};
use crate::ffi;
use std::ffi::CString;
use std::ptr;

/// Wraps `rnp_ffi_t`. Destroyed on drop via `rnp_ffi_destroy`.
///
/// All higher-level handles (keys, signing ops, etc.) borrow the `Context`
/// they were created from for at most its lifetime.
pub struct Context {
    pub(crate) ffi: ffi::rnp_ffi_t,
    pub(crate) callbacks: Callbacks,
}

impl Context {
    /// Create a new context with the GPG keyring format (the most common one).
    pub fn new() -> Result<Self> {
        Self::with_format(KeyringFormat::Gpg, KeyringFormat::Gpg)
    }

    /// Create a new context with explicit keyring formats.
    pub fn with_format(pub_format: KeyringFormat, sec_format: KeyringFormat) -> Result<Self> {
        let pub_c = CString::new(pub_format.as_str()).unwrap();
        let sec_c = CString::new(sec_format.as_str()).unwrap();
        let mut ffi_h: ffi::rnp_ffi_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_ffi_create(
                &mut ffi_h,
                pub_c.as_ptr(),
                sec_c.as_ptr(),
            ))?;
        }
        if ffi_h.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(Context {
            ffi: ffi_h,
            callbacks: Callbacks::new(),
        })
    }

    /// Install a password provider. Must be called before any operation that
    /// may need to unlock a secret key.
    pub fn set_password_provider(&mut self, provider: Box<dyn crate::callbacks::PasswordProvider>) {
        self.callbacks.set_password(self.ffi, provider);
    }

    /// Install a key provider. Called when librnp needs a key that isn't
    /// in the keyring. Inside the callback, use `ctx.load_keys(...)` to
    /// satisfy the request.
    pub fn set_key_provider(&mut self, provider: Box<dyn crate::callbacks::KeyProvider>) {
        self.callbacks.set_key_provider(self.ffi, provider);
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        // Drop the callback registry first so the C side stops calling into
        // freed memory if rnp_ffi_destroy triggers any final callbacks.
        self.callbacks.take();
        if !self.ffi.is_null() {
            unsafe {
                let _ = ffi::rnp_ffi_destroy(self.ffi);
            }
            self.ffi = ptr::null_mut();
        }
    }
}

// librnp handles contain raw pointers and so are not Send/Sync by default;
// we rely on that auto-trait negative impl rather than spelling it out, since
// the explicit `impl !Send` form requires the unstable `negative_impls` feature.

/// Keyring container format passed to `rnp_ffi_create`.
#[derive(Clone, Copy, Debug)]
pub enum KeyringFormat {
    Gpg,
    Kbx,
    G10,
    Json,
}

impl KeyringFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            KeyringFormat::Gpg => "GPG",
            KeyringFormat::Kbx => "KBX",
            KeyringFormat::G10 => "G10",
            KeyringFormat::Json => "JSON",
        }
    }
}
