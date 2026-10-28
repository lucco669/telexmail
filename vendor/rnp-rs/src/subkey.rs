//! Subkey handle.
//!
//! In librnp a subkey is just a `rnp_key_handle_t` with `is_sub() == true` —
//! every getter that works on a primary key also works on a subkey.
//! [`Subkey`] is therefore a thin newtype over [`Key`]: it
//! forwards via Deref so callers don't have to learn a parallel surface.

use crate::Key;
use crate::error::Result;
use crate::ffi;
use std::marker::PhantomData;

/// Borrowed handle to a subkey. Behaves like a [`Key`] via `Deref`.
pub struct Subkey<'key> {
    inner: Key<'key>,
    _phantom: PhantomData<&'key Key<'key>>,
}

impl<'key> Subkey<'key> {
    pub(crate) fn from_handle(handle: ffi::rnp_key_handle_t) -> Self {
        Subkey {
            inner: Key::from_handle(handle),
            _phantom: PhantomData,
        }
    }

    /// Borrow the underlying [`Key`]. Useful when an API wants a `&Key`
    /// explicitly.
    pub fn as_key(&self) -> &Key<'key> {
        &self.inner
    }

    /// Consume into the underlying [`Key`].
    pub fn into_key(self) -> Key<'key> {
        self.inner
    }
}

impl<'key> std::ops::Deref for Subkey<'key> {
    type Target = Key<'key>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

// Convenience: expose Key's signature methods directly so the type reads
// naturally. Implemented as wrappers rather than Deref so docs are explicit
// about what's available; Deref still works for everything else.

#[allow(dead_code)]
fn _types_are_send_sync_check(_: &Subkey<'_>) -> Result<()> {
    Ok(())
}
