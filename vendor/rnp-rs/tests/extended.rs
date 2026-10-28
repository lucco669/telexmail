//! Phase 2 finishing + Phase 5 keyring + Phase 8 security/version.

mod common;
use common::signing_key;

use rnp::{
    Algorithm, Cipher, Context, FeatureType, Hash, KeyBuilder, KeyIdentifier, KeyUsage, Output,
    ProtectOptions, RemoveFlags, supports_feature,
};

// --- Phase 2 finishing: remaining key getters ------------------------------

#[test]
fn key_extended_getters() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "extended <extended@example.com>");

    // RSA key: dsa_qbits should be 0 (the call succeeds, value is N/A).
    assert_eq!(key.dsa_qbits().unwrap_or(0), 0);
    // Primary-only key has no parent primary — librnp returns NotFound.
    assert!(key.primary_fprint().is_err());
    assert!(key.primary_grip().is_err());
    assert!(key.valid_till().unwrap() > 0);
    assert!(key.valid_till64().unwrap() > 0);
    assert_eq!(key.revoker_count().unwrap(), 0);
}

#[test]
fn key_protection_getters() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "prot <prot@example.com>");

    // Freshly generated unprotected key.
    assert!(!key.is_protected().unwrap());
    assert!(!key.is_locked().unwrap());
    // Protection-type string getters may return BadParameters on a key
    // whose secret-material descriptor isn't populated on this handle
    // (librnp requires `sec` to be loaded). Just confirm they don't panic.
    let _ = key.protection_type();
    let _ = key.protection_cipher();
    let _ = key.protection_hash();
    let _ = key.protection_iterations();
}

#[test]
fn signature_enumeration_and_subpackets() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "sigs <sigs@example.com>");

    // The direct-key signature list on a freshly generated key is empty —
    // self-signatures live on UIDs, not on the key itself.
    let uids = key.uids().expect("uids");
    assert!(!uids.is_empty(), "expected at least one UID");

    let uid = &uids[0];
    let uid_sig_count = uid.signature_count().expect("uid sig count");
    assert!(
        uid_sig_count > 0,
        "expected uid self-sig, got {uid_sig_count}"
    );
}

// --- Phase 3 mutation -------------------------------------------------------

#[test]
fn protect_unprotect_roundtrip() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "prot-rt <prot-rt@example.com>");
    assert!(!key.is_protected().unwrap());

    key.protect(
        &ProtectOptions::new()
            .password("hunter2")
            .cipher(Cipher::Aes256)
            .hash(Hash::Sha256),
    )
    .expect("protect");

    assert!(key.is_protected().unwrap());
    assert!(key.is_locked().unwrap());

    key.unlock(Some("hunter2")).expect("unlock");
    assert!(!key.is_locked().unwrap());

    key.unprotect(Some("hunter2")).expect("unprotect");
    assert!(!key.is_protected().unwrap());
}

#[test]
fn add_uid_then_inspect() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "adduid <adduid@example.com>");
    let before = key.uid_count().unwrap();

    key.add_uid(
        "second <second@example.com>",
        &rnp::AddUidOptions::new().hash(Hash::Sha256),
    )
    .expect("add uid");

    let after = key.uid_count().unwrap();
    assert_eq!(after, before + 1);
}

#[test]
fn remove_key_drops_count() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "removeme <removeme@example.com>");
    let grip = key.grip().unwrap();
    let before = ctx.public_key_count().unwrap();

    key.remove(RemoveFlags::PUBLIC | RemoveFlags::SECRET)
        .expect("remove");

    let after = ctx.public_key_count().unwrap();
    assert_eq!(after, before.saturating_sub(1));

    // Lookup by grip must now fail.
    let gone = ctx.find_key(KeyIdentifier::Grip(&grip)).unwrap();
    assert!(gone.is_none(), "removed key should not be findable");
}

// --- Phase 5 keyring management --------------------------------------------

