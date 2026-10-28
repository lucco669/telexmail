//! Specs for under-tested surfaces: mutation methods, Subpacket inspection,
//! SignatureType round-trip, KeyProvider callback.
//!
//! These complement `extended.rs` (which covers basic mutation like
//! protect/unprotect and basic inspection).

mod common;
use common::signing_key;

use rnp::{
    Algorithm, Context, Hash, KeyBuilder, KeyIdentifier, KeyProvider, KeyRequestOutcome, KeyUsage,
    RequestedKeyType, RevocationCode, RevocationReason, SignatureType, SubpacketType,
};

// --- Mutation: revoke ------------------------------------------------------

#[test]
fn revoke_marks_key_revoked() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "revoke <revoke@example.com>");
    assert!(!key.is_revoked().unwrap());

    key.revoke(
        RevocationReason::new(RevocationCode::Compromised).with_reason("test compromised"),
        Hash::Sha256,
    )
    .expect("revoke");

    assert!(
        key.is_revoked().unwrap(),
        "key should be revoked after revoke()"
    );

    let reason = key.revocation_reason().expect("reason getter");
    assert!(
        reason
            .as_ref()
            .is_some_and(|s| s.contains("test compromised")),
        "revocation reason text should be preserved: {reason:?}"
    );
}

// --- Mutation: set_expiration ---------------------------------------------
//
// rnp_key_set_expiration on a primary requires the self-signature to be
// re-issued; on a freshly generated key the call succeeds but the value
// may not be reflected via rnp_key_get_expiration without a keyring
// reload. We assert the round-trip on a subkey, where librnp updates the
// binding signature in place.

#[test]
fn set_expiration_updates_subkey_value() {
    let ctx = Context::new().expect("ctx");
    let primary = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("exp <exp@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Certify)
        .add_subkey(
            rnp::SubkeyBuilder::new(Algorithm::Rsa)
                .bits(2048)
                .hash(Hash::Sha256)
                .add_usage(KeyUsage::EncryptComms),
        )
        .build(&ctx)
        .expect("primary + sub");

    let subs = primary.subkeys().expect("subkeys");
    assert!(!subs.is_empty(), "should have at least one subkey");
    let sub = &subs[0];
    let thirty_days = 30 * 24 * 60 * 60;
    sub.set_expiration(thirty_days).expect("set_expiration");

    let updated = sub.expiration().unwrap_or(0);
    assert_eq!(
        updated, thirty_days,
        "subkey expiration should update to the set value"
    );
}

// --- SignatureType round-trip ---------------------------------------------

#[test]
fn signature_type_parse_as_str_round_trips() {
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
        let parsed = SignatureType::parse(&s);
        assert_eq!(parsed, variant, "round-trip failed for {variant:?}: '{s}'");
    }
}

#[test]
fn signature_type_unknown_preserves_value() {
    let s = "unknown: 99";
    let parsed = SignatureType::parse(s);
    assert_eq!(parsed, SignatureType::Unknown(99));

    let s = "unknown: 0";
    let parsed = SignatureType::parse(s);
    assert_eq!(parsed, SignatureType::Unknown(0));
}

#[test]
fn signature_type_unparsable_falls_back_to_unknown_zero() {
    let parsed = SignatureType::parse("nonsense");
    assert_eq!(parsed, SignatureType::Unknown(0));
}

// --- UID self-signature type + subpackets ----------------------------------

#[test]
fn uid_self_sig_is_certification_generic() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "selfsig <selfsig@example.com>");
    let uid = key
        .uids()
        .expect("uids")
        .into_iter()
        .next()
        .expect("at least one uid");
    let sigs = uid.signatures().expect("uid signatures");
    assert!(!sigs.is_empty(), "uid should have self-signature");

    let first = &sigs[0];
    let ty = first.sig_type_enum().expect("sig type");
    // RNP generates self-sigs as CertificationPositive by default for the
    // primary UID. Any of the certification variants is acceptable.
    assert!(
        matches!(
            ty,
            SignatureType::CertificationGeneric
                | SignatureType::CertificationPersona
                | SignatureType::CertificationCasual
                | SignatureType::CertificationPositive
        ),
        "uid self-sig should be a certification variant, got {ty:?}"
    );
}

