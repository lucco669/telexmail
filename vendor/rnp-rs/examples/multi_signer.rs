//! Multi-signer detached signature via the `Signer` builder.
//!
//! Run with: `cargo run --example multi_signer`

use rnp::{Algorithm, Context, Hash, KeyBuilder, KeyUsage, Mode, Signer};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = Context::new()?;

    let alice = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("alice <alice@example.com>")
        .add_usage(KeyUsage::Sign)
        .build(&ctx)?;
    let bob = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("bob <bob@example.com>")
        .add_usage(KeyUsage::Sign)
        .build(&ctx)?;

    let message = b"contract text -- signed by both parties";
    let signature = Signer::new(&ctx, message, Mode::Detached)
        .add_signer(&alice)
        .add_signer_with_hash(&bob, Hash::Sha384)
        .armor(true)
        .build_to_memory()?;

    println!("{}", String::from_utf8_lossy(&signature));

    let result = rnp::verify_detached(&ctx, message, &signature)?;
    let count = result.signature_count()?;
    println!("\nVerified. Signature count: {count}");
    for (i, sig) in result.iter_signatures().enumerate() {
        let status = if sig.status_is_valid() {
            "valid"
        } else {
            "INVALID"
        };
        let keyid = sig.keyid().unwrap_or_else(|_| "<unknown>".into());
        println!("  sig[{i}]: keyid={keyid} status={status}");
    }
    Ok(())
}
