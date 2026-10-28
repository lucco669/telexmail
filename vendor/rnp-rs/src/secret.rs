//! Password-hygienic string type.
//!
//! [`SecretString`] wraps a [`String`] and zeroises the underlying bytes on
//! `Drop` via [`rnp_buffer_clear`](crate::ffi::rnp_buffer_clear). Use it for
//! password returns and any other short-lived secret material that
//! shouldn't sit in freed heap memory.
//!
//! `SecretString` is intentionally neither `Clone` nor `Display`:
//! cloning defeats the zero-on-drop invariant, and a `Display` impl would
//! let callers accidentally log the secret.

use crate::ffi;
use std::ops::Deref;

/// Owned string whose bytes are zeroed on drop.
pub struct SecretString {
    inner: String,
}

impl SecretString {
    /// Take ownership of `s`. The bytes will be zeroed when this value
    /// drops.
    pub fn new(s: String) -> Self {
        SecretString { inner: s }
    }

    /// Construct from a string literal (for tests).
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        Self::new(s.to_owned())
    }

    /// View the secret as `&str`.
    pub fn as_str(&self) -> &str {
        &self.inner
    }

    /// Consume and yield the underlying [`String`] *without* zeroing.
    /// Use when transferring ownership to a C-side buffer that librnp
    /// itself will clear. If you just need read access, prefer
    /// [`Self::as_str`].
    pub fn into_string(self) -> String {
        // Suppress the Drop zero by wrapping in ManuallyDrop, then move
        // the inner String out by reading through the pointer.
        let me = std::mem::ManuallyDrop::new(self);
        // SAFETY: `me` is ManuallyDrop so its destructor never runs.
        // Reading the inner field out by pointer moves it to the caller
        // without running Drop on `self`.
        unsafe { std::ptr::read(&me.inner) }
    }

    /// Zeroise the underlying bytes now. Called automatically on drop;
    /// exposed for callers who want deterministic zeroing ahead of drop.
    pub fn zeroize(&mut self) {
        zero_string_bytes(&mut self.inner);
    }
}

impl Drop for SecretString {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl Deref for SecretString {
    type Target = str;
    fn deref(&self) -> &str {
        &self.inner
    }
}

impl AsRef<str> for SecretString {
    fn as_ref(&self) -> &str {
        &self.inner
    }
}

impl From<String> for SecretString {
    fn from(s: String) -> Self {
        SecretString::new(s)
    }
}

impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Deliberately do NOT print the contents.
        f.write_str("SecretString(***)")
    }
}

/// Zeroise the bytes backing a string slice. Uses `rnp_buffer_clear` so the
/// compiler can't optimise the store away (it's a non-inlined foreign
/// call).
pub fn zero_string_bytes(s: &mut str) {
    // SAFETY: we have exclusive access via `&mut`, and the cast through
    // `as_bytes_mut` is the canonical way to obtain a mutable byte view
    // of a `String`'s backing storage.
    unsafe {
        let bytes = s.as_bytes_mut();
        let ptr = bytes.as_mut_ptr() as *mut std::ffi::c_void;
        ffi::rnp_buffer_clear(ptr, bytes.len());
    }
}
