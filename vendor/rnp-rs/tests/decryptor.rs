//! Specs for the Decryptor builder (Phase 63).

#![allow(deprecated)]

use rnp::{Algorithm, Context, Decryptor, Encryptor, KeyBuilder, KeyUsage, Output};

fn make_enc_key(ctx: &Context) -> rnp::Key<'_> {
    KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("enc <enc@example.com>")
        .add_usage(KeyUsage::EncryptComms)
        .build(ctx)
        .expect("enc key")
}

#[test]
fn decryptor_round_trips_plaintext() {
    let ctx = Context::new().expect("ctx");
    let key = make_enc_key(&ctx);

    let plaintext = b"decryptor payload";
    let mut out = Output::to_memory().unwrap();
    Encryptor::new(&ctx, plaintext)
        .unwrap()
        .add_recipient(&key)
        .build(&mut out)
        .unwrap();
    let ciphertext = out.into_bytes().unwrap();

    let result = Decryptor::new(&ctx, &ciphertext).build().expect("decrypt");
    assert_eq!(
        result.plaintext(),
        plaintext,
        "decryptor should return the original plaintext"
    );
}

#[test]
fn decryptor_exposes_recipient_metadata() {
    let ctx = Context::new().expect("ctx");
    let key = make_enc_key(&ctx);

    let mut out = Output::to_memory().unwrap();
    Encryptor::new(&ctx, b"meta")
        .unwrap()
        .add_recipient(&key)
        .build(&mut out)
        .unwrap();
    let ciphertext = out.into_bytes().unwrap();

    let result = Decryptor::new(&ctx, &ciphertext).build().expect("decrypt");

    let count = result.recipient_count().expect("count");
    assert!(
        count >= 1,
        "should expose at least one recipient, got {count}"
    );

    let used = result.used_recipient().expect("used");
    assert!(
        used.is_some(),
        "should expose the recipient that actually decrypted"
    );

    let (mode, _cipher, valid) = result.protection_info().expect("protection");
    assert!(
        !mode.is_empty(),
        "protection mode should be a non-empty string"
    );
    assert!(valid, "MDC/AEAD integrity should be verified");
}

#[test]
fn decryptor_into_plaintext_consumes_result() {
    let ctx = Context::new().expect("ctx");
    let key = make_enc_key(&ctx);

    let mut out = Output::to_memory().unwrap();
    Encryptor::new(&ctx, b"consume")
        .unwrap()
        .add_recipient(&key)
        .build(&mut out)
        .unwrap();
    let ciphertext = out.into_bytes().unwrap();

    let result = Decryptor::new(&ctx, &ciphertext).build().expect("decrypt");
    let bytes = result.into_plaintext();
    assert_eq!(bytes, b"consume");
}
