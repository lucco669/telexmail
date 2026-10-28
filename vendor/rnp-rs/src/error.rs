//! Error type for the RNP binding.
//!
//! Maps `rnp_result_t` (a `u32`) to a structured `snafu` error. Each non-success
//! code carries both its raw numeric form and a categorized [`ErrorKind`],
//! letting callers react to *categories* of failure without re-parsing the
//! numeric code.
//!
//! The category split mirrors the high-nibble grouping in
//! [`include/rnp/rnp_err.h`](https://github.com/rnpgp/rnp/blob/main/include/rnp/rnp_err.h):
//!
//! | prefix    | category       |
//! |-----------|----------------|
//! | `0x00…​`  | Success        |
//! | `0x10…​`  | Common         |
//! | `0x11…​`  | Storage        |
//! | `0x12…​`  | Crypto         |
//! | `0x13…​`  | Parsing        |
//! | `0x14…​`  | Sig-validation |

use snafu::Snafu;

/// Subset of `RNP_*` result codes from `rnp_err.h`. bindgen does not pick
/// these up because they live in an anonymous C enum. They are part of
/// librnp's stable ABI and will not change without a major version bump.
///
/// Source: `include/rnp/rnp_err.h`.
pub(crate) mod codes {
    pub const RNP_SUCCESS: u32 = 0x0000_0000;

    // Common (0x10…)
    pub const RNP_ERROR_GENERIC: u32 = 0x1000_0000;
    pub const RNP_ERROR_BAD_FORMAT: u32 = 0x1000_0001;
    pub const RNP_ERROR_BAD_PARAMETERS: u32 = 0x1000_0002;
    pub const RNP_ERROR_NOT_IMPLEMENTED: u32 = 0x1000_0003;
    pub const RNP_ERROR_NOT_SUPPORTED: u32 = 0x1000_0004;
    pub const RNP_ERROR_OUT_OF_MEMORY: u32 = 0x1000_0005;
    pub const RNP_ERROR_SHORT_BUFFER: u32 = 0x1000_0006;
    pub const RNP_ERROR_NULL_POINTER: u32 = 0x1000_0007;
    pub const RNP_ERROR_NOT_FOUND: u32 = 0x1000_0008;

    // Storage (0x11…)
    pub const RNP_ERROR_ACCESS: u32 = 0x1100_0000;
    pub const RNP_ERROR_READ: u32 = 0x1100_0001;
    pub const RNP_ERROR_WRITE: u32 = 0x1100_0002;

    // Crypto (0x12…)
    pub const RNP_ERROR_BAD_STATE: u32 = 0x1200_0000;
    pub const RNP_ERROR_MAC_INVALID: u32 = 0x1200_0001;
    pub const RNP_ERROR_SIGNATURE_INVALID: u32 = 0x1200_0002;
    pub const RNP_ERROR_KEY_GENERATION: u32 = 0x1200_0003;
    pub const RNP_ERROR_BAD_PASSWORD: u32 = 0x1200_0004;
    pub const RNP_ERROR_KEY_NOT_FOUND: u32 = 0x1200_0005;
    pub const RNP_ERROR_NO_SUITABLE_KEY: u32 = 0x1200_0006;
    pub const RNP_ERROR_DECRYPT_FAILED: u32 = 0x1200_0007;
    pub const RNP_ERROR_ENCRYPT_FAILED: u32 = 0x1200_0008;
    pub const RNP_ERROR_RNG: u32 = 0x1200_0009;
    pub const RNP_ERROR_SIGNING_FAILED: u32 = 0x1200_000a;
    pub const RNP_ERROR_NO_SIGNATURES_FOUND: u32 = 0x1200_000b;
    pub const RNP_ERROR_SIGNATURE_EXPIRED: u32 = 0x1200_000c;
    pub const RNP_ERROR_VERIFICATION_FAILED: u32 = 0x1200_000d;
    pub const RNP_ERROR_SIGNATURE_UNKNOWN: u32 = 0x1200_000e;

    // Parsing (0x13…)
    pub const RNP_ERROR_NOT_ENOUGH_DATA: u32 = 0x1300_0000;
    pub const RNP_ERROR_UNKNOWN_TAG: u32 = 0x1300_0001;
    pub const RNP_ERROR_PACKET_NOT_CONSUMED: u32 = 0x1300_0002;
    pub const RNP_ERROR_NO_USERID: u32 = 0x1300_0003;
    pub const RNP_ERROR_EOF: u32 = 0x1300_0004;

