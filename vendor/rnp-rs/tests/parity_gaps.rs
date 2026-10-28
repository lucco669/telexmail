//! Phase 11-17 parity-gap tests: UID signatures, keygen protection, signature
//! lifecycle, verify result, logging/key provider, buffer hygiene, misc
//! getters.

mod common;
use common::signing_key;

use rnp::{
    Algorithm, Cipher, Context, Hash, KeyBuilder, KeyIdentifier, KeyProvider, KeyRequestOutcome,
    KeyUsage, ProtectOptions, RequestedKeyType, SecretString, SubkeyBuilder, SubpacketType,
    request_password,
};

// --- Phase 11: UID signatures ----------------------------------------------

#[test]
fn uid_self_signature_has_key_flags() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "selfsig <selfsig@example.com>");

    let uids = key.uids().expect("uids");
    let uid = &uids[0];
    let sigs = uid.signatures().expect("uid signatures");
    assert!(!sigs.is_empty(), "self-certification should be present");

    // The first self-sig typically carries key flags.
    let self_sig = &sigs[0];
    let flags = self_sig.key_flags().unwrap_or(0);
    assert!(
        flags != 0,
        "self-certification should have key flags, got 0"
    );
}

#[test]
fn uid_revocation_signature_none_when_not_revoked() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "revoketest <rev@example.com>");
    let uid = &key.uids().expect("uids")[0];
    assert!(!uid.is_revoked().unwrap());
    let rev = uid.revocation_signature().expect("call");
    assert!(rev.is_none(), "non-revoked UID has no revocation sig");
}

// --- Phase 12: Keygen protection + v6 --------------------------------------

#[test]
fn keygen_protection_at_generation_time() {
    let ctx = Context::new().expect("ctx");
    let key = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("protected <prot@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Sign)
        .add_usage(KeyUsage::Certify)
        .protection(
            &ProtectOptions::new()
                .password("hunter2")
                .cipher(Cipher::Aes256)
                .hash(Hash::Sha256),
        )
        .build(&ctx)
        .expect("protected key");

    assert!(key.is_protected().unwrap(), "should be protected");
    assert!(
        key.is_locked().unwrap(),
        "should be locked after generation"
    );
    key.unlock(Some("hunter2")).expect("unlock");
    assert!(
        !key.is_locked().unwrap(),
        "should be unlocked after password"
    );
}

// --- Phase 13: Signature lifecycle -----------------------------------------

#[test]
fn signature_export_round_trips_via_dump() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "siglife <siglife@example.com>");
    let uid = &key.uids().expect("uids")[0];
    let sig = &uid.signatures().expect("sigs")[0];

    let bytes = sig.export(rnp::ExportFlags::default()).expect("export");
    assert!(!bytes.is_empty());

    // Re-parse the exported bytes as a packet stream.
    let json =
        rnp::dump_packets_bytes_to_json(&bytes, rnp::JsonDumpFlags::default()).expect("dump");
    assert!(
        json.contains("Signature"),
        "exported bytes should contain a Signature packet: {json}"
    );
}

#[test]
fn signature_find_subpacket_keyflags() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "findpkt <find@example.com>");
    let uid = &key.uids().expect("uids")[0];
    let sig = &uid.signatures().expect("sigs")[0];

    let found = sig
        .find_subpacket(SubpacketType::KeyFlags, None, 0)
        .expect("find");
    assert!(found.is_some(), "self-certification should have KeyFlags");
}

#[test]
fn subpacket_type_enum_round_trip() {
    // Known numeric tags map to typed variants and back.
    assert_eq!(SubpacketType::from_u8(27), SubpacketType::KeyFlags);
    assert_eq!(SubpacketType::KeyFlags.as_u8(), 27);

    // Unknown tag falls through to Other.
    let unknown = SubpacketType::from_u8(200);
    assert_eq!(unknown.as_u8(), 200);
}

// --- Phase 14: Verify result rich ------------------------------------------

#[test]
fn verify_signature_handle_exposes_signer() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "signer <signer@example.com>");

    let plaintext = b"some bytes";
    let signed = rnp::sign(&ctx, plaintext, &key).expect("sign");

    let op =
        rnp::VerifyOp::inline(&ctx, &signed, rnp::Output::to_null().unwrap()).expect("verify op");
    let result = op.execute().expect("execute");
    let sigs = result.signatures().expect("sigs");
    assert_eq!(sigs.len(), 1);

    let s = &sigs[0];
    // The handle gives full Signature access.
    let h = s.handle().expect("handle");
    let _alg = h.alg().expect("alg");

    // The key accessor returns the signing key.
    let signer_key = s.key().expect("key call");
    assert!(signer_key.is_some(), "signer should be in the keyring");
    assert_eq!(signer_key.unwrap().keyid().unwrap(), key.keyid().unwrap());
}

