//! [`VerifyResult`] — owns the op handle post-execute, exposes per-sig /
//! per-recipient / per-symenc / file-info / format / protection-info.

use crate::context::Context;
use crate::error::{self, Result, check};
use crate::ffi;
use crate::ffi_safe::{call_for_usize, cstr_to_optional_string, cstr_to_string};
use crate::ops::{Input, Output};
use std::marker::PhantomData;
use std::os::raw::c_char;
use std::ptr;

use super::{FileInfo, Recipient, Symenc, VerifySignature};

/// Result of a verify (or decrypt) operation. Owns the verify-op handle
/// until drop; methods read out the per-signature / per-recipient state.
///
/// It also owns the op's inputs and output, moved here by
/// [`VerifyOp::execute`](super::VerifyOp::execute). `Drop` destroys the op
/// handle first and the streams after — the canonical upstream ordering —
/// so nothing the op references is freed early.
pub struct VerifyResult<'ctx> {
    pub(crate) ctx: &'ctx Context,
    pub(crate) op: ffi::rnp_op_verify_t,
    pub(crate) _inputs: Vec<Input>,
    pub(crate) _output: Option<Output>,
    pub(crate) _phantom: PhantomData<&'ctx ()>,
}

impl<'ctx> VerifyResult<'ctx> {
    /// Borrow the underlying context (for follow-up `find_key` etc.).
    pub fn context(&self) -> &Context {
        self.ctx
    }

    /// Number of signatures found in the verified stream.
    pub fn signature_count(&self) -> Result<usize> {
        call_for_usize(|out| unsafe { ffi::rnp_op_verify_get_signature_count(self.op, out) })
    }

    /// Borrow the per-signature result at `idx`.
    pub fn signature_at(&self, idx: usize) -> Result<Option<VerifySignature<'_>>> {
        let mut handle: ffi::rnp_op_verify_signature_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_op_verify_get_signature_at(
                self.op,
                idx,
                &mut handle,
            ))?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(VerifySignature::new(handle)))
        }
    }

    /// All per-signature results.
    pub fn signatures(&self) -> Result<Vec<VerifySignature<'_>>> {
        let n = self.signature_count()?;
        (0..n)
            .map(|i| self.signature_at(i)?.ok_or(error::Error::NullPointer))
            .collect()
    }

    /// Lazy iterator over per-signature results. Skips holes (returns
    /// `None` for any index where librnp returns no handle) — use the
    /// strict [`Self::signatures`] collector if holes are unexpected.
    pub fn iter_signatures(&self) -> impl Iterator<Item = VerifySignature<'_>> + '_ {
        let n = self.signature_count().unwrap_or(0);
        (0..n).filter_map(move |i| self.signature_at(i).ok().flatten())
    }

    /// True iff at least one signature verified successfully.
    pub fn any_valid(&self) -> Result<bool> {
        let sigs = self.signatures()?;
        Ok(sigs.iter().any(|s| s.status_is_valid()))
    }

    /// Number of public-key recipients in the verified stream.
    pub fn recipient_count(&self) -> Result<usize> {
        call_for_usize(|out| unsafe { ffi::rnp_op_verify_get_recipient_count(self.op, out) })
    }

    pub fn recipient_at(&self, idx: usize) -> Result<Option<Recipient<'_>>> {
        let mut handle: ffi::rnp_recipient_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_op_verify_get_recipient_at(
                self.op,
                idx,
                &mut handle,
            ))?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(Recipient::new(handle)))
        }
    }

    pub fn recipients(&self) -> Result<Vec<Recipient<'_>>> {
        let n = self.recipient_count()?;
        (0..n)
            .map(|i| self.recipient_at(i)?.ok_or(error::Error::NullPointer))
            .collect()
    }

    /// The recipient that was actually used for decryption, if any.
    pub fn used_recipient(&self) -> Result<Option<Recipient<'_>>> {
        let mut handle: ffi::rnp_recipient_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_op_verify_get_used_recipient(self.op, &mut handle))?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(Recipient::new(handle)))
        }
    }

    /// Number of symmetric-only encrypted session keys (password-based
    /// recipients) in the verified stream.
    pub fn symenc_count(&self) -> Result<usize> {
        call_for_usize(|out| unsafe { ffi::rnp_op_verify_get_symenc_count(self.op, out) })
    }

    pub fn symenc_at(&self, idx: usize) -> Result<Option<Symenc<'_>>> {
        let mut handle: ffi::rnp_symenc_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_op_verify_get_symenc_at(self.op, idx, &mut handle))?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(Symenc::new(handle)))
        }
    }

    pub fn symencs(&self) -> Result<Vec<Symenc<'_>>> {
        let n = self.symenc_count()?;
        (0..n)
            .map(|i| self.symenc_at(i)?.ok_or(error::Error::NullPointer))
            .collect()
    }

    /// The symenc that was actually used for decryption, if any.
    pub fn used_symenc(&self) -> Result<Option<Symenc<'_>>> {
        let mut handle: ffi::rnp_symenc_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_op_verify_get_used_symenc(self.op, &mut handle))?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(Symenc::new(handle)))
        }
    }

    /// File metadata embedded in the literal-data packet, if any.
    pub fn file_info(&self) -> Result<Option<FileInfo>> {
        let mut name_raw: *mut c_char = ptr::null_mut();
        let mut mtime: u32 = 0;
        unsafe {
            let code = ffi::rnp_op_verify_get_file_info(self.op, &mut name_raw, &mut mtime);
            if code == error::NOT_FOUND {
                return Ok(None);
            }
            check(code)?;
            let name = cstr_to_optional_string(name_raw).unwrap_or_default();
            Ok(Some(FileInfo { name, mtime }))
        }
    }

    /// Format character of the verified message (`'b'` binary, `'t'` text,
    /// `'u'` UTF-8, etc. — see RFC 4880 §5.9). Returns `'\0'` if unknown.
    pub fn format(&self) -> Result<char> {
        let mut c: std::os::raw::c_char = 0;
        unsafe {
            check(ffi::rnp_op_verify_get_format(self.op, &mut c))?;
        }
        Ok(c as u8 as char)
    }

    /// Protection info `(mode, cipher, valid)` of the encrypted stream.
    /// `mode` is one of `"none"`, `"cfb"`, `"cfb-mdc"`, `"aead-ocb"`,
    /// `"aead-eax"`. `valid` is true iff MDC/AEAD integrity was verified.
    pub fn protection_info(&self) -> Result<(String, String, bool)> {
        let mut mode_raw: *mut c_char = ptr::null_mut();
        let mut cipher_raw: *mut c_char = ptr::null_mut();
        let mut valid: bool = false;
        unsafe {
            check(ffi::rnp_op_verify_get_protection_info(
                self.op,
                &mut mode_raw,
                &mut cipher_raw,
                &mut valid,
            ))?;
            let mode = cstr_to_string(mode_raw)?;
            let cipher = cstr_to_string(cipher_raw)?;
            Ok((mode, cipher, valid))
        }
    }
}

impl<'ctx> Drop for VerifyResult<'ctx> {
    fn drop(&mut self) {
        if !self.op.is_null() {
            unsafe {
                let _ = ffi::rnp_op_verify_destroy(self.op);
            }
            self.op = ptr::null_mut();
        }
    }
}
