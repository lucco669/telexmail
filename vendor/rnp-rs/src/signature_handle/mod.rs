//! Signature handle and subpacket types.
//!
//! [`Signature`] wraps `rnp_signature_handle_t`. It borrows the parent
//! [`Key`](crate::Key) or [`Uid`](crate::Uid) for its lifetime. A signature
//! is one of:
//!
//! - a self-signature binding a UID or subkey to its primary,
//! - a direct signature on a key,
//! - a third-party certification,
//! - a revocation signature.
//!
//! ## Module layout
//!
//! | Sub-module    | Concern                                       |
//! |---------------|-----------------------------------------------|
//! | `sig_type`    | `SignatureType` enum                          |
//! | `subpacket`   | `Subpacket` handle + `SubpacketType` enum     |
//! | `inspection`  | Read-only getters on `Signature`              |
//! | `mutation`    | Export + remove-from-key on `Signature`       |

use crate::ffi;
use crate::ffi_safe::{call_for_string, call_for_usize};
use std::marker::PhantomData;
use std::ptr;

// ---------------------------------------------------------------------------
// Signature handle: struct + Drop
// ---------------------------------------------------------------------------

/// Borrowed handle to a signature on a [`Key`](crate::Key) or [`Uid`](crate::Uid).
pub struct Signature<'parent> {
    pub(crate) handle: ffi::rnp_signature_handle_t,
    _parent: PhantomData<&'parent ()>,
}

impl<'parent> Signature<'parent> {
    pub(crate) fn from_handle(handle: ffi::rnp_signature_handle_t) -> Self {
        Signature {
            handle,
            _parent: PhantomData,
        }
    }
}

impl<'parent> Drop for Signature<'parent> {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                let _ = ffi::rnp_signature_handle_destroy(self.handle);
            }
            self.handle = ptr::null_mut();
        }
    }
}

// ---------------------------------------------------------------------------
// Sub-modules
// ---------------------------------------------------------------------------

mod sig_type;
pub use sig_type::SignatureType;

mod subpacket;
pub use subpacket::{Subpacket, SubpacketType};

mod inspection;
mod mutation;

// ---------------------------------------------------------------------------
// Internal helper used by inspection (preferred_{cipher,hash,compression}).
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub(crate) enum PreferredKind {
    Cipher,
    Hash,
    Compression,
}

pub(crate) fn preferred_list(
    sig: ffi::rnp_signature_handle_t,
    kind: PreferredKind,
) -> crate::error::Result<Vec<String>> {
    let count = match kind {
        PreferredKind::Cipher => {
            call_for_usize(|out| unsafe { ffi::rnp_signature_get_preferred_alg_count(sig, out) })?
        }
        PreferredKind::Hash => {
            call_for_usize(|out| unsafe { ffi::rnp_signature_get_preferred_hash_count(sig, out) })?
        }
        PreferredKind::Compression => {
            call_for_usize(|out| unsafe { ffi::rnp_signature_get_preferred_zalg_count(sig, out) })?
        }
    };
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let s = match kind {
            PreferredKind::Cipher => {
                call_for_string(|raw| unsafe { ffi::rnp_signature_get_preferred_alg(sig, i, raw) })?
            }
            PreferredKind::Hash => call_for_string(|raw| unsafe {
                ffi::rnp_signature_get_preferred_hash(sig, i, raw)
            })?,
            PreferredKind::Compression => call_for_string(|raw| unsafe {
                ffi::rnp_signature_get_preferred_zalg(sig, i, raw)
            })?,
        };
        out.push(s);
    }
    Ok(out)
}
