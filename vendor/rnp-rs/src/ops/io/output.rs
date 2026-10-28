//! [`Output`] — RAII wrapper around `rnp_output_t`, plus
//! [`OutputFileFlags`] and [`WriterOutcome`].

use crate::error::{self, Result, check};
use crate::ffi;
use std::ffi::CString;
use std::io::Write;
use std::ptr;

use super::armor_type::ArmorType;
use super::input::Input;
use super::stream;

/// Owns an `rnp_output_t` for the lifetime of this value.
///
/// Construct with one of the `to_*` constructors. For memory destinations,
/// [`Output::into_bytes`] consumes the output and returns the buffered
/// bytes; for writer destinations, [`Output::into_writer`] reclaims the
/// writer after the handle is closed.
pub struct Output {
    handle: ffi::rnp_output_t,
    /// Present only for writer-backed outputs; the box's heap address is
    /// the `app_ctx` librnp calls back with.
    state: Option<Box<stream::WriterState>>,
}

/// The reclaimed writer and deferred outcomes of a writer-backed
/// [`Output`], returned by [`Output::into_writer`].
///
/// `discarded` is `true` when librnp closed the stream with a discard
/// request — i.e. the enclosing operation failed (or the output was
/// dropped without [`Output::finish`]) and librnp asked for the already
/// written bytes to be deleted. The writer was *not* flushed in that case;
/// a generic [`std::io::Write`] cannot delete data, so the request is
/// surfaced here for the caller to act on (e.g. remove the partial file).
pub struct WriterOutcome {
    /// The writer handed to [`Output::to_writer`], closed and (unless
    /// discarded) flushed.
    pub writer: Box<dyn Write>,
    /// The deferred [`std::io::Error`] from the failed write or flush, if
    /// any.
    pub io_error: Option<std::io::Error>,
    /// Whether librnp asked for the written data to be discarded.
    pub discarded: bool,
}

impl std::fmt::Debug for WriterOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WriterOutcome")
            .field("writer", &"<writer>")
            .field("io_error", &self.io_error)
            .field("discarded", &self.discarded)
            .finish()
    }
}

/// Flags for [`Output::to_file`]. Wraps the `RNP_OUTPUT_FILE_*` constants.
#[derive(Clone, Copy, Debug, Default)]
pub struct OutputFileFlags(pub u32);

impl OutputFileFlags {
    /// Overwrite an existing file. Wraps `RNP_OUTPUT_FILE_OVERWRITE`.
    pub const OVERWRITE: Self = Self(ffi::RNP_OUTPUT_FILE_OVERWRITE as u32);
    /// Write to a randomly-named temp file and rename on `finish()`. Wraps
    /// `RNP_OUTPUT_FILE_RANDOM`. Callers using this flag must call
    /// [`Output::finish`] before dropping to ensure the rename succeeds.
    pub const RANDOM: Self = Self(ffi::RNP_OUTPUT_FILE_RANDOM as u32);

    pub fn bits(self) -> u32 {
        self.0
    }
}

impl std::ops::BitOr for OutputFileFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl Output {
    /// Buffer output in memory. Use [`Output::into_bytes`] to drain.
    ///
    /// `max_alloc` of `0` means unlimited.
    pub fn to_memory() -> Result<Self> {
        Self::to_memory_with_max(0)
    }

