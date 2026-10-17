//! S/MIME mail (RFC 8551): signed and encrypted messages read and written with X.509
//! certificates, the way Outlook, Apple Mail, Thunderbird and most company mail do it.
//!
//! RustCrypto's `cms`, `x509-cert` and `pkcs12` read and build the structures (MIT or
//! Apache-2.0); this module decides what they mean: whose certificate signed a message and
//! whether it is the sender's, which own certificate can decrypt it, which certificates mail
//! may be encrypted to. Whether a certificate chains to an authority the system trusts is
//! Windows' to say (trust.rs); elsewhere rustls-webpki checks it against the system's
//! authorities, and without any a certificate stays unconfirmed.
//!
//! What is kept where (docs/encryption.md), as with OpenPGP:
//! - Certificates live in the mail base (`smime_certs`, schema 16). An own certificate's
//!   private key is stored encrypted (PKCS #8 with PBES2: PBKDF2-SHA256 and AES-256) under
//!   the password its .p12 file had; the password is asked once, when the file is imported,
//!   kept in memory until Telex closes, and in the system's password store only when the user
//!   ticks Remember.
//! - Encrypted mail is stored as it arrived; decrypted, it lives in the same memory as
//!   OpenPGP's (`openpgp::Session`), never on disk or in the search index.
//! - Errors never carry key material or the libraries' messages: each failure is a
//!   [`Problem`] code, which the window words.

mod ber;
pub mod p12;
pub mod trust;

use std::collections::HashMap;

use base64::Engine;
use chrono::NaiveDateTime;
use cms::cert::{CertificateChoices, IssuerAndSerialNumber};
use cms::content_info::ContentInfo;
use cms::enveloped_data::{EnvelopedData, RecipientIdentifier, RecipientInfo};
use cms::signed_data::{SignedData, SignerIdentifier, SignerInfo};
use const_oid::ObjectIdentifier;
use der::asn1::OctetString;
use der::{Decode, Encode};
use mail_parser::{MessageParser, MessagePart, MimeHeaders, PartType};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::Digest;
use spki::{AlgorithmIdentifierOwned, SubjectPublicKeyInfoOwned};
use x509_cert::Certificate;
use zeroize::Zeroizing;

use crate::model::Message;
use crate::openpgp::{
    canonical, fields, first_part, is_type, looks_like_entity, memfind, protocol_is, split_head,
    stopped, with_outer_headers, ImportReport, KeyRef, Opened, Problem, Saved, SigStatus, SigView,
    Stop, View, WriteProblem, MAX_PLAIN,
};
use crate::store::{MailStore, StoreError};
use crate::tr;

/// The largest CMS structure read from a message (a signature, an encrypted message).
pub const MAX_CMS: usize = 48 << 20;
/// How many certificates of one message are looked at.
const MAX_CERTS: usize = 32;
/// How many signatures of one message are checked.
const MAX_SIGNERS: usize = 8;
/// How many recipients of an encrypted message are looked at.
const MAX_RECIPIENTS: usize = 256;
/// The largest RSA key used (larger ones make every check slow, and nobody has them).
const MAX_RSA_BITS: usize = 8192;
/// PBKDF2 rounds for own private keys at rest.
const SEAL_ROUNDS: u32 = if cfg!(test) { 1_000 } else { 210_000 };

const ID_DATA: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.7.1");
const ID_SIGNED_DATA: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.7.2");
const ID_ENVELOPED_DATA: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.7.3");
const ID_AUTH_ENVELOPED_DATA: ObjectIdentifier =
    ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.16.1.23");
const ID_CONTENT_TYPE: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.3");
const ID_MESSAGE_DIGEST: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.4");
const ID_SIGNING_TIME: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.5");
const ID_SMIME_CAPABILITIES: ObjectIdentifier =
    ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.15");
const EMAIL_ADDRESS: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.1");
const COMMON_NAME: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.5.4.3");
const ORGANIZATION: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.5.4.10");
const RSA_ENCRYPTION: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.1");
const RSAES_OAEP: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.7");
const RSASSA_PSS: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.10");
const EC_PUBLIC_KEY: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.10045.2.1");
const P256: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.10045.3.1.7");
const P384: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.3.132.0.34");
const AES128_CBC: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.1.2");
const AES192_CBC: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.1.22");
const AES256_CBC: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.1.42");
const DES_EDE3_CBC: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.3.7");
const SHA256_OID: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.1");
const EKU_EMAIL: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.3.6.1.5.5.7.3.4");
const EKU_ANY: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.5.29.37.0");
const EXT_SAN: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.5.29.17");
const EXT_KEY_USAGE: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.5.29.15");
const EXT_EKU: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.5.29.37");
const EXT_SKI: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.5.29.14");

// ---- Certificates: what one says ----

/// A certificate's facts, as lists and checks need them.
#[derive(Debug, Clone)]
struct Facts {
    /// SHA-256 of the certificate, upper-case hex (what Windows calls its SHA-256 thumbprint).
    fingerprint: String,
    /// "Anna Kowalska <anna@studio.pl>"
    name: String,
    issuer: String,
    /// Lower-case addresses: subjectAltName's rfc822Name, and the subject's emailAddress.
    emails: Vec<String>,
    serial: String,
    /// How a CMS structure names it: SHA-256 of its issuer (DER) and its serial, in hex.
    issuer_serial: String,
    /// Its subject key identifier, hex, when it has one.
    ski: Option<String>,
    not_before: i64,
    not_after: i64,
    algorithm: String,
    can_sign: bool,
    can_encrypt: bool,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

fn local(secs: i64) -> NaiveDateTime {
    chrono::DateTime::from_timestamp(secs, 0)
        .map(|utc| utc.with_timezone(&chrono::Local).naive_local())
        .unwrap_or_default()
}

/// The text of a directory string (UTF8String, PrintableString, IA5String, BMPString…).
fn text_of(value: &der::Any) -> Option<String> {
    use der::{Tag, Tagged};
    let bytes = value.value();
    match value.tag() {
        Tag::Utf8String | Tag::PrintableString | Tag::Ia5String | Tag::VisibleString => {
            Some(String::from_utf8_lossy(bytes).into_owned())
        }
        // Teletex is treated as Latin-1, as everyone does.
        Tag::TeletexString => Some(bytes.iter().map(|&b| char::from(b)).collect()),
        Tag::BmpString => {
            let units: Vec<u16> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_be_bytes(*c))
                .collect();
            Some(String::from_utf16_lossy(&units))
        }
        _ => None,
    }
}

