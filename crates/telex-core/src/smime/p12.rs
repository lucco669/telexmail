//! PKCS #12 (.p12, .pfx) files read: the certificate with its private key, and the issuers'
//! certificates that came along (RFC 7292).
//!
//! Files from OpenSSL 3 and recent Windows are encrypted with PBES2 (PBKDF2 and AES); older
//! ones (OpenSSL's `-legacy`, Windows' "TripleDES-SHA1") with PKCS #12's own key derivation,
//! 3DES for the key and 40-bit RC2 for the certificates. The file's MAC says whether the
//! password is right before anything is decrypted. Iteration counts are capped, so a file
//! can't make Telex derive keys for minutes.

use der::asn1::{BmpString, OctetString};
use der::{Decode, Encode};
use hmac::{Hmac, Mac};
use pkcs12::kdf::{derive_key_bmp, Pkcs12KeyType};
use sha1::Sha1;
use sha2::{Sha256, Sha384, Sha512};
use spki::AlgorithmIdentifierOwned;
use zeroize::Zeroizing;

use super::ber;

/// The largest .p12 file read (one with a long chain is a few KB).
pub const MAX_FILE: usize = 1024 * 1024;
/// The most iterations a key derivation in the file may ask for.
const MAX_ROUNDS: u32 = 2_000_000;
/// The most bags (keys, certificates) a file may hold.
const MAX_BAGS: usize = 64;

const ID_DATA: &str = "1.2.840.113549.1.7.1";
const ID_ENCRYPTED_DATA: &str = "1.2.840.113549.1.7.6";
const PBES2: &str = "1.2.840.113549.1.5.13";
const PBE_SHA1_3DES: &str = "1.2.840.113549.1.12.1.3";
const PBE_SHA1_2DES: &str = "1.2.840.113549.1.12.1.4";
const PBE_SHA1_RC2_128: &str = "1.2.840.113549.1.12.1.5";
const PBE_SHA1_RC2_40: &str = "1.2.840.113549.1.12.1.6";
const KEY_BAG: &str = "1.2.840.113549.1.12.10.1.1";
const SHROUDED_KEY_BAG: &str = "1.2.840.113549.1.12.10.1.2";
const CERT_BAG: &str = "1.2.840.113549.1.12.10.1.3";
const X509_CERT: &str = "1.2.840.113549.1.9.22.1";
const SHA1_OID: &str = "1.3.14.3.2.26";
const SHA256_OID: &str = "2.16.840.1.101.3.4.2.1";
const SHA384_OID: &str = "2.16.840.1.101.3.4.2.2";
const SHA512_OID: &str = "2.16.840.1.101.3.4.2.3";

