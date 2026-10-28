//! Domain primitives: public-key algorithms, curves, hashes, ciphers,
//! compression, key usage.
//!
//! These types were previously in `keygen.rs` but they're used across the
//! crate — encryption, signing, verification, security queries, key
//! inspection. Centralizing them in `algorithm` makes the dependency graph
//! honest: `keygen` depends on `algorithm`, not vice versa.
//!
//! [`keygen`](crate::keygen) re-exports every type here for backward
//! compatibility. New code should prefer the `algorithm::` path.

// ---------------------------------------------------------------------------
// Public-key algorithms
// ---------------------------------------------------------------------------

/// Public-key algorithm. Maps to the `RNP_ALGNAME_*` string constants in
/// `rnp.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Algorithm {
    /// `"RSA"`. Used for primary or subkey; supports sign + encrypt (legacy).
    Rsa,
    /// `"ELGAMAL"`. Subkey only, encryption.
    ElGamal,
    /// `"DSA"`. Primary only, signing.
    Dsa,
    /// `"ECDH"`. Subkey only, encryption.
    Ecdh,
    /// `"ECDSA"`. Primary or subkey, signing.
    Ecdsa,
    /// `"EDDSA"`. Primary or subkey, signing (Ed25519 curve).
    Eddsa,
    /// `"SM2"`. Primary or subkey, sign + encrypt.
    Sm2,
}

impl Algorithm {
    pub fn as_str(self) -> &'static str {
        match self {
            Algorithm::Rsa => "RSA",
            Algorithm::ElGamal => "ELGAMAL",
            Algorithm::Dsa => "DSA",
            Algorithm::Ecdh => "ECDH",
            Algorithm::Ecdsa => "ECDSA",
            Algorithm::Eddsa => "EDDSA",
            Algorithm::Sm2 => "SM2",
        }
    }

    /// Whether this algorithm can produce signatures. Maps to OpenPGP's
    /// "signing-capable" public-key algorithms (DSA, ECDSA, EdDSA, SM2,
    /// RSA). Useful for builder-time validation.
    pub fn is_signature(self) -> bool {
        matches!(
            self,
            Algorithm::Rsa | Algorithm::Dsa | Algorithm::Ecdsa | Algorithm::Eddsa | Algorithm::Sm2
        )
    }

    /// Whether this algorithm can perform public-key encryption / key
    /// encapsulation. Maps to OpenPGP's encryption-capable algorithms
    /// (RSA, ECDH, ElGamal, SM2).
    pub fn is_encryption(self) -> bool {
        matches!(
            self,
            Algorithm::Rsa | Algorithm::Ecdh | Algorithm::ElGamal | Algorithm::Sm2
        )
    }
}

/// PQC composite algorithms. Available only when the crate is built with
/// `--features pqc` and the linked librnp was built with `ENABLE_PQC=ON`.
///
/// These wrap the `RNP_ALGNAME_KYBER*` / `RNP_ALGNAME_DILITHIUM*` /
/// `RNP_ALGNAME_SPHINCSPLUS_*` string constants in `rnp.h`.
#[cfg(feature = "pqc")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PqcAlgorithm {
    // Key-encapsulation (encryption) composites.
    MlKem768X25519,
    MlKem1024X448,
    MlKem768P384,
    MlKem1024P521,
    MlKem768Bp384,
    MlKem1024Bp512,
    // Signature composites.
    MlDsa65Ed25519,
    MlDsa87Ed448,
    MlDsa65P384,
    MlDsa87P521,
    MlDsa65Bp384,
    MlDsa87Bp512,
    // Standalone SLH-DSA variants.
    SlhDsaShake128f,
    SlhDsaShake128s,
    SlhDsaShake256s,
}

