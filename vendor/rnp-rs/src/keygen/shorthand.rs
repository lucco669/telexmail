//! One-call key generation shortcuts over the `rnp_generate_key_*` family.
//!
//! These mirror the C convenience functions for callers migrating from the
//! C API. For anything configurable (protection, preferences, subkeys,
//! expiry) use [`KeyBuilder`](crate::KeyBuilder) — it drives the richer
//! `rnp_op_generate_*` surface.
//!
//! All functions here generate **unprotected** keys; call
//! [`Key::protect`](crate::Key::protect) afterwards if needed.

use crate::context::Context;
use crate::error::{Result, check};
use crate::ffi;
use crate::key::Key;
use std::ffi::CString;
use std::os::raw::c_char;
use std::ptr;

/// Generate an RSA keypair: `bits` for the primary, `subbits` for the
/// encryption subkey (0 = no subkey). Wraps `rnp_generate_key_rsa`.
pub fn generate_key_rsa<'ctx>(
    ctx: &'ctx Context,
    bits: u32,
    subbits: u32,
    userid: &str,
) -> Result<Key<'ctx>> {
    let uid = CString::new(userid).map_err(|_| crate::error::Error::NulByte)?;
    let mut handle: ffi::rnp_key_handle_t = ptr::null_mut();
    unsafe {
        check(ffi::rnp_generate_key_rsa(
            ctx.ffi,
            bits,
            subbits,
            uid.as_ptr(),
            null_password(),
            &mut handle,
        ))?;
    }
    materialize(handle)
}

/// Generate a DSA primary + ElGamal subkey pair. Wraps
/// `rnp_generate_key_dsa_eg`.
pub fn generate_key_dsa_eg<'ctx>(
    ctx: &'ctx Context,
    bits: u32,
    subbits: u32,
    userid: &str,
) -> Result<Key<'ctx>> {
    let uid = CString::new(userid).map_err(|_| crate::error::Error::NulByte)?;
    let mut handle: ffi::rnp_key_handle_t = ptr::null_mut();
    unsafe {
        check(ffi::rnp_generate_key_dsa_eg(
            ctx.ffi,
            bits,
            subbits,
            uid.as_ptr(),
            null_password(),
            &mut handle,
        ))?;
    }
    materialize(handle)
}

/// Generate an EC keypair on `curve` (e.g. `"NIST P-256"`, `"Curve25519"`).
/// Wraps `rnp_generate_key_ec`.
pub fn generate_key_ec<'ctx>(ctx: &'ctx Context, curve: &str, userid: &str) -> Result<Key<'ctx>> {
    let uid = CString::new(userid).map_err(|_| crate::error::Error::NulByte)?;
    let curve = CString::new(curve).map_err(|_| crate::error::Error::NulByte)?;
    let mut handle: ffi::rnp_key_handle_t = ptr::null_mut();
    unsafe {
        check(ffi::rnp_generate_key_ec(
            ctx.ffi,
            curve.as_ptr(),
            uid.as_ptr(),
            null_password(),
            &mut handle,
        ))?;
    }
    materialize(handle)
}

/// Generate an SM2 keypair. Wraps `rnp_generate_key_sm2`.
pub fn generate_key_sm2<'ctx>(ctx: &'ctx Context, userid: &str) -> Result<Key<'ctx>> {
    let uid = CString::new(userid).map_err(|_| crate::error::Error::NulByte)?;
    let mut handle: ffi::rnp_key_handle_t = ptr::null_mut();
    unsafe {
        check(ffi::rnp_generate_key_sm2(
            ctx.ffi,
            uid.as_ptr(),
            null_password(),
            &mut handle,
        ))?;
    }
    materialize(handle)
}

/// Generate an Ed25519 / X25519 keypair. Wraps `rnp_generate_key_25519`.
pub fn generate_key_25519<'ctx>(ctx: &'ctx Context, userid: &str) -> Result<Key<'ctx>> {
    let uid = CString::new(userid).map_err(|_| crate::error::Error::NulByte)?;
    let mut handle: ffi::rnp_key_handle_t = ptr::null_mut();
    unsafe {
        check(ffi::rnp_generate_key_25519(
            ctx.ffi,
            uid.as_ptr(),
            null_password(),
            &mut handle,
        ))?;
    }
    materialize(handle)
}

/// The generic generator: primary algorithm/bits/curve plus an optional
/// subkey, all as algorithm names as accepted by librnp. `None` arguments
/// are passed as NULL to the C API. Wraps `rnp_generate_key_ex`.
#[allow(clippy::too_many_arguments)]
pub fn generate_key_ex<'ctx>(
    ctx: &'ctx Context,
    key_alg: Option<&str>,
    key_bits: u32,
    key_curve: Option<&str>,
    sub_alg: Option<&str>,
    sub_bits: u32,
    sub_curve: Option<&str>,
    userid: &str,
) -> Result<Key<'ctx>> {
    let key_alg = cstr_opt(key_alg)?;
    let key_curve = cstr_opt(key_curve)?;
    let sub_alg = cstr_opt(sub_alg)?;
    let sub_curve = cstr_opt(sub_curve)?;
    let uid = CString::new(userid).map_err(|_| crate::error::Error::NulByte)?;
    let mut handle: ffi::rnp_key_handle_t = ptr::null_mut();
    unsafe {
        check(ffi::rnp_generate_key_ex(
            ctx.ffi,
            key_alg.as_ref().map_or(ptr::null(), |c| c.as_ptr()),
            sub_alg.as_ref().map_or(ptr::null(), |c| c.as_ptr()),
            key_bits,
            sub_bits,
            key_curve.as_ref().map_or(ptr::null(), |c| c.as_ptr()),
            sub_curve.as_ref().map_or(ptr::null(), |c| c.as_ptr()),
            uid.as_ptr(),
            null_password(),
            &mut handle,
        ))?;
    }
    materialize(handle)
}

fn cstr_opt(s: Option<&str>) -> Result<Option<CString>> {
    s.map(|v| CString::new(v).map_err(|_| crate::error::Error::NulByte))
        .transpose()
}

fn null_password() -> *const c_char {
    ptr::null()
}

/// The handle borrows the context that created it; the caller picks the
/// lifetime (unified with its `&Context` borrow at each call site).
fn materialize<'ctx>(handle: ffi::rnp_key_handle_t) -> Result<Key<'ctx>> {
    if handle.is_null() {
        return Err(crate::error::Error::NullPointer);
    }
    Ok(Key::from_handle(handle))
}
