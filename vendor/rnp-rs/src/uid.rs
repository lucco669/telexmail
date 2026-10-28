//! User-ID handle. Borrows the parent [`Key`](crate::Key) for its lifetime.

use crate::error::{self, Result, check};
use crate::ffi;
use crate::ffi_safe::{call_for_bool, call_for_u32, call_for_usize};
use std::marker::PhantomData;
use std::ptr;

/// Kind of user ID. Wraps the `RNP_USER_*` constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UidType {
    /// A textual user ID (`"alice <alice@example.com>"`).
    UserId,
    /// A binary user attribute (e.g. photo).
    UserAttr,
    /// An unknown / unrecognized type.
    Other(u32),
}

impl UidType {
    pub(crate) fn from_raw(raw: u32) -> Self {
        let user_id = ffi::RNP_USER_ID as u32;
        let user_attr = ffi::RNP_USER_ATTR as u32;
        if raw == user_id {
            UidType::UserId
        } else if raw == user_attr {
            UidType::UserAttr
        } else {
            UidType::Other(raw)
        }
    }
}

/// Borrowed handle to a user ID on a [`Key`](crate::Key).
///
/// Borrows the key for at most its lifetime. Destroyed on drop via
/// `rnp_uid_handle_destroy`.
pub struct Uid<'key> {
    pub(crate) handle: ffi::rnp_uid_handle_t,
    _key: PhantomData<&'key crate::Key<'key>>,
}

impl<'key> Uid<'key> {
    pub(crate) fn from_handle(handle: ffi::rnp_uid_handle_t) -> Self {
        Uid {
            handle,
            _key: PhantomData,
        }
    }

    /// The UID type — textual or attribute.
    pub fn uid_type(&self) -> Result<UidType> {
        let raw = call_for_u32(|out| unsafe { ffi::rnp_uid_get_type(self.handle, out) })?;
        Ok(UidType::from_raw(raw))
    }

    /// The raw UID bytes. For textual UIDs this is the UTF-8 string; for
    /// attributes it's the attribute data.
    pub fn data(&self) -> Result<Vec<u8>> {
        let mut ptr: *mut std::os::raw::c_void = ptr::null_mut();
        let mut len: usize = 0;
        unsafe {
            check(ffi::rnp_uid_get_data(self.handle, &mut ptr, &mut len))?;
            if ptr.is_null() {
                return Ok(Vec::new());
            }
            let v = std::slice::from_raw_parts(ptr as *const u8, len).to_vec();
            ffi::rnp_buffer_destroy(ptr);
            Ok(v)
        }
    }

    /// The UID as a UTF-8 string. Lossy for non-UTF-8 bytes.
    pub fn data_string(&self) -> Result<String> {
        let bytes = self.data()?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// True if this UID is marked as the primary UID on the key.
    pub fn is_primary(&self) -> Result<bool> {
        call_for_bool(|out| unsafe { ffi::rnp_uid_is_primary(self.handle, out) })
    }

    /// True if the UID is currently valid (not revoked, on a valid key, etc.).
    pub fn is_valid(&self) -> Result<bool> {
        call_for_bool(|out| unsafe { ffi::rnp_uid_is_valid(self.handle, out) })
    }

    /// True if the UID has been revoked.
    pub fn is_revoked(&self) -> Result<bool> {
        call_for_bool(|out| unsafe { ffi::rnp_uid_is_revoked(self.handle, out) })
    }

    /// Number of signatures on this UID (self-signatures and
    /// third-party certifications).
    pub fn signature_count(&self) -> Result<usize> {
        call_for_usize(|out| unsafe { ffi::rnp_uid_get_signature_count(self.handle, out) })
    }

    /// Borrow the signature at `idx` on this UID. Returned [`Signature`](crate::Signature)
    /// borrows `self`.
    pub fn signature_at(&self, idx: usize) -> Result<Option<crate::Signature<'_>>> {
        let mut handle: ffi::rnp_signature_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_uid_get_signature_at(self.handle, idx, &mut handle))?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(crate::Signature::from_handle(handle)))
        }
    }

    /// All signatures on this UID.
    pub fn signatures(&self) -> Result<Vec<crate::Signature<'_>>> {
        let n = self.signature_count()?;
        (0..n)
            .map(|i| self.signature_at(i)?.ok_or(error::Error::NullPointer))
            .collect()
    }

    /// The UID's revocation signature, if the UID has been revoked.
    pub fn revocation_signature(&self) -> Result<Option<crate::Signature<'_>>> {
        let mut handle: ffi::rnp_signature_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_uid_get_revocation_signature(
                self.handle,
                &mut handle,
            ))?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(crate::Signature::from_handle(handle)))
        }
    }

    /// Remove this UID from its parent key. Wraps `rnp_uid_remove(key, uid)`.
    pub fn remove(&self, key: &crate::Key<'_>) -> Result<()> {
        unsafe { check(ffi::rnp_uid_remove(key.handle, self.handle)) }
    }
}

impl<'key> Drop for Uid<'key> {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                let _ = ffi::rnp_uid_handle_destroy(self.handle);
            }
            self.handle = ptr::null_mut();
        }
    }
}

// Re-export the error type for cross-module ergonomics.
#[allow(unused_imports)]
use error as _error_alias;
