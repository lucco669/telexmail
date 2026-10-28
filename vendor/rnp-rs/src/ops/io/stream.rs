//! Callback plumbing that bridges librnp's I/O callbacks to
//! [`std::io::Read`] / [`std::io::Write`].
//!
//! librnp exposes stream extensibility through four C callbacks
//! (`rnp_input_reader_t`, `rnp_input_closer_t`, `rnp_output_writer_t`,
//! `rnp_output_closer_t`). This module is the single place where those
//! callbacks are implemented: everything a Rust caller needs to plug an
//! arbitrary reader or writer into any librnp operation lives here, and
//! [`Input`](super::Input) / [`Output`](super::Output) merely expose it at
//! their constructors.
//!
//! ## Ownership
//!
//! The boxed trait object and any deferred failure live in
//! [`ReaderState`] / [`WriterState`]. The heap address of that box is what
//! librnp receives as `app_ctx`; the owning [`Input`](super::Input) /
//! [`Output`](super::Output) keeps the box alive for exactly as long as the
//! C handle exists, and reclaims it when the handle is destroyed. A `Box`
//! has a stable heap address, so moving the Rust-side wrapper never
//! invalidates the pointer handed to C.
//!
//! ## Failure and panic policy
//!
//! The thunks never unwind across the C boundary — that would be undefined
//! behavior. User code runs inside [`std::panic::catch_unwind`]:
//!
//! - An [`std::io::Error`] is reported to librnp as a failed callback, so
//!   the enclosing operation fails with `RNP_ERROR_READ` /
//!   `RNP_ERROR_WRITE`. The original error is retained and surfaced via
//!   [`Input::io_error`](super::Input::io_error) /
//!   [`Output::io_error`](super::Output::io_error) — the rnp-level error
//!   alone does not say *why* the stream failed.
//! - A panic is stashed and re-raised on the Rust side when the state is
//!   reclaimed (see [`finish_reader`] / [`finish_writer`]), so a panicking
//!   reader or writer is never silently swallowed.
//!
//! ## Output closer semantics
//!
//! librnp invokes the output closer exactly once, during
//! `rnp_output_destroy`, passing `discard = !keep`. Successful operations
//! (and [`Output::finish`](super::Output::finish)) set `keep`, so the
//! closer flushes the writer only on success. On `discard` librnp is asking
//! the application to delete what was written; a generic
//! [`std::io::Write`] cannot do that, so the request is recorded in
//! [`WriterState::discarded`] (surfaced via
//! [`Output::into_writer`](super::Output::into_writer)) and the writer is
//! left unflushed.

use std::any::Any;
use std::io::{Error, Read, Write};
use std::os::raw::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

use crate::ffi;

/// Panic payload deferred from a thunk back to Rust-side code.
type PanicPayload = Box<dyn Any + Send + 'static>;

/// A boxed reader plus the deferred outcome of its last failed callback.
pub(super) struct ReaderState {
    reader: Box<dyn Read>,
    pub(super) error: Option<Error>,
    panic: Option<PanicPayload>,
}

impl ReaderState {
    pub(super) fn new(reader: Box<dyn Read>) -> Self {
        ReaderState {
            reader,
            error: None,
            panic: None,
        }
    }
}

/// A boxed writer plus the deferred outcomes of its callbacks.
pub(super) struct WriterState {
    writer: Box<dyn Write>,
    pub(super) error: Option<Error>,
    panic: Option<PanicPayload>,
    /// Set by the closer: `true` when librnp asked for the written data to
    /// be discarded. Unset until the closer runs.
    pub(super) discarded: bool,
    /// Whether the closer has run.
    pub(super) closed: bool,
}

impl WriterState {
    pub(super) fn new(writer: Box<dyn Write>) -> Self {
        WriterState {
            writer,
            error: None,
            panic: None,
            discarded: false,
            closed: false,
        }
    }
}

