//! Specs for algorithm helpers (Phase 88, 89).

use rnp::{Algorithm, Cipher, Compression, Hash, KeyUsage};

#[test]
fn hash_digest_sizes_match_canonical_values() {
    assert_eq!(Hash::Sha1.digest_size(), 20);
    assert_eq!(Hash::Sha224.digest_size(), 28);
    assert_eq!(Hash::Sha256.digest_size(), 32);
    assert_eq!(Hash::Sha384.digest_size(), 48);
    assert_eq!(Hash::Sha512.digest_size(), 64);
    assert_eq!(Hash::Sha3_256.digest_size(), 32);
    assert_eq!(Hash::Sha3_512.digest_size(), 64);
    assert_eq!(Hash::Md5.digest_size(), 16);
    assert_eq!(Hash::Ripemd160.digest_size(), 20);
    assert_eq!(Hash::Sm3.digest_size(), 32);
}

#[test]
fn cipher_key_sizes_match_canonical_values() {
    assert_eq!(Cipher::Aes128.key_size(), 16);
    assert_eq!(Cipher::Aes192.key_size(), 24);
    assert_eq!(Cipher::Aes256.key_size(), 32);
    assert_eq!(Cipher::Camellia128.key_size(), 16);
    assert_eq!(Cipher::Camellia192.key_size(), 24);
    assert_eq!(Cipher::Camellia256.key_size(), 32);
    assert_eq!(Cipher::Tripledes.key_size(), 24);
    // Variable-length legacy ciphers report their default key size.
    assert_eq!(Cipher::Idea.key_size(), 16);
    assert_eq!(Cipher::Cast5.key_size(), 16);
    assert_eq!(Cipher::Blowfish.key_size(), 16);
    assert_eq!(Cipher::Twofish.key_size(), 32);
    assert_eq!(Cipher::Sm4.key_size(), 32);
}

#[test]
fn cipher_block_sizes_match_canonical_values() {
    // AES family + Camellia + Twofish + SM4 = 128-bit blocks.
    for c in [
        Cipher::Aes128,
        Cipher::Aes192,
        Cipher::Aes256,
        Cipher::Camellia128,
        Cipher::Camellia192,
        Cipher::Camellia256,
        Cipher::Twofish,
        Cipher::Sm4,
    ] {
        assert_eq!(c.block_size(), 16, "{c:?} should be 128-bit block");
    }
    // Legacy 64-bit block ciphers.
    for c in [
        Cipher::Blowfish,
        Cipher::Cast5,
        Cipher::Tripledes,
        Cipher::Idea,
    ] {
        assert_eq!(c.block_size(), 8, "{c:?} should be 64-bit block");
    }
}

#[test]
fn algorithm_categorization_partitions_correctly() {
    // Signature-capable.
    for a in [
        Algorithm::Rsa,
        Algorithm::Dsa,
        Algorithm::Ecdsa,
        Algorithm::Eddsa,
        Algorithm::Sm2,
    ] {
        assert!(a.is_signature(), "{a:?} should be signature-capable");
    }

    // Encryption-capable.
    for a in [
        Algorithm::Rsa,
        Algorithm::Ecdh,
        Algorithm::ElGamal,
        Algorithm::Sm2,
    ] {
        assert!(a.is_encryption(), "{a:?} should be encryption-capable");
    }

    // Pure-encryption algorithms are NOT signature-capable.
    assert!(
        !Algorithm::Ecdh.is_signature(),
        "ECDH is encryption-only, not signing"
    );
    assert!(
        !Algorithm::ElGamal.is_signature(),
        "ElGamal is encryption-only, not signing"
    );

    // Pure-signing algorithms are NOT encryption-capable.
    assert!(
        !Algorithm::Dsa.is_encryption(),
        "DSA is signing-only, not encryption"
    );
    assert!(
        !Algorithm::Ecdsa.is_encryption(),
        "ECDSA is signing-only, not encryption"
    );
    assert!(
        !Algorithm::Eddsa.is_encryption(),
        "EdDSA is signing-only, not encryption"
    );

    // RSA + SM2 are dual-use.
    assert!(Algorithm::Rsa.is_signature() && Algorithm::Rsa.is_encryption());
    assert!(Algorithm::Sm2.is_signature() && Algorithm::Sm2.is_encryption());
}

#[test]
fn algorithm_as_str_round_trips_via_librnp() {
    // Indirect: librnp recognises these names. supports_feature returns
    // true for them.
    use rnp::{FeatureType, supports_feature};
    for a in [
        Algorithm::Rsa,
        Algorithm::Dsa,
        Algorithm::Ecdh,
        Algorithm::Ecdsa,
        Algorithm::Eddsa,
        Algorithm::Sm2,
    ] {
        assert!(
            supports_feature(FeatureType::PublicKeyAlgorithm, a.as_str()).unwrap_or(false),
            "librnp should recognise {}",
            a.as_str()
        );
    }
    // ElGamal is gated on older builds; skip if not supported.
    let _ = supports_feature(FeatureType::PublicKeyAlgorithm, Algorithm::ElGamal.as_str());
}

#[test]
fn compression_as_str_matches_librnp_names() {
    use rnp::{FeatureType, supports_feature};
    for c in [Compression::Zip, Compression::Zlib, Compression::Bzip2] {
        assert!(
            supports_feature(FeatureType::CompressionAlgorithm, c.as_str()).unwrap_or(false),
            "librnp should recognise {}",
            c.as_str()
        );
    }
}

#[test]
fn key_usage_as_str_matches_librnp_names() {
    // EncryptComms and EncryptStorage both map to "encrypt" per the
    // OpenPGP usage flags (a single bit covers both).
    assert_eq!(KeyUsage::Certify.as_str(), "certify");
    assert_eq!(KeyUsage::Sign.as_str(), "sign");
    assert_eq!(KeyUsage::EncryptComms.as_str(), "encrypt");
    assert_eq!(KeyUsage::EncryptStorage.as_str(), "encrypt");
}
