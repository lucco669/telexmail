//! Keyring-level operations on [`Context`]: save, unload,
//! import, homedir discovery, and key counts.

use crate::context::Context;
use crate::error::{self, Result, check};
use crate::ffi;
use crate::ffi_safe::{call_for_string, call_for_usize};
use crate::key::{KeyIdentifier, LoadSaveFlags, UnloadFlags};
use crate::ops::Input;
use std::ffi::CString;
use std::os::raw::c_char;
use std::path::PathBuf;
use std::ptr;

impl Context {
    /// Save the keyring's keys to `bytes` (returned as a writer-driven
    /// memory output). For simple cases use [`Context::save_keys_to_memory`].
    pub fn save_keys(
        &self,
        format: crate::context::KeyringFormat,
        flags: LoadSaveFlags,
        output: &mut crate::Output,
    ) -> Result<()> {
        let fmt_c = CString::new(format.as_str()).unwrap();
        unsafe {
            check(ffi::rnp_save_keys(
                self.ffi,
                fmt_c.as_ptr(),
                output.as_ptr(),
                flags.bits(),
            ))
        }
    }

    /// Save the keyring to a memory buffer.
    pub fn save_keys_to_memory(
        &self,
        format: crate::context::KeyringFormat,
        flags: LoadSaveFlags,
    ) -> Result<Vec<u8>> {
        let mut output = crate::Output::to_memory()?;
        self.save_keys(format, flags, &mut output)?;
        output.into_bytes()
    }

    /// Remove all keys from the in-memory keyring.
    pub fn unload_keys(&self, flags: UnloadFlags) -> Result<()> {
        unsafe { check(ffi::rnp_unload_keys(self.ffi, flags.bits())) }
    }

    /// Import keys (binary or armored — anything message-shaped: a byte
    /// slice or a caller-built [`Input`], e.g. from
    /// [`Input::from_reader`](crate::Input::from_reader) to stream key
    /// material from a file or socket). Returns the import status JSON
    /// (counts of new / updated / unchanged keys).
    pub fn import_keys<'s>(
        &self,
        source: impl Into<crate::ops::MessageSource<'s>>,
        flags: crate::key::LoadSaveFlags,
    ) -> Result<String> {
        let mut source = source.into();
        let input = source.0.take()?;
        self.import_keys_input(&input, flags)
    }

    fn import_keys_input(&self, input: &Input, flags: crate::key::LoadSaveFlags) -> Result<String> {
        call_for_string(|raw| unsafe {
            ffi::rnp_import_keys(self.ffi, input.as_ptr(), flags.bits(), raw)
        })
    }

    /// As [`Context::import_keys`], over a caller-built [`Input`].
    /// Deprecated: pass the [`Input`] to [`Context::import_keys`] — it
    /// accepts both.
    #[deprecated(
        since = "0.2.0",
        note = "pass the &Input to Context::import_keys; it accepts both"
    )]
    pub fn import_keys_from_input(
        &self,
        input: &Input,
        flags: crate::key::LoadSaveFlags,
    ) -> Result<String> {
        self.import_keys_input(input, flags)
    }

    /// Import signatures (anything message-shaped: a byte slice or a
    /// caller-built [`Input`]). Returns the status JSON.
    pub fn import_signatures<'s>(
        &self,
        source: impl Into<crate::ops::MessageSource<'s>>,
        flags: u32,
    ) -> Result<String> {
        let mut source = source.into();
        let input = source.0.take()?;
        self.import_signatures_input(&input, flags)
    }

    fn import_signatures_input(&self, input: &Input, flags: u32) -> Result<String> {
        call_for_string(|raw| unsafe {
            ffi::rnp_import_signatures(self.ffi, input.as_ptr(), flags, raw)
        })
    }

    /// As [`Context::import_signatures`], over a caller-built [`Input`].
    /// Deprecated: pass the [`Input`] to [`Context::import_signatures`] —
    /// it accepts both.
    #[deprecated(
        since = "0.2.0",
        note = "pass the &Input to Context::import_signatures; it accepts both"
    )]
    pub fn import_signatures_from_input(&self, input: &Input, flags: u32) -> Result<String> {
        self.import_signatures_input(input, flags)
    }

    /// Number of public keys currently loaded.
    pub fn public_key_count(&self) -> Result<usize> {
        call_for_usize(|out| unsafe { ffi::rnp_get_public_key_count(self.ffi, out) })
    }

    /// Number of secret keys currently loaded.
    pub fn secret_key_count(&self) -> Result<usize> {
        call_for_usize(|out| unsafe { ffi::rnp_get_secret_key_count(self.ffi, out) })
    }

    /// Default OpenPGP home directory (typically `~/.gnupg` or
    /// `$GNUPGHOME`).
    pub fn default_homedir() -> Result<PathBuf> {
        let s = call_for_string(|out| unsafe { ffi::rnp_get_default_homedir(out) })?;
        Ok(PathBuf::from(s))
    }

    /// Detect the keyring format of an existing homedir. Returns
    /// `(public_format, public_path, secret_format, secret_path)`; any of
    /// these may be empty if not detected.
    pub fn detect_homedir_info(path: &str) -> Result<(String, String, String, String)> {
        let c = CString::new(path).map_err(|_| error::Error::PathNul)?;
        let mut pub_fmt: *mut c_char = ptr::null_mut();
        let mut pub_path: *mut c_char = ptr::null_mut();
        let mut sec_fmt: *mut c_char = ptr::null_mut();
        let mut sec_path: *mut c_char = ptr::null_mut();
        unsafe {
            check(ffi::rnp_detect_homedir_info(
                c.as_ptr(),
                &mut pub_fmt,
                &mut pub_path,
                &mut sec_fmt,
                &mut sec_path,
            ))?;
            let pf = crate::ops::cstr_to_optional_string(pub_fmt).unwrap_or_default();
            let pp = crate::ops::cstr_to_optional_string(pub_path).unwrap_or_default();
            let sf = crate::ops::cstr_to_optional_string(sec_fmt).unwrap_or_default();
            let sp = crate::ops::cstr_to_optional_string(sec_path).unwrap_or_default();
            Ok((pf, pp, sf, sp))
        }
    }

    /// Detect the format of a keyring blob (`"GPG"`, `"KBX"`, `"JSON"`).
    pub fn detect_key_format(bytes: &[u8]) -> Result<String> {
        call_for_string(|out| unsafe {
            ffi::rnp_detect_key_format(bytes.as_ptr(), bytes.len(), out)
        })
    }

    /// Iterate over identifiers of all keys in the keyring. See
    /// [`IdentifierIterator`].
    pub fn identifiers(&self, kind: IdentifierKind) -> Result<IdentifierIterator<'_>> {
        IdentifierIterator::new(self, kind)
    }
}

