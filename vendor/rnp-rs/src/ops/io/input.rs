//! [`Input`] — RAII wrapper around `rnp_input_t`.

use crate::error::{self, Result, check};
use crate::ffi;
use std::ffi::CString;
use std::io::Read;
use std::ptr;

use super::stream;

/// Owns an `rnp_input_t` for the lifetime of this value.
///
/// Construct with [`Input::from_memory`], [`Input::from_path`],
/// [`Input::from_stdin`], or [`Input::from_reader`] (any
/// [`std::io::Read`]). Operations consume inputs through their
/// `*_with_input` constructors.
///
/// When the input is reader-backed, the underlying [`std::io::Error`] of a
/// failed read is retrievable via [`Input::io_error`] after the enclosing
/// operation fails — librnp itself only reports a generic
/// `RNP_ERROR_READ`.
pub struct Input {
    handle: ffi::rnp_input_t,
    /// Present only for reader-backed inputs; the box's heap address is the
    /// `app_ctx` librnp calls back with.
    state: Option<Box<stream::ReaderState>>,
}

impl Input {
    /// Wrap memory as an input. The bytes are copied on the C side, so the
    /// caller's slice does not need to outlive the [`Input`].
    pub fn from_memory(bytes: &[u8]) -> Result<Self> {
        let mut handle: ffi::rnp_input_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_input_from_memory(
                &mut handle,
                bytes.as_ptr(),
                bytes.len(),
                true, // copy
            ))?;
        }
        if handle.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(Input {
            handle,
            state: None,
        })
    }

    /// Stream from any [`std::io::Read`].
    ///
    /// The reader is read lazily by librnp while the enclosing operation
    /// runs, so nothing is buffered in memory beyond librnp's own chunk
    /// size. The reader must not be used elsewhere while the [`Input`]
    /// exists. If a read fails, the operation fails and the original
    /// [`std::io::Error`] is available via [`Input::io_error`].
    ///
    /// ```no_run
    /// # use rnp::{Input, Output};
    /// let reader = std::io::Cursor::new(b"stream me".to_vec());
    /// let input = Input::from_reader(reader).unwrap();
    /// let mut output = Output::to_memory().unwrap();
    /// output.pipe(&input).unwrap();
    /// assert_eq!(output.into_bytes().unwrap(), b"stream me");
    /// ```
    pub fn from_reader(reader: impl Read + 'static) -> Result<Self> {
        Self::from_boxed_reader(Box::new(reader))
    }

    /// As [`Input::from_reader`], for an already-boxed reader.
    pub fn from_boxed_reader(reader: Box<dyn Read>) -> Result<Self> {
        let state = Box::new(stream::ReaderState::new(reader));
        let (reader_cb, closer_cb, app_ctx) = stream::reader_callbacks(&state);
        let mut handle: ffi::rnp_input_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_input_from_callback(
                &mut handle,
                reader_cb,
                closer_cb,
                app_ctx,
            ))?;
        }
        if handle.is_null() {
            // No callbacks have fired; `state` drops normally here.
            return Err(error::Error::NullPointer);
        }
        Ok(Input {
            handle,
            state: Some(state),
        })
    }

    /// Open a file at `path` for reading.
    pub fn from_path(path: &str) -> Result<Self> {
        let c = CString::new(path).map_err(|_| error::Error::PathNul)?;
        let mut handle: ffi::rnp_input_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_input_from_path(&mut handle, c.as_ptr()))?;
        }
        if handle.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(Input {
            handle,
            state: None,
        })
    }

    /// Read from process stdin.
    pub fn from_stdin() -> Result<Self> {
        let mut handle: ffi::rnp_input_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_input_from_stdin(&mut handle))?;
        }
        if handle.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(Input {
            handle,
            state: None,
        })
    }

    /// The [`std::io::Error`] recorded when a reader-backed input's read
    /// callback last failed, if any. `None` for non-reader-backed inputs
    /// and when no read has failed.
    pub fn io_error(&self) -> Option<&std::io::Error> {
        self.state.as_ref().and_then(|s| s.error.as_ref())
    }

    /// Take the recorded [`std::io::Error`], leaving the slot empty. See
    /// [`Input::io_error`].
    pub fn take_io_error(&mut self) -> Option<std::io::Error> {
        self.state.as_mut().and_then(|s| s.error.take())
    }

    /// Reclaim the reader from a reader-backed [`Input`], destroying the
    /// C handle first (which stops any further callbacks).
    ///
    /// Returns `None` if this input is not reader-backed. If the reader
    /// panicked during an operation, that panic resumes here. The second
    /// tuple element is the deferred read error, if any.
    pub fn into_reader(mut self) -> Option<(Box<dyn Read>, Option<std::io::Error>)> {
        if !self.handle.is_null() {
            // SAFETY: handle was produced by rnp_input_from_* and not yet
            // destroyed.
            unsafe {
                let _ = ffi::rnp_input_destroy(self.handle);
            }
            self.handle = ptr::null_mut();
        }
        let state = self.state.take()?;
        Some(stream::finish_reader(state))
    }

    /// Raw handle for passing to librnp functions. Crate-internal.
    pub(crate) fn as_ptr(&self) -> ffi::rnp_input_t {
        self.handle
    }
}

impl Drop for Input {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            // SAFETY: handle was produced by rnp_input_from_* and not yet
            // destroyed.
            unsafe {
                let _ = ffi::rnp_input_destroy(self.handle);
            }
            self.handle = ptr::null_mut();
        }
        if let Some(state) = self.state.take() {
            // Drops the boxed reader; re-raises a stashed panic, if any.
            let _ = stream::finish_reader(state);
        }
    }
}
