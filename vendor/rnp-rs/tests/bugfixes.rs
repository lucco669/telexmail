//! Bug fix tests: revocation cert (54), inline subkeys (55).
//! Bug 56 (threshold signer trait) was removed; the trait was inert
//! and didn't belong in the crate.

#![allow(deprecated)]

use rnp::{
    Algorithm, Context, Hash, KeyBuilder, KeyIdentifier, KeyUsage, SubkeyBuilder,
    generate_revocation_certificate,
};

// --- Bug 54: revocation certificate ---------------------------------------

#[test]
fn revocation_certificate_produces_standalone_bytes() {
    let ctx = Context::new().expect("ctx");
    let key = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("rev54 <rev54@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Sign)
        .add_usage(KeyUsage::Certify)
        .build(&ctx)
        .expect("key");

    let cert = generate_revocation_certificate(
        &ctx,
        &key,
        rnp::RevocationReason::new(rnp::RevocationCode::Compromised).with_reason("test revocation"),
    )
    .expect("revocation cert");

    assert!(!cert.is_empty(), "revocation cert must be non-empty");

    // The cert should parse as a packet stream.
    let json = rnp::dump_packets_bytes_to_json(&cert, rnp::JsonDumpFlags::default()).expect("dump");
    assert!(
        json.contains("Signature"),
        "revocation cert should contain a Signature packet: {json}"
    );
}

// --- Bug 55: inline subkey creation via add_subkey ------------------------

#[test]
fn keybuilder_add_subkey_creates_composite_keypair() {
    let ctx = Context::new().expect("ctx");
    let primary = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("composite <comp@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Sign)
        .add_usage(KeyUsage::Certify)
        .add_subkey(
            SubkeyBuilder::new(Algorithm::Rsa)
                .bits(2048)
                .hash(Hash::Sha256)
                .add_usage(KeyUsage::EncryptComms),
        )
        .build(&ctx)
        .expect("primary with inline subkey");

    // Verify the subkey was created.
    let fp = primary.fingerprint().unwrap();
    let found = ctx
        .find_key(KeyIdentifier::Fingerprint(&fp))
        .unwrap()
        .expect("primary found");
    let subs = found.subkeys().expect("subkeys");
    assert_eq!(
        subs.len(),
        1,
        "expected 1 inline subkey, got {}",
        subs.len()
    );
    assert!(subs[0].is_sub().unwrap());
}

#[test]
fn keybuilder_multiple_inline_subkeys() {
    let ctx = Context::new().expect("ctx");
    let primary = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("multi <multi@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Sign)
        .add_usage(KeyUsage::Certify)
        .add_subkey(
            SubkeyBuilder::new(Algorithm::Rsa)
                .bits(2048)
                .hash(Hash::Sha256)
                .add_usage(KeyUsage::EncryptComms),
        )
        .add_subkey(
            SubkeyBuilder::new(Algorithm::Rsa)
                .bits(2048)
                .hash(Hash::Sha256)
                .add_usage(KeyUsage::EncryptStorage),
        )
        .build(&ctx)
        .expect("primary with 2 subkeys");

    let fp = primary.fingerprint().unwrap();
    let found = ctx
        .find_key(KeyIdentifier::Fingerprint(&fp))
        .unwrap()
        .expect("found");
    assert_eq!(found.subkeys().unwrap().len(), 2, "expected 2 subkeys");
}

// --- Bug 56: previously defined a ThresholdSigner trait as a design
// contract for Confium's FROST-style threshold signing. Removed because
// rnp's API has no hook for custom signature computation; the trait was
// inert in the crate and didn't belong in the public surface. See
// TODO.roadmap/67-decision-threshold-signer.md for the decision record.
