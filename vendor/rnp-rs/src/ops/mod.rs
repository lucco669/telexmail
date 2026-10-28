//! Operation-level wrappers: shared `Input`/`Output` RAII handles plus future
//! op-builder types (sign, verify, encrypt, generate).
//!
//! All `rnp_input_t` and `rnp_output_t` ownership in this crate flows through
//! [`Input`] and [`Output`] declared in [`io`]. No other module calls
//! `rnp_input_*` or `rnp_output_*` destroyers directly.

pub mod io;

pub(crate) use io::ByteSource;
pub(crate) use io::or_stream_error;
pub use io::{
    ArmorType, Input, MessageSource, Output, OutputFileFlags, WriterOutcome,
    call_for_optional_string, call_for_string, cstr_to_optional_string, cstr_to_string,
};