#[cfg(feature = "pqc")]
impl PqcAlgorithm {
    pub fn as_str(self) -> &'static str {
        match self {
            PqcAlgorithm::MlKem768X25519 => "ML-KEM-768+X25519",
            PqcAlgorithm::MlKem1024X448 => "ML-KEM-1024+X448",
            PqcAlgorithm::MlKem768P384 => "ML-KEM-768+ECDH-P384",
            PqcAlgorithm::MlKem1024P521 => "ML-KEM-1024+ECDH-P521",
            PqcAlgorithm::MlKem768Bp384 => "ML-KEM-768+ECDH-BP384",
            PqcAlgorithm::MlKem1024Bp512 => "ML-KEM-1024+ECDH-BP512",
            PqcAlgorithm::MlDsa65Ed25519 => "ML-DSA-65+ED25519",
            PqcAlgorithm::MlDsa87Ed448 => "ML-DSA-87+ED448",
            PqcAlgorithm::MlDsa65P384 => "ML-DSA-65+ECDSA-P384",
            PqcAlgorithm::MlDsa87P521 => "ML-DSA-87+ECDSA-P521",
            PqcAlgorithm::MlDsa65Bp384 => "ML-DSA-65+ECDSA-BP384",
            PqcAlgorithm::MlDsa87Bp512 => "ML-DSA-87+ECDSA-BP512",
            PqcAlgorithm::SlhDsaShake128f => "SLH-DSA-SHAKE-128f",
            PqcAlgorithm::SlhDsaShake128s => "SLH-DSA-SHAKE-128s",
            PqcAlgorithm::SlhDsaShake256s => "SLH-DSA-SHAKE-256s",
        }
    }
}

/// Runtime probe: confirm that the linked librnp actually supports PQC.
/// Build-time `--features pqc` only makes the symbols visible to bindgen —
/// the binary must also have been compiled with `ENABLE_PQC=ON`. Call this
/// before any PQC operation.
#[cfg(feature = "pqc")]
pub fn librnp_supports_pqc() -> bool {
    crate::security::supports_feature(
        crate::security::FeatureType::PublicKeyAlgorithm,
        "ML-KEM-768+X25519",
    )
    .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Curves
// ---------------------------------------------------------------------------

/// Elliptic curve name. Pass to [`crate::KeyBuilder::curve`] /
/// [`crate::SubkeyBuilder::curve`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Curve {
    /// `"NIST P-256"` (secp256r1).
    P256,
    /// `"NIST P-384"` (secp384r1).
    P384,
    /// `"NIST P-521"` (secp521r1).
    P521,
    /// `"Ed25519"` — used with `Algorithm::Eddsa`.
    Ed25519,
    /// `"Curve25519"` — used with `Algorithm::Ecdh`.
    Curve25519,
    /// `"brainpoolP256r1"`.
    Bp256,
    /// `"brainpoolP384r1"`.
    Bp384,
    /// `"brainpoolP512r1"`.
    Bp512,
    /// `"secp256k1"` — Bitcoin curve.
    Secp256k1,
    /// `"SM2 P-256"`.
    Sm2P256,
}

impl Curve {
    pub fn as_str(self) -> &'static str {
        match self {
            Curve::P256 => "NIST P-256",
            Curve::P384 => "NIST P-384",
            Curve::P521 => "NIST P-521",
            Curve::Ed25519 => "Ed25519",
            Curve::Curve25519 => "Curve25519",
            Curve::Bp256 => "brainpoolP256r1",
            Curve::Bp384 => "brainpoolP384r1",
            Curve::Bp512 => "brainpoolP512r1",
            Curve::Secp256k1 => "secp256k1",
            Curve::Sm2P256 => "SM2 P-256",
        }
    }
}

// ---------------------------------------------------------------------------
// Hashes
// ---------------------------------------------------------------------------

/// Hash algorithm. Pass to [`crate::KeyBuilder::hash`] and signature
/// operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Hash {
    Sha1,
    Sha224,
    Sha256,
    Sha384,
    Sha512,
    Sha3_256,
    Sha3_512,
    Md5,
    Ripemd160,
    Sm3,
}

