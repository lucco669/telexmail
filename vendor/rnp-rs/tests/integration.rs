//! Phase 24-29 integration tests: cross-feature scenarios.

#![allow(deprecated)]

use rnp::{
    Algorithm, Cipher, Compression, Context, ErrorKind, Hash, KeyBuilder, KeyIdentifier, KeyUsage,
    Output, ProtectOptions, SubkeyBuilder, from_rnp_code, unknown_variant,
};
use std::str::FromStr;

// --- Phase 24: raw key data ------------------------------------------------

#[test]
fn raw_public_data_dumps_as_key_packet() {
    let ctx = Context::new().expect("ctx");
    let key = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("raw <raw@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Sign)
        .add_usage(KeyUsage::Certify)
        .build(&ctx)
        .expect("key");

    let raw = key.raw_public_data().expect("raw_public_data");
    assert!(!raw.is_empty(), "raw public data should be non-empty");

    let json = rnp::dump_packets_bytes_to_json(&raw, rnp::JsonDumpFlags::default()).expect("dump");
    assert!(
        json.contains("Public Key"),
        "raw public data should dump as a Public Key packet: {json}"
    );
}

#[test]
fn raw_secret_data_available_when_unlocked() {
    let ctx = Context::new().expect("ctx");
    let key = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("rawsec <rawsec@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Sign)
        .add_usage(KeyUsage::Certify)
        .build(&ctx)
        .expect("key");
    let raw = key.raw_secret_data().expect("raw_secret_data");
    assert!(!raw.is_empty());
}

// --- Phase 25: FromStr / Display round-trips -------------------------------

#[test]
fn fromstr_display_round_trip() {
    // Each known variant should round-trip via parse -> to_string.
    for s in ["RSA", "EDDSA", "ECDSA", "ECDH", "SM2", "DSA"] {
        let alg = Algorithm::from_str(s).unwrap_or_else(|e| panic!("parse {s}: {e:?}"));
        assert_eq!(alg.to_string(), s);
    }
    for s in ["SHA256", "SHA512", "SHA3-256"] {
        let h = Hash::from_str(s).unwrap();
        assert_eq!(h.to_string(), s);
    }
    for s in ["AES128", "AES256", "TWOFISH"] {
        let c = Cipher::from_str(s).unwrap();
        assert_eq!(c.to_string(), s);
    }
    for s in ["ZIP", "ZLIB", "BZIP2"] {
        let c = Compression::from_str(s).unwrap();
        assert_eq!(c.to_string(), s);
    }
}

#[test]
fn fromstr_case_insensitive_for_algorithm_and_hash() {
    // C-side is case-insensitive for many strings; our FromStr matches.
    assert_eq!(Algorithm::from_str("rsa").unwrap(), Algorithm::Rsa);
    assert_eq!(Algorithm::from_str("EdDSA").unwrap(), Algorithm::Eddsa);
    assert_eq!(Hash::from_str("sha256").unwrap(), Hash::Sha256);
}

#[test]
fn fromstr_unknown_returns_unknown_variant_error() {
    let err = Algorithm::from_str("not-a-real-algorithm").unwrap_err();
    assert!(matches!(err.kind(), ErrorKind::BadParameters));
    let msg = format!("{err}");
    assert!(msg.contains("algorithm"), "got: {msg}");
    assert!(msg.contains("not-a-real-algorithm"));
}

#[test]
fn unknown_variant_helper_direct() {
    let err = unknown_variant("test kind", "test value");
    let msg = format!("{err}");
    assert!(msg.contains("test kind"));
    assert!(msg.contains("test value"));
}

// --- Phase 26: error interop -----------------------------------------------

#[test]
fn from_rnp_code_constructs_correct_kind() {
    let err = from_rnp_code(0x1000_0008); // RNP_ERROR_NOT_FOUND
    assert_eq!(err.kind(), ErrorKind::NotFound);
    // The message comes from librnp's rnp_result_to_string; we don't
    // assert its exact text (it varies between versions) but it should
    // be non-empty.
    assert!(!format!("{err}").is_empty());
}

#[test]
fn io_error_round_trip_through_rnp_error() {
    // Round-trip an io::Error through rnp::Error::Io. The exact kind
    // isn't preserved on the way back (we wrap as io::Error::new which
    // loses the original kind), but the message text is.
    let io = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
    let rnp_err: rnp::Error = io.into();
    assert!(matches!(rnp_err, rnp::Error::Io { .. }));

    let back: std::io::Error = rnp_err.into();
    // The Display text preserves the original message.
    assert!(format!("{back}").contains("denied"));
}

#[test]
fn rnp_error_to_io_error_preserves_kind_for_not_found() {
    let err = from_rnp_code(0x1000_0008);
    let io: std::io::Error = err.into();
    assert_eq!(io.kind(), std::io::ErrorKind::NotFound);
}

// --- Phase 27: cross-feature scenarios -------------------------------------

#[test]
fn multi_recipient_encryption_round_trips() {
    let ctx = Context::new().expect("ctx");
    let alice = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("alice <alice@multi.example>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::EncryptComms)
        .build(&ctx)
        .expect("alice");
    let bob = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("bob <bob@multi.example>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::EncryptComms)
        .build(&ctx)
        .expect("bob");

    let plaintext = b"shared secret";
    let mut output = Output::to_memory().unwrap();
    rnp::Encryptor::new(&ctx, plaintext)
        .unwrap()
        .add_recipient(&alice)
        .add_recipient(&bob)
        .cipher(Cipher::Aes256)
        .build(&mut output)
        .expect("encrypt");
    let ciphertext = output.into_bytes().unwrap();

    // Either key alone should decrypt.
    let recovered = rnp::decrypt(&ctx, &ciphertext).expect("decrypt");
    assert_eq!(recovered.as_slice(), plaintext);
}

#[test]
fn sign_and_encrypt_combined() {
    let ctx = Context::new().expect("ctx");
    let signer = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("signer <signer@se.example>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Sign)
        .add_usage(KeyUsage::Certify)
        .build(&ctx)
        .expect("signer");
    let recipient = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("recipient <rc@se.example>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::EncryptComms)
        .build(&ctx)
        .expect("recipient");

    let plaintext = b"signed and encrypted";
    let mut output = Output::to_memory().unwrap();
    rnp::Encryptor::new(&ctx, plaintext)
        .unwrap()
        .add_recipient(&recipient)
        .add_signature(&signer)
        .cipher(Cipher::Aes256)
        .build(&mut output)
        .expect("sign+encrypt");
    let ciphertext = output.into_bytes().unwrap();

    let recovered = rnp::decrypt(&ctx, &ciphertext).expect("decrypt");
    assert_eq!(recovered.as_slice(), plaintext);
}

#[test]
fn password_and_recipient_encryption() {
    let ctx = Context::new().expect("ctx");
    let recipient = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("pwrc <pwrc@example>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::EncryptComms)
        .build(&ctx)
        .expect("recipient");

    let plaintext = b"either/or";
    let mut output = Output::to_memory().unwrap();
    rnp::Encryptor::new(&ctx, plaintext)
        .unwrap()
        .add_recipient(&recipient)
        .add_password("shared-pass", Default::default())
        .cipher(Cipher::Aes256)
        .build(&mut output)
        .expect("build");
    let ciphertext = output.into_bytes().unwrap();

    // Set up a password provider so decrypt can use the password path.
    struct Pw;
    impl rnp::PasswordProvider for Pw {
        fn get_password(
            &self,
            _k: Option<&rnp::Key>,
            _c: &str,
        ) -> Option<std::borrow::Cow<'_, str>> {
            Some("shared-pass".into())
        }
    }
    let mut ctx2 = Context::new().expect("ctx2");
    ctx2.set_password_provider(Box::new(Pw));
    let recovered = rnp::decrypt(&ctx2, &ciphertext).expect("decrypt via password");
    assert_eq!(recovered.as_slice(), plaintext);
}

#[test]
fn multiple_subkeys_on_one_primary() {
    let ctx = Context::new().expect("ctx");
    let primary = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("multi-sub <ms@example>")
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
        .expect("sub1");
    SubkeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::EncryptStorage)
        .build(&ctx, &primary)
        .expect("sub2");

    let fp = primary.fingerprint().unwrap();
    let again = ctx
        .find_key(KeyIdentifier::Fingerprint(&fp))
        .unwrap()
        .expect("primary");
    let subs = again.subkeys().unwrap();
    assert_eq!(subs.len(), 2, "expected 2 subkeys, got {}", subs.len());
}

#[test]
fn keygen_protection_via_request_password_provider() {
    // Generate a key with `request_password()` set; the password comes
    // from the provider at execution time.
    struct Pw;
    impl rnp::PasswordProvider for Pw {
        fn get_password(
            &self,
            _k: Option<&rnp::Key>,
            _c: &str,
        ) -> Option<std::borrow::Cow<'_, str>> {
            Some("provider-pw".into())
        }
    }
    let mut ctx = Context::new().expect("ctx");
    ctx.set_password_provider(Box::new(Pw));

    let key = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("reqpw <reqpw@example>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Sign)
        .add_usage(KeyUsage::Certify)
        .protection(
            &ProtectOptions::new()
                .cipher(Cipher::Aes256)
                .hash(Hash::Sha256),
        )
        .request_password()
        .build(&ctx)
        .expect("protected key");

    assert!(key.is_protected().unwrap());
    key.unlock(Some("provider-pw")).expect("unlock");
    assert!(!key.is_locked().unwrap());
}