/// The first value of an attribute in a name (CN, O, emailAddress).
fn name_part(name: &x509_cert::name::Name, oid: ObjectIdentifier) -> Option<String> {
    name.0
        .iter()
        .flat_map(|rdn| rdn.0.iter())
        .find(|atv| atv.oid == oid)
        .and_then(|atv| text_of(&atv.value))
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn display_name(name: &x509_cert::name::Name) -> String {
    name_part(name, COMMON_NAME)
        .or_else(|| name_part(name, ORGANIZATION))
        .unwrap_or_else(|| name.to_string())
}

fn issuer_serial_key(issuer: &x509_cert::name::Name, serial: &[u8]) -> String {
    let issuer = issuer.to_der().unwrap_or_default();
    format!(
        "{}:{}",
        hex(&sha2::Sha256::digest(issuer)),
        hex(trim_serial(serial))
    )
}

/// A serial number's bytes without the leading zero DER puts before a high bit.
fn trim_serial(serial: &[u8]) -> &[u8] {
    match serial {
        [0, rest @ ..] if !rest.is_empty() => rest,
        s => s,
    }
}

fn extension(cert: &Certificate, oid: ObjectIdentifier) -> Option<&[u8]> {
    cert.tbs_certificate
        .extensions
        .as_ref()?
        .iter()
        .find(|e| e.extn_id == oid)
        .map(|e| e.extn_value.as_bytes())
}

fn facts(der: &[u8]) -> Option<Facts> {
    let cert = Certificate::from_der(der).ok()?;
    let tbs = &cert.tbs_certificate;
    let mut emails: Vec<String> = Vec::new();
    if let Some(san) = extension(&cert, EXT_SAN)
        .and_then(|b| x509_cert::ext::pkix::SubjectAltName::from_der(b).ok())
    {
        for name in san.0 {
            if let x509_cert::ext::pkix::name::GeneralName::Rfc822Name(mail) = name {
                emails.push(mail.as_str().trim().to_lowercase());
            }
        }
    }
    if let Some(mail) = name_part(&tbs.subject, EMAIL_ADDRESS) {
        emails.push(mail.to_lowercase());
    }
    emails.retain(|e| e.contains('@') && !e.contains(char::is_whitespace));
    emails.dedup();
    let subject = display_name(&tbs.subject);
    let name = match emails.first() {
        Some(e) if subject.to_lowercase() != *e => format!("{subject} <{e}>"),
        Some(e) => e.clone(),
        None => subject.clone(),
    };
    let usage = extension(&cert, EXT_KEY_USAGE)
        .and_then(|b| x509_cert::ext::pkix::KeyUsage::from_der(b).ok());
    let eku_ok = extension(&cert, EXT_EKU)
        .and_then(|b| x509_cert::ext::pkix::ExtendedKeyUsage::from_der(b).ok())
        .is_none_or(|eku| eku.0.iter().any(|o| *o == EKU_EMAIL || *o == EKU_ANY));
    let spki = &tbs.subject_public_key_info;
    let rsa = spki.algorithm.oid == RSA_ENCRYPTION;
    let can_sign = eku_ok
        && usage
            .as_ref()
            .is_none_or(|u| u.digital_signature() || u.non_repudiation());
    let can_encrypt = eku_ok && rsa && usage.as_ref().is_none_or(|u| u.key_encipherment());
    let serial = tbs.serial_number.as_bytes();
    Some(Facts {
        fingerprint: hex(&sha2::Sha256::digest(der)),
        name,
        issuer: display_name(&tbs.issuer),
        emails,
        serial: hex(trim_serial(serial)),
        issuer_serial: issuer_serial_key(&tbs.issuer, serial),
        ski: extension(&cert, EXT_SKI)
            .and_then(|b| OctetString::from_der(b).ok())
            .map(|o| hex(o.as_bytes())),
        not_before: tbs.validity.not_before.to_unix_duration().as_secs() as i64,
        not_after: tbs.validity.not_after.to_unix_duration().as_secs() as i64,
        algorithm: algorithm_name(spki),
        can_sign,
        can_encrypt,
    })
}

fn algorithm_name(spki: &SubjectPublicKeyInfoOwned) -> String {
    if spki.algorithm.oid == RSA_ENCRYPTION {
        return match rsa_public(spki) {
            Some(k) => format!("RSA {}", rsa::traits::PublicKeyParts::size(&k) * 8),
            None => "RSA".to_owned(),
        };
    }
    if spki.algorithm.oid == EC_PUBLIC_KEY {
        return match curve_of(spki) {
            Some(c) if c == P256 => "ECDSA P-256".to_owned(),
            Some(c) if c == P384 => "ECDSA P-384".to_owned(),
            _ => "ECDSA".to_owned(),
        };
    }
    spki.algorithm.oid.to_string()
}

fn curve_of(spki: &SubjectPublicKeyInfoOwned) -> Option<ObjectIdentifier> {
    spki.algorithm.parameters.as_ref()?.decode_as().ok()
}

fn rsa_public(spki: &SubjectPublicKeyInfoOwned) -> Option<rsa::RsaPublicKey> {
    let bits = spki.subject_public_key.as_bytes()?;
    let key = rsa::pkcs1::RsaPublicKey::from_der(bits).ok()?;
    let n = rsa::BigUint::from_bytes_be(key.modulus.as_bytes());
    let e = rsa::BigUint::from_bytes_be(key.public_exponent.as_bytes());
    rsa::RsaPublicKey::new_with_max_size(n, e, MAX_RSA_BITS).ok()
}

/// A certificate's public key, as [`public_key_of`] gives a private key's (EC points
/// uncompressed), to pair a key with its certificate.
pub(crate) fn cert_public_key(der: &[u8]) -> Option<Vec<u8>> {
    let cert = Certificate::from_der(der).ok()?;
    let spki = &cert.tbs_certificate.subject_public_key_info;
    let bits = spki.subject_public_key.as_bytes()?;
    if spki.algorithm.oid == EC_PUBLIC_KEY {
        let curve = curve_of(spki)?;
        if curve == P256 {
            let p = p256::PublicKey::from_sec1_bytes(bits).ok()?;
            return Some(p256::EncodedPoint::from(p).as_bytes().to_vec());
        }
        if curve == P384 {
            let p = p384::PublicKey::from_sec1_bytes(bits).ok()?;
            return Some(p384::EncodedPoint::from(p).as_bytes().to_vec());
        }
        return None;
    }
    Some(bits.to_vec())
}

/// A private key (PKCS #8) that Telex can use.
pub(crate) enum PrivateKey {
    Rsa(Box<rsa::RsaPrivateKey>),
    P256(p256::SecretKey),
    P384(p384::SecretKey),
}

fn private_key(pkcs8: &[u8]) -> Option<PrivateKey> {
    use pkcs8::DecodePrivateKey;
    if let Ok(k) = rsa::RsaPrivateKey::from_pkcs8_der(pkcs8) {
        return Some(PrivateKey::Rsa(Box::new(k)));
    }
    if let Ok(k) = p256::SecretKey::from_pkcs8_der(pkcs8) {
        return Some(PrivateKey::P256(k));
    }
    p384::SecretKey::from_pkcs8_der(pkcs8)
        .ok()
        .map(PrivateKey::P384)
}

/// A private key's public key, as [`cert_public_key`] gives a certificate's.
pub(crate) fn public_key_of(pkcs8: &[u8]) -> Option<Vec<u8>> {
    match private_key(pkcs8)? {
        PrivateKey::Rsa(k) => {
            let public = rsa::pkcs1::EncodeRsaPublicKey::to_pkcs1_der(&k.to_public_key()).ok()?;
            Some(public.as_bytes().to_vec())
        }
        PrivateKey::P256(k) => Some(p256::EncodedPoint::from(k.public_key()).as_bytes().to_vec()),
        PrivateKey::P384(k) => Some(p384::EncodedPoint::from(k.public_key()).as_bytes().to_vec()),
    }
}

/// Certificates in PEM or DER, or a certificates-only CMS file (.p7b, .p7c).
fn read_certificates(data: &[u8]) -> Vec<Vec<u8>> {
    let text = String::from_utf8_lossy(data);
    if text.contains("-----BEGIN CERTIFICATE-----") {
        let mut out = Vec::new();
        let mut rest = text.as_ref();
        while let Some(start) = rest.find("-----BEGIN CERTIFICATE-----") {
            let Some(end) = rest[start..].find("-----END CERTIFICATE-----") else {
                break;
            };
            let block = &rest[start..start + end + "-----END CERTIFICATE-----".len()];
            if let Ok((_, der)) = der::pem::decode_vec(block.as_bytes()) {
                if facts(&der).is_some() {
                    out.push(der);
                }
            }
            rest = &rest[start + end..][1..];
            if out.len() >= MAX_CERTS {
                break;
            }
        }
        return out;
    }
    let Ok(der) = ber::to_der(data) else {
        return Vec::new();
    };
    if facts(&der).is_some() {
        return vec![der];
    }
    ContentInfo::from_der(&der)
        .ok()
        .filter(|ci| ci.content_type == ID_SIGNED_DATA)
        .and_then(|ci| ci.content.decode_as::<SignedData>().ok())
        .map(|sd| message_certificates(&sd))
        .unwrap_or_default()
}

fn message_certificates(sd: &SignedData) -> Vec<Vec<u8>> {
    sd.certificates
        .iter()
        .flat_map(|set| set.0.iter())
        .filter_map(|c| match c {
            CertificateChoices::Certificate(cert) => cert.to_der().ok(),
            CertificateChoices::Other(_) => None,
        })
        .take(MAX_CERTS)
        .collect()
}

/// Several certificates, DER one after another, as `smime_certs.chain` keeps them.
fn join_chain(chain: &[Vec<u8>]) -> Vec<u8> {
    chain.concat()
}

fn split_chain(data: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut rest = data;
    while !rest.is_empty() && out.len() < MAX_CERTS {
        let Ok(cert) = Certificate::from_der_partial(rest) else {
            break;
        };
        let Ok(der) = cert.0.to_der() else { break };
        rest = cert.1;
        out.push(der);
    }
    out
}

trait FromDerPartial: Sized {
    fn from_der_partial(data: &[u8]) -> der::Result<(Self, &[u8])>;
}

impl FromDerPartial for Certificate {
    fn from_der_partial(data: &[u8]) -> der::Result<(Self, &[u8])> {
        let mut reader = der::SliceReader::new(data)?;
        let cert = <Certificate as der::Decode>::decode(&mut reader)?;
        let used = u32::from(der::Reader::position(&reader)) as usize;
        Ok((cert, &data[used..]))
    }
}

// ---- Keys at rest ----

/// An own private key (PKCS #8) encrypted under `password` (PBES2: PBKDF2-SHA256, AES-256).
fn seal(pkcs8: &[u8], password: &str) -> Result<Vec<u8>, StoreError> {
    use rand::RngCore;
    let mut salt = [0u8; 16];
    let mut iv = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut salt);
    rand::thread_rng().fill_bytes(&mut iv);
    let failed = || {
        StoreError::Invalid(tr!(
            "Telex couldn't keep the certificate's key.",
            "Telex nie zdołał zachować klucza certyfikatu."
        ))
    };
    let params = pkcs5::pbes2::Parameters::pbkdf2_sha256_aes256cbc(SEAL_ROUNDS, &salt, &iv)
        .map_err(|_| failed())?;
    let info = pkcs8::PrivateKeyInfo::try_from(pkcs8).map_err(|_| failed())?;
    let sealed = info
        .encrypt_with_params(params, password.as_bytes())
        .map_err(|_| failed())?;
    Ok(sealed.as_bytes().to_vec())
}

/// [`seal`] undone: the private key, or `None` when the password doesn't open it.
fn unseal(sealed: &[u8], password: &str) -> Option<Zeroizing<Vec<u8>>> {
    let info = pkcs8::EncryptedPrivateKeyInfo::try_from(sealed).ok()?;
    let doc = info.decrypt(password.as_bytes()).ok()?;
    Some(Zeroizing::new(doc.as_bytes().to_vec()))
}

/// An own certificate as stored.
struct OwnRow {
    fingerprint: String,
    cert: Vec<u8>,
    chain: Vec<Vec<u8>>,
    /// The private key, sealed (`seal`).
    sealed: Vec<u8>,
    protected: bool,
    facts: Facts,
}

