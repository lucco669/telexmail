//! Raw FFI bindings to `librnp`, re-exported from the [`rnp_sys`] crate.
//!
//! The contents of this module are entirely machine-generated and mirror the
//! upstream C API verbatim, including its naming conventions — see the
//! `rnp-sys` crate for generation details (bindgen at build time, or the
//! pregenerated file when the headers are known to match). Kept as a module
//! so the safe wrappers' `crate::ffi` paths stay stable.

pub use rnp_sys::*;
