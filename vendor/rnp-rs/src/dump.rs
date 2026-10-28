//! Packet dumps and per-object JSON serialization.
//!
//! Wraps `rnp_dump_packets_to_output`, `rnp_dump_packets_to_json`, and the
//! per-object `rnp_key_*_to_json` family. The JSON output shape is defined
//! by librnp and may evolve between versions; this module returns raw
//! `String`s rather than parsing into Rust structs to avoid coupling to a
//! fixed shape.

use crate::error::{Result, check};
use crate::ffi;
use crate::ffi_safe::call_for_string;
use crate::ops::{Input, Output};

/// Flags controlling the human-readable packet dump (`rnp_dump_packets_to_output`).
/// Wraps the `RNP_DUMP_*` constants.
#[derive(Clone, Copy, Debug, Default)]
pub struct DumpFlags(pub u32);

impl DumpFlags {
    /// Include MPI values in the dump.
    pub const MPI: Self = Self(ffi::RNP_DUMP_MPI as u32);
    /// Dump raw packet bytes.
    pub const RAW: Self = Self(ffi::RNP_DUMP_RAW as u32);
    /// Include key grips.
    pub const GRIP: Self = Self(ffi::RNP_DUMP_GRIP as u32);

    pub fn bits(self) -> u32 {
        self.0
    }
}

impl std::ops::BitOr for DumpFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Flags controlling the JSON packet dump (`rnp_dump_packets_to_json`).
/// Wraps the `RNP_JSON_DUMP_*` constants.
#[derive(Clone, Copy, Debug, Default)]
pub struct JsonDumpFlags(pub u32);

impl JsonDumpFlags {
    pub const MPI: Self = Self(ffi::RNP_JSON_DUMP_MPI as u32);
    pub const RAW: Self = Self(ffi::RNP_JSON_DUMP_RAW as u32);
    pub const GRIP: Self = Self(ffi::RNP_JSON_DUMP_GRIP as u32);

    pub fn bits(self) -> u32 {
        self.0
    }
}

impl std::ops::BitOr for JsonDumpFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Flags controlling which fields `rnp_key_to_json` emits. Wraps the
/// `RNP_JSON_*` constants (distinct from [`JsonDumpFlags`]).
#[derive(Clone, Copy, Debug, Default)]
pub struct JsonFlags(pub u32);

impl JsonFlags {
    /// Include public-key MPIs.
    pub const PUBLIC_MPIS: Self = Self(ffi::RNP_JSON_PUBLIC_MPIS as u32);
    /// Include secret-key MPIs (requires unlocked key).
    pub const SECRET_MPIS: Self = Self(ffi::RNP_JSON_SECRET_MPIS as u32);
    /// Include signatures array.
    pub const SIGNATURES: Self = Self(ffi::RNP_JSON_SIGNATURES as u32);
    /// Include per-signature MPIs.
    pub const SIGNATURE_MPIS: Self = Self(ffi::RNP_JSON_SIGNATURE_MPIS as u32);

    pub fn bits(self) -> u32 {
        self.0
    }
}

impl std::ops::BitOr for JsonFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Dump packet information from `input` to `output` in human-readable form.
pub fn dump_packets_to_output(input: &Input, output: &mut Output, flags: DumpFlags) -> Result<()> {
    unsafe {
        check(ffi::rnp_dump_packets_to_output(
            input.as_ptr(),
            output.as_ptr(),
            flags.bits(),
        ))
    }
}

/// Dump packet information from `input` as a JSON string.
pub fn dump_packets_to_json(input: &Input, flags: JsonDumpFlags) -> Result<String> {
    call_for_string(|out| unsafe {
        ffi::rnp_dump_packets_to_json(input.as_ptr(), flags.bits(), out)
    })
}

/// Convenience: dump `bytes` packet structure as a JSON string.
pub fn dump_packets_bytes_to_json(bytes: &[u8], flags: JsonDumpFlags) -> Result<String> {
    let input = Input::from_memory(bytes)?;
    dump_packets_to_json(&input, flags)
}

// Re-export error for symmetry with `armor`.
#[allow(unused_imports)]
use crate::error as _error_reexport;
