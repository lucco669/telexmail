//! OpenPGP key handles.
//!
//! A [`Key`] wraps an `rnp_key_handle_t`. It borrows the [`Context`](crate::Context)
//! it came from for its lifetime. Child handles ([`crate::Uid`], [`crate::Subkey`],
//! [`crate::Signature`]) borrow the `Key` for *their* lifetime.
//!
//! ## Module layout
//!
//! | Sub-module    | Concern                                        |
//! |---------------|------------------------------------------------|
//! | `identifier`  | `KeyIdentifier` enum (lookup key for find_key) |
//! | `flags`       | Bitflag types consumed by export/remove/unload |
//! | `inspection`  | Read-only getters on `Key`                     |
//! | `mutation`    | Operations that change key state + option types|
//! | `lookup`      | `Context::find_key` / `Context::load_keys`     |

use crate::ffi;
use std::marker::PhantomData;
use std::ptr;

/// Borrowed handle to a key in the FFI's keyring.
pub struct Key<'ctx> {
    pub(crate) handle: ffi::rnp_key_handle_t,
    _ctx: PhantomData<&'ctx crate::Context>,
}

impl<'ctx> Key<'ctx> {
    /// Crate-internal constructor: wrap a raw handle, borrowing `ctx`.
    pub(crate) fn from_handle(handle: ffi::rnp_key_handle_t) -> Self {
        Key {
            handle,
            _ctx: PhantomData,
        }
    }
}

impl<'ctx> Drop for Key<'ctx> {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                let _ = ffi::rnp_key_handle_destroy(self.handle);
            }
            self.handle = ptr::null_mut();
        }
    }
}

// --- sub-modules -----------------------------------------------------------

mod identifier;
pub use identifier::KeyIdentifier;

mod flags;
pub use flags::{ExportFlags, LoadSaveFlags, RemoveFlags, RemoveSignaturesFlags, UnloadFlags};

mod inspection;

mod mutation;
pub(crate) use mutation::export_revocation_impl;
pub use mutation::{AddUidOptions, ProtectOptions, RevocationCode, RevocationReason};

mod lookup;