    // Sig-validation (0x14…) — 28 codes through RNP_ERROR_SIG_UNUSABLE_KEY.
    pub const RNP_ERROR_SIG_ERROR: u32 = 0x1400_0000;
    pub const RNP_ERROR_SIG_PARSE_ERROR: u32 = 0x1400_0001;
    pub const RNP_ERROR_SIG_SIGNER_UNTRUSTED: u32 = 0x1400_0002;
    pub const RNP_ERROR_SIG_PUB_ALG_MISMATCH: u32 = 0x1400_0003;
    pub const RNP_ERROR_SIG_WEAK_HASH: u32 = 0x1400_0004;
    pub const RNP_ERROR_SIG_HASH_ALG_MISMATCH: u32 = 0x1400_0005;
    pub const RNP_ERROR_SIG_LBITS_MISMATCH: u32 = 0x1400_0006;
    pub const RNP_ERROR_SIG_FROM_FUTURE: u32 = 0x1400_0007;
    pub const RNP_ERROR_SIG_EXPIRED: u32 = 0x1400_0008;
    pub const RNP_ERROR_SIG_OLDER_KEY: u32 = 0x1400_0009;
    pub const RNP_ERROR_SIG_EXPIRED_KEY: u32 = 0x1400_000a;
    pub const RNP_ERROR_SIG_FP_MISMATCH: u32 = 0x1400_000b;
    pub const RNP_ERROR_SIG_UNKNOWN_NOTATION: u32 = 0x1400_000c;
    pub const RNP_ERROR_SIG_NOT_DOCUMENT: u32 = 0x1400_000d;
    pub const RNP_ERROR_SIG_NO_SIGNER_ID: u32 = 0x1400_000e;
    pub const RNP_ERROR_SIG_NO_SIGNER_KEY: u32 = 0x1400_000f;
    pub const RNP_ERROR_SIG_NO_HASH_CTX: u32 = 0x1400_0010;
    pub const RNP_ERROR_SIG_WRONG_KEY_SIG: u32 = 0x1400_0011;
    pub const RNP_ERROR_SIG_UID_MISSING: u32 = 0x1400_0012;
    pub const RNP_ERROR_SIG_WRONG_BINDING: u32 = 0x1400_0013;
    pub const RNP_ERROR_SIG_WRONG_DIRECT: u32 = 0x1400_0014;
    pub const RNP_ERROR_SIG_WRONG_REV: u32 = 0x1400_0015;
    pub const RNP_ERROR_SIG_UNSUPPORTED: u32 = 0x1400_0016;
    pub const RNP_ERROR_SIG_NO_PRIMARY_BINDING: u32 = 0x1400_0017;
    pub const RNP_ERROR_SIG_BINDING_PARSE: u32 = 0x1400_0018;
    pub const RNP_ERROR_SIG_WRONG_BIND_TYPE: u32 = 0x1400_0019;
    pub const RNP_ERROR_SIG_INVALID_BINDING: u32 = 0x1400_001a;
    pub const RNP_ERROR_SIG_UNUSABLE_KEY: u32 = 0x1400_001b;
}

/// Coarse category of an RNP failure, derived from the high nibble of the
/// `rnp_result_t` plus a few exact-code overrides.
///
/// Designed for `match`-based recovery: callers should branch on the kind
/// rather than re-parsing the numeric code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    Success,

    // --- Common (0x10…) -----------------------------------------------------
    Generic,
    BadFormat,
    BadParameters,
    NotImplemented,
    NotSupported,
    OutOfMemory,
    ShortBuffer,
    NullPointer,
    NotFound,

    // --- Storage (0x11…) ----------------------------------------------------
    Access,
    Read,
    Write,

    // --- Crypto (0x12…) -----------------------------------------------------
    BadState,
    MacInvalid,
    SignatureInvalid,
    KeyGeneration,
    BadPassword,
    KeyNotFound,
    NoSuitableKey,
    DecryptFailed,
    EncryptFailed,
    Rng,
    SigningFailed,
    NoSignaturesFound,
    SignatureExpiredCrypto,
    VerificationFailed,
    SignatureUnknown,

    // --- Parsing (0x13…) ----------------------------------------------------
    NotEnoughData,
    UnknownTag,
    PacketNotConsumed,
    NoUserid,
    Eof,

    // --- Sig-validation (0x14…) --------------------------------------------
    SigError,
    SigParseError,
    SigSignerUntrusted,
    SigPubAlgMismatch,
    SigWeakHash,
    SigHashAlgMismatch,
    SigLbitsMismatch,
    SigFromFuture,
    SigExpired,
    SigOlderKey,
    SigExpiredKey,
    SigFpMismatch,
    SigUnknownNotation,
    SigNotDocument,
    SigNoSignerId,
    SigNoSignerKey,
    SigNoHashCtx,
    SigWrongKeySig,
    SigUidMissing,
    SigWrongBinding,
    SigWrongDirect,
    SigWrongRev,
    SigUnsupported,
    SigNoPrimaryBinding,
    SigBindingParse,
    SigWrongBindType,
    SigInvalidBinding,
    SigUnusableKey,

    /// A code outside the known ranges.
    Other,
}

