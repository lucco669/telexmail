//! Specs for Key::is_expired (Phase 83) and VerifyResult::iter_signatures
//! (Phase 84).

mod common;
use common::signing_key;

use rnp::{Algorithm, Context, KeyBuilder, KeyUsage, Mode, Signer};

#[test]
fn is_expired_false_for_fresh_key() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "fresh <fresh@example.com>");
    assert!(
        !key.is_expired().expect("is_expired"),
        "freshly generated key should not be expired"
    );
}

#[test]
fn iter_signatures_walks_a_multisig_message() {
    let ctx = Context::new().expect("ctx");
    let k1 = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("iter1 <i1@example.com>")
        .add_usage(KeyUsage::Sign)
        .build(&ctx)
        .expect("k1");
    let k2 = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("iter2 <i2@example.com>")
        .add_usage(KeyUsage::Sign)
        .build(&ctx)
        .expect("k2");

    let signed = Signer::new(&ctx, b"iter payload", Mode::Inline)
        .add_signer(&k1)
        .add_signer(&k2)
        .build_to_memory()
        .expect("sign");

    let result = rnp::verify(&ctx, &signed).expect("verify");
    let count = result.iter_signatures().count();
    assert!(
        count >= 2,
        "iter_signatures should yield at least 2 sigs, got {count}"
    );

    // Also exercises early-exit: stop at first valid.
    let first_valid = result.iter_signatures().find(|s| s.status_is_valid());
    assert!(
        first_valid.is_some(),
        "iter_signatures should find at least one valid sig"
    );
}