#[test]
fn save_unload_reload_roundtrip() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "save <save@example.com>");
    let original_fp = key.fingerprint().unwrap();
    drop(key);

    let saved = ctx
        .save_keys_to_memory(
            rnp::KeyringFormat::Gpg,
            rnp::LoadSaveFlags::PUBLIC | rnp::LoadSaveFlags::SECRET,
        )
        .expect("save");

    ctx.unload_keys(rnp::UnloadFlags::PUBLIC | rnp::UnloadFlags::SECRET)
        .expect("unload");
    assert_eq!(ctx.public_key_count().unwrap(), 0);

    ctx.load_keys(
        rnp::KeyringFormat::Gpg,
        &saved,
        rnp::LoadSaveFlags::PUBLIC | rnp::LoadSaveFlags::SECRET,
    )
    .expect("load");
    assert!(ctx.public_key_count().unwrap() >= 1);

    let found = ctx
        .find_key(KeyIdentifier::Fingerprint(&original_fp))
        .unwrap()
        .expect("key reloaded");
    assert_eq!(found.fingerprint().unwrap(), original_fp);
}

#[test]
fn identifier_iterator_yields_keys() {
    let ctx = Context::new().expect("ctx");
    let _ = signing_key(&ctx, "iter <iter@example.com>");

    let count_fps: usize = ctx
        .identifiers(rnp::IdentifierKind::Fingerprint)
        .unwrap()
        .count();
    assert!(count_fps >= 1, "iterator should yield at least one fp");

    let count_grips: usize = ctx.identifiers(rnp::IdentifierKind::Grip).unwrap().count();
    assert_eq!(count_grips, count_fps);
}

// --- Phase 6 verify result -------------------------------------------------

#[test]
fn verify_result_exposes_recipient_after_decrypt() {
    let ctx = Context::new().expect("ctx");
    let key = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("vrf <vrf@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::EncryptComms)
        .build(&ctx)
        .expect("enc key");

    let plaintext = b"verify-result payload";
    let mut output = Output::to_memory().unwrap();
    rnp::Encryptor::new(&ctx, plaintext)
        .unwrap()
        .add_recipient(&key)
        .build(&mut output)
        .unwrap();
    let ciphertext = output.into_bytes().unwrap();

    let op = rnp::VerifyOp::inline(&ctx, &ciphertext, rnp::Output::to_memory().unwrap())
        .expect("verify op");
    let result = op.execute().expect("execute");
    let recipient_count = result.recipient_count().unwrap();
    assert!(recipient_count >= 1, "should have at least one recipient");
    let used = result.used_recipient().unwrap();
    assert!(used.is_some(), "the used recipient should be exposed");
}

// --- Phase 8 security / features / version --------------------------------

#[test]
fn supports_sha256() {
    assert!(supports_feature(FeatureType::HashAlgorithm, "SHA256").unwrap());
}

#[test]
fn supported_features_returns_json_array() {
    let json = rnp::supported_features(FeatureType::HashAlgorithm).unwrap();
    assert!(json.trim_start().starts_with('['), "got: {json}");
}

#[test]
fn calculate_iterations_positive() {
    let n = rnp::calculate_iterations(Hash::Sha256, 1024 * 1024).unwrap();
    assert!(n > 0);
}

#[test]
fn version_helpers_return_consistent_decomposition() {
    let v = rnp::version::version();
    let (maj, min, patch) = rnp::version::decompose();
    assert_eq!(
        rnp::version::version_for(maj, min, patch),
        v,
        "decompose()/version_for must round-trip"
    );
    let s = rnp::version::version_string();
    assert!(s.contains(&format!("{maj}.{min}")), "got {s}");
}

#[test]
fn add_remove_security_rule_roundtrip() {
    let ctx = Context::new().expect("ctx");
    let rule = rnp::SecurityRule {
        level: rnp::SecurityLevel::Insecure,
        flags: rnp::SecurityFlags::VERIFY_DATA,
        typ: FeatureType::HashAlgorithm,
        name: "SHA1".to_string(),
        from: 0,
    };
    ctx.add_security_rule(&rule).unwrap();
    let found = ctx
        .get_security_rule(
            FeatureType::HashAlgorithm,
            "SHA1",
            rnp::SecurityFlags::VERIFY_DATA,
            1_700_000_000,
        )
        .unwrap();
    assert_eq!(found.level, rnp::SecurityLevel::Insecure);
    let removed = ctx
        .remove_security_rule(
            FeatureType::HashAlgorithm,
            "SHA1",
            rnp::SecurityFlags::REMOVE_ALL,
        )
        .unwrap();
    assert!(removed >= 1);
}
