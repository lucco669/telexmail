//! Encryption and decryption.
//!
//! ## Module layout
//!
//! | Sub-module    | Concern                                              |
//! |---------------|------------------------------------------------------|
//! | `encryptor`   | `Encryptor` builder + `apply_config`                 |
//! | `decryptor`   | `Decryptor` builder + `DecryptResult` + free funcs   |
//!
//! Shared types (`EncryptFlags`, `AeadType`, `AddPasswordOptions`) live in
//! this file.

use crate::algorithm::{Cipher, Hash};
use crate::ffi;

mod decryptor;
mod encryptor;

#[allow(deprecated)]
pub use decryptor::{DecryptResult, Decryptor, decrypt, decrypt_from_input, decrypt_to};
pub use encryptor::Encryptor;

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

/// Flags for [`Encryptor::flags`](crate::Encryptor::flags). Wraps `RNP_ENCRYPT_*`.
#[derive(Clone, Copy, Debug, Default)]
pub struct EncryptFlags(pub u32);

impl EncryptFlags {
    /// Don't wrap plaintext in a literal-data packet — used when encrypting
    /// already-signed data. Wraps `RNP_ENCRYPT_NOWRAP`.
    pub const NOWRAP: Self = Self(ffi::RNP_ENCRYPT_NOWRAP as u32);

    pub fn bits(self) -> u32 {
        self.0
    }
}

impl std::ops::BitOr for EncryptFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// AEAD algorithm. Pass to [`Encryptor::aead`](crate::Encryptor::aead).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AeadType {
    Ocb,
    Eax,
    Gcm,
}

impl AeadType {
    pub fn as_str(self) -> &'static str {
        match self {
            AeadType::Ocb => "OCB",
            AeadType::Eax => "EAX",
            AeadType::Gcm => "GCM",
        }
    }
}

/// S2K / encryption config for a password added via
/// [`Encryptor::add_password`](crate::Encryptor::add_password).
///
/// Built via [`AddPasswordOptions::default`] then chained setters. OCP —
/// adding a new S2K parameter doesn't break call sites.
#[derive(Default)]
pub struct AddPasswordOptions {
    pub(crate) hash: Option<Hash>,
    pub(crate) iterations: Option<usize>,
    pub(crate) cipher: Option<Cipher>,
}

impl AddPasswordOptions {
    pub fn hash(mut self, h: Hash) -> Self {
        self.hash = Some(h);
        self
    }

    pub fn iterations(mut self, n: usize) -> Self {
        self.iterations = Some(n);
        self
    }

    pub fn cipher(mut self, c: Cipher) -> Self {
        self.cipher = Some(c);
        self
    }
}