impl OwnRow {
    fn key_ref(&self) -> KeyRef {
        KeyRef {
            fingerprint: self.fingerprint.clone(),
            name: self.facts.name.clone(),
        }
    }
}

/// What S/MIME keeps while Telex runs, never on disk: own private keys opened this session.
#[derive(Default)]
pub struct Session {
    keys: HashMap<String, Zeroizing<Vec<u8>>>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Session({} keys)", self.keys.len())
    }
}

// ---- Certificates in the mail base ----

/// Where a certificate came from, as `smime_certs.source` says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// A .p12 or .pfx file with its private key: the user's own.
    Own,
    /// A certificate file the user imported: confirmed by doing so.
    File,
    /// It came with signed mail from its owner.
    Message,
}

impl Source {
    fn as_str(self) -> &'static str {
        match self {
            Self::Own => "own",
            Self::File => "file",
            Self::Message => "message",
        }
    }
}

/// A certificate in Settings › Keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CertInfo {
    pub fingerprint: String,
    pub name: String,
    pub emails: Vec<String>,
    pub issuer: String,
    pub serial: String,
    pub not_before: NaiveDateTime,
    pub not_after: NaiveDateTime,
    /// "RSA 2048", "ECDSA P-256"
    pub algorithm: String,
    /// The user's own: Telex has its private key.
    pub secret: bool,
    /// Its private key has a password.
    pub protected: bool,
    /// own, file, message
    pub source: String,
    /// The user said it is the person's.
    pub verified: bool,
    /// It chains to an authority the system trusts (now).
    pub trusted: bool,
    pub expired: bool,
    pub can_sign: bool,
    pub can_encrypt: bool,
    pub added: NaiveDateTime,
}

fn save_cert(
    db: &Connection,
    der: &[u8],
    chain: &[Vec<u8>],
    secret: Option<(&[u8], bool)>,
    source: Source,
    now: NaiveDateTime,
) -> Result<(KeyRef, Saved), StoreError> {
    let f = facts(der).ok_or_else(unreadable)?;
    let reference = KeyRef {
        fingerprint: f.fingerprint.clone(),
        name: f.name.clone(),
    };
    let stored: Option<(bool, bool)> = db
        .query_row(
            "SELECT secret IS NOT NULL, length(chain) > 0 FROM smime_certs WHERE fingerprint = ?1",
            [&f.fingerprint],
            |r| Ok((r.get(0)?, r.get::<_, Option<bool>>(1)?.unwrap_or(false))),
        )
        .optional()?;
    let chain = join_chain(chain);
    let saved = match stored {
        None => Saved::Added,
        Some((had_secret, had_chain)) => {
            let gains_secret = secret.is_some() && !had_secret;
            if !gains_secret {
                // A copy that came with mail changes nothing but a missing chain; a file the
                // user picked confirms it.
                if !had_chain && !chain.is_empty() {
                    db.execute(
                        "UPDATE smime_certs SET chain = ?2 WHERE fingerprint = ?1",
                        params![f.fingerprint, chain],
                    )?;
                }
                if source == Source::File {
                    db.execute(
                        "UPDATE smime_certs SET verified = 1 WHERE fingerprint = ?1",
                        [&f.fingerprint],
                    )?;
                }
                return Ok((reference, Saved::Unchanged));
            }
            Saved::Updated
        }
    };
    let (sealed, protected) = match secret {
        Some((s, p)) => (Some(s), p),
        None => (None, false),
    };
    db.execute(
        "INSERT INTO smime_certs (fingerprint, cert, secret, protected, chain, name, emails,
             issuer, serial, issuer_serial, ski, not_before, not_after, source, verified, added)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
         ON CONFLICT (fingerprint) DO UPDATE SET secret = excluded.secret,
             protected = excluded.protected, source = excluded.source,
             chain = CASE WHEN length(excluded.chain) > 0 THEN excluded.chain ELSE smime_certs.chain END,
             verified = MAX(smime_certs.verified, excluded.verified)",
        params![
            f.fingerprint,
            der,
            sealed,
            protected,
            chain,
            f.name,
            f.emails.join(" "),
            f.issuer,
            f.serial,
            f.issuer_serial,
            f.ski,
            local(f.not_before),
            local(f.not_after),
            source.as_str(),
            source != Source::Message,
            now,
        ],
    )?;
    Ok((reference, saved))
}

fn unreadable() -> StoreError {
    StoreError::Invalid(tr!(
        "That certificate can't be read.",
        "Tego certyfikatu nie da się odczytać."
    ))
}

fn no_such_cert() -> StoreError {
    StoreError::Invalid(tr!(
        "That certificate isn't in Telex any more.",
        "Tego certyfikatu nie ma już w Telexie."
    ))
}

/// A stored certificate: (DER, chain, verified, source).
type Stored = (Vec<u8>, Vec<Vec<u8>>, bool, String);

fn stored_by(db: &Connection, column: &str, value: &str) -> Option<Stored> {
    let sql = format!(
        "SELECT cert, chain, verified, source FROM smime_certs WHERE {column} = ?1 LIMIT 1"
    );
    db.query_row(&sql, [value], |r| {
        Ok((
            r.get::<_, Vec<u8>>(0)?,
            r.get::<_, Option<Vec<u8>>>(1)?.unwrap_or_default(),
            r.get::<_, bool>(2)?,
            r.get::<_, String>(3)?,
        ))
    })
    .optional()
    .ok()
    .flatten()
    .map(|(c, chain, v, s)| (c, split_chain(&chain), v, s))
}

// ---- Finding S/MIME in a message ----

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Found {
    /// multipart/signed with application/pkcs7-signature: the signed part as signed and the
    /// signature (CMS SignedData, DER).
    Signed {
        signed: Vec<u8>,
        signature: Vec<u8>,
        partial: bool,
    },
    /// application/pkcs7-mime, signed-data: the message is inside the signature.
    Opaque { cms: Vec<u8>, partial: bool },
    /// application/pkcs7-mime, enveloped-data (or authEnveloped-data): encrypted.
    Enveloped { cms: Vec<u8>, partial: bool },
}

fn is_signature_type(part: &MessagePart<'_>) -> bool {
    is_type(part, "application", "pkcs7-signature")
        || is_type(part, "application", "x-pkcs7-signature")
}

fn file_name_ends(part: &MessagePart<'_>, ext: &str) -> bool {
    part.attachment_name()
        .is_some_and(|n| n.trim().to_ascii_lowercase().ends_with(ext))
}

fn is_mime_type(part: &MessagePart<'_>) -> bool {
    is_type(part, "application", "pkcs7-mime")
        || is_type(part, "application", "x-pkcs7-mime")
        || (is_type(part, "application", "octet-stream") && file_name_ends(part, ".p7m"))
}

/// A part that only carries S/MIME's machinery (a signature, an encrypted message): not an
/// attachment to list.
pub(crate) fn is_machinery(part: &MessagePart<'_>) -> bool {
    is_signature_type(part)
        || is_mime_type(part)
        || (is_type(part, "application", "octet-stream") && file_name_ends(part, ".p7s"))
}

/// A CMS structure from a message: DER, and its content type.
fn content_info(data: &[u8]) -> Option<(ContentInfo, Vec<u8>)> {
    if data.len() > MAX_CMS {
        return None;
    }
    let der = ber::to_der(data).ok()?;
    let ci = ContentInfo::from_der(&der).ok()?;
    Some((ci, der))
}

/// The S/MIME a message holds, if any. Cheap for mail without any.
pub(crate) fn find(raw: &[u8]) -> Option<Found> {
    if !memfind(raw, b"pkcs7") && !memfind(raw, b"PKCS7") && !memfind(raw, b".p7m") {
        return None;
    }
    let p = MessageParser::default().parse(raw)?;
    for (index, part) in p.parts.iter().enumerate() {
        let partial = index != 0;
        if let PartType::Multipart(kids) = &part.body {
            if is_type(part, "multipart", "signed")
                && (protocol_is(part, "application/pkcs7-signature")
                    || protocol_is(part, "application/x-pkcs7-signature"))
            {
                let boundary = part.content_type()?.attribute("boundary")?;
                let body = raw.get(part.offset_body as usize..part.offset_end as usize)?;
                let signed = first_part(body, boundary.as_bytes())?;
                let signature = kids
                    .iter()
                    .map(|&k| &p.parts[k as usize])
                    .find(|k| is_signature_type(k))?;
                return Some(Found::Signed {
                    signed: canonical(signed),
                    signature: signature.contents().to_vec(),
                    partial,
                });
            }
            continue;
        }
        if is_mime_type(part) {
            match content_info(part.contents()) {
                Some((ci, der)) if ci.content_type == ID_SIGNED_DATA => {
                    return Some(Found::Opaque { cms: der, partial })
                }
                Some((ci, der))
                    if ci.content_type == ID_ENVELOPED_DATA
                        || ci.content_type == ID_AUTH_ENVELOPED_DATA =>
                {
                    return Some(Found::Enveloped { cms: der, partial })
                }
                Some(_) => {}
                // Damaged (or not CMS at all): what its smime-type says it is, so the line
                // above it can say it can't be read.
                None => {
                    let signed = part
                        .content_type()
                        .and_then(|ct| ct.attribute("smime-type"))
                        .is_some_and(|t| t.trim().eq_ignore_ascii_case("signed-data"));
                    let cms = part.contents().to_vec();
                    return Some(if signed {
                        Found::Opaque { cms, partial }
                    } else {
                        Found::Enveloped { cms, partial }
                    });
                }
            }
        }
    }
    None
}