    /// As [`Output::to_memory`], but with an upper bound on the allocation.
    pub fn to_memory_with_max(max_alloc: usize) -> Result<Self> {
        let mut handle: ffi::rnp_output_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_output_to_memory(&mut handle, max_alloc))?;
        }
        if handle.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(Output {
            handle,
            state: None,
        })
    }

    /// Discard all output.
    pub fn to_null() -> Result<Self> {
        let mut handle: ffi::rnp_output_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_output_to_null(&mut handle))?;
        }
        if handle.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(Output {
            handle,
            state: None,
        })
    }

    /// Stream to any [`std::io::Write`].
    ///
    /// Chunks are written lazily while the enclosing operation runs, so
    /// nothing is buffered in memory beyond librnp's own pipeline. On
    /// success the writer is flushed when the output is finished or
    /// dropped; if the operation fails, librnp requests a discard — see
    /// [`WriterOutcome::discarded`]. A failed write or flush surfaces as a
    /// failed operation plus the original [`std::io::Error`] via
    /// [`Output::io_error`].
    ///
    /// ```no_run
    /// # use std::io::Write;
    /// # use std::sync::{Arc, Mutex};
    /// # use rnp::{Input, Output};
    /// # // A writer is moved into the Output, so retain access to the
    /// # // collected bytes through shared state.
    /// # #[derive(Clone, Default)]
    /// # struct SharedBuf(Arc<Mutex<Vec<u8>>>);
    /// # impl Write for SharedBuf {
    /// #     fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
    /// #         self.0.lock().unwrap().extend_from_slice(buf);
    /// #         Ok(buf.len())
    /// #     }
    /// #     fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
    /// # }
    /// let sink = SharedBuf::default();
    /// let input = Input::from_memory(b"stream me").unwrap();
    /// let mut output = Output::to_writer(sink.clone()).unwrap();
    /// output.pipe(&input).unwrap();
    /// drop(output); // closes the stream: flushes on success
    /// assert_eq!(*sink.0.lock().unwrap(), b"stream me");
    /// ```
    ///
    /// The writer is *moved* into the [`Output`] (the `+ 'static` bound
    /// rules out borrowing a local), so keep a handle to whatever state it
    /// writes into — as above — or reclaim it afterwards with
    /// [`Output::into_writer`]. The writer must not be used elsewhere while
    /// the [`Output`] exists.
    pub fn to_writer(writer: impl Write + 'static) -> Result<Self> {
        Self::to_boxed_writer(Box::new(writer))
    }

    /// As [`Output::to_writer`], for an already-boxed writer.
    pub fn to_boxed_writer(writer: Box<dyn Write>) -> Result<Self> {
        let state = Box::new(stream::WriterState::new(writer));
        let (writer_cb, closer_cb, app_ctx) = stream::writer_callbacks(&state);
        let mut handle: ffi::rnp_output_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_output_to_callback(
                &mut handle,
                writer_cb,
                closer_cb,
                app_ctx,
            ))?;
        }
        if handle.is_null() {
            // No callbacks have fired; `state` drops normally here.
            return Err(error::Error::NullPointer);
        }
        Ok(Output {
            handle,
            state: Some(state),
        })
    }

    /// Write to `path`, overwriting if it already exists.
    pub fn to_path(path: &str) -> Result<Self> {
        let c = CString::new(path).map_err(|_| error::Error::PathNul)?;
        let mut handle: ffi::rnp_output_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_output_to_path(&mut handle, c.as_ptr()))?;
        }
        if handle.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(Output {
            handle,
            state: None,
        })
    }

    /// Write to `path` with explicit flags (overwrite / random-temp-rename).
    pub fn to_file(path: &str, flags: OutputFileFlags) -> Result<Self> {
        let c = CString::new(path).map_err(|_| error::Error::PathNul)?;
        let mut handle: ffi::rnp_output_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_output_to_file(
                &mut handle,
                c.as_ptr(),
                flags.bits(),
            ))?;
        }
        if handle.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(Output {
            handle,
            state: None,
        })
    }

    /// Write to process stdout.
    pub fn to_stdout() -> Result<Self> {
        let mut handle: ffi::rnp_output_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_output_to_stdout(&mut handle))?;
        }
        if handle.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(Output {
            handle,
            state: None,
        })
    }

    /// Wrap `base` with an ASCII-armor encoder. The returned [`Output`] owns
    /// the armor handle; `base` is borrowed for the lifetime of the wrapper
    /// (the C side stores the pointer).
    pub fn to_armor(base: &Output, ty: ArmorType) -> Result<Self> {
        let mut handle: ffi::rnp_output_t = ptr::null_mut();
        let c = CString::new(ty.as_str()).unwrap();
        unsafe {
            check(ffi::rnp_output_to_armor(
                base.handle,
                &mut handle,
                c.as_ptr(),
            ))?;
        }
        if handle.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(Output {
            handle,
            state: None,
        })
    }

    /// Write `bytes` to the output stream. Returns the number of bytes
    /// actually written.
    pub fn write(&mut self, bytes: &[u8]) -> Result<usize> {
        let mut written: usize = 0;
        unsafe {
            check(ffi::rnp_output_write(
                self.handle,
                bytes.as_ptr() as *const _,
                bytes.len(),
                &mut written,
            ))?;
        }
        Ok(written)
    }

    /// Finalize the output. Required for [`OutputFileFlags::RANDOM`] (causes
    /// the temp file to be renamed to its final path) and harmless otherwise.
    pub fn finish(&mut self) -> Result<()> {
        unsafe { check(ffi::rnp_output_finish(self.handle)) }
    }

    /// Override the default armor line length (76). Only meaningful when the
    /// output was created via [`Output::to_armor`].
    pub fn set_armor_line_length(&mut self, line_len: usize) -> Result<()> {
        unsafe { check(ffi::rnp_output_armor_set_line_length(self.handle, line_len)) }
    }

    /// Pipe `input` through to this output until EOF. Consumes neither.
    pub fn pipe(&mut self, input: &Input) -> Result<()> {
        unsafe { check(ffi::rnp_output_pipe(input.as_ptr(), self.handle)) }
    }

    /// Consume a memory-backed output and return its buffered bytes.
    ///
    /// Only valid on outputs created via [`Output::to_memory`] /
    /// [`Output::to_memory_with_max`].
    pub fn into_bytes(self) -> Result<Vec<u8>> {
        // Pull the buffer out, then drop the Output handle. We do this by
        // taking ownership of `self` so Drop still runs and frees the
        // output structure (but not the byte buffer, which we own after the
        // do_copy=true call).
        let mut buf: *mut u8 = ptr::null_mut();
        let mut len: usize = 0;
        unsafe {
            let get = check(ffi::rnp_output_memory_get_buf(
                self.handle,
                &mut buf,
                &mut len,
                true, // copy — buf is owned by us, freed via rnp_buffer_destroy
            ));
            if let Err(e) = &get {
                // A memory output that never wrote a byte never allocates a
                // buffer; upstream then reports BAD_PARAMETERS while still
                // writing writeb (0) to `len`. Treat that documented case
                // as an empty buffer instead of an error.
                if matches!(
                    e,
                    crate::error::Error::Rnp { code, .. }
                        if *code == crate::error::codes::RNP_ERROR_BAD_PARAMETERS && len == 0
                ) {
                    return Ok(Vec::new());
                }
                // Drop runs here and calls rnp_output_destroy.
                return get.map(|_| Vec::new());
            }
            if buf.is_null() || len == 0 {
                Ok(Vec::new())
            } else {
                let v = std::slice::from_raw_parts(buf, len).to_vec();
                ffi::rnp_buffer_destroy(buf as *mut _);
                Ok(v)
            }
        }
    }

    /// The [`std::io::Error`] recorded when a writer-backed output's write
    /// or flush callback last failed, if any. `None` for non-writer-backed
    /// outputs and when nothing has failed.
    pub fn io_error(&self) -> Option<&std::io::Error> {
        self.state.as_ref().and_then(|s| s.error.as_ref())
    }

    /// Take the recorded [`std::io::Error`], leaving the slot empty. See
    /// [`Output::io_error`].
    pub fn take_io_error(&mut self) -> Option<std::io::Error> {
        self.state.as_mut().and_then(|s| s.error.take())
    }

    /// Reclaim the writer from a writer-backed [`Output`], destroying the
    /// C handle first. Closing the handle runs librnp's closer, which
    /// flushes the writer on success or records a discard request on
    /// failure — see [`WriterOutcome`].
    ///
    /// Returns `None` if this output is not writer-backed. If the writer
    /// panicked during an operation, that panic resumes here.
    pub fn into_writer(mut self) -> Option<WriterOutcome> {
        if !self.handle.is_null() {
            // SAFETY: handle was produced by rnp_output_to_* and not yet
            // destroyed; destroying it fires the closer exactly once.
            unsafe {
                let _ = ffi::rnp_output_destroy(self.handle);
            }
            self.handle = ptr::null_mut();
        }
        let state = self.state.take()?;
        let (writer, io_error, discarded, _closed) = stream::finish_writer(state);
        Some(WriterOutcome {
            writer,
            io_error,
            discarded,
        })
    }

    /// Raw handle for passing to librnp functions. Crate-internal.
    pub(crate) fn as_ptr(&self) -> ffi::rnp_output_t {
        self.handle
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            // SAFETY: handle was produced by rnp_output_to_* and not yet
            // destroyed; destroying it fires the closer for writer-backed
            // outputs while `state` is still alive.
            unsafe {
                let _ = ffi::rnp_output_destroy(self.handle);
            }
            self.handle = ptr::null_mut();
        }
        if let Some(state) = self.state.take() {
            // Drops the boxed writer; re-raises a stashed panic, if any.
            let _ = stream::finish_writer(state);
        }
    }
}
