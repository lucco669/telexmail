//! `FromStr` and `Display` for the crate's model enums.
//!
//! Each enum's `as_str()` returns the C-side string; the impls here
//! provide the reverse mapping plus the standard Rust string-conversion
//! traits. This unlocks config-file parsers, CLI arg libraries, and
//! serde (via `#[serde(with = "...")]` patterns).
//!
//! Adding a new model enum: implement `as_str()` on it, then add a
//! `from_str` match arm + blanket `FromStr`/`Display` impls. The trait
//! is the same pattern for every enum — could be macro-generated, but
//! the manual form is clearer and lets each enum document its variants.

use crate::error::{Error, unknown_variant};
use crate::{
    Algorithm, ArmorType, Cipher, Compression, Curve, FeatureType, Hash, KeyUsage,
    encrypt::AeadType,
};
use std::str::FromStr;

// Helper: build an UnknownVariant error.
fn unknown(kind: &'static str, value: &str) -> Error {
    unknown_variant(kind, value)
}

// -----------------------------------------------------------------------
// Algorithm
// -----------------------------------------------------------------------

impl std::fmt::Display for Algorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Algorithm {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "RSA" => Ok(Algorithm::Rsa),
            "ELGAMAL" => Ok(Algorithm::ElGamal),
            "DSA" => Ok(Algorithm::Dsa),
            "ECDH" => Ok(Algorithm::Ecdh),
            "ECDSA" => Ok(Algorithm::Ecdsa),
            "EDDSA" => Ok(Algorithm::Eddsa),
            "SM2" => Ok(Algorithm::Sm2),
            _ => Err(unknown("algorithm", s)),
        }
    }
}

// -----------------------------------------------------------------------
// Curve
// -----------------------------------------------------------------------

impl std::fmt::Display for Curve {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Curve {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "NIST P-256" => Ok(Curve::P256),
            "NIST P-384" => Ok(Curve::P384),
            "NIST P-521" => Ok(Curve::P521),
            "Ed25519" => Ok(Curve::Ed25519),
            "Curve25519" => Ok(Curve::Curve25519),
            "brainpoolP256r1" => Ok(Curve::Bp256),
            "brainpoolP384r1" => Ok(Curve::Bp384),
            "brainpoolP512r1" => Ok(Curve::Bp512),
            "secp256k1" => Ok(Curve::Secp256k1),
            "SM2 P-256" => Ok(Curve::Sm2P256),
            _ => Err(unknown("curve", s)),
        }
    }
}

// -----------------------------------------------------------------------
// Hash
// -----------------------------------------------------------------------

impl std::fmt::Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Hash {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "SHA1" => Ok(Hash::Sha1),
            "SHA224" => Ok(Hash::Sha224),
            "SHA256" => Ok(Hash::Sha256),
            "SHA384" => Ok(Hash::Sha384),
            "SHA512" => Ok(Hash::Sha512),
            "SHA3-256" => Ok(Hash::Sha3_256),
            "SHA3-512" => Ok(Hash::Sha3_512),
            "MD5" => Ok(Hash::Md5),
            "RIPEMD160" => Ok(Hash::Ripemd160),
            "SM3" => Ok(Hash::Sm3),
            _ => Err(unknown("hash", s)),
        }
    }
}

// -----------------------------------------------------------------------
// Cipher
// -----------------------------------------------------------------------

impl std::fmt::Display for Cipher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Cipher {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "IDEA" => Ok(Cipher::Idea),
            "TRIPLEDES" => Ok(Cipher::Tripledes),
            "CAST5" => Ok(Cipher::Cast5),
            "BLOWFISH" => Ok(Cipher::Blowfish),
            "AES128" => Ok(Cipher::Aes128),
            "AES192" => Ok(Cipher::Aes192),
            "AES256" => Ok(Cipher::Aes256),
            "TWOFISH" => Ok(Cipher::Twofish),
            "CAMELLIA128" => Ok(Cipher::Camellia128),
            "CAMELLIA192" => Ok(Cipher::Camellia192),
            "CAMELLIA256" => Ok(Cipher::Camellia256),
            "SM4" => Ok(Cipher::Sm4),
            _ => Err(unknown("cipher", s)),
        }
    }
}