/// The message is S/MIME-encrypted.
pub fn is_encrypted(raw: &[u8]) -> bool {
    matches!(find(raw), Some(Found::Enveloped { .. }))
}

// ---- Checking signatures ----

#[derive(Debug, Clone, Copy)]
enum Hash {
    Sha1,
    Sha224,
    Sha256,
    Sha384,
    Sha512,
}

fn hash_of(oid: &ObjectIdentifier) -> Option<Hash> {
    Some(match oid.to_string().as_str() {
        "1.3.14.3.2.26" => Hash::Sha1,
        "2.16.840.1.101.3.4.2.4" => Hash::Sha224,
        "2.16.840.1.101.3.4.2.1" => Hash::Sha256,
        "2.16.840.1.101.3.4.2.2" => Hash::Sha384,
        "2.16.840.1.101.3.4.2.3" => Hash::Sha512,
        _ => return None,
    })
}

fn digest(h: Hash, data: &[u8]) -> Vec<u8> {
    match h {
        Hash::Sha1 => sha1::Sha1::digest(data).to_vec(),
        Hash::Sha224 => sha2::Sha224::digest(data).to_vec(),
        Hash::Sha256 => sha2::Sha256::digest(data).to_vec(),
        Hash::Sha384 => sha2::Sha384::digest(data).to_vec(),
        Hash::Sha512 => sha2::Sha512::digest(data).to_vec(),
    }
}

/// What checking one signature found, before deciding what it means.
enum Checked {
    /// Valid or not, made with `cert` (DER), at `created` (Unix seconds) if it says.
    Made {
        valid: bool,
        cert: Vec<u8>,
        chain: Vec<Vec<u8>>,
        created: Option<i64>,
        verified: bool,
        own: bool,
    },
    /// The certificate isn't in the message or the mail base: its serial number.
    NoCert { serial: String, issuer: String },
    /// Made in a way Telex doesn't check (RSASSA-PSS, MD5, an unknown algorithm).
    Unsupported,
}

/// Does `sig` verify `hashed` (the digest of what was signed, made with `hash`) with `spki`?
/// `None`: an algorithm Telex doesn't check.
fn verify(
    spki: &SubjectPublicKeyInfoOwned,
    sig_alg: &AlgorithmIdentifierOwned,
    hash: Hash,
    hashed: &[u8],
    sig: &[u8],
) -> Option<bool> {
    if sig_alg.oid == RSASSA_PSS {
        return None;
    }
    if spki.algorithm.oid == RSA_ENCRYPTION {
        let key = rsa_public(spki)?;
        let scheme = match hash {
            Hash::Sha1 => rsa::Pkcs1v15Sign::new::<sha1::Sha1>(),
            Hash::Sha224 => rsa::Pkcs1v15Sign::new::<sha2::Sha224>(),
            Hash::Sha256 => rsa::Pkcs1v15Sign::new::<sha2::Sha256>(),
            Hash::Sha384 => rsa::Pkcs1v15Sign::new::<sha2::Sha384>(),
            Hash::Sha512 => rsa::Pkcs1v15Sign::new::<sha2::Sha512>(),
        };
        return Some(key.verify(scheme, hashed, sig).is_ok());
    }
    if spki.algorithm.oid == EC_PUBLIC_KEY {
        use signature::hazmat::PrehashVerifier;
        let bits = spki.subject_public_key.as_bytes()?;
        let curve = curve_of(spki)?;
        if curve == P256 {
            let key = p256::ecdsa::VerifyingKey::from_sec1_bytes(bits).ok()?;
            let Ok(sig) = p256::ecdsa::Signature::from_der(sig) else {
                return Some(false);
            };
            return Some(key.verify_prehash(hashed, &sig).is_ok());
        }
        if curve == P384 {
            let key = p384::ecdsa::VerifyingKey::from_sec1_bytes(bits).ok()?;
            let Ok(sig) = p384::ecdsa::Signature::from_der(sig) else {
                return Some(false);
            };
            return Some(key.verify_prehash(hashed, &sig).is_ok());
        }
    }
    None
}

/// The certificate a signature (or a recipient) names: by issuer and serial, or by subject
/// key identifier.
fn names_cert(sid: &SignerIdentifier, cert: &Certificate) -> bool {
    match sid {
        SignerIdentifier::IssuerAndSerialNumber(is) => {
            is.issuer == cert.tbs_certificate.issuer
                && trim_serial(is.serial_number.as_bytes())
                    == trim_serial(cert.tbs_certificate.serial_number.as_bytes())
        }
        SignerIdentifier::SubjectKeyIdentifier(ski) => {
            extension(cert, EXT_SKI)
                .and_then(|b| OctetString::from_der(b).ok())
                .as_ref()
                == Some(&ski.0)
        }
    }
}

fn attribute(info: &SignerInfo, oid: ObjectIdentifier) -> Option<&der::Any> {
    info.signed_attrs
        .as_ref()?
        .iter()
        .find(|a| a.oid == oid)
        .and_then(|a| a.values.iter().next())
}

/// The signing time a signature claims (signed, so the signer stands behind it).
fn signing_time(info: &SignerInfo) -> Option<i64> {
    let value = attribute(info, ID_SIGNING_TIME)?;
    let time = x509_cert::time::Time::from_der(&value.to_der().ok()?).ok()?;
    Some(time.to_unix_duration().as_secs() as i64)
}

/// Check every signature of `sd` over `content` (the detached data, or its own eContent).
fn check(db: &Connection, sd: &SignedData, content: &[u8]) -> Vec<Checked> {
    let certs = message_certificates(sd);
    let parsed: Vec<(Certificate, &Vec<u8>)> = certs
        .iter()
        .filter_map(|d| Certificate::from_der(d).ok().map(|c| (c, d)))
        .collect();
    let mut out = Vec::new();
    for info in sd.signer_infos.0.iter().take(MAX_SIGNERS) {
        let from_message = parsed.iter().find(|(c, _)| names_cert(&info.sid, c));
        let (cert_der, chain, verified, own) = match from_message {
            Some((_, der)) => {
                let fp = hex(&sha2::Sha256::digest(der));
                let stored = stored_by(db, "fingerprint", &fp);
                (
                    (*der).clone(),
                    certs.iter().filter(|c| c != der).cloned().collect(),
                    stored.as_ref().is_some_and(|s| s.2),
                    stored.as_ref().is_some_and(|s| s.3 == "own"),
                )
            }
            None => {
                let found = match &info.sid {
                    SignerIdentifier::IssuerAndSerialNumber(is) => stored_by(
                        db,
                        "issuer_serial",
                        &issuer_serial_key(&is.issuer, is.serial_number.as_bytes()),
                    ),
                    SignerIdentifier::SubjectKeyIdentifier(ski) => {
                        stored_by(db, "ski", &hex(ski.0.as_bytes()))
                    }
                };
                match found {
                    Some((der, chain, verified, source)) => (der, chain, verified, source == "own"),
                    None => {
                        let (serial, issuer) = match &info.sid {
                            SignerIdentifier::IssuerAndSerialNumber(is) => (
                                hex(trim_serial(is.serial_number.as_bytes())),
                                display_name(&is.issuer),
                            ),
                            SignerIdentifier::SubjectKeyIdentifier(ski) => {
                                (hex(ski.0.as_bytes()), String::new())
                            }
                        };
                        out.push(Checked::NoCert { serial, issuer });
                        continue;
                    }
                }
            }
        };
        let Some(hash) = hash_of(&info.digest_alg.oid) else {
            out.push(Checked::Unsupported);
            continue;
        };
        let Ok(cert) = Certificate::from_der(&cert_der) else {
            out.push(Checked::Unsupported);
            continue;
        };
        let content_digest = digest(hash, content);
        // With signed attributes, the signature covers them (their DER as a SET), and they
        // carry the content's digest; without, it covers the content itself.
        let (hashed, attrs_ok) = match &info.signed_attrs {
            Some(attrs) => {
                let md_ok = attribute(info, ID_MESSAGE_DIGEST)
                    .and_then(|v| OctetString::from_der(&v.to_der().ok()?).ok())
                    .is_some_and(|md| md.as_bytes() == content_digest.as_slice());
                let ct_ok = attribute(info, ID_CONTENT_TYPE)
                    .and_then(|v| ObjectIdentifier::from_der(&v.to_der().ok()?).ok())
                    .is_some_and(|ct| ct == sd.encap_content_info.econtent_type);
                match attrs.to_der() {
                    Ok(der) => (digest(hash, &der), md_ok && ct_ok),
                    Err(_) => (Vec::new(), false),
                }
            }
            None => (content_digest, true),
        };
        let valid = match verify(
            &cert.tbs_certificate.subject_public_key_info,
            &info.signature_algorithm,
            hash,
            &hashed,
            info.signature.as_bytes(),
        ) {
            Some(v) => v && attrs_ok,
            None => {
                out.push(Checked::Unsupported);
                continue;
            }
        };
        out.push(Checked::Made {
            valid,
            cert: cert_der,
            chain,
            created: signing_time(info),
            verified,
            own,
        });
    }
    out
}

