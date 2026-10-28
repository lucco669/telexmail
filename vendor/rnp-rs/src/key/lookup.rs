//! Context-side keyring lookup: `find_key` and `load_keys`.
//!
//! These methods logically belong to the "key" domain but live on
//! [`Context`](crate::Context) because they query the keyring attached to
//! the FFI instance. Splitting them out of `context.rs` keeps the key
//! surface in one place.

use crate::context::{Context, KeyringFormat};
use crate::error::{self, Result, check};
use crate::ffi;
use crate::ops::Input;
use std::ffi::CString;
use std::ptr;

use super::{Key, KeyIdentifier, LoadSaveFlags};

impl Context {
    /// Locate a key by userid, keyid, fingerprint or grip.
    ///
    /// Returns `Ok(None)` when no key matches (librnp signals this by leaving
    /// the out-handle NULL while returning `RNP_SUCCESS`).
    pub fn find_key(&self, id: KeyIdentifier<'_>) -> Result<Option<Key<'_>>> {
        let type_c = CString::new(id.type_str()).unwrap();
        let value_c = CString::new(id.value_str()).map_err(|_| error::Error::NulByte)?;
        let mut handle: ffi::rnp_key_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_locate_key(
                self.ffi,
                type_c.as_ptr(),
                value_c.as_ptr(),
                &mut handle,
            ))?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(Key::from_handle(handle)))
        }
    }

    /// Load keys from a raw byte buffer (e.g. an exported keyring, or an
    /// armored key block).
    ///
    /// `format` selects the on-the-wire keyring format (GPG, KBX, G10). For
    /// OpenPGP-armored or binary key data use `KeyringFormat::Gpg`.
    pub fn load_keys<'s>(
        &self,
        format: KeyringFormat,
        source: impl Into<crate::ops::MessageSource<'s>>,
        flags: LoadSaveFlags,
    ) -> Result<()> {
        let mut source = source.into();
        let input = source.0.take()?;
        self.load_keys_input(format, &input, flags)
    }

    fn load_keys_input(
        &self,
        format: KeyringFormat,
        input: &Input,
        flags: LoadSaveFlags,
    ) -> Result<()> {
        let fmt_c = CString::new(format.as_str()).unwrap();
        unsafe {
            check(ffi::rnp_load_keys(
                self.ffi,
                fmt_c.as_ptr(),
                input.as_ptr(),
                flags.bits(),
            ))
        }
    }

    /// As [`Context::load_keys`], over a caller-built [`Input`]. Deprecated:
    /// pass the [`Input`] to [`Context::load_keys`] — it accepts both.
    #[deprecated(
        since = "0.2.0",
        note = "pass the &Input to Context::load_keys; it accepts both"
    )]
    pub fn load_keys_from_input(
        &self,
        format: KeyringFormat,
        input: &Input,
        flags: LoadSaveFlags,
    ) -> Result<()> {
        self.load_keys_input(format, input, flags)
    }
}
