//! Specs for the Signer builder (Phase 64).

#![allow(deprecated)]

use rnp::{Algorithm, Context, Hash, KeyBuilder, KeyUsage, Mode, Signer};

fn make_signer_key(ctx: &Context) -> rnp::Key<'_> {
    KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("signer <signer@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Sign)
        .add_usage(KeyUsage::Certify)
        .build(ctx)
        .expect("signer key")
}

#[test]
fn signer_builder_inline_round_trips() {
    let ctx = Context::new().expect("ctx");
    let key = make_signer_key(&ctx);

    let signed = Signer::new(&ctx, b"inline payload", Mode::Inline)
        .add_signer(&key)
        .hash(Hash::Sha256)
        .build_to_memory()
        .expect("sign");

    let result = rnp::verify(&ctx, &signed).expect("verify");
    assert!(
        result.any_valid().expect("any_valid"),
        "inline-signed message should verify"
    );
}

#[test]
fn signer_builder_detached_round_trips() {
    let ctx = Context::new().expect("ctx");
    let key = make_signer_key(&ctx);

    let msg = b"detached payload";
    let sig = Signer::new(&ctx, msg, Mode::Detached)
        .add_signer(&key)
        .build_to_memory()
        .expect("sign detached");

    let result = rnp::verify_detached(&ctx, msg, &sig).expect("verify");
    assert!(
        result.any_valid().expect("any_valid"),
        "detached signature should verify"
    );
}

#[test]
fn signer_builder_cleartext_round_trips() {
    let ctx = Context::new().expect("ctx");
    let key = make_signer_key(&ctx);

    let msg = b"cleartext line\nsecond line\n";
    let signed = Signer::new(&ctx, msg, Mode::Cleartext)
        .add_signer(&key)
        .build_to_memory()
        .expect("sign cleartext");

    let s = String::from_utf8_lossy(&signed);
    assert!(
        s.contains("-----BEGIN PGP SIGNED MESSAGE-----"),
        "cleartext message should have the magic header: {s}"
    );

    let result = rnp::verify(&ctx, &signed).expect("verify");
    assert!(result.any_valid().expect("any_valid"));
}

#[test]
fn signer_builder_armor_flag_produces_armor() {
    let ctx = Context::new().expect("ctx");
    let key = make_signer_key(&ctx);

    let signed = Signer::new(&ctx, b"x", Mode::Detached)
        .add_signer(&key)
        .armor(true)
        .build_to_memory()
        .expect("sign");

    let s = String::from_utf8_lossy(&signed);
    assert!(
        s.starts_with("-----BEGIN PGP SIGNATURE-----"),
        "armored detached sig should start with armor header: {s}"
    );
}

#[test]
fn signer_builder_multiple_signers_all_verify() {
    let ctx = Context::new().expect("ctx");
    let key1 = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("s1 <s1@example.com>")
        .add_usage(KeyUsage::Sign)
        .build(&ctx)
        .expect("k1");
    let key2 = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("s2 <s2@example.com>")
        .add_usage(KeyUsage::Sign)
        .build(&ctx)
        .expect("k2");

    let signed = Signer::new(&ctx, b"multi", Mode::Inline)
        .add_signer(&key1)
        .add_signer(&key2)
        .build_to_memory()
        .expect("sign");

    let result = rnp::verify(&ctx, &signed).expect("verify");
    let sig_count = result.signature_count().expect("count");
    assert!(
        sig_count >= 2,
        "multi-signer inline message should carry 2+ signatures, got {sig_count}"
    );
}

#[test]
fn signer_builder_per_signer_hash_overrides_default() {
    let ctx = Context::new().expect("ctx");
    let key = make_signer_key(&ctx);

    let signed = Signer::new(&ctx, b"sha386", Mode::Detached)
        .add_signer_with_hash(&key, Hash::Sha384)
        .build_to_memory()
        .expect("sign");

    let result = rnp::verify_detached(&ctx, b"sha386", &signed).expect("verify");
    assert!(result.any_valid().expect("any_valid"));

    let sig = result.signature_at(0).expect("at 0").expect("present");
    let hash = sig.hash().expect("hash");
    assert_eq!(
        hash, "SHA384",
        "per-signer hash should override the default"
    );
}

#[test]
fn signer_builder_no_signers_is_error() {
    let ctx = Context::new().expect("ctx");
    let res = Signer::new(&ctx, b"x", Mode::Inline).build_to_memory();
    assert!(res.is_err(), "building without any signer should fail");
}