/// What a checked signature means for the sender `from`, at `now`.
fn verdict(checked: &Checked, from: &str, now: i64) -> SigView {
    let from = from.trim().to_lowercase();
    match checked {
        Checked::NoCert { serial, issuer } => SigView {
            status: SigStatus::NoKey,
            key: None,
            key_id: serial.clone(),
            created: None,
            issuer: (!issuer.is_empty()).then(|| issuer.clone()),
        },
        Checked::Unsupported => SigView {
            status: SigStatus::Unsupported,
            key: None,
            key_id: String::new(),
            created: None,
            issuer: None,
        },
        Checked::Made {
            valid,
            cert,
            chain,
            created,
            verified,
            own,
        } => {
            let f = facts(cert);
            let view = |status| SigView {
                status,
                key: f.as_ref().map(|f| KeyRef {
                    fingerprint: f.fingerprint.clone(),
                    name: f.name.clone(),
                }),
                key_id: f.as_ref().map(|f| f.serial.clone()).unwrap_or_default(),
                created: created.map(local),
                issuer: f.as_ref().map(|f| f.issuer.clone()),
            };
            let Some(f) = &f else {
                return view(SigStatus::Bad);
            };
            if !valid {
                return view(SigStatus::Bad);
            }
            let at = created.unwrap_or(now);
            if at < f.not_before || at > f.not_after {
                return view(SigStatus::Expired);
            }
            if !f.emails.contains(&from) {
                return view(SigStatus::OtherSender);
            }
            let confirmed = *own || *verified || {
                match trust::chain(cert, chain, at) {
                    trust::Trust::Revoked => return view(SigStatus::Revoked),
                    trust::Trust::Trusted => f.can_sign,
                    _ => false,
                }
            };
            view(if confirmed {
                SigStatus::Good
            } else {
                SigStatus::Unconfirmed
            })
        }
    }
}

/// Of several signatures, the one to show (as OpenPGP ranks them).
fn best(views: Vec<SigView>) -> Option<SigView> {
    crate::openpgp::best(views)
}

/// A SignedData from a CMS structure (DER or BER).
fn signed_data(data: &[u8]) -> Option<SignedData> {
    let (ci, _) = content_info(data)?;
    (ci.content_type == ID_SIGNED_DATA)
        .then(|| ci.content.decode_as::<SignedData>().ok())
        .flatten()
}

/// The content an opaque signature carries.
fn econtent(sd: &SignedData) -> Option<Vec<u8>> {
    let any = sd.encap_content_info.econtent.as_ref()?;
    // eContent is [0] EXPLICIT OCTET STRING; `der` gives the inner element.
    any.decode_as::<OctetString>()
        .ok()
        .map(OctetString::into_bytes)
}

fn bad() -> SigView {
    SigView {
        status: SigStatus::Bad,
        key: None,
        key_id: String::new(),
        created: None,
        issuer: None,
    }
}

impl MailStore {
    /// The verdict on a SignedData over `content` in `message`; a valid signer's certificate
    /// (for the From address, within its dates) is kept for encrypting to them later, unless the
    /// message is in Spam or Trash.
    fn smime_judge(&self, sd: &SignedData, content: &[u8], message: &Message) -> SigView {
        let from = message.from_email.as_str();
        let now = now_secs();
        let checked = check(&self.db, sd, content);
        let junk = matches!(
            self.kind_of(&message.folder),
            Some(crate::model::FolderKind::Spam | crate::model::FolderKind::Trash)
        );
        if !junk {
            learn(&self.db, &checked, from, now);
        }
        let views: Vec<SigView> = checked.iter().map(|c| verdict(c, from, now)).collect();
        best(views).unwrap_or_else(bad)
    }
}

/// Keep the certificates of valid signatures by the sender (`source` message): S/MIME's
/// way of learning whom mail can be encrypted to. A certificate Telex has stays as it is.
fn learn(db: &Connection, checked: &[Checked], from: &str, now: i64) {
    let from = from.trim().to_lowercase();
    let when = chrono::Local::now().naive_local();
    for c in checked {
        let Checked::Made {
            valid: true,
            cert,
            chain,
            ..
        } = c
        else {
            continue;
        };
        let Some(f) = facts(cert) else { continue };
        if f.emails.contains(&from) && f.not_before <= now && now <= f.not_after {
            let _ = save_cert(db, cert, chain, None, Source::Message, when);
        }
    }
}

/// Learn the certificate of a signed message as it arrives (sync): only mail whose
/// signature checks, from the address the certificate names.
pub(crate) fn note_signed(db: &Connection, raw: &[u8], from: &str) -> Result<(), StoreError> {
    let (sd, content) = match find(raw) {
        Some(Found::Signed {
            signed, signature, ..
        }) => match signed_data(&signature) {
            Some(sd) => (sd, signed),
            None => return Ok(()),
        },
        Some(Found::Opaque { cms, .. }) => match signed_data(&cms) {
            Some(sd) => match econtent(&sd) {
                Some(c) => (sd, c),
                None => return Ok(()),
            },
            None => return Ok(()),
        },
        _ => return Ok(()),
    };
    let checked = check(db, &sd, &content);
    learn(db, &checked, from, now_secs());
    Ok(())
}

// ---- Decrypting ----

fn recipient_named(rid: &RecipientIdentifier, f: &Facts) -> bool {
    match rid {
        RecipientIdentifier::IssuerAndSerialNumber(is) => {
            issuer_serial_key(&is.issuer, is.serial_number.as_bytes()) == f.issuer_serial
        }
        RecipientIdentifier::SubjectKeyIdentifier(ski) => {
            f.ski.as_deref() == Some(hex(ski.0.as_bytes()).as_str())
        }
    }
}

fn describe(rid: &RecipientIdentifier) -> String {
    match rid {
        RecipientIdentifier::IssuerAndSerialNumber(is) => format!(
            "{} · {}",
            display_name(&is.issuer),
            hex(trim_serial(is.serial_number.as_bytes()))
        ),
        RecipientIdentifier::SubjectKeyIdentifier(ski) => hex(ski.0.as_bytes()),
    }
}

/// The content-encryption key from a key transport recipient, with an RSA private key.
fn unwrap_key(key: &PrivateKey, alg: &AlgorithmIdentifierOwned, wrapped: &[u8]) -> Option<Vec<u8>> {
    let PrivateKey::Rsa(key) = key else {
        return None;
    };
    if alg.oid == RSA_ENCRYPTION {
        return key.decrypt(rsa::Pkcs1v15Encrypt, wrapped).ok();
    }
    if alg.oid == RSAES_OAEP {
        // Defaults: SHA-1 for both the hash and MGF1 (RFC 4055).
        let (h, mgf) = match &alg.parameters {
            None => (Hash::Sha1, Hash::Sha1),
            Some(p) => {
                let der = p.to_der().ok()?;
                let params = rsa::pkcs1::RsaOaepParams::from_der(&der).ok()?;
                let h = hash_of(&params.hash.oid)?;
                let mgf = params
                    .mask_gen
                    .parameters
                    .as_ref()
                    .and_then(|a| hash_of(&a.oid))
                    .unwrap_or(Hash::Sha1);
                if params
                    .p_source
                    .parameters
                    .as_ref()
                    .is_some_and(|l| !l.value().is_empty())
                {
                    return None;
                }
                (h, mgf)
            }
        };
        let oaep = match (h, mgf) {
            (Hash::Sha1, Hash::Sha1) => rsa::Oaep::new::<sha1::Sha1>(),
            (Hash::Sha256, Hash::Sha256) => rsa::Oaep::new::<sha2::Sha256>(),
            (Hash::Sha256, Hash::Sha1) => {
                rsa::Oaep::new_with_mgf_hash::<sha2::Sha256, sha1::Sha1>()
            }
            (Hash::Sha384, Hash::Sha384) => rsa::Oaep::new::<sha2::Sha384>(),
            (Hash::Sha512, Hash::Sha512) => rsa::Oaep::new::<sha2::Sha512>(),
            _ => return None,
        };
        return key.decrypt(oaep, wrapped).ok();
    }
    None
}

/// The content, decrypted with `cek` as `alg` says (AES-CBC, 3DES-CBC). `Err(true)`: an
/// algorithm Telex doesn't read; `Err(false)`: it doesn't decrypt.
fn decrypt_content(
    alg: &AlgorithmIdentifierOwned,
    cek: &[u8],
    data: &[u8],
) -> Result<Vec<u8>, bool> {
    use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
    let iv: OctetString = alg
        .parameters
        .as_ref()
        .and_then(|p| p.decode_as().ok())
        .ok_or(false)?;
    let iv = iv.as_bytes();
    let run = |r: Result<Vec<u8>, _>| r.map_err(|_| false);
    if alg.oid == AES128_CBC {
        let d = cbc::Decryptor::<aes::Aes128>::new_from_slices(cek, iv).map_err(|_| false)?;
        return run(d.decrypt_padded_vec_mut::<Pkcs7>(data));
    }
    if alg.oid == AES192_CBC {
        let d = cbc::Decryptor::<aes::Aes192>::new_from_slices(cek, iv).map_err(|_| false)?;
        return run(d.decrypt_padded_vec_mut::<Pkcs7>(data));
    }
    if alg.oid == AES256_CBC {
        let d = cbc::Decryptor::<aes::Aes256>::new_from_slices(cek, iv).map_err(|_| false)?;
        return run(d.decrypt_padded_vec_mut::<Pkcs7>(data));
    }
    if alg.oid == DES_EDE3_CBC {
        let d = cbc::Decryptor::<des::TdesEde3>::new_from_slices(cek, iv).map_err(|_| false)?;
        return run(d.decrypt_padded_vec_mut::<Pkcs7>(data));
    }
    Err(true)
}

