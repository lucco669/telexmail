//! Phase 6: encryption + decryption round-trips.

use rnp::{Algorithm, Cipher, Context, Encryptor, Hash, KeyBuilder, KeyUsage, Output, decrypt};
use rnp::{Compression, ErrorKind};

fn make_encryption_key<'a>(ctx: &'a Context, uid: &str) -> rnp::Key<'a> {
    KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid(uid)
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::EncryptComms)
        .build(ctx)
        .expect("encryption key")
}

#[test]
fn encrypt_to_recipient_decrypt_roundtrip() {
    let ctx = Context::new().expect("ctx");
    let key = make_encryption_key(&ctx, "enc <enc@example.com>");

    let plaintext = b"the eagle flies at midnight";
    let mut output = Output::to_memory().expect("output");
    Encryptor::new(&ctx, plaintext)
        .expect("builder")
        .add_recipient(&key)
        .cipher(Cipher::Aes256)
        .build(&mut output)
        .expect("encrypt");
    let ciphertext = output.into_bytes().expect("drain");
    assert_ne!(&ciphertext[..], &plaintext[..]);

    let recovered = decrypt(&ctx, &ciphertext).expect("decrypt");
    assert_eq!(recovered.as_slice(), plaintext);
}

#[test]
fn encrypt_with_password_roundtrip() {
    let ctx = Context::new().expect("ctx");

    let plaintext = b"password protected";
    let mut output = Output::to_memory().expect("output");
    Encryptor::new(&ctx, plaintext)
        .expect("builder")
        .add_password("hunter2", Default::default())
        .cipher(Cipher::Aes256)
        .build(&mut output)
        .expect("encrypt");
    let ciphertext = output.into_bytes().expect("drain");

    // Set up a password provider so decrypt can return the password.
    struct StaticPw;
    impl rnp::PasswordProvider for StaticPw {
        fn get_password(
            &self,
            _key: Option<&rnp::Key>,
            _context: &str,
        ) -> Option<std::borrow::Cow<'_, str>> {
            Some("hunter2".into())
        }
    }
    let mut ctx2 = Context::new().expect("ctx2");
    ctx2.set_password_provider(Box::new(StaticPw));

    let recovered = decrypt(&ctx2, &ciphertext).expect("decrypt");
    assert_eq!(recovered.as_slice(), plaintext);
}

#[test]
fn encrypt_armored_output_has_armor_header() {
    let ctx = Context::new().expect("ctx");
    let key = make_encryption_key(&ctx, "armored <armored@example.com>");

    let plaintext = b"armored payload";
    let mut output = Output::to_memory().expect("output");
    Encryptor::new(&ctx, plaintext)
        .expect("builder")
        .add_recipient(&key)
        .armor(true)
        .build(&mut output)
        .expect("encrypt");
    let armored = output.into_bytes().expect("drain");
    let s = String::from_utf8(armored).expect("ascii");
    assert!(
        s.starts_with("-----BEGIN PGP MESSAGE-----"),
        "armored output should start with header, got: {s}"
    );
}

#[test]
fn decrypt_with_garbage_input_fails() {
    let ctx = Context::new().expect("ctx");
    let result = decrypt(&ctx, b"not valid ciphertext at all");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_ne!(
        err.kind(),
        ErrorKind::Success,
        "garbage should not decrypt cleanly"
    );
}

#[test]
fn encrypt_with_compression() {
    let ctx = Context::new().expect("ctx");
    let key = make_encryption_key(&ctx, "compressed <c@example.com>");

    let plaintext = b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    let mut output = Output::to_memory().expect("output");
    Encryptor::new(&ctx, plaintext)
        .expect("builder")
        .add_recipient(&key)
        .compression(Compression::Zlib, 6)
        .build(&mut output)
        .expect("encrypt");
    let ciphertext = output.into_bytes().expect("drain");

    let recovered = decrypt(&ctx, &ciphertext).expect("decrypt");
    assert_eq!(recovered.as_slice(), plaintext);
}
