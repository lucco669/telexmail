//! Mutation methods on [`Signature`]: export, remove from key.

use crate::error::{Result, check};
use crate::ffi;
use crate::key::{ExportFlags, Key};
use crate::ops::Output;

use super::Signature;

impl<'parent> Signature<'parent> {
    /// Export this signature as raw bytes. Pass `ExportFlags::ARMORED` for
    /// ASCII armor, otherwise binary OpenPGP packets are produced.
    pub fn export(&self, flags: ExportFlags) -> Result<Vec<u8>> {
        let output = Output::to_memory()?;
        unsafe {
            check(ffi::rnp_signature_export(
                self.handle,
                output.as_ptr(),
                flags.bits(),
            ))?;
        }
        output.into_bytes()
    }

    /// Remove this signature from its key. The signature handle is
    /// destroyed by librnp; the Rust wrapper's `Drop` will see a null
    /// handle and skip the destroy.
    ///
    /// After this call, any other handles to the same signature are
    /// invalid.
    pub fn remove_from_key(self, key: &Key<'_>) -> Result<()> {
        unsafe { check(ffi::rnp_signature_remove(key.handle, self.handle))? }
        // The C side destroyed the handle. Forget our Drop so we don't
        // double-destroy.
        let me = std::mem::ManuallyDrop::new(self);
        let _ = &me;
        Ok(())
    }
}