impl MailStore {
    /// An own private key, opened: from this session's memory, or (a key without a password)
    /// opened now. `None`: it needs its password.
    fn open_key(&self, fingerprint: &str, sealed: &[u8], protected: bool) -> Option<PrivateKey> {
        if let Some(pkcs8) = self.smime.borrow().keys.get(fingerprint) {
            return private_key(pkcs8);
        }
        if protected {
            return None;
        }
        let pkcs8 = unseal(sealed, "")?;
        let key = private_key(&pkcs8);
        self.smime
            .borrow_mut()
            .keys
            .insert(fingerprint.to_owned(), pkcs8);
        key
    }

    /// Own certificates with their private keys, newest first; with `email`, only that
    /// address's that are valid now.
    fn own_certs(&self, email: Option<&str>) -> Vec<OwnRow> {
        let email = email.map(|e| e.trim().to_lowercase());
        let now = now_secs();
        self.db
            .prepare_cached(
                "SELECT fingerprint, cert, chain, secret, protected FROM smime_certs
                 WHERE secret IS NOT NULL
                   AND (?1 IS NULL OR (' ' || emails || ' ') LIKE ('% ' || ?1 || ' %'))
                 ORDER BY not_before DESC",
            )
            .and_then(|mut s| {
                s.query_map([&email], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Vec<u8>>(1)?,
                        r.get::<_, Option<Vec<u8>>>(2)?.unwrap_or_default(),
                        r.get::<_, Vec<u8>>(3)?,
                        r.get::<_, bool>(4)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()
            })
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(fingerprint, cert, chain, sealed, protected)| {
                let facts = facts(&cert)?;
                let valid = facts.not_before <= now && now <= facts.not_after;
                (email.is_none() || valid).then(|| OwnRow {
                    fingerprint,
                    cert,
                    chain: split_chain(&chain),
                    sealed,
                    protected,
                    facts,
                })
            })
            .collect()
    }

    /// Decrypt an EnvelopedData with the user's certificates.
    fn smime_decrypt(&self, cms_der: &[u8]) -> Result<Vec<u8>, Stop> {
        let failed = || Stop::Problem(Problem::Failed);
        let (ci, _) = content_info(cms_der).ok_or_else(failed)?;
        if ci.content_type == ID_AUTH_ENVELOPED_DATA {
            return Err(Stop::Problem(Problem::Unsupported));
        }
        let ed: EnvelopedData = ci.content.decode_as().map_err(|_| failed())?;
        let content = &ed.encrypted_content;
        let data = content.encrypted_content.as_ref().ok_or_else(failed)?;
        if data.as_bytes().len() > MAX_PLAIN + 64 {
            return Err(Stop::Problem(Problem::TooLarge));
        }
        let own = self.own_certs(None);
        let mut recipients = Vec::new();
        let mut matched = Vec::new();
        for ri in ed.recip_infos.0.iter().take(MAX_RECIPIENTS) {
            let RecipientInfo::Ktri(k) = ri else {
                continue;
            };
            recipients.push(describe(&k.rid));
            for o in &own {
                if recipient_named(&k.rid, &o.facts) {
                    matched.push((k, o));
                }
            }
        }
        if matched.is_empty() {
            return Err(Stop::NotForYou(recipients));
        }
        let mut locked = Vec::new();
        let mut unsupported = false;
        for (k, o) in &matched {
            let Some(key) = self.open_key(&o.fingerprint, &o.sealed, o.protected) else {
                locked.push(KeyRef {
                    fingerprint: o.fingerprint.clone(),
                    name: o.facts.name.clone(),
                });
                continue;
            };
            let Some(cek) = unwrap_key(&key, &k.key_enc_alg, k.enc_key.as_bytes()) else {
                continue;
            };
            let cek = Zeroizing::new(cek);
            match decrypt_content(&content.content_enc_alg, &cek, data.as_bytes()) {
                Ok(plain) => return Ok(plain),
                Err(u) => unsupported |= u,
            }
        }
        if !locked.is_empty() {
            return Err(Stop::Locked(locked));
        }
        Err(Stop::Problem(if unsupported {
            Problem::Unsupported
        } else {
            Problem::Failed
        }))
    }

    /// S/MIME of a stored message: what to show and the line above it; `None` when it has
    /// none. Called by `pgp_open`, which keeps what it gives in memory.
    pub(crate) fn smime_open(&self, message: &Message, raw: &[u8]) -> Option<Opened> {
        let found = find(raw)?;
        let mut view = View {
            smime: true,
            ..View::default()
        };
        // A signed entity (a whole MIME part) under the outer message's headers.
        let entity = |data: Vec<u8>| -> Vec<u8> {
            let inner = if looks_like_entity(&data) {
                data
            } else {
                let mut wrapped = b"Content-Type: text/plain; charset=utf-8\r\n\r\n".to_vec();
                wrapped.extend_from_slice(&data);
                wrapped
            };
            with_outer_headers(raw, &inner)
        };
        match found {
            Found::Signed {
                signed,
                signature,
                partial,
            } => {
                view.partial = partial;
                view.signature = Some(match signed_data(&signature) {
                    Some(sd) => self.smime_judge(&sd, &signed, message),
                    None => bad(),
                });
                Some(Opened {
                    view,
                    raw: None,
                    body: None,
                })
            }
            Found::Opaque { cms, partial } => {
                view.partial = partial;
                let sd = signed_data(&cms);
                let content = sd.as_ref().and_then(econtent);
                match (sd, content) {
                    (Some(sd), Some(content)) if content.len() <= MAX_PLAIN => {
                        view.signature = Some(self.smime_judge(&sd, &content, message));
                        Some(Opened {
                            view,
                            raw: Some(std::sync::Arc::new(entity(content))),
                            body: None,
                        })
                    }
                    _ => {
                        view.signature = Some(bad());
                        Some(Opened {
                            view,
                            raw: None,
                            body: None,
                        })
                    }
                }
            }
            Found::Enveloped { cms, partial } => {
                view.encrypted = true;
                view.partial = partial;
                match self.smime_decrypt(&cms) {
                    Ok(plain) => {
                        view.decrypted = true;
                        let mut readable = entity(plain);
                        // Signed, then encrypted: the signature is inside.
                        match find(&readable) {
                            Some(Found::Signed {
                                signed, signature, ..
                            }) => {
                                view.signature = Some(match signed_data(&signature) {
                                    Some(sd) => self.smime_judge(&sd, &signed, message),
                                    None => bad(),
                                });
                            }
                            Some(Found::Opaque { cms, .. }) => {
                                let sd = signed_data(&cms);
                                match sd.as_ref().and_then(econtent) {
                                    Some(content) => {
                                        let sd = sd.expect("parsed above");
                                        view.signature =
                                            Some(self.smime_judge(&sd, &content, message));
                                        readable = entity(content);
                                    }
                                    None => view.signature = Some(bad()),
                                }
                            }
                            _ => {}
                        }
                        Some(Opened {
                            view,
                            raw: Some(std::sync::Arc::new(readable)),
                            body: None,
                        })
                    }
                    Err(stop) => {
                        stopped(&mut view, stop);
                        Some(Opened {
                            view,
                            raw: None,
                            body: Some(String::new()),
                        })
                    }
                }
            }
        }
    }
}

// ---- Certificates: Settings › Keys ----

