//! End-to-end example: generate a keypair, encrypt a message, decrypt it.
//!
//! Run with: `cargo run --example encrypt_decrypt`

use rnp::{Algorithm, Cipher, Context, Encryptor, Hash, KeyBuilder, KeyUsage, Output};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = Context::new()?;

    let key = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("bob <bob@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::EncryptComms)
        .build(&ctx)?;
    println!(
        "generated keypair: {} (fingerprint: {})",
        key.keyid()?,
        key.fingerprint()?
    );

    let plaintext = b"the eagle flies at midnight";
    let mut output = Output::to_memory()?;
    Encryptor::new(&ctx, plaintext)?
        .add_recipient(&key)
        .cipher(Cipher::Aes256)
        .armor(true)
        .build(&mut output)?;
    let ciphertext = output.into_bytes()?;
    println!(
        "ciphertext (armored):\n----\n{}----",
        String::from_utf8_lossy(&ciphertext)
    );

    let recovered = rnp::decrypt(&ctx, &ciphertext)?;
    assert_eq!(recovered.as_slice(), plaintext);
    println!("decrypted back: {} bytes match", recovered.len());

    Ok(())
}