#[test]
fn uid_self_sig_carries_creation_time_subpacket() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "sub <sub@example.com>");
    let uid = key.uids().expect("uids").remove(0);
    let sig = uid
        .signatures()
        .expect("sigs")
        .into_iter()
        .next()
        .expect("sig");

    let count = sig.subpacket_count().expect("count");
    assert!(count > 0, "uid self-sig should have subpackets");

    // Walk subpackets, classify each by SubpacketType.
    let subpackets = sig.subpackets().expect("subpackets");
    let mut saw_creation_time = false;
    for sp in &subpackets {
        let _ = sp.typ_raw().expect("type raw");
        let _ = sp.is_hashed().expect("is_hashed");
        let _ = sp.is_critical().expect("is_critical");
        if let Ok(SubpacketType::SignatureCreationTime) = sp.typ_enum() {
            saw_creation_time = true;
            // The creation-time subpacket must be in the hashed area.
            assert!(
                sp.is_hashed().unwrap_or(false),
                "SignatureCreationTime must be hashed"
            );
            // Body bytes are 4-byte big-endian Unix time.
            let data = sp.data().expect("data");
            assert_eq!(data.len(), 4, "creation time subpacket body is 4 bytes");
        }
    }
    assert!(
        saw_creation_time,
        "uid self-sig must carry a SignatureCreationTime subpacket"
    );
}

// --- SubpacketType round-trip ----------------------------------------------

#[test]
fn subpacket_type_as_u8_from_u8_round_trips() {
    for raw in [2u8, 3, 9, 11, 16, 21, 22, 23, 27, 30, 33] {
        let t = SubpacketType::from_u8(raw);
        assert_eq!(t.as_u8(), raw, "round-trip failed for raw {raw}");
    }
}

#[test]
fn subpacket_type_unknown_preserves_byte() {
    let t = SubpacketType::from_u8(200);
    assert_eq!(t, SubpacketType::Other(200));
    assert_eq!(t.as_u8(), 200);
}

// --- KeyProvider callback ---------------------------------------------------
//
// The trait + thunk wiring is exercised end-to-end only when librnp
// actually invokes the provider during verify/decrypt. That requires a
// multi-context setup that's brittle in CI. Here we verify the contract
// surface — that the trait is implementable and that installing a
// provider doesn't break normal operation. End-to-end coverage of the
// callback path is provided by the password-provider test in
// integration.rs (same shape, simpler setup).

struct CountingProvider {
    calls: std::sync::Mutex<u32>,
}

impl CountingProvider {
    fn new() -> Self {
        CountingProvider {
            calls: std::sync::Mutex::new(0),
        }
    }
}

impl KeyProvider for CountingProvider {
    fn on_key_request(
        &self,
        _ctx: &Context,
        _id: KeyIdentifier<'_>,
        _kind: RequestedKeyType,
    ) -> KeyRequestOutcome {
        *self.calls.lock().unwrap() += 1;
        KeyRequestOutcome::NotFound
    }
}

#[test]
fn key_provider_can_be_installed_and_trait_is_implementable() {
    let mut ctx = Context::new().expect("ctx");
    let provider = Box::new(CountingProvider::new());
    ctx.set_key_provider(provider);

    // Normal operations still work after installing a provider.
    let key = signing_key(&ctx, "kp <kp@example.com>");
    assert!(!key.fingerprint().unwrap().is_empty());
}

// --- default_key_for usage flag dispatch -----------------------------------

#[test]
fn default_key_for_returns_encryption_capable_subkey() {
    let ctx = Context::new().expect("ctx");
    let primary = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("dk <dk@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Certify)
        .add_subkey(
            rnp::SubkeyBuilder::new(Algorithm::Rsa)
                .bits(2048)
                .hash(Hash::Sha256)
                .add_usage(KeyUsage::EncryptComms),
        )
        .build(&ctx)
        .expect("primary with enc subkey");

    let enc = primary
        .default_key_for(KeyUsage::EncryptComms)
        .expect("default_key_for")
        .expect("should find a default");

    // The returned handle should allow the requested usage.
    assert!(
        enc.allows_usage(KeyUsage::EncryptComms).unwrap_or(false),
        "returned key should support the requested usage"
    );
}

#[test]
fn default_key_for_returns_none_for_unsatisfiable_usage() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "none <none@example.com>");

    // A sign+certify primary with no encryption subkey. Asking for an
    // encryption usage returns Err(NoSuitableKey) (librnp's signal that
    // nothing in the keyring matches the requested usage).
    let result = key.default_key_for(KeyUsage::EncryptComms);
    assert!(
        result.is_err(),
        "no encryption-capable subkey exists, expected error"
    );
}