impl MailStore {
    /// Every certificate Telex knows: own ones first, then by name.
    pub fn smime_certs(&self) -> Result<Vec<CertInfo>, StoreError> {
        let mut stmt = self.db.prepare(
            "SELECT cert, chain, secret IS NOT NULL, protected, source, verified, added
             FROM smime_certs ORDER BY secret IS NULL, lower(name), fingerprint",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, Vec<u8>>(0)?,
                    r.get::<_, Option<Vec<u8>>>(1)?.unwrap_or_default(),
                    r.get::<_, bool>(2)?,
                    r.get::<_, bool>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, bool>(5)?,
                    r.get::<_, NaiveDateTime>(6)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let now = now_secs();
        Ok(rows
            .into_iter()
            .filter_map(
                |(cert, chain, secret, protected, source, verified, added)| {
                    let f = facts(&cert)?;
                    let trusted =
                        trust::chain(&cert, &split_chain(&chain), now) == trust::Trust::Trusted;
                    Some(CertInfo {
                        fingerprint: f.fingerprint,
                        name: f.name,
                        emails: f.emails,
                        issuer: f.issuer,
                        serial: f.serial,
                        not_before: local(f.not_before),
                        not_after: local(f.not_after),
                        algorithm: f.algorithm,
                        secret,
                        protected,
                        source,
                        verified,
                        trusted,
                        expired: now > f.not_after,
                        can_sign: f.can_sign,
                        can_encrypt: f.can_encrypt,
                        added,
                    })
                },
            )
            .collect())
    }

    /// Import a .p12 or .pfx file opened with `password`: the certificate with its private
    /// key (kept encrypted under the same password, and open for this session) and its
    /// issuers. `Ok(None)`: the password doesn't open it.
    pub fn import_p12(
        &mut self,
        data: &[u8],
        password: &str,
        now: NaiveDateTime,
    ) -> Result<Option<ImportReport>, StoreError> {
        let file = match p12::read(data, password) {
            Ok(f) => f,
            Err(p12::P12Error::WrongPassword) => return Ok(None),
            Err(p12::P12Error::NotP12) => {
                return Err(StoreError::Invalid(tr!(
                    "That isn't a .p12 or .pfx file Telex can read.",
                    "To nie jest plik .p12 ani .pfx, który Telex potrafi odczytać."
                )))
            }
            Err(p12::P12Error::Unsupported) => {
                return Err(StoreError::Invalid(tr!(
                    "That file is protected in a way Telex doesn't read. Export it again with AES encryption.",
                    "Ten plik jest zabezpieczony w sposób, którego Telex nie odczytuje. Wyeksportuj go ponownie z szyfrowaniem AES."
                )))
            }
            Err(p12::P12Error::NoKey) => {
                return Err(StoreError::Invalid(tr!(
                    "That file has no private key with its certificate.",
                    "W tym pliku nie ma klucza prywatnego z certyfikatem."
                )))
            }
        };
        if private_key(&file.key).is_none() {
            return Err(StoreError::Invalid(tr!(
                "Telex can't use that kind of key (only RSA, and ECDSA P-256 or P-384 for signing).",
                "Telex nie potrafi użyć takiego klucza (tylko RSA oraz ECDSA P-256 lub P-384 do podpisywania)."
            )));
        }
        let sealed = seal(&file.key, password)?;
        let (reference, saved) = save_cert(
            &self.db,
            &file.cert,
            &file.chain,
            Some((&sealed, !password.is_empty())),
            Source::Own,
            now,
        )?;
        self.smime
            .borrow_mut()
            .keys
            .insert(reference.fingerprint.clone(), file.key);
        self.pgp.borrow_mut().forget_opened();
        let mut report = ImportReport::default();
        match saved {
            Saved::Added => report.added.push(reference),
            Saved::Updated => report.updated.push(reference),
            Saved::Unchanged => report.unchanged.push(reference),
        }
        Ok(Some(report))
    }

    /// Import certificates from a file the user picked (PEM, DER, .p7b): other people's, to
    /// encrypt to, confirmed by bringing them.
    pub fn import_certificates(
        &mut self,
        data: &[u8],
        now: NaiveDateTime,
    ) -> Result<ImportReport, StoreError> {
        let certs = read_certificates(data);
        if certs.is_empty() {
            return Err(StoreError::Invalid(tr!(
                "There is no certificate Telex can use in it.",
                "Nie ma w nim certyfikatu, którego Telex może użyć."
            )));
        }
        let mut report = ImportReport::default();
        let tx = self.db.transaction()?;
        for (i, cert) in certs.iter().enumerate() {
            // Certificates of authorities (no address) only help chains: kept with the rest.
            let Some(f) = facts(cert) else { continue };
            if f.emails.is_empty() {
                report.skipped += 1;
                continue;
            }
            let others: Vec<Vec<u8>> = certs
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, c)| c.clone())
                .collect();
            let (reference, saved) = save_cert(&tx, cert, &others, None, Source::File, now)?;
            match saved {
                Saved::Added => report.added.push(reference),
                Saved::Updated => report.updated.push(reference),
                Saved::Unchanged => report.unchanged.push(reference),
            }
        }
        tx.commit()?;
        self.pgp.borrow_mut().forget_opened();
        Ok(report)
    }

    /// Remove a certificate (and its private key) from the mail base.
    pub fn delete_cert(&mut self, fingerprint: &str) -> Result<(), StoreError> {
        self.db.execute(
            "DELETE FROM smime_certs WHERE fingerprint = ?1",
            [fingerprint],
        )?;
        self.smime.borrow_mut().keys.remove(fingerprint);
        self.pgp.borrow_mut().forget_opened();
        Ok(())
    }

    /// The user compared the certificate's fingerprint (or no longer vouches for it).
    pub fn set_cert_verified(
        &mut self,
        fingerprint: &str,
        verified: bool,
    ) -> Result<(), StoreError> {
        let n = self.db.execute(
            "UPDATE smime_certs SET verified = ?2 WHERE fingerprint = ?1",
            params![fingerprint, verified],
        )?;
        if n == 0 {
            return Err(no_such_cert());
        }
        self.pgp.borrow_mut().forget_opened();
        Ok(())
    }

    /// A certificate as PEM, to give to people (never its private key).
    pub fn export_cert(&self, fingerprint: &str) -> Result<String, StoreError> {
        let der: Vec<u8> = self
            .db
            .query_row(
                "SELECT cert FROM smime_certs WHERE fingerprint = ?1",
                [fingerprint],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(no_such_cert)?;
        der::pem::encode_string("CERTIFICATE", der::pem::LineEnding::CRLF, &der)
            .map_err(|_| unreadable())
    }

    /// Give an own certificate's password: true when it opens the key, which then stays open
    /// until Telex closes (or the certificate goes).
    pub fn smime_unlock(&mut self, fingerprint: &str, password: &str) -> Result<bool, StoreError> {
        let sealed: Vec<u8> = self
            .db
            .query_row(
                "SELECT secret FROM smime_certs WHERE fingerprint = ?1 AND secret IS NOT NULL",
                [fingerprint],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(no_such_cert)?;
        let Some(pkcs8) = unseal(&sealed, password) else {
            return Ok(false);
        };
        self.smime
            .borrow_mut()
            .keys
            .insert(fingerprint.to_owned(), pkcs8);
        self.pgp.borrow_mut().forget_opened();
        Ok(true)
    }

    /// Close an own certificate's key opened this session.
    pub fn smime_lock(&mut self, fingerprint: &str) {
        self.smime.borrow_mut().keys.remove(fingerprint);
        self.pgp.borrow_mut().forget_opened();
    }

    /// The own certificates whose key is open this session.
    pub fn smime_unlocked(&self) -> Vec<String> {
        self.smime.borrow().keys.keys().cloned().collect()
    }
}

// ---- Writing: signing and encrypting ----

/// An own certificate ready to sign.
pub struct OwnCert {
    pub fingerprint: String,
    pub cert: Vec<u8>,
    pub chain: Vec<Vec<u8>>,
    key: PrivateKey,
}

/// What a message needs to go out S/MIME-signed or encrypted (`openpgp::Protect::smime`).
#[derive(Default)]
pub struct Protect {
    pub sign: Option<OwnCert>,
    /// Certificates to encrypt to (the recipients' and the sender's own); empty: not
    /// encrypted.
    pub encrypt: Vec<Vec<u8>>,
}

impl MailStore {
    /// The sender's own certificate to sign with, its key open.
    pub fn own_cert_for(&self, email: &str) -> Result<OwnCert, WriteProblem> {
        let o = self
            .own_certs(Some(email))
            .into_iter()
            .find(|o| o.facts.can_sign)
            .ok_or_else(|| WriteProblem::NoOwnCert {
                email: email.trim().to_lowercase(),
            })?;
        let key = self
            .open_key(&o.fingerprint, &o.sealed, o.protected)
            .ok_or_else(|| WriteProblem::CertLocked { key: o.key_ref() })?;
        Ok(OwnCert {
            fingerprint: o.fingerprint,
            cert: o.cert,
            chain: o.chain,
            key,
        })
    }

    /// The sender's own certificate that mail can be encrypted to (their copy).
    pub(crate) fn own_cert_public(&self, email: &str) -> Option<(KeyRef, Vec<u8>)> {
        self.own_certs(Some(email))
            .into_iter()
            .find(|o| o.facts.can_encrypt)
            .map(|o| (o.key_ref(), o.cert))
    }

    /// The sender's own certificate (to sign or encrypt with), and whether its key is open.
    pub(crate) fn own_cert_state(&self, email: &str) -> Option<(KeyRef, bool)> {
        let o = self
            .own_certs(Some(email))
            .into_iter()
            .find(|o| o.facts.can_sign || o.facts.can_encrypt)?;
        let open = !o.protected || self.smime.borrow().keys.contains_key(&o.fingerprint);
        Some((o.key_ref(), open))
    }

    /// The certificate to encrypt to for one address: valid now, able to encrypt, and either
    /// confirmed by the user (imported, checked) or chained to an authority the system trusts;
    /// the newest of those.
    pub fn cert_for(&self, email: &str) -> Option<Vec<u8>> {
        let email = email.trim().to_lowercase();
        let now = now_secs();
        let rows: Vec<(Vec<u8>, Vec<u8>, bool)> = self
            .db
            .prepare_cached(
                "SELECT cert, chain, verified OR secret IS NOT NULL FROM smime_certs
                 WHERE (' ' || emails || ' ') LIKE ('% ' || ?1 || ' %') ORDER BY not_before DESC",
            )
            .and_then(|mut s| {
                s.query_map([&email], |r| {
                    Ok((
                        r.get(0)?,
                        r.get::<_, Option<Vec<u8>>>(1)?.unwrap_or_default(),
                        r.get(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()
            })
            .ok()?;
        rows.into_iter()
            .filter(|(cert, chain, confirmed)| {
                facts(cert).is_some_and(|f| {
                    f.can_encrypt
                        && f.not_before <= now
                        && now <= f.not_after
                        && (*confirmed
                            || trust::chain(cert, &split_chain(chain), now)
                                == trust::Trust::Trusted)
                })
            })
            .map(|(cert, _, _)| cert)
            .next()
    }

    /// The certificates to encrypt to for each address; addresses without one come back in
    /// the error.
    pub fn certs_for(&self, emails: &[String]) -> Result<Vec<Vec<u8>>, WriteProblem> {
        let mut certs = Vec::new();
        let mut missing = Vec::new();
        for email in emails {
            match self.cert_for(email) {
                Some(c) => certs.push(c),
                None => missing.push(email.trim().to_lowercase()),
            }
        }
        if missing.is_empty() {
            Ok(certs)
        } else {
            Err(WriteProblem::NoCertFor { emails: missing })
        }
    }
}

/// A CMS signature (SignedData, DER) over `data`, detached, SHA-256, with the signing time,
/// the algorithms Telex reads (S/MIME capabilities) and the certificate and its issuers.
pub fn sign_detached(own: &OwnCert, data: &[u8]) -> Result<Vec<u8>, WriteProblem> {
    use cms::builder::{create_signing_time_attribute, SignedDataBuilder, SignerInfoBuilder};
    use cms::signed_data::EncapsulatedContentInfo;
    let failed = |_| WriteProblem::Failed;
    let cert = Certificate::from_der(&own.cert).map_err(failed)?;
    let content_digest = sha2::Sha256::digest(data).to_vec();
    let eci = EncapsulatedContentInfo {
        econtent_type: ID_DATA,
        econtent: None,
    };
    let digest_alg = AlgorithmIdentifierOwned {
        oid: SHA256_OID,
        parameters: None,
    };
    let sid = SignerIdentifier::IssuerAndSerialNumber(IssuerAndSerialNumber {
        issuer: cert.tbs_certificate.issuer.clone(),
        serial_number: cert.tbs_certificate.serial_number.clone(),
    });
    let mut builder = SignedDataBuilder::new(&eci);
    builder
        .add_digest_algorithm(digest_alg.clone())
        .map_err(|_| WriteProblem::Failed)?;
    builder
        .add_certificate(CertificateChoices::Certificate(cert))
        .map_err(|_| WriteProblem::Failed)?;
    for issuer in &own.chain {
        let c = Certificate::from_der(issuer).map_err(failed)?;
        builder
            .add_certificate(CertificateChoices::Certificate(c))
            .map_err(|_| WriteProblem::Failed)?;
    }
    let attrs = [
        create_signing_time_attribute().map_err(|_| WriteProblem::Failed)?,
        capabilities().ok_or(WriteProblem::Failed)?,
    ];
    macro_rules! sign_with {
        ($signer:expr, $sig:ty) => {{
            let signer = $signer;
            let mut info =
                SignerInfoBuilder::new(&signer, sid, digest_alg, &eci, Some(&content_digest))
                    .map_err(|_| WriteProblem::Failed)?;
            for a in attrs {
                info.add_signed_attribute(a)
                    .map_err(|_| WriteProblem::Failed)?;
            }
            builder
                .add_signer_info::<_, $sig>(info)
                .map_err(|_| WriteProblem::Failed)?;
        }};
    }
    match &own.key {
        PrivateKey::Rsa(k) => sign_with!(
            rsa::pkcs1v15::SigningKey::<sha2::Sha256>::new((**k).clone()),
            rsa::pkcs1v15::Signature
        ),
        PrivateKey::P256(k) => {
            sign_with!(p256::ecdsa::SigningKey::from(k), p256::ecdsa::DerSignature)
        }
        PrivateKey::P384(k) => {
            sign_with!(p384::ecdsa::SigningKey::from(k), p384::ecdsa::DerSignature)
        }
    }
    builder
        .build()
        .map_err(|_| WriteProblem::Failed)?
        .to_der()
        .map_err(failed)
}

/// The S/MIME capabilities attribute (RFC 8551 §2.5.2): AES-256, AES-192, AES-128, so the
/// recipient's program answers in AES.
fn capabilities() -> Option<x509_cert::attr::Attribute> {
    let list: Vec<AlgorithmIdentifierOwned> = [AES256_CBC, AES192_CBC, AES128_CBC]
        .into_iter()
        .map(|oid| AlgorithmIdentifierOwned {
            oid,
            parameters: None,
        })
        .collect();
    let value = der::Any::from_der(&list.to_der().ok()?).ok()?;
    let mut values = der::asn1::SetOfVec::new();
    values.insert(value).ok()?;
    Some(x509_cert::attr::Attribute {
        oid: ID_SMIME_CAPABILITIES,
        values,
    })
}

/// `data` encrypted (EnvelopedData, DER, AES-256-CBC) to each certificate's RSA key.
pub fn encrypt(data: &[u8], to: &[Vec<u8>]) -> Result<Vec<u8>, WriteProblem> {
    use cms::builder::{
        ContentEncryptionAlgorithm, EnvelopedDataBuilder, KeyEncryptionInfo,
        KeyTransRecipientInfoBuilder,
    };
    let failed = |_| WriteProblem::Failed;
    let mut rngs: Vec<rand::rngs::ThreadRng> = to.iter().map(|_| rand::thread_rng()).collect();
    let mut builder =
        EnvelopedDataBuilder::new(None, data, ContentEncryptionAlgorithm::Aes256Cbc, None)
            .map_err(|_| WriteProblem::Failed)?;
    for (der, rng) in to.iter().zip(rngs.iter_mut()) {
        let cert = Certificate::from_der(der).map_err(failed)?;
        let key = rsa_public(&cert.tbs_certificate.subject_public_key_info)
            .ok_or(WriteProblem::Failed)?;
        let rid = RecipientIdentifier::IssuerAndSerialNumber(IssuerAndSerialNumber {
            issuer: cert.tbs_certificate.issuer.clone(),
            serial_number: cert.tbs_certificate.serial_number.clone(),
        });
        let info = KeyTransRecipientInfoBuilder::new(rid, KeyEncryptionInfo::Rsa(key), rng)
            .map_err(|_| WriteProblem::Failed)?;
        builder
            .add_recipient_info(info)
            .map_err(|_| WriteProblem::Failed)?;
    }
    let enveloped = builder
        .build_with_rng(&mut rand::thread_rng())
        .map_err(|_| WriteProblem::Failed)?;
    ContentInfo {
        content_type: ID_ENVELOPED_DATA,
        content: der::Any::encode_from(&enveloped).map_err(failed)?,
    }
    .to_der()
    .map_err(failed)
}

/// Base64 in lines of 76, as MIME wants it.
fn base64_lines(data: &[u8]) -> String {
    let b64 = base64::engine::general_purpose::STANDARD.encode(data);
    b64.as_bytes()
        .chunks(76)
        .map(|c| std::str::from_utf8(c).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\r\n")
}

fn boundary() -> String {
    use rand::Rng;
    let n: u128 = rand::thread_rng().gen();
    format!("telex-smime-{n:032x}")
}

/// A composed message (smtp.rs) made S/MIME-signed (multipart/signed with
/// application/pkcs7-signature) and encrypted (application/pkcs7-mime, enveloped-data), signed
/// first when both, as Outlook and Thunderbird do. Unlike OpenPGP's, the subject stays outside:
/// S/MIME programs show the outer one.
pub fn protect(raw: &[u8], p: &Protect) -> Result<Vec<u8>, WriteProblem> {
    let (head, body) = split_head(raw);
    let mut outer: Vec<&[u8]> = Vec::new();
    let mut content: Vec<&[u8]> = Vec::new();
    for (name, field) in fields(head) {
        let lower = name.to_ascii_lowercase();
        if lower.starts_with("content-") {
            content.push(field);
        } else if lower != "mime-version" {
            outer.push(field);
        }
    }
    let mut entity = Vec::with_capacity(body.len() + 512);
    for field in &content {
        entity.extend_from_slice(field);
    }
    entity.extend_from_slice(b"\r\n");
    entity.extend_from_slice(body);
    let mut entity = canonical(&entity);
    if let Some(own) = &p.sign {
        let signature = sign_detached(own, &entity)?;
        let b = boundary();
        let mut signed = format!(
            "Content-Type: multipart/signed; protocol=\"application/pkcs7-signature\";\r\n \
             micalg=sha-256; boundary=\"{b}\"\r\n\r\n\
             This is a cryptographically signed message in MIME format.\r\n\r\n--{b}\r\n"
        )
        .into_bytes();
        signed.extend_from_slice(&entity);
        signed.extend_from_slice(
            format!(
                "\r\n--{b}\r\nContent-Type: application/pkcs7-signature; name=\"smime.p7s\"\r\n\
                 Content-Transfer-Encoding: base64\r\n\
                 Content-Disposition: attachment; filename=\"smime.p7s\"\r\n\
                 Content-Description: S/MIME Cryptographic Signature\r\n\r\n{}\r\n--{b}--\r\n",
                base64_lines(&signature)
            )
            .as_bytes(),
        );
        entity = signed;
    }
    if !p.encrypt.is_empty() {
        let enveloped = encrypt(&entity, &p.encrypt)?;
        entity = format!(
            "Content-Type: application/pkcs7-mime; smime-type=enveloped-data; name=\"smime.p7m\"\r\n\
             Content-Transfer-Encoding: base64\r\n\
             Content-Disposition: attachment; filename=\"smime.p7m\"\r\n\
             Content-Description: S/MIME Encrypted Message\r\n\r\n{}\r\n",
            base64_lines(&enveloped)
        )
        .into_bytes();
    }
    let mut out = Vec::with_capacity(raw.len() * 2);
    for field in &outer {
        out.extend_from_slice(field);
    }
    out.extend_from_slice(b"MIME-Version: 1.0\r\n");
    out.extend_from_slice(&entity);
    Ok(out)
}

#[cfg(test)]
mod tests;