// SAFETY: `app_ctx` is the address of a live `ReaderState` box handed to
// librnp by `Input::from_boxed_reader`. librnp only invokes thunks while
// the owning `Input` (and therefore the box) is alive, and never
// concurrently (handles are not thread-safe, so all use is same-thread).
// `buf`/`len`/`read` are librnp's buffer, its length, and an out-parameter.
unsafe extern "C" fn reader_thunk(
    app_ctx: *mut c_void,
    buf: *mut c_void,
    len: usize,
    read: *mut usize,
) -> bool {
    let state = unsafe { &mut *app_ctx.cast::<ReaderState>() };
    let out = unsafe { std::slice::from_raw_parts_mut(buf.cast::<u8>(), len) };
    // A single `read()` call maps 1:1 onto librnp's contract: partial reads
    // are fine, EOF is `true` with zero bytes read (upstream's own file
    // source does the same).
    match catch_unwind(AssertUnwindSafe(|| state.reader.read(out))) {
        Ok(Ok(n)) => {
            unsafe { *read = n };
            true
        }
        Ok(Err(e)) => {
            state.error = Some(e);
            false
        }
        Err(p) => {
            state.panic = Some(p);
            false
        }
    }
}

// SAFETY: same lifetime contract as `reader_thunk`; `buf`/`len` describe
// the chunk librnp wants written.
unsafe extern "C" fn writer_thunk(app_ctx: *mut c_void, buf: *const c_void, len: usize) -> bool {
    let state = unsafe { &mut *app_ctx.cast::<WriterState>() };
    let data = unsafe { std::slice::from_raw_parts(buf.cast::<u8>(), len) };
    match catch_unwind(AssertUnwindSafe(|| state.writer.write_all(data))) {
        Ok(Ok(())) => true,
        Ok(Err(e)) => {
            state.error = Some(e);
            false
        }
        Err(p) => {
            state.panic = Some(p);
            false
        }
    }
}

// SAFETY: called by librnp exactly once, from `rnp_output_destroy`, after
// which no further thunks fire for this state.
unsafe extern "C" fn writer_closer(app_ctx: *mut c_void, discard: bool) {
    let state = unsafe { &mut *app_ctx.cast::<WriterState>() };
    state.discarded = discard;
    state.closed = true;
    if discard {
        return;
    }
    match catch_unwind(AssertUnwindSafe(|| state.writer.flush())) {
        Ok(Ok(())) => {}
        Ok(Err(e)) => state.error = Some(e),
        Err(p) => state.panic = Some(p),
    }
}

/// The callback pair handed to `rnp_input_from_callback` for `state`. The
/// box's address is passed to librnp as `app_ctx`; the box itself stays
/// Rust-owned. No closer is needed — the reader is dropped by the Rust side
/// when the state is reclaimed.
pub(super) fn reader_callbacks(
    state: &ReaderState,
) -> (
    ffi::rnp_input_reader_t,
    ffi::rnp_input_closer_t,
    *mut c_void,
) {
    (Some(reader_thunk), None, ptr_of(state))
}

/// The callback pair handed to `rnp_output_to_callback` for `state`.
pub(super) fn writer_callbacks(
    state: &WriterState,
) -> (
    ffi::rnp_output_writer_t,
    ffi::rnp_output_closer_t,
    *mut c_void,
) {
    (Some(writer_thunk), Some(writer_closer), ptr_of(state))
}

/// Stable heap address of a boxed state, for use as librnp's `app_ctx`.
/// The caller passes a reference into a `Box` whose address was already
/// handed to librnp — the box (not a stack copy) must stay the owner.
fn ptr_of<T>(state: &T) -> *mut c_void {
    std::ptr::from_ref(state) as *mut c_void
}

/// Reclaim a reader state after the C handle is gone. Re-raises a stashed
/// panic (see the module docs for the policy). The `Box` is moved in from
/// the owning `Input`, keeping the `app_ctx` address stable to the end.
#[allow(clippy::boxed_local)]
pub(super) fn finish_reader(state: Box<ReaderState>) -> (Box<dyn Read>, Option<Error>) {
    let ReaderState {
        reader,
        error,
        panic,
    } = *state;
    if let Some(p) = panic {
        resume_unwind(p);
    }
    (reader, error)
}

/// Reclaim a writer state after the C handle is gone (which also ran the
/// closer). Re-raises a stashed panic. The `Box` is moved in from the
/// owning `Output`, keeping the `app_ctx` address stable to the end.
#[allow(clippy::boxed_local)]
pub(super) fn finish_writer(
    state: Box<WriterState>,
) -> (Box<dyn Write>, Option<Error>, bool, bool) {
    let WriterState {
        writer,
        error,
        panic,
        discarded,
        closed,
    } = *state;
    let _ = closed;
    if let Some(p) = panic {
        resume_unwind(p);
    }
    (writer, error, discarded, closed)
}
