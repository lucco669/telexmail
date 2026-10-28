//! [`VerifyOp`] — builder over `rnp_op_verify_*`.

use crate::context::Context;
use crate::error::{Result, check};
use crate::ffi;
use crate::ops::{Input, Output};
use std::marker::PhantomData;
use std::ptr;

use super::VerifyResult;

/// Flags for [`VerifyOp::set_flags`]. Wraps `RNP_VERIFY_*`.
#[derive(Clone, Copy, Debug, Default)]
pub struct VerifyFlags(pub u32);

impl VerifyFlags {
    /// Don't inspect signatures during decrypt.
    pub const IGNORE_SIGS_ON_DECRYPT: Self = Self(ffi::RNP_VERIFY_IGNORE_SIGS_ON_DECRYPT as u32);
    /// Require all signatures to verify.
    pub const REQUIRE_ALL_SIGS: Self = Self(ffi::RNP_VERIFY_REQUIRE_ALL_SIGS as u32);
    /// Allow hidden (all-zero keyid) recipients.
    pub const ALLOW_HIDDEN_RECIPIENT: Self = Self(ffi::RNP_VERIFY_ALLOW_HIDDEN_RECIPIENT as u32);

    pub fn bits(self) -> u32 {
        self.0
    }
}

impl std::ops::BitOr for VerifyFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Builder over `rnp_op_verify_*`. Construct for inline or detached
/// verification, optionally set flags, then call [`VerifyOp::execute`] to
/// obtain a [`VerifyResult`].
///
/// ## Handle ownership
///
/// Upstream does **not** destroy an op's inputs or output —
/// `~rnp_op_verify_st` frees only its recipient/symenc bookkeeping, and the
/// canonical C examples destroy the op first and the streams after. This
/// builder therefore owns the inputs and the output itself, hands them to
/// [`VerifyResult`] on success, and destroys them in the canonical order
/// (op first, then streams) on every path, including drops without
/// [`VerifyOp::execute`].
pub struct VerifyOp<'ctx> {
    ctx: &'ctx Context,
    op: ffi::rnp_op_verify_t,
    /// The message input, plus the detached-signature input when detached.
    inputs: Vec<Input>,
    // Kept alive so it outlives the op, mirroring the C examples.
    _output: Option<Output>,
    _phantom: PhantomData<&'ctx ()>,
}

impl<'ctx> VerifyOp<'ctx> {
    /// Begin inline verification. `signed_message` is the message produced
    /// by inline signing — anything message-shaped: a byte slice or a
    /// caller-built [`Input`] (e.g. from
    /// [`Input::from_reader`](crate::Input::from_reader), to verify a
    /// streamed message; consumed when the op executes). `output` is where
    /// the embedded plaintext will be written (use `Output::to_null()` to
    /// discard).
    pub fn inline<'s>(
        ctx: &'ctx Context,
        signed_message: impl Into<crate::ops::MessageSource<'s>>,
        output: Output,
    ) -> Result<Self> {
        let mut source = signed_message.into();
        Self::create_inline(ctx, source.0.take()?, output)
    }

    /// As [`VerifyOp::inline`], over a caller-built [`Input`]. Deprecated:
    /// pass the [`Input`] to [`VerifyOp::inline`] — it accepts both.
    #[deprecated(
        since = "0.2.0",
        note = "pass the Input to VerifyOp::inline; it accepts both"
    )]
    pub fn inline_with_input(ctx: &'ctx Context, input: Input, output: Output) -> Result<Self> {
        Self::create_inline(ctx, input, output)
    }

    fn create_inline(ctx: &'ctx Context, input: Input, output: Output) -> Result<Self> {
        let mut op: ffi::rnp_op_verify_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_op_verify_create(
                &mut op,
                ctx.ffi,
                input.as_ptr(),
                output.as_ptr(),
            ))?;
        }
        Ok(VerifyOp {
            ctx,
            op,
            inputs: vec![input],
            _output: Some(output),
            _phantom: PhantomData,
        })
    }

    /// Begin detached verification. `message` and `signature` are each
    /// anything message-shaped: byte slices or caller-built [`Input`]s
    /// (e.g. from [`Input::from_reader`](crate::Input::from_reader), to
    /// verify streamed data; consumed when the op executes). `signature`
    /// is the detached signature over `message`.
    pub fn detached<'m, 's>(
        ctx: &'ctx Context,
        message: impl Into<crate::ops::MessageSource<'m>>,
        signature: impl Into<crate::ops::MessageSource<'s>>,
    ) -> Result<Self> {
        let mut msg_src = message.into();
        let mut sig_src = signature.into();
        Self::create_detached(ctx, msg_src.0.take()?, sig_src.0.take()?)
    }

    /// As [`VerifyOp::detached`], over caller-built [`Input`]s. Deprecated:
    /// pass the [`Input`]s to [`VerifyOp::detached`] — it accepts both.
    #[deprecated(
        since = "0.2.0",
        note = "pass the Inputs to VerifyOp::detached; it accepts both"
    )]
    pub fn detached_with_input(
        ctx: &'ctx Context,
        message: Input,
        signature: Input,
    ) -> Result<Self> {
        Self::create_detached(ctx, message, signature)
    }

    fn create_detached(ctx: &'ctx Context, message: Input, signature: Input) -> Result<Self> {
        let null_out = Output::to_null()?;
        let mut op: ffi::rnp_op_verify_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_op_verify_detached_create(
                &mut op,
                ctx.ffi,
                message.as_ptr(),
                signature.as_ptr(),
            ))?;
        }
        Ok(VerifyOp {
            ctx,
            op,
            inputs: vec![message, signature],
            _output: Some(null_out),
            _phantom: PhantomData,
        })
    }

    pub fn set_flags(&mut self, flags: VerifyFlags) -> Result<()> {
        unsafe { check(ffi::rnp_op_verify_set_flags(self.op, flags.bits())) }
    }

    /// Execute the verification, returning the result for inspection.
    /// Ownership of the op's inputs and output moves into the result so
    /// the streams outlive the op handle.
    ///
    /// If a reader-backed input fails mid-verification, the returned error
    /// is the original [`std::io::Error`] rather than librnp's generic
    /// read/write code.
    pub fn execute(self) -> Result<VerifyResult<'ctx>> {
        let (result, status) = self.execute_inspect();
        status?;
        Ok(result)
    }

    /// Preserve owned metadata even when validation fails. The status remains mandatory:
    /// callers must not treat output as authentic solely because metadata is available.
    pub fn execute_inspect(mut self) -> (VerifyResult<'ctx>, Result<()>) {
        let mut exec = unsafe { check(ffi::rnp_op_verify_execute(self.op)) };
        if exec.is_err() {
            if let Some(io) = self
                .inputs
                .iter_mut()
                .find_map(|input| input.take_io_error())
            {
                exec = Err(io.into());
            }
        }
        let result = VerifyResult {
            ctx: self.ctx,
            op: std::mem::replace(&mut self.op, ptr::null_mut()),
            _inputs: std::mem::take(&mut self.inputs),
            _output: self._output.take(),
            _phantom: PhantomData,
        };
        (result, exec)
    }
}

impl Drop for VerifyOp<'_> {
    fn drop(&mut self) {
        if !self.op.is_null() {
            // SAFETY: op was created by rnp_op_verify_*_create and not yet
            // destroyed. Destroying it first matches the canonical C
            // ordering; the input/output fields drop afterwards.
            unsafe {
                let _ = ffi::rnp_op_verify_destroy(self.op);
            }
            self.op = ptr::null_mut();
        }
    }
}