// --- Phase 15: Key provider -------------------------------------------------

#[test]
fn key_provider_callback_invoked() {
    // Use two contexts: one to generate + export a key, another to verify
    // a signature without the key in the keyring (so the provider fires).
    let setup_ctx = Context::new().expect("setup ctx");
    let key = signing_key(&setup_ctx, "kp <kp@example.com>");
    let plaintext = b"kp-test";
    let sig = rnp::sign_detached(&setup_ctx, plaintext, &key).expect("sign detached");
    let pub_key_bytes = key
        .export(rnp::ExportFlags::PUBLIC | rnp::ExportFlags::ARMORED)
        .expect("export");

    let mut verify_ctx = Context::new().expect("verify ctx");
    let provider = StaticKeyProvider::new(pub_key_bytes);
    verify_ctx.set_key_provider(Box::new(provider));

    let result = rnp::verify_detached(&verify_ctx, plaintext, &sig).expect("verify");
    assert!(
        result.any_valid().unwrap_or(false),
        "key provider should have satisfied the lookup"
    );
}

struct StaticKeyProvider {
    bytes: std::sync::Arc<Vec<u8>>,
}

impl StaticKeyProvider {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes: std::sync::Arc::new(bytes),
        }
    }
}

impl KeyProvider for StaticKeyProvider {
    fn on_key_request(
        &self,
        ctx: &Context,
        _id: KeyIdentifier<'_>,
        _kind: RequestedKeyType,
    ) -> KeyRequestOutcome {
        let bytes = (*self.bytes).clone();
        match ctx.load_keys(rnp::KeyringFormat::Gpg, &bytes, rnp::LoadSaveFlags::PUBLIC) {
            Ok(()) => KeyRequestOutcome::Found,
            Err(_) => KeyRequestOutcome::NotFound,
        }
    }
}

// --- Phase 16: Buffer hygiene ----------------------------------------------

#[test]
fn secret_string_round_trips() {
    let s = SecretString::from_str("hunter2");
    assert_eq!(s.as_str(), "hunter2");
    assert_eq!(format!("{:?}", s), "SecretString(***)");
}

#[test]
fn secret_string_into_yields_original() {
    let s = SecretString::from_str("secret-value");
    let owned = s.into_string();
    assert_eq!(owned, "secret-value");
}

#[test]
fn request_password_returns_secret_string() {
    // Set up a password provider; request_password should retrieve via it.
    struct StaticPw;
    impl rnp::PasswordProvider for StaticPw {
        fn get_password(
            &self,
            _key: Option<&rnp::Key>,
            _ctx: &str,
        ) -> Option<std::borrow::Cow<'_, str>> {
            Some("pw-via-provider".into())
        }
    }
    let mut ctx = Context::new().expect("ctx");
    ctx.set_password_provider(Box::new(StaticPw));

    let pw = request_password(&ctx, None, "test").expect("call");
    assert!(pw.is_some(), "provider returned Some");
    assert_eq!(pw.unwrap().as_str(), "pw-via-provider");
}

// --- Phase 17: Misc getters ------------------------------------------------

#[test]
fn default_key_for_returns_subkey_for_encrypt_usage() {
    let ctx = Context::new().expect("ctx");
    let primary = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("dk <dk@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Sign)
        .add_usage(KeyUsage::Certify)
        .build(&ctx)
        .expect("primary");

    SubkeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::EncryptComms)
        .build(&ctx, &primary)
        .expect("subkey");

    let fp = primary.fingerprint().unwrap();
    let primary_again = ctx
        .find_key(KeyIdentifier::Fingerprint(&fp))
        .unwrap()
        .expect("primary");

    let default_enc = primary_again
        .default_key_for(KeyUsage::EncryptComms)
        .expect("call");
    assert!(default_enc.is_some(), "should return an encryption subkey");
    let enc_key = default_enc.unwrap();
    assert!(
        enc_key.is_sub().unwrap(),
        "default for encrypt should be a subkey"
    );
}

#[test]
fn signature_signer_key_returns_primary_for_self_cert() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "sig-key <sk@example.com>");
    let uid = &key.uids().expect("uids")[0];
    let sig = &uid.signatures().expect("sigs")[0];

    let signer = sig.signer_key().expect("call");
    assert!(signer.is_some(), "self-cert signer should be in keyring");
    assert_eq!(signer.unwrap().keyid().unwrap(), key.keyid().unwrap());
}