// -----------------------------------------------------------------------
// Compression
// -----------------------------------------------------------------------

impl std::fmt::Display for Compression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Compression {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "ZIP" => Ok(Compression::Zip),
            "ZLIB" => Ok(Compression::Zlib),
            "BZIP2" => Ok(Compression::Bzip2),
            _ => Err(unknown("compression", s)),
        }
    }
}

// -----------------------------------------------------------------------
// KeyUsage
// -----------------------------------------------------------------------

impl std::fmt::Display for KeyUsage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for KeyUsage {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "certify" => Ok(KeyUsage::Certify),
            "sign" => Ok(KeyUsage::Sign),
            "encrypt" => Ok(KeyUsage::EncryptComms),
            _ => Err(unknown("key usage", s)),
        }
    }
}

// -----------------------------------------------------------------------
// AeadType
// -----------------------------------------------------------------------

impl std::fmt::Display for AeadType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for AeadType {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "OCB" => Ok(AeadType::Ocb),
            "EAX" => Ok(AeadType::Eax),
            "GCM" => Ok(AeadType::Gcm),
            _ => Err(unknown("aead", s)),
        }
    }
}

// -----------------------------------------------------------------------
// ArmorType
// -----------------------------------------------------------------------

impl std::fmt::Display for ArmorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ArmorType {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "message" => Ok(ArmorType::Message),
            "public key" => Ok(ArmorType::PublicKey),
            "secret key" => Ok(ArmorType::SecretKey),
            "signature" => Ok(ArmorType::Signature),
            "cleartext signed message" => Ok(ArmorType::Cleartext),
            _ => Err(unknown("armor type", s)),
        }
    }
}

// -----------------------------------------------------------------------
// FeatureType
// -----------------------------------------------------------------------

impl std::fmt::Display for FeatureType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for FeatureType {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // The RNP_FEATURE_* constants are &[u8; N] with a trailing NUL.
        // Strip the NUL before comparing.
        let strip_nul = |b: &'static [u8]| -> &'static str {
            let end = b.iter().position(|&x| x == 0).unwrap_or(b.len());
            std::str::from_utf8(&b[..end]).expect("RNP_FEATURE_* is ASCII")
        };
        let symm = strip_nul(crate::ffi::RNP_FEATURE_SYMM_ALG);
        let aead = strip_nul(crate::ffi::RNP_FEATURE_AEAD_ALG);
        let prot = strip_nul(crate::ffi::RNP_FEATURE_PROT_MODE);
        let pk = strip_nul(crate::ffi::RNP_FEATURE_PK_ALG);
        let hash = strip_nul(crate::ffi::RNP_FEATURE_HASH_ALG);
        let comp = strip_nul(crate::ffi::RNP_FEATURE_COMP_ALG);
        let curve = strip_nul(crate::ffi::RNP_FEATURE_CURVE);
        match s {
            x if x == symm => Ok(FeatureType::SymmetricAlgorithm),
            x if x == aead => Ok(FeatureType::AeadAlgorithm),
            x if x == prot => Ok(FeatureType::ProtectionMode),
            x if x == pk => Ok(FeatureType::PublicKeyAlgorithm),
            x if x == hash => Ok(FeatureType::HashAlgorithm),
            x if x == comp => Ok(FeatureType::CompressionAlgorithm),
            x if x == curve => Ok(FeatureType::Curve),
            _ => Err(unknown("feature type", s)),
        }
    }
}

// -----------------------------------------------------------------------
// KeyringFormat (in context.rs)
// -----------------------------------------------------------------------

impl std::fmt::Display for crate::context::KeyringFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for crate::context::KeyringFormat {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "GPG" => Ok(crate::context::KeyringFormat::Gpg),
            "KBX" => Ok(crate::context::KeyringFormat::Kbx),
            "G10" => Ok(crate::context::KeyringFormat::G10),
            "JSON" => Ok(crate::context::KeyringFormat::Json),
            _ => Err(unknown("keyring format", s)),
        }
    }
}

// Suppress unused warning for the trait import we use implicitly.
#[allow(dead_code)]
fn _silence() {}