impl ErrorKind {
    /// Map a raw `rnp_result_t` to its [`ErrorKind`].
    pub fn from_code(code: u32) -> Self {
        use codes::*;
        match code {
            RNP_SUCCESS => ErrorKind::Success,

            // Common
            RNP_ERROR_GENERIC => ErrorKind::Generic,
            RNP_ERROR_BAD_FORMAT => ErrorKind::BadFormat,
            RNP_ERROR_BAD_PARAMETERS => ErrorKind::BadParameters,
            RNP_ERROR_NOT_IMPLEMENTED => ErrorKind::NotImplemented,
            RNP_ERROR_NOT_SUPPORTED => ErrorKind::NotSupported,
            RNP_ERROR_OUT_OF_MEMORY => ErrorKind::OutOfMemory,
            RNP_ERROR_SHORT_BUFFER => ErrorKind::ShortBuffer,
            RNP_ERROR_NULL_POINTER => ErrorKind::NullPointer,
            RNP_ERROR_NOT_FOUND => ErrorKind::NotFound,

            // Storage
            RNP_ERROR_ACCESS => ErrorKind::Access,
            RNP_ERROR_READ => ErrorKind::Read,
            RNP_ERROR_WRITE => ErrorKind::Write,

            // Crypto
            RNP_ERROR_BAD_STATE => ErrorKind::BadState,
            RNP_ERROR_MAC_INVALID => ErrorKind::MacInvalid,
            RNP_ERROR_SIGNATURE_INVALID => ErrorKind::SignatureInvalid,
            RNP_ERROR_KEY_GENERATION => ErrorKind::KeyGeneration,
            RNP_ERROR_BAD_PASSWORD => ErrorKind::BadPassword,
            RNP_ERROR_KEY_NOT_FOUND => ErrorKind::KeyNotFound,
            RNP_ERROR_NO_SUITABLE_KEY => ErrorKind::NoSuitableKey,
            RNP_ERROR_DECRYPT_FAILED => ErrorKind::DecryptFailed,
            RNP_ERROR_ENCRYPT_FAILED => ErrorKind::EncryptFailed,
            RNP_ERROR_RNG => ErrorKind::Rng,
            RNP_ERROR_SIGNING_FAILED => ErrorKind::SigningFailed,
            RNP_ERROR_NO_SIGNATURES_FOUND => ErrorKind::NoSignaturesFound,
            RNP_ERROR_SIGNATURE_EXPIRED => ErrorKind::SignatureExpiredCrypto,
            RNP_ERROR_VERIFICATION_FAILED => ErrorKind::VerificationFailed,
            RNP_ERROR_SIGNATURE_UNKNOWN => ErrorKind::SignatureUnknown,

            // Parsing
            RNP_ERROR_NOT_ENOUGH_DATA => ErrorKind::NotEnoughData,
            RNP_ERROR_UNKNOWN_TAG => ErrorKind::UnknownTag,
            RNP_ERROR_PACKET_NOT_CONSUMED => ErrorKind::PacketNotConsumed,
            RNP_ERROR_NO_USERID => ErrorKind::NoUserid,
            RNP_ERROR_EOF => ErrorKind::Eof,

            // Sig-validation
            RNP_ERROR_SIG_ERROR => ErrorKind::SigError,
            RNP_ERROR_SIG_PARSE_ERROR => ErrorKind::SigParseError,
            RNP_ERROR_SIG_SIGNER_UNTRUSTED => ErrorKind::SigSignerUntrusted,
            RNP_ERROR_SIG_PUB_ALG_MISMATCH => ErrorKind::SigPubAlgMismatch,
            RNP_ERROR_SIG_WEAK_HASH => ErrorKind::SigWeakHash,
            RNP_ERROR_SIG_HASH_ALG_MISMATCH => ErrorKind::SigHashAlgMismatch,
            RNP_ERROR_SIG_LBITS_MISMATCH => ErrorKind::SigLbitsMismatch,
            RNP_ERROR_SIG_FROM_FUTURE => ErrorKind::SigFromFuture,
            RNP_ERROR_SIG_EXPIRED => ErrorKind::SigExpired,
            RNP_ERROR_SIG_OLDER_KEY => ErrorKind::SigOlderKey,
            RNP_ERROR_SIG_EXPIRED_KEY => ErrorKind::SigExpiredKey,
            RNP_ERROR_SIG_FP_MISMATCH => ErrorKind::SigFpMismatch,
            RNP_ERROR_SIG_UNKNOWN_NOTATION => ErrorKind::SigUnknownNotation,
            RNP_ERROR_SIG_NOT_DOCUMENT => ErrorKind::SigNotDocument,
            RNP_ERROR_SIG_NO_SIGNER_ID => ErrorKind::SigNoSignerId,
            RNP_ERROR_SIG_NO_SIGNER_KEY => ErrorKind::SigNoSignerKey,
            RNP_ERROR_SIG_NO_HASH_CTX => ErrorKind::SigNoHashCtx,
            RNP_ERROR_SIG_WRONG_KEY_SIG => ErrorKind::SigWrongKeySig,
            RNP_ERROR_SIG_UID_MISSING => ErrorKind::SigUidMissing,
            RNP_ERROR_SIG_WRONG_BINDING => ErrorKind::SigWrongBinding,
            RNP_ERROR_SIG_WRONG_DIRECT => ErrorKind::SigWrongDirect,
            RNP_ERROR_SIG_WRONG_REV => ErrorKind::SigWrongRev,
            RNP_ERROR_SIG_UNSUPPORTED => ErrorKind::SigUnsupported,
            RNP_ERROR_SIG_NO_PRIMARY_BINDING => ErrorKind::SigNoPrimaryBinding,
            RNP_ERROR_SIG_BINDING_PARSE => ErrorKind::SigBindingParse,
            RNP_ERROR_SIG_WRONG_BIND_TYPE => ErrorKind::SigWrongBindType,
            RNP_ERROR_SIG_INVALID_BINDING => ErrorKind::SigInvalidBinding,
            RNP_ERROR_SIG_UNUSABLE_KEY => ErrorKind::SigUnusableKey,

            _ => {
                // Fall back to category-by-prefix so future codes added by
                // librnp still classify sanely.
                match code >> 24 {
                    0x10 => ErrorKind::Generic,
                    0x11 => ErrorKind::Access,
                    0x12 => ErrorKind::BadState,
                    0x13 => ErrorKind::NotEnoughData,
                    0x14 => ErrorKind::SigError,
                    _ => ErrorKind::Other,
                }
            }
        }
    }
}

