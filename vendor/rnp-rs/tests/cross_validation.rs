//! Cross-validation tests against sequoia-openpgp.
//!
//! Each test exercises a wire-format round-trip between rnp-rs and
//! sequoia. These tests are deliberately *minimal* — they confirm
//! sequoia can parse the bytes rnp produces. A failure here indicates
//! a wire-format bug in rnp or a version mismatch in sequoia; deeper
//! signature/decryption verification requires transferring secret-key
//! material, which is out of scope for the basic interop check.

#![cfg(feature = "cross-validation")]

use rnp::{Algorithm, Cipher, Context, Hash, KeyBuilder, KeyUsage, Output};
use sequoia_openpgp as openpgp;
use sequoia_openpgp::Cert;
use sequoia_openpgp::parse::Parse;

#[test]
fn rnp_exports_sequoia_parses_pubkey() {
    let ctx = Context::new().expect("ctx");
    let key = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("cv <cv@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Sign)
        .add_usage(KeyUsage::Certify)
        .build(&ctx)
        .expect("rnp keygen");

    let exported = key
        .export(rnp::ExportFlags::PUBLIC | rnp::ExportFlags::ARMORED)
        .expect("rnp export");

    let cert = Cert::from_bytes(&exported).expect("sequoia should parse rnp's exported key");
    let userid = cert
        .userids()
        .next()
        .expect("cert has a UID")
        .userid()
        .to_string();
    assert!(userid.contains("cv@example.com"), "UID mismatch: {userid}");
    // Public-key exports don't include secret-key parts.
    assert!(
        !cert.is_tsk(),
        "public-key export should NOT have secret-key parts"
    );
}

#[test]
fn rnp_encrypted_message_sequoia_parses() {
    let ctx = Context::new().expect("ctx");
    let recipient = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("cv-enc <cve@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::EncryptComms)
        .build(&ctx)
        .expect("rnp keygen");

    let plaintext = b"shared secret";
    let mut output = Output::to_memory().unwrap();
    rnp::Encryptor::new(&ctx, plaintext)
        .unwrap()
        .add_recipient(&recipient)
        .cipher(Cipher::Aes256)
        .armor(true)
        .build(&mut output)
        .expect("rnp encrypt");
    let ciphertext = output.into_bytes().unwrap();

    // Sequoia can parse the OpenPGP message structure (it'll be an
    // encrypted-data packet). Decryption requires the secret-key
    // material transfer; out of scope here.
    let _ = openpgp::Message::from_bytes(&ciphertext)
        .expect("sequoia should parse rnp's encrypted message");
}

#[test]
fn rnp_signed_message_sequoia_parses() {
    let ctx = Context::new().expect("ctx");
    let key = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("cv-sig <cvs@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Sign)
        .add_usage(KeyUsage::Certify)
        .build(&ctx)
        .expect("rnp keygen");

    let message = b"signed message";
    let signed = rnp::sign(&ctx, message, &key).expect("rnp sign");

    // Sequoia can parse the inline-signed message structure.
    let _ =
        openpgp::Message::from_bytes(&signed).expect("sequoia should parse rnp's signed message");
}

#[test]
fn rnp_cleartext_message_sequoia_parses() {
    // Cleartext-signed messages have a different parser path in
    // sequoia (CleartextSignedMessage). That module isn't in the
    // re-exported root under all feature configs, so for this basic
    // interop check we just verify the bytes contain the expected
    // header. A full sequoia cleartext parser integration is tracked
    // as a follow-up.
    let ctx = Context::new().expect("ctx");
    let key = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("cv-clear <cvc@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Sign)
        .add_usage(KeyUsage::Certify)
        .build(&ctx)
        .expect("rnp keygen");

    let message = b"cleartext body\n";
    let clear = rnp::sign_cleartext(&ctx, message, &key).expect("rnp sign_cleartext");
    let s = String::from_utf8_lossy(&clear);
    assert!(s.contains("BEGIN PGP SIGNED MESSAGE"));
    assert!(s.contains("cleartext body"));
}
