//! Version helpers.
//!
//! The C-side `rnp_version_string()` / `rnp_version_string_full()` /
//! `rnp_backend_string()` / `rnp_backend_version()` return *static* C
//! strings — they must NOT be passed to `rnp_buffer_destroy`. We convert
//! them at the boundary with a small local helper that copies without
//! freeing.

use crate::ffi;
use std::ffi::CStr;
use std::os::raw::c_char;

/// Copy a static librnp-owned C string into an owned [`String`]. The
/// pointer is read once and never freed — safe for the version/backend
/// strings.
///
/// # Safety
///
/// `ptr` must be either null or a valid NUL-terminated C string owned by
/// librnp that lives for the program duration.
unsafe fn copy_static_cstr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    // SAFETY: caller guarantees ptr points at a valid C string.
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

/// librnp version as a packed `u32`. Use [`decompose`] to read fields.
pub fn version() -> u32 {
    unsafe { ffi::rnp_version() }
}

/// Compose a version `u32` from major.minor.patch.
pub fn version_for(major: u32, minor: u32, patch: u32) -> u32 {
    unsafe { ffi::rnp_version_for(major, minor, patch) }
}

/// Major component of a version packed by [`version`] / [`version_for`].
pub fn version_major(v: u32) -> u32 {
    unsafe { ffi::rnp_version_major(v) }
}

/// Minor component.
pub fn version_minor(v: u32) -> u32 {
    unsafe { ffi::rnp_version_minor(v) }
}

/// Patch component.
pub fn version_patch(v: u32) -> u32 {
    unsafe { ffi::rnp_version_patch(v) }
}

/// `(major, minor, patch)` of the linked librnp.
pub fn decompose() -> (u32, u32, u32) {
    let v = version();
    (version_major(v), version_minor(v), version_patch(v))
}

/// Short version string (e.g. `"0.18.1"`).
pub fn version_string() -> String {
    unsafe { copy_static_cstr(ffi::rnp_version_string()) }
}

/// Full version string with backend / git info.
pub fn version_string_full() -> String {
    unsafe { copy_static_cstr(ffi::rnp_version_string_full()) }
}

/// Build commit timestamp.
pub fn version_commit_timestamp() -> u64 {
    unsafe { ffi::rnp_version_commit_timestamp() }
}

/// Crypto backend identifier string (e.g. `"botan"`).
pub fn backend_string() -> String {
    unsafe { copy_static_cstr(ffi::rnp_backend_string()) }
}

/// Crypto backend version string.
pub fn backend_version() -> String {
    unsafe { copy_static_cstr(ffi::rnp_backend_version()) }
}

/// Enable librnp debug output. Wraps `rnp_enable_debug`.
pub fn enable_debug(file: &str) {
    if let Ok(c) = std::ffi::CString::new(file) {
        unsafe {
            let _ = ffi::rnp_enable_debug(c.as_ptr());
        }
    }
}

/// Disable librnp debug output. Wraps `rnp_disable_debug`.
pub fn disable_debug() {
    unsafe {
        let _ = ffi::rnp_disable_debug();
    }
}