/// A failure returned by the underlying librnp call, or a wrapper-level
/// precondition violation.
#[derive(Debug, Snafu)]
pub enum Error {
    /// The RNP call returned a non-success result code.
    #[snafu(display("rnp call failed: {message} (code 0x{code:08x}, kind {kind:?})"))]
    Rnp {
        code: u32,
        kind: ErrorKind,
        message: String,
    },

    /// A required pointer/handle was unexpectedly null.
    #[snafu(display("librnp returned a null pointer where one was required"))]
    NullPointer,

    /// A NUL byte was found in a string that must be passed to librnp as a
    /// C string.
    #[snafu(display("string contained an interior NUL byte"))]
    NulByte,

    /// A NUL byte was found in a path that must be passed to librnp.
    #[snafu(display("path contained an interior NUL byte"))]
    PathNul,

    /// A string-to-enum conversion failed because the string doesn't
    /// match any known variant. Surfaces from `FromStr` impls on the
    /// model enums.
    #[snafu(display("unknown {kind} variant: {value}"))]
    UnknownVariant { kind: &'static str, value: String },

    /// An I/O operation at the Rust/C boundary failed.
    #[snafu(display("i/o error: {source}"))]
    Io { source: std::io::Error },
}

impl Error {
    /// Coarse category of the failure, regardless of the underlying cause.
    pub fn kind(&self) -> ErrorKind {
        match self {
            Error::Rnp { kind, .. } => *kind,
            Error::NullPointer => ErrorKind::NullPointer,
            Error::NulByte | Error::PathNul => ErrorKind::BadParameters,
            Error::UnknownVariant { .. } => ErrorKind::BadParameters,
            Error::Io { .. } => ErrorKind::Read,
        }
    }

