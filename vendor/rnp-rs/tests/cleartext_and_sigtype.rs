//! Phase 20-21 tests: cleartext signing, SignatureType enum, examples smoke.

mod common;
use common::signing_key;

use rnp::{Context, SignatureType, sign_cleartext, verify};

#[test]
fn cleartext_sign_verify_roundtrip() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "clear <clear@example.com>");

    let message = b"some readable text\nthat should remain visible\n";
    let clear = sign_cleartext(&ctx, message, &key).expect("sign cleartext");

    // The cleartext body must be visible in the output.
    let s = String::from_utf8_lossy(&clear);
    assert!(
        s.contains("readable text"),
        "cleartext body should be visible, got: {s}"
    );
    assert!(
        s.contains("BEGIN PGP SIGNATURE"),
        "armored signature block must be present, got: {s}"
    );

    // And it must verify via the standard inline verify path (cleartext
    // is a special form of inline).
    let result = verify(&ctx, &clear).expect("verify");
    assert!(
        result.any_valid().unwrap_or(false),
        "cleartext signature must verify"
    );
}

#[test]
fn cleartext_tamper_breaks_verification() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "tamp <tamp@example.com>");

    let message = b"unique-tamper-marker-here\n";
    let mut clear = sign_cleartext(&ctx, message, &key).expect("sign");

    // Find the unique marker in the cleartext body (between the
    // "Hash:" header and the "-----BEGIN PGP SIGNATURE-----" trailer)
    // and flip a byte. Cleartext hashing canonicalizes line endings but
    // doesn't ignore byte changes inside the body.
    let marker = b"unique-tamper-marker";
    let pos = clear
        .windows(marker.len())
        .position(|w| w == marker)
        .expect("marker should be in the cleartext body");
    clear[pos] ^= 0xff;

    let result = verify(&ctx, &clear);
    let valid = result
        .map(|r| r.any_valid().unwrap_or(false))
        .unwrap_or(false);
    assert!(!valid, "tampered cleartext must not verify");
}

#[test]
fn signature_type_enum_known_variants() {
    // Parse the librnp-returned strings and verify the typed mapping.
    assert_eq!(SignatureType::parse("binary"), SignatureType::Binary);
    assert_eq!(SignatureType::parse("text"), SignatureType::Text);
    assert_eq!(
        SignatureType::parse("certification (positive)"),
        SignatureType::CertificationPositive
    );
    assert_eq!(
        SignatureType::parse("subkey binding"),
        SignatureType::SubkeyBinding
    );
    assert_eq!(
        SignatureType::parse("key revocation"),
        SignatureType::KeyRevocation
    );
}

#[test]
fn signature_type_enum_unknown_falls_through() {
    let u = SignatureType::parse("unknown: 99");
    assert_eq!(u, SignatureType::Unknown(99));
}

#[test]
fn signature_type_enum_round_trip() {
    for variant in [
        SignatureType::Binary,
        SignatureType::Text,
        SignatureType::Standalone,
        SignatureType::CertificationGeneric,
        SignatureType::CertificationPersona,
        SignatureType::CertificationCasual,
        SignatureType::CertificationPositive,
        SignatureType::SubkeyBinding,
        SignatureType::PrimaryKeyBinding,
        SignatureType::Direct,
        SignatureType::KeyRevocation,
        SignatureType::SubkeyRevocation,
        SignatureType::CertificationRevocation,
        SignatureType::Timestamp,
        SignatureType::ThirdParty,
    ] {
        let s = variant.as_str();
        assert_eq!(SignatureType::parse(&s), variant);
    }
    // Unknown round-trips with the tag preserved.
    let s = SignatureType::Unknown(42).as_str();
    assert_eq!(SignatureType::parse(&s), SignatureType::Unknown(42));
}

#[test]
fn uid_self_signature_is_certification() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "selfcert <sc@example.com>");
    let uid = &key.uids().expect("uids")[0];
    let sig = &uid.signatures().expect("sigs")[0];

    let typ = sig.sig_type_enum().expect("sig type");
    assert!(
        matches!(
            typ,
            SignatureType::CertificationGeneric
                | SignatureType::CertificationCasual
                | SignatureType::CertificationPositive
                | SignatureType::CertificationPersona
        ),
        "self-certification should be one of the certification variants, got {typ:?}"
    );
}