/// Which identifier to enumerate over a keyring.
#[derive(Clone, Copy, Debug)]
pub enum IdentifierKind {
    Userid,
    Keyid,
    Grip,
    Fingerprint,
}

impl IdentifierKind {
    fn as_str(self) -> &'static str {
        match self {
            IdentifierKind::Userid => "userid",
            IdentifierKind::Keyid => "keyid",
            IdentifierKind::Grip => "grip",
            IdentifierKind::Fingerprint => "fingerprint",
        }
    }
}

/// Iterator over keyring identifiers — wraps `rnp_identifier_iterator_*`.
pub struct IdentifierIterator<'ctx> {
    handle: ffi::rnp_identifier_iterator_t,
    _ctx: std::marker::PhantomData<&'ctx Context>,
}

impl<'ctx> IdentifierIterator<'ctx> {
    fn new(ctx: &'ctx Context, kind: IdentifierKind) -> Result<Self> {
        let kind_c = CString::new(kind.as_str()).unwrap();
        let mut handle: ffi::rnp_identifier_iterator_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_identifier_iterator_create(
                ctx.ffi,
                &mut handle,
                kind_c.as_ptr(),
            ))?;
        }
        if handle.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(IdentifierIterator {
            handle,
            _ctx: std::marker::PhantomData,
        })
    }
}

impl<'ctx> Iterator for IdentifierIterator<'ctx> {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        // Per the C docstring: the returned buffer is owned by the iterator
        // ("its life is tied to the iterator" / "should not be freed by the
        // application"). Copy the bytes and leave the original alone.
        let mut raw: *const c_char = ptr::null();
        let code = unsafe { ffi::rnp_identifier_iterator_next(self.handle, &mut raw) };
        if code != error::SUCCESS || raw.is_null() {
            return None;
        }
        unsafe {
            let cs = std::ffi::CStr::from_ptr(raw);
            Some(cs.to_string_lossy().into_owned())
        }
    }
}

impl<'ctx> Drop for IdentifierIterator<'ctx> {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                let _ = ffi::rnp_identifier_iterator_destroy(self.handle);
            }
            self.handle = ptr::null_mut();
        }
    }
}

// Re-export KeyIdentifier for symmetry.
#[allow(unused_imports)]
use KeyIdentifier as _KeyIdentifierAlias;
