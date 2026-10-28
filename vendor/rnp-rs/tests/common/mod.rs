//! Shared helpers for integration tests.
//!
//! Each test file declares `mod common;` (Rust's convention finds
//! `tests/common/mod.rs` automatically) and uses `common::signing_key`
//! / `common::encryption_key` instead of the deprecated
//! `generate_test_key` shim.

#![allow(dead_code)]

use rnp::{Algorithm, Context, Hash, KeyBuilder, KeyUsage};

/// Build an RSA-2048 key with SHA-256, Sign + Certify usages, and the given
/// userid. Replaces the deprecated `generate_test_key` shim.
pub fn signing_key<'a>(ctx: &'a Context, uid: &str) -> rnp::Key<'a> {
    KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid(uid)
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Sign)
        .add_usage(KeyUsage::Certify)
        .build(ctx)
        .expect("signing key")
}

/// Build an RSA-2048 encryption key with the given userid.
pub fn encryption_key<'a>(ctx: &'a Context, uid: &str) -> rnp::Key<'a> {
    KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid(uid)
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::EncryptComms)
        .build(ctx)
        .expect("encryption key")
}
