//! Behavior tests for the wrappers added by the FFI surface-parity audit
//! (see `tests/ffi_parity.rs`): keygen shorthands, key status flags,
//! signature error list / revoker, and op-level sign options.

mod common;
use common::signing_key;

use rnp::algorithm::Compression;
use rnp::keygen::{
    generate_key_25519, generate_key_dsa_eg, generate_key_ec, generate_key_ex, generate_key_rsa,
    generate_key_sm2,
};
use rnp::{Context, ErrorKind, Mode, Signer};

/// Current Unix time. A signature whose creation time pre-dates its key
/// does not verify, so call this *after* building the test key.
fn unix_now() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0)
}

// --- Keygen shorthands -------------------------------------------------------

#[test]
fn shorthand_rsa_generates_usable_key() {
    let ctx = Context::new().expect("ctx");
    let key = generate_key_rsa(&ctx, 2048, 0, "short-rsa <sr@example.com>").expect("generate");
    assert_eq!(
        key.uid_string_at(0).expect("uid").as_deref(),
        Some("short-rsa <sr@example.com>")
    );
    assert!(!key.is_expired().expect("expired"));
    assert_eq!(key.alg().expect("alg"), "RSA");
}

#[test]
fn shorthand_dsa_eg_generates_pair() {
    let ctx = Context::new().expect("ctx");
    let key =
        generate_key_dsa_eg(&ctx, 1024, 1024, "short-dsa <sd@example.com>").expect("generate");
    assert_eq!(
        key.subkey_count().expect("subkeys"),
        1,
        "ElGamal subkey expected"
    );
}

#[test]
fn shorthand_ec_generates_key() {
    let ctx = Context::new().expect("ctx");
    let key = generate_key_ec(&ctx, "NIST P-256", "short-ec <se@example.com>").expect("generate");
    assert_eq!(key.curve().expect("curve").as_deref(), Some("NIST P-256"));
}

#[test]
fn shorthand_25519_generates_key() {
    let ctx = Context::new().expect("ctx");
    let key = generate_key_25519(&ctx, "short-25 <s25@example.com>").expect("generate");
    assert_eq!(key.alg().expect("alg"), "EDDSA");
}

#[test]
fn shorthand_sm2_generates_when_supported() {
    let ctx = Context::new().expect("ctx");
    match generate_key_sm2(&ctx, "short-sm2 <ss@example.com>") {
        Ok(key) => assert_eq!(key.alg().expect("alg"), "SM2"),
        Err(e) if matches!(e.kind(), ErrorKind::NotSupported) => {
            eprintln!("skipping: SM2 not supported by this librnp build")
        }
        Err(e) => panic!("unexpected error: {e:?}"),
    }
}

#[test]
fn shorthand_ex_generates_eddsa_key() {
    let ctx = Context::new().expect("ctx");
    let key = generate_key_ex(
        &ctx,
        Some("EdDSA"),
        0,
        None,
        None,
        0,
        None,
        "short-ex <sx@example.com>",
    )
    .expect("generate");
    assert_eq!(key.alg().expect("alg"), "EDDSA");
    assert_eq!(key.uid_count().expect("count"), 1);

    // RSA primary + RSA encryption subkey through the same call.
    let pair = generate_key_ex(
        &ctx,
        Some("RSA"),
        2048,
        None,
        Some("RSA"),
        2048,
        None,
        "short-ex2 <sx2@example.com>",
    )
    .expect("generate with subkey");
    assert_eq!(pair.subkey_count().expect("subkeys"), 1);
}

// --- Key status flags --------------------------------------------------------

#[test]
fn fresh_key_status_flags_are_false() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "flags <flags@example.com>");
    assert!(!key.is_expired().expect("expired"));
    assert!(!key.is_compromised().expect("compromised"));
    assert!(!key.is_retired().expect("retired"));
    assert!(!key.is_superseded().expect("superseded"));
    assert!(key.revocation_signature().expect("rev sig").is_none());
}

#[test]
fn revoked_key_reports_compromised_flag() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "rev-comp <rc@example.com>");
    key.revoke(
        rnp::RevocationReason::new(rnp::RevocationCode::Compromised),
        rnp::Hash::Sha256,
    )
    .expect("revoke");
    assert!(key.is_revoked().expect("revoked"));
    assert!(key.is_compromised().expect("compromised"));
    let sig = key
        .revocation_signature()
        .expect("rev sig call")
        .expect("revoked key has a revocation signature");
    // The revocation signature itself carries the reason.
    let reason = sig.revocation_reason().expect("reason");
    assert!(reason.is_some(), "revocation signature has a reason");
}

// --- Signature error list + revoker ------------------------------------------

#[test]
fn valid_signature_has_no_errors() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "sig-err <serr@example.com>");
    let signed = Signer::new(&ctx, b"payload", Mode::Inline)
        .add_signer(&key)
        .build_to_memory()
        .expect("sign");
    let result = rnp::verify(&ctx, &signed).expect("verify");
    let sig = &result.signatures().expect("sigs")[0];
    let handle = sig.handle().expect("handle");
    assert_eq!(handle.error_count().expect("count"), 0);
    assert_eq!(handle.error_at(0).expect("at"), None);
    assert!(handle.revoker().expect("revoker").is_none());
}

// --- Op-level sign options ---------------------------------------------------

#[test]
fn signer_op_level_options_land_in_packets() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "op-lvl <ol@example.com>");
    let creation = unix_now();

    let signed = Signer::new(&ctx, b"op-level payload", Mode::Inline)
        .add_signer(&key)
        .compression(Compression::Zlib, 6)
        .file_name("notes.txt")
        .file_mtime(1_700_000_000)
        .creation_time(creation)
        .expiration_time(86_400)
        .build_to_memory()
        .expect("sign");

    // Read the literal-data packet fields back through the verify path —
    // stable across librnp versions (the JSON dump's field naming is not).
    let result = rnp::verify(&ctx, &signed).expect("verify");
    let sig = &result.signatures().expect("sigs")[0];
    let (sig_creation, sig_expiration) = sig.times().expect("times");
    assert_eq!(sig_creation, creation, "op-level creation time");
    assert_eq!(sig_expiration, 86_400, "op-level expiration time");
    let info = result
        .file_info()
        .expect("file info")
        .expect("literal data present");
    #[cfg(not(feature = "crypto-refresh"))]
    {
        assert_eq!(info.name, "notes.txt", "op-level file name");
        assert_eq!(info.mtime, 1_700_000_000, "op-level file mtime");
    }
    // RFC 9580: literal-data filename and timestamp SHOULD NOT be set.
    // librnp built with ENABLE_CRYPTO_REFRESH deliberately omits them, so
    // the op-level setters succeed but the literal header stays empty.
    #[cfg(feature = "crypto-refresh")]
    {
        let _ = &info;
        assert!(info.name.is_empty(), "crypto-refresh build omits the name");
    }
}

#[test]
fn error_at_reports_raw_code() {
    // from_rnp_code is the readable companion to Signature::error_at.
    let e = rnp::from_rnp_code(0x1100_0001);
    assert!(matches!(e.kind(), ErrorKind::Read));
}
