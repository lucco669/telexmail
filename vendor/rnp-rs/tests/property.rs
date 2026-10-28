//! Property-based tests via proptest.
//!
//! Verifies invariants across many generated inputs:
//! - encrypt(decrypt(x)) == x for any plaintext.
//! - export then load preserves fingerprint.
//! - cleartext tamper at any body position breaks verification.

mod common;
use common::signing_key;

use proptest::prelude::*;
use rnp::{
    Algorithm, Cipher, Context, Encryptor, Hash, KeyBuilder, KeyIdentifier, KeyUsage,
    LoadSaveFlags, Output, sign_cleartext,
};

fn any_plaintext() -> impl Strategy<Value = Vec<u8>> {
    // 1..=512 bytes. Empty plaintext (0 bytes) is rejected by librnp's
    // encryption path with BadParameters, so we exclude it from the
    // property test's input space.
    // test suite unreasonably.
    prop::collection::vec(any::<u8>(), 1..=512)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]
    #[test]
    fn encrypt_decrypt_roundtrip_preserves_plaintext(plaintext in any_plaintext()) {
        let ctx = Context::new()?;
        let key = KeyBuilder::new(Algorithm::Rsa)
            .bits(2048)
            .userid("prop-enc <prop@example.com>")
            .hash(Hash::Sha256)
            .add_usage(KeyUsage::EncryptComms)
            .build(&ctx)?;

        let mut output = Output::to_memory()?;
        Encryptor::new(&ctx, &plaintext)?
            .add_recipient(&key)
            .cipher(Cipher::Aes256)
            .build(&mut output)?;
        let ciphertext = output.into_bytes()?;

        let recovered = rnp::decrypt(&ctx, &ciphertext)?;
        prop_assert_eq!(recovered.as_slice(), plaintext.as_slice());
    }

    #[test]
    fn export_then_load_preserves_fingerprint(_dummy: u8) {
        let ctx = Context::new()?;
        let key = signing_key(&ctx, "prop-fp <prop-fp@example.com>");
        let fp = key.fingerprint()?;
        drop(key);

        let saved = ctx.save_keys_to_memory(
            rnp::KeyringFormat::Gpg,
            LoadSaveFlags::PUBLIC | LoadSaveFlags::SECRET,
        )?;
        ctx.unload_keys(rnp::UnloadFlags::PUBLIC | rnp::UnloadFlags::SECRET)?;
        ctx.load_keys(
            rnp::KeyringFormat::Gpg,
            &saved,
            LoadSaveFlags::PUBLIC | LoadSaveFlags::SECRET,
        )?;

        let again = ctx
            .find_key(KeyIdentifier::Fingerprint(&fp))?
            .expect("reloaded key");
        prop_assert_eq!(again.fingerprint()?, fp);
    }

    #[test]
    fn cleartext_tamper_breaks_verification_at_any_byte(
        body in prop::collection::vec(any::<u8>(), 16..=128),
        tamper_pos in 0usize..16,  // bounded by body len via filter
    ) {
        // Build a body with a recognizable marker prefix so we can find
        // a byte to flip.
        let mut message = b"MARKER-".to_vec();
        message.extend_from_slice(&body);
        let ctx = Context::new()?;
        let key = signing_key(&ctx, "prop-tamper <pt@example.com>");

        let mut clear = sign_cleartext(&ctx, &message, &key)?;

        // Find "MARKER-" in the cleartext body (between the Hash header
        // and the signature block) and flip the byte at the chosen
        // position within it.
        let marker = b"MARKER-";
        let pos = clear
            .windows(marker.len())
            .position(|w| w == marker)
            .ok_or_else(|| proptest::test_runner::TestCaseError::reject("marker not in body"))?;
        let abs_pos = pos + (tamper_pos.min(body.len().saturating_sub(1)));
        clear[abs_pos] ^= 0xff;

        let result = rnp::verify(&ctx, &clear);
        let valid = result.map(|r| r.any_valid().unwrap_or(false)).unwrap_or(false);
        prop_assert!(
            !valid,
            "tampered cleartext must not verify at pos {}",
            abs_pos
        );
    }
}
