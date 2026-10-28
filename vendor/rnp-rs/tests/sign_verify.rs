//! Round-trip: generate a key, sign a message, verify the signature.
//!
//! Tests intentionally exercise the deprecated `generate_test_key` shim — it
//! must keep working until removed.

mod common;
use common::signing_key;

use rnp::{Context, KeyIdentifier, sign, sign_detached, verify, verify_detached};

#[test]
fn inline_sign_verify_roundtrip() {
    let ctx = Context::new().expect("create context");
    let key = signing_key(&ctx, "roundtrip <roundtrip@example.com>");

    let message = b"the quick brown fox jumps over the lazy dog";
    let signed = sign(&ctx, message, &key).expect("sign");
    assert!(!signed.is_empty());

    let result = verify(&ctx, &signed).expect("verify op ran");
    assert!(
        result.any_valid().unwrap_or(false),
        "signature should verify"
    );
}

#[test]
fn detached_sign_verify_roundtrip() {
    let ctx = Context::new().expect("create context");
    let key = signing_key(&ctx, "detached <detached@example.com>");

    let message = b"some bytes that need to be authenticated";
    let sig = sign_detached(&ctx, message, &key).expect("sign detached");
    assert!(!sig.is_empty());

    let result = verify_detached(&ctx, message, &sig).expect("verify detached ran");
    assert!(
        result.any_valid().unwrap_or(false),
        "detached signature should verify"
    );
}

#[test]
fn verify_rejects_tampered_message() {
    let ctx = Context::new().expect("create context");
    let key = signing_key(&ctx, "tamper <tamper@example.com>");

    let message = b"original message";
    let sig = sign_detached(&ctx, message, &key).expect("sign detached");

    // Flip a byte in the message — verification must fail.
    let mut tampered = message.to_vec();
    tampered[0] ^= 0xff;
    let result = verify_detached(&ctx, &tampered, &sig);
    let valid = result
        .map(|r| r.any_valid().unwrap_or(false))
        .unwrap_or(false);
    assert!(!valid, "tampered message must not verify");
}

#[test]
fn find_key_by_userid() {
    let ctx = Context::new().expect("create context");
    let _key = signing_key(&ctx, "findable <findable@example.com>");
    let found = ctx
        .find_key(KeyIdentifier::Userid("findable <findable@example.com>"))
        .expect("find_key call ran");
    assert!(found.is_some(), "key should be findable by userid");
}
