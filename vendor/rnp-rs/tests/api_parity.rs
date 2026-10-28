//! Specs for the new API-parity functions.

mod common;
use common::signing_key;

use rnp::{Algorithm, Context, Hash, KeyBuilder, KeyUsage, Mode, Signer};

#[test]
fn uid_remove_works() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "uidrm <uidrm@example.com>");
    let before = key.uid_count().unwrap();

    key.add_uid(
        "temp <temp@example.com>",
        &rnp::AddUidOptions::new().hash(Hash::Sha256),
    )
    .expect("add uid");
    let after_add = key.uid_count().unwrap();
    assert_eq!(after_add, before + 1);

    // Remove the newly-added UID
    let uids = key.uids().unwrap();
    let temp_uid = uids
        .iter()
        .find(|u| u.data_string().unwrap().contains("temp@example.com"))
        .expect("find temp uid");
    temp_uid.remove(&key).expect("remove uid");

    let after_remove = key.uid_count().unwrap();
    assert_eq!(after_remove, before);
}

#[test]
fn keygen_clear_usage_resets_vec() {
    let ctx = Context::new().expect("ctx");
    // clear_usage empties the Vec so only re-added usages are applied.
    // librnp may still apply certify defaults for primary keys.
    let key = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("clearu <clearu@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Sign)
        .add_usage(KeyUsage::Certify)
        .clear_usage()
        .add_usage(KeyUsage::EncryptComms)
        .build(&ctx);
    assert!(key.is_ok(), "build should succeed after clear+re-add");
}

#[test]
fn keygen_clear_pref_hash_resets_vec() {
    let ctx = Context::new().expect("ctx");
    let key = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("clearph <clearph@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Sign)
        .add_pref_hash(Hash::Sha1)
        .add_pref_hash(Hash::Sha256)
        .clear_pref_hash()
        .add_pref_hash(Hash::Sha512)
        .build(&ctx);
    assert!(key.is_ok(), "build should succeed after clear+re-add");
}

#[test]
fn signer_per_signature_timestamps() {
    let ctx = Context::new().expect("ctx");
    let k1 = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("ts1 <ts1@example.com>")
        .add_usage(KeyUsage::Sign)
        .build(&ctx)
        .expect("k1");

    // Use realistic creation time (now) with a 7-day expiration
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32;

    let signed = Signer::new(&ctx, b"ts-payload", Mode::Detached)
        .add_signer_with_options(
            &k1,
            Hash::Sha256,
            Some(now),
            Some(604800), // 7 days expiration
        )
        .build_to_memory()
        .expect("sign");

    assert!(!signed.is_empty());

    let result = rnp::verify_detached(&ctx, b"ts-payload", &signed).expect("verify");
    assert!(result.signature_count().unwrap() >= 1);

    // Check the per-signature creation time was applied
    let sig = result.signature_at(0).unwrap().unwrap();
    let (create, expires) = sig.times().unwrap();
    assert_eq!(create, now);
    assert_eq!(expires, 604800);
}

#[test]
fn debug_enable_disable() {
    // Just verify these don't panic
    rnp::version::enable_debug("stderr");
    rnp::version::disable_debug();
}
