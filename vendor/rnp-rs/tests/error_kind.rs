//! Coverage for [`rnp::ErrorKind`] mapping over the `rnp_err.h` range.
//!
//! The mapping is a stable contract: callers branch on `Error::kind()` to
//! decide recovery (e.g. retry on `BadPassword`, give up on `KeyNotFound`).
//! Any change to a documented code's kind is breaking.

use rnp::Result;

/// Helper: synthesize a "fake" rnp result by calling a known-failing FFI
/// path with a known-bad input. We don't have direct access to the numeric
/// codes through the safe API, so we exercise the categorization through
/// `Error::kind()` on real failures.
///
/// This test guards the *category-by-prefix* fallback (any code in
/// `0x14…​` classifies under `SigError` or a more specific kind) and the
/// `from_code` direct map for the well-known codes.
#[test]
fn from_code_known_mappings() {
    use rnp::ErrorKind as K;

    // Success.
    assert_eq!(K::from_code(0x0000_0000), K::Success);

    // Common range.
    assert_eq!(K::from_code(0x1000_0000), K::Generic);
    assert_eq!(K::from_code(0x1000_0008), K::NotFound);
    assert_eq!(K::from_code(0x1000_0004), K::NotSupported);

    // Storage range.
    assert_eq!(K::from_code(0x1100_0000), K::Access);
    assert_eq!(K::from_code(0x1100_0002), K::Write);

    // Crypto range.
    assert_eq!(K::from_code(0x1200_0002), K::SignatureInvalid);
    assert_eq!(K::from_code(0x1200_0004), K::BadPassword);
    assert_eq!(K::from_code(0x1200_0005), K::KeyNotFound);
    assert_eq!(K::from_code(0x1200_0007), K::DecryptFailed);
    assert_eq!(K::from_code(0x1200_000b), K::NoSignaturesFound);

    // Parsing range.
    assert_eq!(K::from_code(0x1300_0004), K::Eof);

    // Sig-validation range.
    assert_eq!(K::from_code(0x1400_0000), K::SigError);
    assert_eq!(K::from_code(0x1400_0004), K::SigWeakHash);
    assert_eq!(K::from_code(0x1400_0008), K::SigExpired);
    assert_eq!(K::from_code(0x1400_001b), K::SigUnusableKey);
}

/// Unknown codes still classify by their high nibble, never panicking.
#[test]
fn from_code_unknown_falls_back_to_category() {
    use rnp::ErrorKind as K;

    // An unknown code in the Common range.
    assert_eq!(K::from_code(0x1000_00ff), K::Generic);
    // An unknown code in the Sig-validation range.
    assert_eq!(K::from_code(0x1400_00ff), K::SigError);
    // A truly out-of-range code.
    assert_eq!(K::from_code(0xff00_0000), K::Other);
}

/// A real failed call surfaces the expected kind end-to-end.
///
/// `find_key` on a userid that doesn't exist returns `Ok(None)`, but loading
/// malformed key bytes triggers a parse error.
#[test]
fn kind_surfaces_on_real_failure() {
    use rnp::ErrorKind as K;

    let ctx = rnp::Context::new().expect("context");
    // Load garbage as a key — librnp should reject it with a parse-range code.
    let result: Result<()> = ctx.load_keys(
        rnp::KeyringFormat::Gpg,
        b"definitely not a valid keyring",
        rnp::LoadSaveFlags::PUBLIC,
    );
    let err = result.expect_err("garbage input must fail");
    let k = err.kind();
    // Could be BadFormat / NotEnoughData / UnknownTag — all are reasonable
    // for garbage input. The point of the test is that *some* non-Success
    // kind is reported, never `Other` accidentally for this input.
    assert_ne!(k, K::Success, "error kind must not be Success");
    assert_ne!(
        k,
        K::Other,
        "garbage input should map to a known category, not Other"
    );
}

/// The wrapper-level errors (NulByte, NullPointer) also classify sanely.
#[test]
fn wrapper_error_kinds() {
    use rnp::ErrorKind as K;

    // Construct via load_keys with an interior-NUL userid-bearing bytes is
    // awkward; instead exercise via the path input variant indirectly.
    // We test the variants through Error::NulByte / PathNul directly via
    // the public re-exports.
    let nul_byte = rnp::Error::NulByte;
    assert_eq!(nul_byte.kind(), K::BadParameters);

    let path_nul = rnp::Error::PathNul;
    assert_eq!(path_nul.kind(), K::BadParameters);
}