/// What a file held.
pub struct P12 {
    /// The private key, PKCS #8 (PrivateKeyInfo, DER), in memory only.
    pub key: Zeroizing<Vec<u8>>,
    /// The certificate of that key (DER).
    pub cert: Vec<u8>,
    /// The other certificates: its issuers, as far as the file has them.
    pub chain: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum P12Error {
    /// Not a PKCS #12 file.
    NotP12,
    /// The password doesn't open it.
    WrongPassword,
    /// Encrypted in a way Telex doesn't read (RC4, scrypt, PBMAC1, too many iterations).
    Unsupported,
    /// It opened, but holds no private key with its certificate.
    NoKey,
}

/// The file is PKCS #12 (it may still need a password).
pub fn is_p12(data: &[u8]) -> bool {
    data.len() <= MAX_FILE
        && ber::to_der(data)
            .ok()
            .and_then(|d| pkcs12::pfx::Pfx::from_der(&d).ok())
            .is_some()
}

/// The certificate, its key and its issuers from a PKCS #12 file opened with `password`.
pub fn read(data: &[u8], password: &str) -> Result<P12, P12Error> {
    if data.len() > MAX_FILE {
        return Err(P12Error::NotP12);
    }
    let der = ber::to_der(data).map_err(|_| P12Error::NotP12)?;
    let pfx = pkcs12::pfx::Pfx::from_der(&der).map_err(|_| P12Error::NotP12)?;
    if pfx.auth_safe.content_type.to_string() != ID_DATA {
        return Err(P12Error::Unsupported);
    }
    let safe = pfx
        .auth_safe
        .content
        .decode_as::<OctetString>()
        .map_err(|_| P12Error::NotP12)?;
    let safe = safe.as_bytes();
    // The MAC proves the password before anything is decrypted. Without one (rare), a wrong
    // password shows when the key doesn't decrypt.
    if let Some(mac) = &pfx.mac_data {
        check_mac(mac, safe, password)?;
    }
    let contents: Vec<cms::content_info::ContentInfo> = ber::to_der(safe)
        .ok()
        .and_then(|d| Vec::from_der(&d).ok())
        .ok_or(P12Error::NotP12)?;
    let mut keys: Vec<Zeroizing<Vec<u8>>> = Vec::new();
    let mut certs: Vec<Vec<u8>> = Vec::new();
    let mut bags = 0;
    for info in contents.iter().take(MAX_BAGS) {
        let plain: Zeroizing<Vec<u8>> = match info.content_type.to_string().as_str() {
            ID_DATA => Zeroizing::new(
                info.content
                    .decode_as::<OctetString>()
                    .map_err(|_| P12Error::NotP12)?
                    .into_bytes(),
            ),
            ID_ENCRYPTED_DATA => {
                let ed = info
                    .content
                    .decode_as::<cms::encrypted_data::EncryptedData>()
                    .map_err(|_| P12Error::NotP12)?;
                let enc = ed.enc_content_info;
                let data = enc.encrypted_content.ok_or(P12Error::NotP12)?;
                decrypt(&enc.content_enc_alg, data.as_bytes(), password)?
            }
            _ => continue,
        };
        let bag_list: Vec<pkcs12::safe_bag::SafeBag> = ber::to_der(&plain)
            .ok()
            .and_then(|d| Vec::from_der(&d).ok())
            .ok_or(P12Error::WrongPassword)?;
        for bag in bag_list {
            bags += 1;
            if bags > MAX_BAGS {
                break;
            }
            let value = explicit_inner(&bag.bag_value).ok_or(P12Error::NotP12)?;
            match bag.bag_id.to_string().as_str() {
                KEY_BAG => keys.push(Zeroizing::new(value.to_vec())),
                SHROUDED_KEY_BAG => {
                    let epki = pkcs12::pbe_params::EncryptedPrivateKeyInfo::from_der(value)
                        .map_err(|_| P12Error::NotP12)?;
                    keys.push(decrypt(
                        &epki.encryption_algorithm,
                        epki.encrypted_data.as_bytes(),
                        password,
                    )?);
                }
                CERT_BAG => {
                    let bag = pkcs12::cert_type::CertBag::from_der(value)
                        .map_err(|_| P12Error::NotP12)?;
                    if bag.cert_id.to_string() == X509_CERT {
                        certs.push(bag.cert_value.into_bytes());
                    }
                }
                _ => {}
            }
        }
    }
    // The certificate is the one whose public key the private key has.
    for key in keys {
        let Some(public) = super::public_key_of(&key) else {
            continue;
        };
        if let Some(i) = certs
            .iter()
            .position(|c| super::cert_public_key(c).as_deref() == Some(public.as_slice()))
        {
            let cert = certs.remove(i);
            return Ok(P12 {
                key,
                cert,
                chain: certs,
            });
        }
    }
    Err(P12Error::NoKey)
}

/// A bag's value: the contents of its `[0] EXPLICIT` wrapper.
fn explicit_inner(tlv: &[u8]) -> Option<&[u8]> {
    if tlv.first() != Some(&0xa0) {
        return Some(tlv);
    }
    let len_byte = *tlv.get(1)?;
    let (len, start) = if len_byte & 0x80 == 0 {
        (usize::from(len_byte), 2)
    } else {
        let n = usize::from(len_byte & 0x7f);
        if n == 0 || n > 4 {
            return None;
        }
        let mut len = 0usize;
        for b in tlv.get(2..2 + n)? {
            len = (len << 8) | usize::from(*b);
        }
        (len, 2 + n)
    };
    tlv.get(start..start.checked_add(len)?)
}

/// The password as PKCS #12's key derivation takes it: UTF-16 (big-endian) with a final 0.
/// An empty password is written two ways by different programs: as that final 0 alone, or
/// as nothing at all.
fn bmp_password(password: &str) -> Option<BmpString> {
    BmpString::from_utf8(password).ok()
}

fn check_mac(mac: &pkcs12::mac_data::MacData, data: &[u8], password: &str) -> Result<(), P12Error> {
    let rounds = u32::try_from(mac.iterations).map_err(|_| P12Error::NotP12)?;
    if rounds > MAX_ROUNDS || rounds == 0 {
        return Err(P12Error::Unsupported);
    }
    let alg = mac.mac.algorithm.oid.to_string();
    let expected = mac.mac.digest.as_bytes();
    let salt = mac.mac_salt.as_bytes();
    let pw = bmp_password(password).ok_or(P12Error::WrongPassword)?;
    let mut candidates: Vec<Zeroizing<Vec<u8>>> = Vec::new();
    let mut with_nul = Zeroizing::new(pw.into_bytes().to_vec());
    with_nul.extend([0, 0]);
    candidates.push(with_nul);
    if password.is_empty() {
        candidates.push(Zeroizing::new(Vec::new()));
    }
    for pass in &candidates {
        let ok = match alg.as_str() {
            SHA1_OID => mac_matches::<Sha1, Hmac<Sha1>>(pass, salt, rounds, data, expected),
            SHA256_OID => mac_matches::<Sha256, Hmac<Sha256>>(pass, salt, rounds, data, expected),
            SHA384_OID => mac_matches::<Sha384, Hmac<Sha384>>(pass, salt, rounds, data, expected),
            SHA512_OID => mac_matches::<Sha512, Hmac<Sha512>>(pass, salt, rounds, data, expected),
            _ => return Err(P12Error::Unsupported),
        };
        if ok {
            return Ok(());
        }
    }
    Err(P12Error::WrongPassword)
}

fn mac_matches<D, M>(pass: &[u8], salt: &[u8], rounds: u32, data: &[u8], expected: &[u8]) -> bool
where
    D: sha2::Digest
        + sha2::digest::FixedOutputReset
        + sha2::digest::core_api::BlockSizeUser
        + sha2::digest::OutputSizeUser,
    M: Mac + hmac::digest::KeyInit,
{
    let len = <D as sha2::digest::OutputSizeUser>::output_size();
    let key = Zeroizing::new(pkcs12::kdf::derive_key::<D>(
        pass,
        salt,
        Pkcs12KeyType::Mac,
        rounds as i32,
        len,
    ));
    let Ok(mut mac) = <M as hmac::digest::KeyInit>::new_from_slice(&key) else {
        return false;
    };
    mac.update(data);
    mac.verify_slice(expected).is_ok()
}

/// Decrypt one encrypted part of the file (a key or the certificates).
fn decrypt(
    alg: &AlgorithmIdentifierOwned,
    data: &[u8],
    password: &str,
) -> Result<Zeroizing<Vec<u8>>, P12Error> {
    let oid = alg.oid.to_string();
    if oid == PBES2 {
        let encoded = alg.to_der().map_err(|_| P12Error::NotP12)?;
        let scheme = pkcs5::EncryptionScheme::try_from(encoded.as_slice())
            .map_err(|_| P12Error::Unsupported)?;
        let params = scheme.pbes2().ok_or(P12Error::Unsupported)?;
        match params.kdf.pbkdf2() {
            Some(p) if p.iteration_count <= MAX_ROUNDS => {}
            _ => return Err(P12Error::Unsupported),
        }
        // OpenSSL and Windows give PBES2 the password as UTF-8.
        return scheme
            .decrypt(password.as_bytes(), data)
            .map(Zeroizing::new)
            .map_err(|_| P12Error::WrongPassword);
    }
    let params: pkcs12::pbe_params::Pkcs12PbeParams = alg
        .parameters
        .as_ref()
        .and_then(|p| p.decode_as().ok())
        .ok_or(P12Error::NotP12)?;
    let rounds = u32::try_from(params.iterations).map_err(|_| P12Error::NotP12)?;
    if rounds == 0 || rounds > MAX_ROUNDS {
        return Err(P12Error::Unsupported);
    }
    let salt = params.salt.as_bytes();
    let pw = bmp_password(password).ok_or(P12Error::WrongPassword)?;
    let derive = |kind, len| {
        Zeroizing::new(derive_key_bmp::<Sha1>(
            pw.clone(),
            salt,
            kind,
            rounds as i32,
            len,
        ))
    };
    use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
    let wrong = |_| P12Error::WrongPassword;
    let plain = match oid.as_str() {
        PBE_SHA1_3DES => {
            let (key, iv) = (
                derive(Pkcs12KeyType::EncryptionKey, 24),
                derive(Pkcs12KeyType::Iv, 8),
            );
            cbc::Decryptor::<des::TdesEde3>::new_from_slices(&key, &iv)
                .map_err(|_| P12Error::NotP12)?
                .decrypt_padded_vec_mut::<Pkcs7>(data)
                .map_err(wrong)?
        }
        PBE_SHA1_2DES => {
            let (key, iv) = (
                derive(Pkcs12KeyType::EncryptionKey, 16),
                derive(Pkcs12KeyType::Iv, 8),
            );
            cbc::Decryptor::<des::TdesEde2>::new_from_slices(&key, &iv)
                .map_err(|_| P12Error::NotP12)?
                .decrypt_padded_vec_mut::<Pkcs7>(data)
                .map_err(wrong)?
        }
        PBE_SHA1_RC2_128 | PBE_SHA1_RC2_40 => {
            use cbc::cipher::{InnerIvInit, KeyInit};
            let len = if oid == PBE_SHA1_RC2_40 { 5 } else { 16 };
            let (key, iv) = (
                derive(Pkcs12KeyType::EncryptionKey, len),
                derive(Pkcs12KeyType::Iv, 8),
            );
            let cipher = rc2::Rc2::new_from_slice(&key).map_err(|_| P12Error::NotP12)?;
            cbc::Decryptor::<rc2::Rc2>::inner_iv_slice_init(cipher, &iv)
                .map_err(|_| P12Error::NotP12)?
                .decrypt_padded_vec_mut::<Pkcs7>(data)
                .map_err(wrong)?
        }
        _ => return Err(P12Error::Unsupported),
    };
    Ok(Zeroizing::new(plain))
}
