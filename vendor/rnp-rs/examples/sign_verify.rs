//! End-to-end example: generate a key, sign a message, verify it.
//!
//! Run with: `cargo run --example sign_verify`

use rnp::{Algorithm, Context, Hash, KeyBuilder, KeyUsage};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = Context::new()?;
    println!("librnp version: {}", rnp::version_string());

    let key = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("alice <alice@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Sign)
        .add_usage(KeyUsage::Certify)
        .build(&ctx)?;
    println!(
        "generated key: {} (fingerprint: {})",
        key.keyid()?,
        key.fingerprint()?
    );

    let message = b"the quick brown fox jumps over the lazy dog";
    let signed = rnp::sign(&ctx, message, &key)?;
    println!("signed {} bytes -> {} bytes", message.len(), signed.len());

    let result = rnp::verify(&ctx, &signed)?;
    let ok = result.any_valid()?;
    println!("verify: {ok}");
    assert!(ok);

    let detached = rnp::sign_detached(&ctx, message, &key)?;
    let result = rnp::verify_detached(&ctx, message, &detached)?;
    let ok = result.any_valid()?;
    println!("verify_detached: {ok}");
    assert!(ok);

    let cleartext = rnp::sign_cleartext(&ctx, message, &key)?;
    println!(
        "cleartext signed message:\n----\n{}----",
        String::from_utf8_lossy(&cleartext)
    );

    Ok(())
}