impl Hash {
    pub fn as_str(self) -> &'static str {
        match self {
            Hash::Sha1 => "SHA1",
            Hash::Sha224 => "SHA224",
            Hash::Sha256 => "SHA256",
            Hash::Sha384 => "SHA384",
            Hash::Sha512 => "SHA512",
            Hash::Sha3_256 => "SHA3-256",
            Hash::Sha3_512 => "SHA3-512",
            Hash::Md5 => "MD5",
            Hash::Ripemd160 => "RIPEMD160",
            Hash::Sm3 => "SM3",
        }
    }

    /// Digest size in bytes (e.g. SHA-256 → 32). Used for S2K iteration
    /// sizing and signature-digest allocation.
    pub fn digest_size(self) -> usize {
        match self {
            Hash::Sha1 | Hash::Ripemd160 => 20,
            Hash::Sha224 => 28,
            Hash::Sha256 | Hash::Sha3_256 | Hash::Sm3 => 32,
            Hash::Sha384 => 48,
            Hash::Sha512 | Hash::Sha3_512 => 64,
            Hash::Md5 => 16,
        }
    }
}

// ---------------------------------------------------------------------------
// Symmetric ciphers
// ---------------------------------------------------------------------------

/// Symmetric cipher. Used in protection, encryption, AEAD.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Cipher {
    Idea,
    Tripledes,
    Cast5,
    Blowfish,
    Aes128,
    Aes192,
    Aes256,
    Twofish,
    Camellia128,
    Camellia192,
    Camellia256,
    Sm4,
}

impl Cipher {
    pub fn as_str(self) -> &'static str {
        match self {
            Cipher::Idea => "IDEA",
            Cipher::Tripledes => "TRIPLEDES",
            Cipher::Cast5 => "CAST5",
            Cipher::Blowfish => "BLOWFISH",
            Cipher::Aes128 => "AES128",
            Cipher::Aes192 => "AES192",
            Cipher::Aes256 => "AES256",
            Cipher::Twofish => "TWOFISH",
            Cipher::Camellia128 => "CAMELLIA128",
            Cipher::Camellia192 => "CAMELLIA192",
            Cipher::Camellia256 => "CAMELLIA256",
            Cipher::Sm4 => "SM4",
        }
    }

    /// Symmetric key size in bytes (e.g. AES-256 → 32).
    pub fn key_size(self) -> usize {
        match self {
            Cipher::Idea | Cipher::Cast5 | Cipher::Blowfish => 16,
            Cipher::Tripledes => 24,
            Cipher::Aes128 | Cipher::Camellia128 => 16,
            Cipher::Aes192 | Cipher::Camellia192 => 24,
            Cipher::Aes256 | Cipher::Camellia256 | Cipher::Twofish | Cipher::Sm4 => 32,
        }
    }

    /// Block size in bytes (e.g. AES → 16).
    pub fn block_size(self) -> usize {
        match self {
            // 64-bit block ciphers.
            Cipher::Blowfish | Cipher::Cast5 | Cipher::Tripledes | Cipher::Idea => 8,
            // 128-bit block ciphers (the AES family + Camellia + Twofish + SM4).
            Cipher::Aes128
            | Cipher::Aes192
            | Cipher::Aes256
            | Cipher::Camellia128
            | Cipher::Camellia192
            | Cipher::Camellia256
            | Cipher::Twofish
            | Cipher::Sm4 => 16,
        }
    }
}

// ---------------------------------------------------------------------------
// Compression
// ---------------------------------------------------------------------------

/// Compression algorithm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Compression {
    Zip,
    Zlib,
    Bzip2,
}

impl Compression {
    pub fn as_str(self) -> &'static str {
        match self {
            Compression::Zip => "ZIP",
            Compression::Zlib => "ZLIB",
            Compression::Bzip2 => "BZIP2",
        }
    }
}

// ---------------------------------------------------------------------------
// Key usage flags
// ---------------------------------------------------------------------------

/// Key usage flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeyUsage {
    Certify,
    Sign,
    EncryptComms,
    EncryptStorage,
}

impl KeyUsage {
    pub fn as_str(self) -> &'static str {
        match self {
            KeyUsage::Certify => "certify",
            KeyUsage::Sign => "sign",
            KeyUsage::EncryptComms => "encrypt",
            KeyUsage::EncryptStorage => "encrypt",
        }
    }
}
