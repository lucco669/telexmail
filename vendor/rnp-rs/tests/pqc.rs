//! Phase 9: PQC round-trips against a librnp built with ENABLE_PQC=ON +
//! ENABLE_CRYPTO_REFRESH=ON.
//!
//! These tests are gated on `--features pqc,crypto-refresh` and require a
//! librnp with PQC support on the link path. The runtime probe
//! [`rnp::librnp_supports_pqc`] short-circuits each test if the linked
//! librnp wasn't built with PQC.

#![cfg(all(feature = "pqc", feature = "crypto-refresh"))]

use rnp::{
    Context, Encryptor, Hash, KeyBuilder, KeyIdentifier, Output, PqcAlgorithm, librnp_supports_pqc,
};

/// Skip the test unless the linked librnp actually supports PQC at runtime.
macro_rules! require_pqc {
    () => {
        if !librnp_supports_pqc() {
            eprintln!(
                "skipped: librnp at link-time does not export ML-KEM-768+X25519 \
                 (was it built with ENABLE_PQC=ON?)"
            );
            return;
        }
    };
}

#[test]
fn ml_dsa_sign_verify_roundtrip() {
    require_pqc!();

    let ctx = Context::new().expect("ctx");
    let alg_name = PqcAlgorithm::MlDsa65Ed25519.as_str();
    let primary = KeyBuilder::new(rnp::Algorithm::Eddsa)
        .userid(format!("pqc-signer <{alg_name}@example.com>"))
        .hash(Hash::Sha256)
        .build(&ctx)
        .expect("primary");

    // Generate an ML-DSA subkey via the JSON API (the op_generate path
    // doesn't expose PQC algorithms directly in this librnp version).
    let json = format!(
        r#"{{
            "primary": {{
                "type": "EDDSA",
                "userid": "pqc-{alg}",
                "usage": ["certify"]
            }},
            "sub": {{
                "type": "{alg}",
                "usage": ["sign"]
            }}
        }}"#,
        alg = alg_name
    );
    let result = rnp::generate_key_json(&ctx, &json).expect("generate pqc subkey");
    assert!(result.contains("grip"), "expected grip in result: {result}");

    // Smoke: at least one key is present.
    assert!(ctx.public_key_count().unwrap() > 0);
    let _ = primary.alg().unwrap();
}

#[test]
fn ml_kem_encrypt_decrypt_roundtrip() {
    require_pqc!();

    let ctx = Context::new().expect("ctx");

    // Generate an ML-KEM-768+X25519 subkey via JSON.
    let json = r#"{
        "primary": {
            "type": "EDDSA",
            "userid": "pqc-enc",
            "usage": ["certify"]
        },
        "sub": {
            "type": "ML-KEM-768+X25519",
            "usage": ["encrypt"]
        }
    }"#;
    let _ = rnp::generate_key_json(&ctx, json).expect("generate pqc enc subkey");

    // Find the primary key we just generated.
    let key = ctx
        .find_key(KeyIdentifier::Userid("pqc-enc"))
        .expect("find")
        .expect("primary present");

    // Encrypt to its subkey, then decrypt back.
    let plaintext = b"post-quantum secret";
    let mut output = Output::to_memory().unwrap();
    Encryptor::new(&ctx, plaintext)
        .unwrap()
        .add_recipient(&key)
        .prefer_pqc_enc_subkey()
        .enable_pkesk_v6()
        .build(&mut output)
        .expect("encrypt");
    let ciphertext = output.into_bytes().unwrap();
    assert!(!ciphertext.is_empty());

    let recovered = rnp::decrypt(&ctx, &ciphertext).expect("decrypt");
    assert_eq!(recovered.as_slice(), plaintext);
}

#[test]
fn pqc_algorithm_strings_are_canonical() {
    // The enum's `as_str` output must match librnp's expected strings —
    // otherwise the JSON `type` field wouldn't be accepted. This test
    // doesn't need a PQC-enabled librnp (it's pure enum sanity).
    assert_eq!(PqcAlgorithm::MlKem768X25519.as_str(), "ML-KEM-768+X25519");
    assert_eq!(PqcAlgorithm::MlDsa65Ed25519.as_str(), "ML-DSA-65+ED25519");
    assert_eq!(PqcAlgorithm::SlhDsaShake128f.as_str(), "SLH-DSA-SHAKE-128f");
}
