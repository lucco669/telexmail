//! Phase 4: KeyBuilder + SubkeyBuilder + algorithm enums.

use rnp::{
    Algorithm, Cipher, Compression, Context, Hash, KeyBuilder, KeyIdentifier, KeyUsage,
    SubkeyBuilder, generate_key_json,
};

#[test]
fn key_builder_rsa_basic() {
    let ctx = Context::new().expect("ctx");
    let key = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("builder <builder@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Sign)
        .add_usage(KeyUsage::Certify)
        .build(&ctx)
        .expect("build rsa key");

    let uid = key
        .primary_uid()
        .expect("primary uid")
        .expect("uid present");
    assert_eq!(uid, "builder <builder@example.com>");
}

#[test]
fn key_builder_eddsa() {
    // EDDSA uses Ed25519 implicitly — set_curve is rejected for it.
    let ctx = Context::new().expect("ctx");
    let key = KeyBuilder::new(Algorithm::Eddsa)
        .userid("eddsa <eddsa@example.com>")
        .hash(Hash::Sha256)
        .build(&ctx)
        .expect("build eddsa key");
    let uid = key.primary_uid().expect("uid").expect("present");
    assert!(uid.contains("eddsa"));
}

#[test]
fn key_builder_preferences() {
    let ctx = Context::new().expect("ctx");
    let key = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("p refs <prefs@example.com>")
        .hash(Hash::Sha256)
        .add_pref_hash(Hash::Sha512)
        .add_pref_hash(Hash::Sha256)
        .add_pref_cipher(Cipher::Aes256)
        .add_pref_compression(Compression::Zlib)
        .pref_keyserver("hkp://keys.example.com")
        .build(&ctx)
        .expect("build with prefs");
    assert!(key.primary_uid().unwrap().is_some());
}

#[test]
fn subkey_builder_attaches_to_primary() {
    let ctx = Context::new().expect("ctx");
    let primary = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("primary <primary@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Certify)
        .add_usage(KeyUsage::Sign)
        .build(&ctx)
        .expect("primary");

    let _sub = SubkeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::EncryptComms)
        .build(&ctx, &primary)
        .expect("subkey");

    // Primary should now be locatable by userid after both generations.
    let found = ctx
        .find_key(KeyIdentifier::Userid("primary <primary@example.com>"))
        .expect("find")
        .expect("present");
    assert!(found.primary_uid().unwrap().is_some());
}

#[test]
fn generate_key_json_smoke() {
    let ctx = Context::new().expect("ctx");
    // Schema follows librnp's expected `primary`/`sub` shape — see
    // ../rnp/src/examples/generate.c.
    let json = r#"{
        "primary": {
            "type": "RSA",
            "length": 1024,
            "userid": "alaric <alaric@example.com>"
        }
    }"#;
    let result = generate_key_json(&ctx, json).expect("generate");
    assert!(
        result.contains("grip"),
        "result should include the primary key grip: {result}"
    );
}
