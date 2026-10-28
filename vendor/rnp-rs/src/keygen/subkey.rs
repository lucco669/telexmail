//! [`SubkeyBuilder`] — builder over `rnp_op_generate_subkey_*`.

use crate::algorithm::{Algorithm, Curve, Hash, KeyUsage};
use crate::context::Context;
use crate::error::{self, Result, check};
use crate::ffi;
use crate::key::Key;
use std::ffi::CString;
use std::ptr;

use super::ProtectConfig;

/// Builder for generating a subkey off an existing primary key.
///
/// Wraps `rnp_op_generate_subkey_create`. Same setter surface as
/// [`KeyBuilder`](super::KeyBuilder) minus `userid` and `v6` (those are
/// primary-only).
pub struct SubkeyBuilder {
    pub(crate) alg_str: String,
    pub(crate) bits: Option<u32>,
    pub(crate) hash: Option<Hash>,
    pub(crate) dsa_qbits: Option<u32>,
    pub(crate) curve: Option<Curve>,
    pub(crate) expiration: Option<u32>,
    pub(crate) usages: Vec<KeyUsage>,
    pub(crate) protection: Option<ProtectConfig>,
    pub(crate) request_password: bool,
}

impl SubkeyBuilder {
    pub fn new(alg: Algorithm) -> Self {
        Self::new_str(alg.as_str())
    }

    /// Create a subkey builder from a raw algorithm name string.
    /// Use this for algorithms not in the [`Algorithm`] enum (e.g.
    /// PQC composite algorithms under the `pqc` feature).
    pub fn new_str(alg: &str) -> Self {
        SubkeyBuilder {
            alg_str: alg.to_string(),
            bits: None,
            hash: None,
            dsa_qbits: None,
            curve: None,
            expiration: None,
            usages: Vec::new(),
            protection: None,
            request_password: false,
        }
    }

    pub fn bits(mut self, n: u32) -> Self {
        self.bits = Some(n);
        self
    }

    pub fn hash(mut self, h: Hash) -> Self {
        self.hash = Some(h);
        self
    }

    pub fn dsa_qbits(mut self, n: u32) -> Self {
        self.dsa_qbits = Some(n);
        self
    }

    pub fn curve(mut self, c: Curve) -> Self {
        self.curve = Some(c);
        self
    }

    pub fn expiration(mut self, seconds: u32) -> Self {
        self.expiration = Some(seconds);
        self
    }

    pub fn add_usage(mut self, u: KeyUsage) -> Self {
        self.usages.push(u);
        self
    }

    /// Execute the subkey generation. The returned [`Key`] borrows `ctx`.
    pub fn build<'ctx>(self, ctx: &'ctx Context, primary: &Key<'_>) -> Result<Key<'ctx>> {
        self.build_with_ffi(ctx, primary.handle)
    }

    /// Internal: build against a raw handle (used by `KeyBuilder::build`
    /// for inline subkey creation).
    pub(crate) fn build_with_ffi<'ctx>(
        self,
        ctx: &'ctx Context,
        primary_handle: ffi::rnp_key_handle_t,
    ) -> Result<Key<'ctx>> {
        let alg_c = CString::new(self.alg_str.as_str()).unwrap();
        let mut op: ffi::rnp_op_generate_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_op_generate_subkey_create(
                &mut op,
                ctx.ffi,
                primary_handle,
                alg_c.as_ptr(),
            ))?;
        }

        if let Err(e) = unsafe { apply_subkey_setters(op, &self) } {
            unsafe {
                let _ = ffi::rnp_op_generate_destroy(op);
            }
            return Err(e);
        }

        unsafe {
            check(ffi::rnp_op_generate_execute(op))?;
            let mut handle: ffi::rnp_key_handle_t = ptr::null_mut();
            check(ffi::rnp_op_generate_get_key(op, &mut handle))?;
            let _ = ffi::rnp_op_generate_destroy(op);
            if handle.is_null() {
                return Err(error::Error::NullPointer);
            }
            Ok(Key::from_handle(handle))
        }
    }
}

unsafe fn apply_subkey_setters(op: ffi::rnp_op_generate_t, b: &SubkeyBuilder) -> Result<()> {
    unsafe {
        super::apply_generate_common(
            op,
            super::GenerateCommon {
                bits: b.bits,
                hash: b.hash,
                dsa_qbits: b.dsa_qbits,
                curve: b.curve,
                expiration: b.expiration,
                usages: &b.usages,
                protection: b.protection.as_ref(),
                request_password: b.request_password,
            },
        )
    }
}
