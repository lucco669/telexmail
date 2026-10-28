//! End-to-end example: generate an RSA primary + ECDH subkey, save the
//! keyring, reload it, and verify round-trip.
//!
//! Run with: `cargo run --example keygen`

use rnp::{
    Algorithm, Context, Curve, Hash, KeyBuilder, KeyIdentifier, KeyUsage, LoadSaveFlags,
    SubkeyBuilder, UnloadFlags,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = Context::new()?;

    let primary = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("carol <carol@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Sign)
        .add_usage(KeyUsage::Certify)
        .build(&ctx)?;
    println!("primary: {}", primary.fingerprint()?);

    let subkey = SubkeyBuilder::new(Algorithm::Ecdh)
        .curve(Curve::P256)
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::EncryptComms)
        .build(&ctx, &primary)?;
    println!(
        "subkey alg: {}, curve: {:?}",
        subkey.alg()?,
        subkey.curve()?
    );

    // Save the keyring to memory.
    let saved = ctx.save_keys_to_memory(
        rnp::KeyringFormat::Gpg,
        LoadSaveFlags::PUBLIC | LoadSaveFlags::SECRET,
    )?;
    println!("keyring serialized: {} bytes", saved.len());

    // Drop primary, subkey, then wipe the keyring.
    drop(primary);
    drop(subkey);
    ctx.unload_keys(UnloadFlags::PUBLIC | UnloadFlags::SECRET)?;
    assert_eq!(ctx.public_key_count()?, 0);

    // Reload.
    ctx.load_keys(
        rnp::KeyringFormat::Gpg,
        &saved,
        LoadSaveFlags::PUBLIC | LoadSaveFlags::SECRET,
    )?;
    println!("reloaded: {} public keys", ctx.public_key_count()?);

    let _reloaded = ctx
        .find_key(KeyIdentifier::Userid("carol <carol@example.com>"))?
        .expect("carol's key should be findable after reload");

    Ok(())
}
