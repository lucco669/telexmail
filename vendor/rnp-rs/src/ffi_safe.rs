//! Centralized safety wrappers around the librnp C FFI.
//!
//! This module is the **seam** between Rust and `librnp`. Every call into the
//! C API that follows one of the library's standard shapes should go through
//! a helper here, not be spelled out inline at the call site. This gives:
//!
//! - **Locality** — every unsafe FFI call lives in one file, auditable in
//!   one read.
//! - **DRY** — the `let mut raw: *mut c_char = ptr::null_mut(); check(...)?;
//!   cstr_to_string(raw)` triplet appears ~25 times in the crate; centralizing
//!   it removes ~75 lines of repetition.
//! - **Consistent error mapping** — `RNP_ERROR_NOT_FOUND` → `Ok(None)`,
//!   null out-pointer → `Error::NullPointer`, etc.
//!
//! ## When to add a new helper
//!
//! Add a new wrapper when the same FFI shape appears in 3+ call sites. For
//! one-off calls, an inline `unsafe { check(ffi::rnp_x(...))? }` is fine —
//! forcing it through a helper would be premature abstraction.
//!
//! ## When NOT to use these
//!
//! If a call needs to inspect the raw `rnp_result_t` for non-error semantics
//! (e.g. distinguishing `NOT_FOUND` from `NULL_POINTER`), spell it out
//! inline. The helpers here encode one canonical mapping.

use crate::error::{self, Result, check};
use crate::ffi;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::ptr;

// ---------------------------------------------------------------------------
// C-string → String conversion (single source of `rnp_buffer_destroy`).
// ---------------------------------------------------------------------------

/// Convert a librnp-returned NUL-terminated string into an owned [`String`]
/// and free the C buffer.
///
/// Returns `None` if `raw` is null. Use [`cstr_to_string`] when null is an
/// error condition.
///
/// # Safety
///
/// `raw` must be either null or a pointer returned by librnp that the caller
/// is responsible for freeing via `rnp_buffer_destroy`.
pub unsafe fn cstr_to_optional_string(raw: *mut c_char) -> Option<String> {
    if raw.is_null() {
        return None;
    }
    // SAFETY: caller guarantees raw is a valid librnp-owned C string.
    let cstr = unsafe { CStr::from_ptr(raw) };
    // Fast path: if the bytes are valid UTF-8 (the common case for
    // algorithm names, keyids, fingerprints, etc.) skip the lossy
    // conversion's separate validation+alloc pass.
    let s = match cstr.to_str() {
        Ok(valid) => valid.to_owned(),
        Err(_) => cstr.to_string_lossy().into_owned(),
    };
    unsafe { ffi::rnp_buffer_destroy(raw as *mut _) };
    Some(s)
}

/// Like [`cstr_to_optional_string`] but treats null as
/// [`Error::NullPointer`](crate::error::Error::NullPointer).
///
/// # Safety
///
/// See [`cstr_to_optional_string`].
pub unsafe fn cstr_to_string(raw: *mut c_char) -> Result<String> {
    unsafe { cstr_to_optional_string(raw) }.ok_or(error::Error::NullPointer)
}

// ---------------------------------------------------------------------------
// Out-pointer shapes — the canonical wrappers for FFI getters.
// ---------------------------------------------------------------------------

/// Run an FFI getter that writes a NUL-terminated string into an out-param,
/// then convert the result to an owned [`String`] and free the buffer.
///
/// This is the canonical wrapper for the very common C-API shape
/// `rnp_X_get_Y(handle, *mut *mut c_char) -> rnp_result_t`.
///
/// # Safety
///
/// `f` must populate the out-pointer with a librnp-allocated buffer that
/// `rnp_buffer_destroy` can free.
pub fn call_for_string<F>(mut f: F) -> Result<String>
where
    F: FnMut(*mut *mut c_char) -> u32,
{
    let mut raw: *mut c_char = ptr::null_mut();
    let code = f(&mut raw);
    check(code)?;
    // SAFETY: the caller's closure populates `raw` per the FFI contract;
    // cstr_to_string frees it via rnp_buffer_destroy.
    unsafe { cstr_to_string(raw) }
}

/// Like [`call_for_string`] but maps `RNP_ERROR_NOT_FOUND` (and a null
/// out-pointer) to `Ok(None)`. Use for getters that legitimately return
/// "no value" rather than treating it as an error.
pub fn call_for_optional_string<F>(mut f: F) -> Result<Option<String>>
where
    F: FnMut(*mut *mut c_char) -> u32,
{
    let mut raw: *mut c_char = ptr::null_mut();
    let code = f(&mut raw);
    if code == error::NOT_FOUND {
        return Ok(None);
    }
    check(code)?;
    // SAFETY: as above.
    Ok(unsafe { cstr_to_optional_string(raw) })
}

/// Run an FFI getter that writes a `u32` into an out-param.
pub fn call_for_u32<F>(mut f: F) -> Result<u32>
where
    F: FnMut(*mut u32) -> u32,
{
    let mut n: u32 = 0;
    let code = f(&mut n);
    check(code)?;
    Ok(n)
}

/// Run an FFI getter that writes a `u64` into an out-param.
pub fn call_for_u64<F>(mut f: F) -> Result<u64>
where
    F: FnMut(*mut u64) -> u32,
{
    let mut n: u64 = 0;
    let code = f(&mut n);
    check(code)?;
    Ok(n)
}

/// Run an FFI getter that writes a `usize` into an out-param.
pub fn call_for_usize<F>(mut f: F) -> Result<usize>
where
    F: FnMut(*mut usize) -> u32,
{
    let mut n: usize = 0;
    let code = f(&mut n);
    check(code)?;
    Ok(n)
}

/// Run an FFI getter that writes a `bool` into an out-param.
pub fn call_for_bool<F>(mut f: F) -> Result<bool>
where
    F: FnMut(*mut bool) -> u32,
{
    let mut b: bool = false;
    let code = f(&mut b);
    check(code)?;
    Ok(b)
}

/// Run an FFI call that allocates a byte buffer (`*mut u8` + `len`) that the
/// caller must free via `rnp_buffer_destroy`. Returns the bytes as an owned
/// `Vec<u8>`.
pub fn call_for_owned_bytes<F>(mut f: F) -> Result<Vec<u8>>
where
    F: FnMut(*mut *mut u8, *mut usize) -> u32,
{
    let mut ptr: *mut u8 = ptr::null_mut();
    let mut len: usize = 0;
    let code = f(&mut ptr, &mut len);
    check(code)?;
    if ptr.is_null() {
        return Ok(Vec::new());
    }
    // SAFETY: caller's closure populates `ptr`/`len` per the FFI contract;
    // we copy the bytes into a Vec and destroy the C buffer.
    unsafe {
        let v = std::slice::from_raw_parts(ptr, len).to_vec();
        ffi::rnp_buffer_destroy(ptr as *mut _);
        Ok(v)
    }
}
