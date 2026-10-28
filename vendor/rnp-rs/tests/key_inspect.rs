//! Phase 2: read-only key inspection.

mod common;
use common::signing_key;

use rnp::{Algorithm, Context, KeyBuilder, KeyIdentifier, KeyUsage, UidType};

#[test]
fn key_scalar_getters() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "inspector <inspector@example.com>");

    assert_eq!(key.alg().unwrap(), "RSA");
    assert_eq!(key.bits().unwrap(), 2048);
    assert_eq!(key.version().unwrap(), 4);
    assert!(key.creation().unwrap() > 0);
    // librnp's default expiration for a freshly-generated key (currently
    // 2 years). Just assert it's present, not a specific value.
    assert!(key.expiration().unwrap() > 0);

    let keyid = key.keyid().unwrap();
    assert!(!keyid.is_empty());
    let fp = key.fingerprint().unwrap();
    assert!(fp.len() >= 32);
    let grip = key.grip().unwrap();
    assert!(!grip.is_empty());

    assert!(key.have_public().unwrap());
    assert!(key.have_secret().unwrap());
    assert!(key.is_primary().unwrap());
    assert!(!key.is_sub().unwrap());
    assert!(key.is_valid().unwrap());
    assert!(!key.is_revoked().unwrap());
    assert!(!key.is_locked().unwrap());
    assert!(!key.is_protected().unwrap());
}

#[test]
fn key_uids() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "uidhost <uidhost@example.com>");

    let count = key.uid_count().unwrap();
    assert!(count >= 1, "expected at least 1 uid, got {count}");

    let uids = key.uids().unwrap();
    assert_eq!(uids.len(), count);

    let first = &uids[0];
    assert_eq!(first.uid_type().unwrap(), UidType::UserId);
    let data = first.data_string().unwrap();
    assert!(data.contains("uidhost"));
}

#[test]
fn key_subkeys() {
    let ctx = Context::new().expect("ctx");
    let primary = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("with-sub <with-sub@example.com>")
        .hash(rnp::Hash::Sha256)
        .add_usage(KeyUsage::Sign)
        .add_usage(KeyUsage::Certify)
        .build(&ctx)
        .expect("primary");

    // No subkeys yet.
    assert_eq!(primary.subkey_count().unwrap(), 0);

    rnp::SubkeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .hash(rnp::Hash::Sha256)
        .add_usage(KeyUsage::EncryptComms)
        .build(&ctx, &primary)
        .expect("subkey");

    // Now one subkey. Re-fetch the primary to see the updated count.
    let fp = primary.fingerprint().unwrap();
    let again = ctx
        .find_key(KeyIdentifier::Fingerprint(&fp))
        .unwrap()
        .expect("primary found");
    let subs = again.subkeys().unwrap();
    assert_eq!(subs.len(), 1);
    let sub = &subs[0];
    assert!(sub.is_sub().unwrap());
    assert_eq!(sub.alg().unwrap(), "RSA");
}