    /// The raw `rnp_result_t` code if this is an `Error::Rnp`, else `None`.
    pub fn rnp_code(&self) -> Option<u32> {
        match self {
            Error::Rnp { code, .. } => Some(*code),
            _ => None,
        }
    }
}

/// Construct an [`Error::Rnp`] from a raw `rnp_result_t` code, looking
/// up its kind and message. Use when integrating with hand-written FFI
/// bindings that return raw `u32` codes.
pub fn from_rnp_code(code: u32) -> Error {
    let kind = ErrorKind::from_code(code);
    let message = result_to_string(code);
    Error::Rnp {
        code,
        kind,
        message,
    }
}

/// Construct an `Error::UnknownVariant`. Public so other modules (e.g.
/// `strconv`) can raise it without depending on snafu internals.
pub fn unknown_variant(kind: &'static str, value: impl Into<String>) -> Error {
    Error::UnknownVariant {
        kind,
        value: value.into(),
    }
}

/// Bridge `std::io::Error` into [`Error`] so `?` works at IO/FFI
/// boundaries.
impl From<std::io::Error> for Error {
    fn from(source: std::io::Error) -> Self {
        Error::Io { source }
    }
}

/// Bridge [`Error`] into `std::io::Error` so `?` works when a caller is
/// exposing an IO API on top of rnp-rs. The `io::Error`'s kind is
/// derived from [`Error::kind`] where there's a sensible mapping.
impl From<Error> for std::io::Error {
    fn from(err: Error) -> Self {
        let kind = match err.kind() {
            ErrorKind::NotFound | ErrorKind::KeyNotFound => std::io::ErrorKind::NotFound,
            ErrorKind::Access => std::io::ErrorKind::PermissionDenied,
            ErrorKind::Eof => std::io::ErrorKind::UnexpectedEof,
            ErrorKind::Write => std::io::ErrorKind::WriteZero,
            ErrorKind::Read => std::io::ErrorKind::Other,
            _ => std::io::ErrorKind::Other,
        };
        std::io::Error::new(kind, err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Re-export of `RNP_SUCCESS` for other modules in this crate.
pub(crate) const SUCCESS: u32 = codes::RNP_SUCCESS;

/// Re-export of `RNP_ERROR_NOT_FOUND`.
pub(crate) const NOT_FOUND: u32 = codes::RNP_ERROR_NOT_FOUND;

/// Re-export of `RNP_ERROR_NO_SIGNATURES_FOUND` — used by the verify path
/// when execute succeeded but no signature validated.
#[allow(dead_code)]
pub(crate) const NO_SIGNATURES_FOUND: u32 = codes::RNP_ERROR_NO_SIGNATURES_FOUND;

/// Check an `rnp_result_t` return value; convert non-success into an
/// [`Error::Rnp`].
pub(crate) fn check(code: u32) -> Result<()> {
    if code == codes::RNP_SUCCESS {
        Ok(())
    } else {
        let kind = ErrorKind::from_code(code);
        let message = result_to_string(code);
        Err(Error::Rnp {
            code,
            kind,
            message,
        })
    }
}

/// Construct an [`Error::Rnp`] for [`NO_SIGNATURES_FOUND`] with a
/// descriptive message. Used by the verify path when execute returned
/// success but no signature in the message validated.
#[allow(dead_code)]
pub(crate) fn no_signatures_error() -> Error {
    Error::Rnp {
        code: NO_SIGNATURES_FOUND,
        kind: ErrorKind::NoSignaturesFound,
        message: "no valid signatures found in the message".to_string(),
    }
}

fn result_to_string(code: u32) -> String {
    // SAFETY: rnp_result_to_string returns a static C string (or NULL on a
    // truly bad input). We never free it.
    let ptr = unsafe { crate::ffi::rnp_result_to_string(code) };
    if ptr.is_null() {
        return format!("unknown rnp error 0x{code:08x}");
    }
    // SAFETY: librnp returns a NUL-terminated string.
    let cstr = unsafe { std::ffi::CStr::from_ptr(ptr) };
    cstr.to_string_lossy().into_owned()
}
