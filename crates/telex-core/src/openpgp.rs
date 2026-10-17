//! OpenPGP mail: signed and encrypted messages read (PGP/MIME, RFC 3156, and inline PGP), the
//! keys Telex knows, and Autocrypt (Level 1) keys collected from incoming mail.
//!
//! rPGP (the `pgp` crate, MIT or Apache-2.0, pure Rust, audited) does the cryptography; this
//! module decides what it means: which key a signature is from and whether that key is the
//! sender's, which own key can decrypt a message, which part of a key may encrypt.
//!
//! What is kept where (docs/encryption.md):
//! - Keys live in the mail base (`pgp_keys`), so backups carry them. A secret key is stored as
//!   it came, protected by its passphrase (OpenPGP's own S2K encryption); the passphrase is
//!   asked when needed, kept in memory until Telex closes, and in the system's password store
//!   only when the user ticks Remember (src-tauri's secrets.rs).
//! - Encrypted mail is stored as it arrived, encrypted. Its text is not indexed (search finds
//!   it by its sender and subject only). Opening it decrypts it in memory; the decrypted
//!   message stays in memory while it is one of the last [`KEEP_OPENED`] opened, and reaches
//!   the disk only when the user opens or saves one of its attachments.
//! - Errors never carry key material or rPGP's own messages: each failure is one of the
//!   [`Problem`] codes, which the window words.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::io::{BufReader, Cursor, Read};
use std::sync::Arc;

use base64::Engine;
use chrono::{DateTime, Local, NaiveDateTime};
use mail_parser::{MessageParser, MessagePart, MimeHeaders, PartType};
use pgp::composed::{
    CleartextSignedMessage, Deserializable, DetachedSignature, Esk, Message as PgpMessage,
    PublicOrSecret, SignedPublicKey, SignedSecretKey,
};
use pgp::packet::{PublicKey, PublicSubkey, Signature, SignatureType};
use pgp::ser::Serialize as _;
use pgp::types::{KeyDetails, Password, Tag};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::model::{Message, MessageId};
use crate::store::{MailStore, StoreError};
use crate::tr;

/// The largest key file or key block read (a key with many signatures is a few hundred KB).
pub const MAX_KEY: usize = 1024 * 1024;
/// The largest Autocrypt key taken from a header (Autocrypt keys are minimal, about 1 KB).
pub const MAX_AUTOCRYPT: usize = 64 * 1024;
/// The most a decrypted message may grow to: a compressed message can't make more of itself
/// than this (a "compression bomb"), and a bigger one isn't decrypted.
pub const MAX_PLAIN: usize = if cfg!(test) { 1 << 20 } else { 64 << 20 };
/// How many decrypted messages stay in memory.
pub const KEEP_OPENED: usize = 8;
/// How many signatures of one message are checked.
const MAX_SIGNATURES: usize = 8;
/// How many keys a message may carry for the user to import.
const MAX_OFFERED: usize = 4;

// ---- What the window is told ----

/// A key, as lists and messages name it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyRef {
    pub fingerprint: String,
    /// Its first user id ("Anna Kowalska <anna@studio.pl>").
    pub name: String,
}

/// What a signature says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SigStatus {
    /// Valid, from a key the user has confirmed (their own, imported from a file, or checked),
    /// whose user ids include the From address.
    Good,
    /// Valid, from a key of the From address that Telex collected (Autocrypt, a key attached
    /// to mail) and the user hasn't confirmed.
    Unconfirmed,
    /// Valid, but the key belongs to someone else: none of its addresses is the From address.
    OtherSender,
    /// Signed with a key Telex doesn't have.
    NoKey,
    /// The signature doesn't match: the message was changed, or the signature is broken.
    Bad,
    /// The key was revoked by its owner.
    Revoked,
    /// The key had expired when the message was signed.
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SigView {
    pub status: SigStatus,
    /// The signing key (the primary key's fingerprint), when Telex has it.
    pub key: Option<KeyRef>,
    /// The key id the signature names, for "key 1234 5678 9ABC DEF0 isn't here".
    pub key_id: String,
    /// When it was signed (local time).
    pub created: Option<NaiveDateTime>,
}

/// Why an encrypted message isn't shown decrypted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Problem {
    /// One of the user's keys can decrypt it, once its passphrase is given (`locked`).
    Locked,
    /// It isn't encrypted to any of the user's keys (`recipients` says to which).
    NotForYou,
    /// It is damaged or was changed on the way (the integrity check failed), or its format
    /// can't be read.
    Failed,
    /// Decrypted, it would be larger than [`MAX_PLAIN`].
    TooLarge,
    /// An old or rare format Telex doesn't read (no integrity protection, BZip2).
    Unsupported,
}

/// OpenPGP of an opened message, for the line above it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct View {
    pub encrypted: bool,
    /// Shown decrypted: the text and attachments come from the decrypted message, in memory.
    pub decrypted: bool,
    pub problem: Option<Problem>,
    /// Own keys that can decrypt it once unlocked.
    pub locked: Vec<KeyRef>,
    /// The key ids it is encrypted to, when none is the user's.
    pub recipients: Vec<String>,
    pub signature: Option<SigView>,
    /// Only part of the message is signed or encrypted (a list's footer outside it, text
    /// around an inline block): what is outside isn't covered.
    pub partial: bool,
    /// Keys the message carries (its Autocrypt header, an attached key) that Telex doesn't
    /// have yet: the window offers to import them.
    pub offered: Vec<KeyRef>,
}

/// A message after OpenPGP: what to show instead of the stored one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opened {
    pub view: View,
    /// The readable message (decrypted content under the outer headers), in memory only;
    /// `None`: the stored message is the readable one.
    pub raw: Option<Arc<Vec<u8>>>,
    /// The text to show when it isn't the stored body: inline PGP's text.
    pub body: Option<String>,
}

/// A key in Settings › Keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyInfo {
    pub fingerprint: String,
    pub user_ids: Vec<String>,
    pub emails: Vec<String>,
    pub created: NaiveDateTime,
    pub expires: Option<NaiveDateTime>,
    /// "Ed25519", "RSA 3072"
    pub algorithm: String,
    /// The user's own: Telex has its secret part.
    pub secret: bool,
    /// Its secret part has a passphrase.
    pub protected: bool,
    /// own, file, autocrypt, message
    pub source: String,
    pub verified: bool,
    pub revoked: bool,
    pub can_encrypt: bool,
    pub added: NaiveDateTime,
}

/// What an import did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ImportReport {
    pub added: Vec<KeyRef>,
    pub updated: Vec<KeyRef>,
    pub unchanged: Vec<KeyRef>,
    /// Blocks that weren't keys Telex can use.
    pub skipped: usize,
}

/// Where a key came from, as `pgp_keys.source` says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Made in Telex.
    Own,
    /// A file the user picked: confirmed by doing so.
    File,
    /// An Autocrypt header of incoming mail.
    Autocrypt,
    /// A key attached to a message, imported by the user.
    Message,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Own => "own",
            Self::File => "file",
            Self::Autocrypt => "autocrypt",
            Self::Message => "message",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "own" => Self::Own,
            "file" => Self::File,
            "autocrypt" => Self::Autocrypt,
            "message" => Self::Message,
            _ => return None,
        })
    }

    /// The user vouched for a key from here by bringing it in themselves.
    fn confirmed(self) -> bool {
        matches!(self, Self::Own | Self::File)
    }
}

// ---- The store's session: passphrases and opened mail, memory only ----

/// What OpenPGP keeps while Telex runs, never on disk.
#[derive(Debug, Default)]
pub struct Session {
    /// Passphrases given (or read from the password store) this session, by fingerprint.
    passphrases: HashMap<String, Password>,
    /// The last opened messages: (id, a hash of the stored message) → what was found.
    opened: Vec<((MessageId, u64), Arc<Opened>)>,
}

impl Session {
    /// Forget what was decrypted (keys changed: a message may read differently now).
    fn forget_opened(&mut self) {
        self.opened.clear();
    }
}

// ---- Keys: reading, describing ----

/// Upper-case hex.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

fn fingerprint_of(key: &impl KeyDetails) -> String {
    hex(key.fingerprint().as_bytes())
}

fn key_id_of(key: &impl KeyDetails) -> String {
    hex(key.legacy_key_id().as_ref())
}

fn local_time(secs: u32) -> NaiveDateTime {
    DateTime::from_timestamp(i64::from(secs), 0)
        .map(|utc| utc.with_timezone(&Local).naive_local())
        .unwrap_or_default()
}

/// The address in a user id: "Anna <anna@x.pl>" → "anna@x.pl", lower-case.
pub fn email_of_user_id(uid: &str) -> Option<String> {
    let inner = match (uid.rfind('<'), uid.rfind('>')) {
        (Some(a), Some(b)) if a < b => &uid[a + 1..b],
        _ => uid,
    };
    let inner = inner.trim();
    (inner.contains('@') && !inner.contains(char::is_whitespace)).then(|| inner.to_lowercase())
}

/// Every key in `data` (armored or binary, one or many, public or secret). Blocks that
/// aren't keys, or whose self-signatures don't hold, are counted in the second number.
pub fn read_keys(data: &[u8]) -> (Vec<PublicOrSecret>, usize) {
    if data.len() > MAX_KEY * 8 {
        return (Vec::new(), 1);
    }
    let mut keys = Vec::new();
    let mut skipped = 0;
    // Several armored blocks one after another (a file of exported keys): each is read alone,
    // so one bad block doesn't hide the rest.
    let text = std::str::from_utf8(data).ok();
    let blocks: Vec<&[u8]> = match text {
        Some(t) if t.contains("-----BEGIN PGP") => {
            armored_blocks(t).into_iter().map(str::as_bytes).collect()
        }
        _ => vec![data],
    };
    for block in blocks {
        if block.len() > MAX_KEY * 4 {
            skipped += 1;
            continue;
        }
        let Ok((iter, _)) = PublicOrSecret::from_reader_many(Cursor::new(block)) else {
            skipped += 1;
            continue;
        };
        for key in iter.take(1000) {
            match key {
                Ok(key) if usable(&key) => keys.push(key),
                _ => skipped += 1,
            }
        }
    }
    (keys, skipped)
}

/// Every "-----BEGIN PGP …-----" … "-----END PGP …-----" block of a text.
fn armored_blocks(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("-----BEGIN PGP") {
        let block = &rest[start..];
        let Some(end) = block.find("-----END PGP") else {
            break;
        };
        let line_end = block[end..].find('\n').map_or(block.len(), |i| end + i + 1);
        out.push(&block[..line_end]);
        rest = &block[line_end..];
    }
    out
}

/// A key worth keeping: its primary key signs its user ids (at least one).
fn usable(key: &PublicOrSecret) -> bool {
    let public = public_of(key);
    !valid_user_ids(&public).is_empty()
}

fn public_of(key: &PublicOrSecret) -> SignedPublicKey {
    match key {
        PublicOrSecret::Public(p) => p.clone(),
        PublicOrSecret::Secret(s) => s.to_public_key(),
    }
}

/// The user ids the key's own primary key certifies.
fn valid_user_ids(key: &SignedPublicKey) -> Vec<String> {
    key.details
        .users
        .iter()
        .filter(|u| {
            u.signatures.iter().any(|sig| {
                is_self_issued(sig, &key.primary_key)
                    && sig
                        .verify_certification(&key.primary_key, Tag::UserId, &u.id)
                        .is_ok()
            })
        })
        .filter_map(|u| u.id.as_str().map(str::to_owned))
        .collect()
}

/// The signature names the primary key as its issuer (or names no issuer at all).
fn is_self_issued(sig: &Signature, primary: &PublicKey) -> bool {
    let ids = sig.issuer_key_id();
    let fps = sig.issuer_fingerprint();
    (ids.is_empty() && fps.is_empty())
        || ids.iter().any(|id| **id == primary.legacy_key_id())
        || fps.iter().any(|fp| **fp == primary.fingerprint())
}

/// The newest valid self-signature on the primary key's user ids (or a direct-key one): it
/// carries the key's flags and expiry.
fn primary_self_signature<'a>(key: &'a SignedPublicKey) -> Option<&'a Signature> {
    let mut best: Option<&Signature> = None;
    let mut consider = |sig: &'a Signature| {
        if best.is_none_or(|b| sig.created() > b.created()) {
            best = Some(sig);
        }
    };
    for u in &key.details.users {
        for sig in &u.signatures {
            if is_self_issued(sig, &key.primary_key)
                && sig
                    .verify_certification(&key.primary_key, Tag::UserId, &u.id)
                    .is_ok()
            {
                consider(sig);
            }
        }
    }
    for sig in &key.details.direct_signatures {
        if sig.verify_key(&key.primary_key).is_ok() {
            consider(sig);
        }
    }
    best
}

/// When the key expires, if it does.
fn expires(key: &SignedPublicKey) -> Option<u32> {
    let sig = primary_self_signature(key)?;
    let after = sig.key_expiration_time()?.as_secs();
    (after > 0).then(|| key.primary_key.created_at().as_secs().saturating_add(after))
}

/// Its owner revoked it.
fn revoked(key: &SignedPublicKey) -> bool {
    key.details.revocation_signatures.iter().any(|sig| {
        sig.typ() == Some(SignatureType::KeyRevocation) && sig.verify_key(&key.primary_key).is_ok()
    })
}

fn now_secs() -> u32 {
    u32::try_from(chrono::Utc::now().timestamp()).unwrap_or(u32::MAX)
}

/// A subkey that its primary key binds, with the binding's flags; `None` when no binding
/// holds, it was revoked or it has expired.
fn bound_subkey<'a>(
    key: &'a SignedPublicKey,
    sub: &'a pgp::composed::SignedPublicSubKey,
    at: u32,
) -> Option<&'a Signature> {
    let revoked = sub.signatures.iter().any(|sig| {
        sig.typ() == Some(SignatureType::SubkeyRevocation)
            && sig
                .verify_subkey_binding(&key.primary_key, &sub.key)
                .is_ok()
    });
    if revoked {
        return None;
    }
    let binding = sub
        .signatures
        .iter()
        .filter(|sig| sig.typ() == Some(SignatureType::SubkeyBinding))
        .filter(|sig| {
            sig.verify_subkey_binding(&key.primary_key, &sub.key)
                .is_ok()
        })
        .max_by_key(|sig| sig.created())?;
    if let Some(after) = binding.key_expiration_time() {
        let after = after.as_secs();
        if after > 0 && sub.key.created_at().as_secs().saturating_add(after) < at {
            return None;
        }
    }
    Some(binding)
}

/// The part of a key to encrypt to: the newest valid subkey that may encrypt, else the
/// primary key when it may. `None` for a key that can't receive encrypted mail now.
pub(crate) fn encryption_key(key: &SignedPublicKey) -> Option<EncryptTo<'_>> {
    let now = now_secs();
    if revoked(key) || expires(key).is_some_and(|t| t < now) {
        return None;
    }
    let sub = key
        .public_subkeys
        .iter()
        .filter(|sub| sub.key.algorithm().can_encrypt())
        .filter(|sub| {
            bound_subkey(key, sub, now).is_some_and(|b| {
                let flags = b.key_flags();
                flags.encrypt_comms() || flags.encrypt_storage()
            })
        })
        .max_by_key(|sub| sub.key.created_at());
    if let Some(sub) = sub {
        return Some(EncryptTo::Sub(&sub.key));
    }
    let primary = primary_self_signature(key)?;
    let flags = primary.key_flags();
    (key.primary_key.algorithm().can_encrypt()
        && (flags.encrypt_comms() || flags.encrypt_storage()))
    .then_some(EncryptTo::Primary(&key.primary_key))
}

/// A key part to encrypt to.
pub(crate) enum EncryptTo<'a> {
    Primary(&'a PublicKey),
    Sub(&'a PublicSubkey),
}

fn algorithm_name(key: &PublicKey) -> String {
    use pgp::types::PublicParams;
    match key.public_params() {
        PublicParams::RSA(p) => {
            let bits = rsa_bits(p);
            format!("RSA {bits}")
        }
        PublicParams::EdDSALegacy(_) | PublicParams::Ed25519(_) => "Ed25519".to_owned(),
        PublicParams::Ed448(_) => "Ed448".to_owned(),
        PublicParams::ECDSA(_) => "ECDSA".to_owned(),
        PublicParams::DSA(_) => "DSA".to_owned(),
        PublicParams::ECDH(_) | PublicParams::X25519(_) => "X25519".to_owned(),
        _ => format!("{:?}", key.algorithm()),
    }
}

fn rsa_bits(p: &pgp::types::RsaPublicParams) -> usize {
    use rsa::traits::PublicKeyParts;
    p.key.n().bits()
}

/// What a key is, for the keys page and the store.
struct Facts {
    fingerprint: String,
    user_ids: Vec<String>,
    emails: Vec<String>,
    created: NaiveDateTime,
    expires: Option<NaiveDateTime>,
    /// Every key id and fingerprint of its parts.
    ids: Vec<String>,
}

fn facts(key: &SignedPublicKey) -> Facts {
    let user_ids = valid_user_ids(key);
    let mut emails: Vec<String> = user_ids
        .iter()
        .filter_map(|u| email_of_user_id(u))
        .collect();
    emails.dedup();
    let mut ids = vec![
        fingerprint_of(&key.primary_key),
        key_id_of(&key.primary_key),
    ];
    for sub in &key.public_subkeys {
        ids.push(fingerprint_of(&sub.key));
        ids.push(key_id_of(&sub.key));
    }
    Facts {
        fingerprint: fingerprint_of(&key.primary_key),
        user_ids,
        emails,
        created: local_time(key.primary_key.created_at().as_secs()),
        expires: expires(key).map(local_time),
        ids,
    }
}

// ---- Keys in the mail base ----

/// Save a key (public, or with its secret part), merging with what is stored: a secret part
/// already there stays; a newer certificate replaces the older one; the source and the
/// user's confirmation stay unless this one is confirmed by its source.
pub(crate) fn save_key(
    db: &Connection,
    key: &PublicOrSecret,
    source: Source,
    now: NaiveDateTime,
) -> Result<(KeyRef, Saved), StoreError> {
    let public = public_of(key);
    let f = facts(&public);
    let public_bytes = public.to_bytes().map_err(|_| unreadable_key())?;
    let secret_bytes = match key {
        PublicOrSecret::Secret(s) => Some(s.to_bytes().map_err(|_| unreadable_key())?),
        PublicOrSecret::Public(_) => None,
    };
    let stored: Option<(Vec<u8>, bool)> = db
        .query_row(
            "SELECT public, secret IS NOT NULL FROM pgp_keys WHERE fingerprint = ?1",
            [&f.fingerprint],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let name = f.user_ids.first().cloned().unwrap_or_default();
    let reference = KeyRef {
        fingerprint: f.fingerprint.clone(),
        name,
    };
    let saved = match &stored {
        None => Saved::Added,
        Some((old, had_secret)) => {
            let gains_secret = secret_bytes.is_some() && !had_secret;
            // A copy that came by itself (an Autocrypt header, a key in a message) never
            // replaces one Telex has: anyone can send a copy with its revocation left out.
            let automatic = matches!(source, Source::Autocrypt | Source::Message);
            if (*old == public_bytes || automatic) && !gains_secret {
                return Ok((reference, Saved::Unchanged));
            }
            Saved::Updated
        }
    };
    // A copy the user brought (a file, a pasted key, their own) replaces the stored one
    // whole: rPGP doesn't merge certificates, and the user chose this one.
    db.execute(
        "INSERT INTO pgp_keys (fingerprint, public, secret, user_ids, emails, created, expires,
             source, verified, added)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT (fingerprint) DO UPDATE SET public = excluded.public,
             secret = COALESCE(excluded.secret, pgp_keys.secret),
             user_ids = excluded.user_ids, emails = excluded.emails, expires = excluded.expires,
             source = CASE WHEN excluded.secret IS NOT NULL OR ?9 THEN excluded.source
                      ELSE pgp_keys.source END,
             verified = MAX(pgp_keys.verified, excluded.verified)",
        params![
            f.fingerprint,
            public_bytes,
            secret_bytes,
            f.user_ids.join("\n"),
            f.emails.join(" "),
            f.created,
            f.expires,
            source.as_str(),
            source.confirmed() || secret_bytes.is_some(),
            now,
        ],
    )?;
    db.execute(
        "DELETE FROM pgp_key_ids WHERE fingerprint = ?1",
        [&f.fingerprint],
    )?;
    for id in &f.ids {
        db.execute(
            "INSERT OR IGNORE INTO pgp_key_ids (id, fingerprint) VALUES (?1, ?2)",
            params![id, f.fingerprint],
        )?;
    }
    Ok((reference, saved))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Saved {
    Added,
    Updated,
    Unchanged,
}

fn unreadable_key() -> StoreError {
    StoreError::Invalid(tr!(
        "That key can't be read.",
        "Tego klucza nie da się odczytać."
    ))
}

/// The stored certificates whose parts include one of `ids` (key ids or fingerprints).
fn keys_by_ids(
    db: &Connection,
    ids: &[String],
) -> Result<Vec<(SignedPublicKey, bool, Source)>, StoreError> {
    let mut found = Vec::new();
    let mut seen = HashSet::new();
    let mut stmt = db.prepare_cached(
        "SELECT k.fingerprint, k.public, k.verified, k.source FROM pgp_key_ids i
         JOIN pgp_keys k ON k.fingerprint = i.fingerprint WHERE i.id = ?1",
    )?;
    for id in ids {
        let rows = stmt
            .query_map([id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Vec<u8>>(1)?,
                    r.get::<_, bool>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (fp, public, verified, source) in rows {
            if !seen.insert(fp) {
                continue;
            }
            if let Ok(key) = SignedPublicKey::from_bytes(Cursor::new(public)) {
                let source = Source::from_name(&source).unwrap_or(Source::Message);
                found.push((key, verified, source));
            }
        }
    }
    Ok(found)
}

/// The user's own keys (those with a secret part), with their fingerprints.
fn own_keys(db: &Connection) -> Result<Vec<(String, SignedSecretKey)>, StoreError> {
    let mut stmt = db.prepare_cached(
        "SELECT fingerprint, secret FROM pgp_keys WHERE secret IS NOT NULL ORDER BY created DESC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows
        .into_iter()
        .filter_map(|(fp, bytes)| {
            SignedSecretKey::from_bytes(Cursor::new(bytes))
                .ok()
                .map(|k| (fp, k))
        })
        .collect())
}

/// The secret key needs a passphrase: its primary key or a subkey is encrypted (a missing
/// primary, as GnuPG's offline keys have, counts as encrypted).
fn is_protected(key: &SignedSecretKey) -> bool {
    key.primary_key.secret_params().is_encrypted()
        || key
            .secret_subkeys
            .iter()
            .any(|s| s.secret_params().is_encrypted())
}

/// The passphrase opens the key: its primary key or one of its subkeys unlocks with it.
fn opens(key: &SignedSecretKey, pw: &Password) -> bool {
    matches!(key.primary_key.unlock(pw, |_, _| Ok(())), Ok(Ok(())))
        || key
            .secret_subkeys
            .iter()
            .any(|s| matches!(s.unlock(pw, |_, _| Ok(())), Ok(Ok(()))))
}

impl MailStore {
    /// Every key Telex knows: own keys first, then by name.
    pub fn pgp_keys(&self) -> Result<Vec<KeyInfo>, StoreError> {
        let mut stmt = self.db.prepare(
            "SELECT fingerprint, public, secret, user_ids, emails, created, expires, source,
                 verified, added
             FROM pgp_keys ORDER BY secret IS NULL, lower(user_ids), fingerprint",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Vec<u8>>(1)?,
                    r.get::<_, Option<Vec<u8>>>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, NaiveDateTime>(5)?,
                    r.get::<_, Option<NaiveDateTime>>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, bool>(8)?,
                    r.get::<_, NaiveDateTime>(9)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows
            .into_iter()
            .map(
                |(
                    fingerprint,
                    public,
                    secret,
                    uids,
                    emails,
                    created,
                    expires,
                    source,
                    verified,
                    added,
                )| {
                    let key = SignedPublicKey::from_bytes(Cursor::new(public)).ok();
                    let protected = secret
                        .as_deref()
                        .and_then(|s| SignedSecretKey::from_bytes(Cursor::new(s)).ok())
                        .is_some_and(|s| is_protected(&s));
                    KeyInfo {
                        fingerprint,
                        user_ids: uids.lines().map(str::to_owned).collect(),
                        emails: emails.split_whitespace().map(str::to_owned).collect(),
                        created,
                        expires,
                        algorithm: key
                            .as_ref()
                            .map(|k| algorithm_name(&k.primary_key))
                            .unwrap_or_default(),
                        secret: secret.is_some(),
                        protected,
                        source,
                        verified,
                        revoked: key.as_ref().is_some_and(revoked),
                        can_encrypt: key.as_ref().is_some_and(|k| encryption_key(k).is_some()),
                        added,
                    }
                },
            )
            .collect())
    }

    /// Import the keys in a file or block (armored or binary). The user brought them, so
    /// their keys count as confirmed (`source` file), unless they came from a message.
    pub fn import_keys(
        &mut self,
        data: &[u8],
        source: Source,
        now: NaiveDateTime,
    ) -> Result<ImportReport, StoreError> {
        let (keys, skipped) = read_keys(data);
        let mut report = ImportReport {
            skipped,
            ..ImportReport::default()
        };
        if keys.is_empty() {
            return Err(StoreError::Invalid(tr!(
                "There is no OpenPGP key Telex can use in it.",
                "Nie ma w nim klucza OpenPGP, którego Telex może użyć."
            )));
        }
        let tx = self.db.transaction()?;
        for key in &keys {
            let (reference, saved) = save_key(&tx, key, source, now)?;
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

    /// Remove a key (and its secret part) from the mail base.
    pub fn delete_key(&mut self, fingerprint: &str) -> Result<(), StoreError> {
        self.db
            .execute("DELETE FROM pgp_keys WHERE fingerprint = ?1", [fingerprint])?;
        let mut session = self.pgp.borrow_mut();
        session.passphrases.remove(fingerprint);
        session.forget_opened();
        Ok(())
    }

    /// The user compared the fingerprint (or no longer vouches for it).
    pub fn set_key_verified(
        &mut self,
        fingerprint: &str,
        verified: bool,
    ) -> Result<(), StoreError> {
        let n = self.db.execute(
            "UPDATE pgp_keys SET verified = ?2 WHERE fingerprint = ?1",
            params![fingerprint, verified],
        )?;
        if n == 0 {
            return Err(no_such_key());
        }
        self.pgp.borrow_mut().forget_opened();
        Ok(())
    }

    /// A key's certificate, armored, to send or publish.
    pub fn export_public_key(&self, fingerprint: &str) -> Result<String, StoreError> {
        let key = self.public_key(fingerprint)?;
        key.to_armored_string(None.into())
            .map_err(|_| unreadable_key())
    }

    /// An own key with its secret part, armored, protected by its passphrase as stored.
    pub fn export_secret_key(&self, fingerprint: &str) -> Result<String, StoreError> {
        let bytes: Option<Vec<u8>> = self
            .db
            .query_row(
                "SELECT secret FROM pgp_keys WHERE fingerprint = ?1",
                [fingerprint],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        let key = bytes
            .and_then(|b| SignedSecretKey::from_bytes(Cursor::new(b)).ok())
            .ok_or_else(no_such_key)?;
        key.to_armored_string(None.into())
            .map_err(|_| unreadable_key())
    }

    pub(crate) fn public_key(&self, fingerprint: &str) -> Result<SignedPublicKey, StoreError> {
        let bytes: Option<Vec<u8>> = self
            .db
            .query_row(
                "SELECT public FROM pgp_keys WHERE fingerprint = ?1",
                [fingerprint],
                |r| r.get(0),
            )
            .optional()?;
        bytes
            .and_then(|b| SignedPublicKey::from_bytes(Cursor::new(b)).ok())
            .ok_or_else(no_such_key)
    }

    /// Give an own key's passphrase: true when it opens the key; it is then kept in memory
    /// until Telex closes (or the key goes). A key without a passphrase opens with any.
    pub fn pgp_unlock(&mut self, fingerprint: &str, passphrase: &str) -> Result<bool, StoreError> {
        let key = own_keys(&self.db)?
            .into_iter()
            .find(|(fp, _)| fp == fingerprint)
            .map(|(_, k)| k)
            .ok_or_else(no_such_key)?;
        let pw = Password::from(passphrase);
        if !opens(&key, &pw) {
            return Ok(false);
        }
        let mut session = self.pgp.borrow_mut();
        session.passphrases.insert(fingerprint.to_owned(), pw);
        session.forget_opened();
        Ok(true)
    }

    /// Forget a passphrase given this session.
    pub fn pgp_lock(&mut self, fingerprint: &str) {
        let mut session = self.pgp.borrow_mut();
        session.passphrases.remove(fingerprint);
        session.forget_opened();
    }

    /// The own keys whose passphrase is known this session.
    pub fn pgp_unlocked(&self) -> Vec<String> {
        self.pgp.borrow().passphrases.keys().cloned().collect()
    }
}

/// The kind of key Telex makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyKind {
    /// Ed25519 to sign and X25519 to encrypt, in the version 4 format every OpenPGP program
    /// of the last ten years reads (GnuPG 2.1+, Thunderbird, OpenKeychain, Proton). Small and
    /// fast: the default.
    Ed25519,
    /// RSA 3072 for both, for old programs that read no elliptic curves.
    Rsa3072,
}

/// A new key for `name <email>`, protected by `passphrase` (OpenPGP's iterated and salted
/// SHA-256 with AES): a primary key that certifies and signs and a subkey that encrypts.
pub fn generate(
    name: &str,
    email: &str,
    passphrase: &str,
    kind: KeyKind,
) -> Result<SignedSecretKey, StoreError> {
    use pgp::composed::{EncryptionCaps, KeyType, SecretKeyParamsBuilder, SubkeyParamsBuilder};
    use pgp::crypto::{ecc_curve::ECCCurve, hash::HashAlgorithm, sym::SymmetricKeyAlgorithm};
    use pgp::types::CompressionAlgorithm;
    use smallvec::smallvec;

    let email = email.trim();
    if email_of_user_id(email).is_none() {
        return Err(StoreError::Invalid(tr!(
            "A key needs an email address.",
            "Klucz potrzebuje adresu e-mail."
        )));
    }
    if passphrase.is_empty() {
        return Err(StoreError::Invalid(tr!(
            "Choose a passphrase for the key.",
            "Wybierz hasło do klucza."
        )));
    }
    let name = name.trim().replace(['<', '>', '\n', '\r'], "");
    let uid = if name.is_empty() {
        format!("<{email}>")
    } else {
        format!("{name} <{email}>")
    };
    let (primary, sub) = match kind {
        KeyKind::Ed25519 => (
            KeyType::Ed25519Legacy,
            KeyType::ECDH(ECCCurve::Curve25519Legacy),
        ),
        KeyKind::Rsa3072 => (KeyType::Rsa(3072), KeyType::Rsa(3072)),
    };
    let failed = || {
        StoreError::Invalid(tr!(
            "Telex couldn't make the key.",
            "Telex nie zdołał utworzyć klucza."
        ))
    };
    let mut rng = rand::thread_rng();
    let subkey = SubkeyParamsBuilder::default()
        .key_type(sub)
        .can_encrypt(EncryptionCaps::All)
        .passphrase(Some(passphrase.to_owned()))
        .build()
        .map_err(|_| failed())?;
    let params = SecretKeyParamsBuilder::default()
        .key_type(primary)
        .can_certify(true)
        .can_sign(true)
        .primary_user_id(uid)
        .passphrase(Some(passphrase.to_owned()))
        .preferred_symmetric_algorithms(smallvec![
            SymmetricKeyAlgorithm::AES256,
            SymmetricKeyAlgorithm::AES128,
        ])
        .preferred_hash_algorithms(smallvec![HashAlgorithm::Sha512, HashAlgorithm::Sha256])
        .preferred_compression_algorithms(smallvec![
            CompressionAlgorithm::ZLIB,
            CompressionAlgorithm::ZIP,
        ])
        .subkeys(vec![subkey])
        .build()
        .map_err(|_| failed())?;
    params.generate(&mut rng).map_err(|_| failed())
}

impl MailStore {
    /// Make a key for an identity's address and keep it with its secret part; the passphrase
    /// is then known for this session.
    pub fn generate_key(
        &mut self,
        name: &str,
        email: &str,
        passphrase: &str,
        kind: KeyKind,
        now: NaiveDateTime,
    ) -> Result<KeyRef, StoreError> {
        let key = generate(name, email, passphrase, kind)?;
        let (reference, _) = save_key(&self.db, &PublicOrSecret::Secret(key), Source::Own, now)?;
        let mut session = self.pgp.borrow_mut();
        session
            .passphrases
            .insert(reference.fingerprint.clone(), Password::from(passphrase));
        session.forget_opened();
        Ok(reference)
    }
}

// ---- Writing: signing and encrypting ----

/// The part of an own key that signs: the primary key when its self-signature lets it (or
/// says nothing), else a signing subkey whose secret is here.
fn signing_part(key: &SignedSecretKey) -> Option<&dyn pgp::types::SigningKey> {
    let public = key.to_public_key();
    let primary_signs = primary_self_signature(&public).is_some_and(|s| {
        let flags = s.key_flags();
        flags.sign() || !(flags.certify() || flags.encrypt_comms() || flags.encrypt_storage())
    });
    if primary_signs {
        return Some(&key.primary_key);
    }
    let now = now_secs();
    key.secret_subkeys
        .iter()
        .filter(|s| {
            public
                .public_subkeys
                .iter()
                .find(|p| p.key.fingerprint() == s.public_key().fingerprint())
                .and_then(|p| bound_subkey(&public, p, now))
                .is_some_and(|b| b.key_flags().sign())
        })
        .max_by_key(|s| s.public_key().created_at())
        .map(|s| &s.key as &dyn pgp::types::SigningKey)
}

/// Why signing or encrypting didn't happen, for the window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum WriteProblem {
    /// No own key for this address.
    NoOwnKey {
        email: String,
    },
    /// The own key needs its passphrase.
    Locked {
        key: KeyRef,
    },
    /// No key (or none that can encrypt now) for these recipients.
    NoKeyFor {
        emails: Vec<String>,
    },
    Failed,
}

impl std::fmt::Display for WriteProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::NoOwnKey { email } => tr!(
                "There is no OpenPGP key for {email}. Make one in Settings › Keys.",
                "Nie ma klucza OpenPGP dla {email}. Utwórz go w Ustawieniach › Klucze."
            ),
            Self::Locked { key } => {
                let name = &key.name;
                tr!(
                    "The key {name} needs its passphrase.",
                    "Klucz {name} wymaga hasła."
                )
            }
            Self::NoKeyFor { emails } => {
                let list = emails.join(", ");
                tr!(
                    "Can't encrypt: there is no key for {list}. Import their key, or send without encryption.",
                    "Nie można zaszyfrować: brak klucza dla {list}. Zaimportuj ich klucz albo wyślij bez szyfrowania."
                )
            }
            Self::Failed => tr!(
                "Signing or encrypting failed.",
                "Podpisywanie lub szyfrowanie nie powiodło się."
            ),
        };
        f.write_str(&text)
    }
}

/// An own key ready to sign or to decrypt a copy for oneself.
pub struct OwnKey {
    pub fingerprint: String,
    pub key: SignedSecretKey,
    pub password: Password,
}

impl MailStore {
    /// The own key for an address, with its passphrase if it is known (or not needed).
    pub fn own_key_for(&self, email: &str) -> Result<OwnKey, WriteProblem> {
        let email = email.trim().to_lowercase();
        let rows: Vec<(String, Vec<u8>)> = self
            .db
            .prepare_cached(
                "SELECT fingerprint, secret FROM pgp_keys WHERE secret IS NOT NULL
                   AND (' ' || emails || ' ') LIKE ('% ' || ?1 || ' %') ORDER BY created DESC",
            )
            .and_then(|mut s| {
                s.query_map([&email], |r| Ok((r.get(0)?, r.get(1)?)))?
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(|_| WriteProblem::Failed)?;
        let now = now_secs();
        let (fingerprint, key) = rows
            .into_iter()
            .filter_map(|(fp, b)| {
                SignedSecretKey::from_bytes(Cursor::new(b))
                    .ok()
                    .map(|k| (fp, k))
            })
            .find(|(_, k)| {
                let p = k.to_public_key();
                !revoked(&p) && expires(&p).is_none_or(|t| t > now)
            })
            .ok_or_else(|| WriteProblem::NoOwnKey {
                email: email.clone(),
            })?;
        let password = match self.pgp.borrow().passphrases.get(&fingerprint) {
            Some(Password::Static(pw)) => Password::from(&pw[..]),
            Some(Password::Dynamic(f)) => Password::from(&f()[..]),
            None if !is_protected(&key) => Password::empty(),
            None => {
                return Err(WriteProblem::Locked {
                    key: KeyRef {
                        name: valid_user_ids(&key.to_public_key())
                            .into_iter()
                            .next()
                            .unwrap_or_default(),
                        fingerprint,
                    },
                })
            }
        };
        Ok(OwnKey {
            fingerprint,
            key,
            password,
        })
    }

    /// The keys to encrypt to for each address: a confirmed key first, else the newest one
    /// that can encrypt (Autocrypt's when it says so). Addresses without one come back in the
    /// error.
    pub fn keys_for(&self, emails: &[String]) -> Result<Vec<SignedPublicKey>, WriteProblem> {
        let mut keys = Vec::new();
        let mut missing = Vec::new();
        for email in emails {
            let email = email.trim().to_lowercase();
            match self.key_for(&email) {
                Some(k) => keys.push(k),
                None => missing.push(email),
            }
        }
        if missing.is_empty() {
            Ok(keys)
        } else {
            Err(WriteProblem::NoKeyFor { emails: missing })
        }
    }

    /// The key to encrypt to for one address, if any.
    pub fn key_for(&self, email: &str) -> Option<SignedPublicKey> {
        let email = email.trim().to_lowercase();
        let autocrypt: Option<String> = self
            .db
            .query_row(
                "SELECT fingerprint FROM autocrypt_peers WHERE email = ?1",
                [&email],
                |r| r.get(0),
            )
            .optional()
            .ok()
            .flatten()
            .flatten();
        let rows: Vec<(String, Vec<u8>, bool, bool)> = self
            .db
            .prepare_cached(
                "SELECT fingerprint, public, verified, secret IS NOT NULL FROM pgp_keys
                 WHERE (' ' || emails || ' ') LIKE ('% ' || ?1 || ' %')",
            )
            .and_then(|mut s| {
                s.query_map([&email], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
                })?
                .collect::<Result<Vec<_>, _>>()
            })
            .ok()?;
        rows.into_iter()
            .filter_map(|(fp, b, verified, own)| {
                let key = SignedPublicKey::from_bytes(Cursor::new(b)).ok()?;
                encryption_key(&key)?;
                let rank = (
                    own || verified,
                    autocrypt.as_deref() == Some(fp.as_str()),
                    key.primary_key.created_at(),
                );
                Some((rank, key))
            })
            .max_by(|a, b| a.0.cmp(&b.0))
            .map(|(_, k)| k)
    }
}

/// A detached, armored signature over `data` (RFC 3156's application/pgp-signature).
pub fn sign_detached(own: &OwnKey, data: &[u8]) -> Result<String, WriteProblem> {
    use pgp::crypto::hash::HashAlgorithm;
    let signer = signing_part(&own.key).ok_or(WriteProblem::Failed)?;
    let sig = DetachedSignature::sign_binary_data(
        rand::thread_rng(),
        &SignerRef(signer),
        &own.password,
        HashAlgorithm::Sha256,
        data,
    )
    .map_err(|_| WriteProblem::Failed)?;
    sig.to_armored_string(None.into())
        .map_err(|_| WriteProblem::Failed)
}

/// A `&dyn SigningKey` that is a `SigningKey` itself (rPGP's functions take `impl`).
struct SignerRef<'a>(&'a dyn pgp::types::SigningKey);

impl std::fmt::Debug for SignerRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SignerRef")
    }
}

impl KeyDetails for SignerRef<'_> {
    fn version(&self) -> pgp::types::KeyVersion {
        self.0.version()
    }
    fn legacy_key_id(&self) -> pgp::types::KeyId {
        self.0.legacy_key_id()
    }
    fn fingerprint(&self) -> pgp::types::Fingerprint {
        self.0.fingerprint()
    }
    fn algorithm(&self) -> pgp::crypto::public_key::PublicKeyAlgorithm {
        self.0.algorithm()
    }
    fn created_at(&self) -> pgp::types::Timestamp {
        self.0.created_at()
    }
    fn legacy_v3_expiration_days(&self) -> Option<u16> {
        self.0.legacy_v3_expiration_days()
    }
    fn public_params(&self) -> &pgp::types::PublicParams {
        self.0.public_params()
    }
}

impl pgp::types::SigningKey for SignerRef<'_> {
    fn sign(
        &self,
        key_pw: &Password,
        hash: pgp::crypto::hash::HashAlgorithm,
        data: &[u8],
    ) -> pgp::errors::Result<pgp::types::SignatureBytes> {
        self.0.sign(key_pw, hash, data)
    }
    fn hash_alg(&self) -> pgp::crypto::hash::HashAlgorithm {
        self.0.hash_alg()
    }
}

/// `data` encrypted to `to` (each key's encryption part), armored; signed inside when a
/// signer is given (RFC 3156 §6.2, one OpenPGP message that is signed and encrypted).
/// SEIPD version 1 with AES-256: what every OpenPGP program reads.
pub fn encrypt(
    data: &[u8],
    to: &[SignedPublicKey],
    signer: Option<&OwnKey>,
) -> Result<String, WriteProblem> {
    encrypt_hidden(data, to, &[], signer)
}

/// What compose asks for one message.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize, Serialize)]
pub struct Choice {
    #[serde(default)]
    pub sign: bool,
    #[serde(default)]
    pub encrypt: bool,
    /// Attach the sender's public key as a file.
    #[serde(default)]
    pub attach_key: bool,
}

/// Everything a message needs to go out signed or encrypted: gathered by
/// [`MailStore::protection`], applied by [`protect`] (smtp.rs builds the message first).
#[derive(Default)]
pub struct Protect {
    pub sign: Option<OwnKey>,
    /// Keys to encrypt to (the recipients' and the sender's own); empty: not encrypted.
    pub encrypt: Vec<SignedPublicKey>,
    /// Bcc recipients' keys: encrypted to without naming them (an anonymous recipient), so
    /// the others can't tell who else got it.
    pub hidden: Vec<SignedPublicKey>,
    /// An Autocrypt header's value for the sender.
    pub autocrypt: Option<String>,
    /// The sender's public key as a file: (name, armored).
    pub key_file: Option<(String, String)>,
}

impl Protect {
    /// Nothing to add: the message goes out as it is built.
    pub fn is_empty(&self) -> bool {
        self.sign.is_none()
            && self.encrypt.is_empty()
            && self.hidden.is_empty()
            && self.autocrypt.is_none()
            && self.key_file.is_none()
    }
}

/// What compose shows next to Sign and Encrypt.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ComposeState {
    /// The sender's own key, if there is one.
    pub own: Option<KeyRef>,
    /// Its passphrase is known this session (or it has none): signing can go ahead.
    pub unlocked: bool,
    /// Recipients without a key that can be encrypted to.
    pub missing: Vec<String>,
    /// Every recipient has a key (and the sender has one, for their own copy).
    pub can_encrypt: bool,
    /// Where compose starts: signed, encrypted.
    pub sign: bool,
    pub encrypt: bool,
}

/// The sender's own certificate, for encrypting their copy and their Autocrypt header.
fn own_public(db: &Connection, email: &str) -> Option<(String, SignedPublicKey)> {
    let email = email.trim().to_lowercase();
    let rows: Vec<(String, Vec<u8>)> = db
        .prepare_cached(
            "SELECT fingerprint, public FROM pgp_keys WHERE secret IS NOT NULL
               AND (' ' || emails || ' ') LIKE ('% ' || ?1 || ' %') ORDER BY created DESC",
        )
        .ok()?
        .query_map([&email], |r| Ok((r.get(0)?, r.get(1)?)))
        .ok()?
        .filter_map(Result::ok)
        .collect();
    rows.into_iter()
        .filter_map(|(fp, b)| {
            SignedPublicKey::from_bytes(Cursor::new(b))
                .ok()
                .map(|k| (fp, k))
        })
        .find(|(_, k)| encryption_key(k).is_some())
}

/// The smallest form of a certificate for an Autocrypt header: the primary key, the user
/// ids of `email` with their self-signatures, and the subkey to encrypt to.
fn minimal(key: &SignedPublicKey, email: &str) -> SignedPublicKey {
    use pgp::composed::SignedKeyDetails;
    let users = key
        .details
        .users
        .iter()
        .filter(|u| u.id.as_str().and_then(email_of_user_id).as_deref() == Some(email))
        .map(|u| {
            let mut u = u.clone();
            u.signatures.retain(|s| is_self_issued(s, &key.primary_key));
            u
        })
        .collect();
    let now = now_secs();
    let subkeys = key
        .public_subkeys
        .iter()
        .filter(|s| {
            bound_subkey(key, s, now).is_some_and(|b| {
                let f = b.key_flags();
                f.encrypt_comms() || f.encrypt_storage()
            })
        })
        .cloned()
        .collect();
    SignedPublicKey::new(
        key.primary_key.clone(),
        SignedKeyDetails::new(
            Vec::new(),
            key.details.direct_signatures.clone(),
            users,
            Vec::new(),
        ),
        subkeys,
    )
}

/// The Autocrypt header's value for an address and its key (Level 1, no prefer-encrypt).
pub fn autocrypt_value(email: &str, key: &SignedPublicKey) -> Option<String> {
    let email = email.trim().to_lowercase();
    let small = minimal(key, &email);
    if small.details.users.is_empty() {
        return None;
    }
    let bytes = small.to_bytes().ok()?;
    let data = base64::engine::general_purpose::STANDARD.encode(bytes);
    // Folded at 76 columns, each continuation line starting with a space.
    let folded: Vec<&str> = data
        .as_bytes()
        .chunks(76)
        .map(|c| std::str::from_utf8(c).unwrap_or_default())
        .collect();
    Some(format!(
        "addr={email}; keydata=\r\n {}",
        folded.join("\r\n ")
    ))
}

impl MailStore {
    /// The keys and parts a message from `from` to `to` (and `bcc`) needs for `choice`. For a
    /// draft (`draft`), only the sender's own copy is encrypted and nothing is signed.
    pub fn protection(
        &self,
        from: &str,
        to: &[String],
        bcc: &[String],
        choice: Choice,
        autocrypt: bool,
        draft: bool,
    ) -> Result<Protect, WriteProblem> {
        let own = own_public(&self.db, from);
        let mut p = Protect::default();
        if choice.encrypt {
            let (_, own_key) = own.clone().ok_or_else(|| WriteProblem::NoOwnKey {
                email: from.trim().to_lowercase(),
            })?;
            if !draft {
                let mut everyone: Vec<String> = Vec::new();
                for e in to {
                    let e = e.trim().to_lowercase();
                    if e != from.trim().to_lowercase() && !everyone.contains(&e) {
                        everyone.push(e);
                    }
                }
                let hidden: Vec<String> = bcc
                    .iter()
                    .map(|e| e.trim().to_lowercase())
                    .filter(|e| !everyone.contains(e))
                    .collect();
                let mut all = everyone.clone();
                all.extend(hidden.iter().cloned());
                // One error naming everyone without a key.
                self.keys_for(&all)?;
                p.encrypt = self.keys_for(&everyone)?;
                p.hidden = self.keys_for(&hidden)?;
            }
            p.encrypt.push(own_key);
        }
        if draft {
            return Ok(p);
        }
        if choice.sign {
            p.sign = Some(self.own_key_for(from)?);
        }
        if let Some((fp, key)) = &own {
            if autocrypt {
                p.autocrypt = autocrypt_value(from, key);
            }
            if choice.attach_key {
                let armored = key
                    .to_armored_string(None.into())
                    .map_err(|_| WriteProblem::Failed)?;
                p.key_file = Some((
                    format!("OpenPGP_0x{}.asc", &fp[fp.len().saturating_sub(16)..]),
                    armored,
                ));
            }
        }
        Ok(p)
    }

    /// Where compose's Sign and Encrypt start for a message from `from` to `to`, and what
    /// is missing. A reply to encrypted mail starts encrypted; else each recipient's last
    /// choice counts (all of them must have chosen it).
    pub fn compose_state(
        &self,
        from: &str,
        to: &[String],
        reply_to: Option<MessageId>,
        sign_by_default: bool,
    ) -> ComposeState {
        let own_secret = self.own_key_for(from);
        let own = own_public(&self.db, from);
        let mut state = ComposeState {
            own: own.as_ref().map(|(fp, k)| KeyRef {
                fingerprint: fp.clone(),
                name: valid_user_ids(k).into_iter().next().unwrap_or_default(),
            }),
            unlocked: own_secret.is_ok(),
            ..ComposeState::default()
        };
        let recipients: Vec<String> = to
            .iter()
            .map(|e| e.trim().to_lowercase())
            .filter(|e| !e.is_empty() && *e != from.trim().to_lowercase())
            .collect();
        state.missing = recipients
            .iter()
            .filter(|e| self.key_for(e).is_none())
            .cloned()
            .collect();
        state.can_encrypt = own.is_some() && !recipients.is_empty() && state.missing.is_empty();
        let replied = reply_to
            .and_then(|id| self.raw_of(id).ok().flatten())
            .map(|raw| find(&raw));
        let replied_encrypted = matches!(
            replied,
            Some(Some(
                Found::MimeEncrypted { .. } | Found::InlineEncrypted { .. }
            ))
        );
        let replied_signed = matches!(replied, Some(Some(_)));
        let prefs: Vec<(bool, bool)> = recipients
            .iter()
            .filter_map(|e| {
                self.db
                    .query_row(
                        "SELECT encrypt, sign FROM pgp_prefs WHERE email = ?1",
                        [e],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .ok()
            })
            .collect();
        let all = |f: fn(&(bool, bool)) -> bool| {
            !recipients.is_empty() && prefs.len() == recipients.len() && prefs.iter().all(f)
        };
        state.encrypt = state.can_encrypt && (replied_encrypted || all(|p| p.0));
        state.sign = state.own.is_some()
            && (sign_by_default || replied_signed || state.encrypt || all(|p| p.1));
        state
    }

    /// Remember what was chosen for these recipients, for the next message to them.
    pub fn remember_choice(&mut self, to: &[String], choice: Choice) -> Result<(), StoreError> {
        let tx = self.db.transaction()?;
        for e in to {
            let e = e.trim().to_lowercase();
            if e.is_empty() {
                continue;
            }
            tx.execute(
                "INSERT INTO pgp_prefs (email, encrypt, sign) VALUES (?1, ?2, ?3)
                 ON CONFLICT (email) DO UPDATE SET encrypt = excluded.encrypt, sign = excluded.sign",
                params![e, choice.encrypt, choice.sign],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}

/// A MIME boundary nobody's text contains.
fn boundary() -> String {
    use rand::Rng;
    let n: u128 = rand::thread_rng().gen();
    format!("telex-pgp-{n:032x}")
}

/// A composed message (smtp.rs) made signed (multipart/signed, RFC 3156 §5) or encrypted
/// (multipart/encrypted, §4; signed inside when signing too, §6.2), with an Autocrypt header.
/// Encrypted mail keeps its real subject inside (protected headers) and says "..." outside.
pub fn protect(raw: &[u8], p: &Protect) -> Result<Vec<u8>, WriteProblem> {
    let (head, body) = split_head(raw);
    let all = fields(head);
    let mut outer: Vec<&[u8]> = Vec::new();
    let mut content: Vec<Vec<u8>> = Vec::new();
    let mut subject: Option<&[u8]> = None;
    for (name, field) in &all {
        let lower = name.to_ascii_lowercase();
        if lower.starts_with("content-") {
            content.push(field.to_vec());
        } else if lower == "mime-version" {
        } else {
            if lower == "subject" {
                subject = Some(field);
            }
            outer.push(field);
        }
    }
    let encrypting = !p.encrypt.is_empty() || !p.hidden.is_empty();
    let mut out = Vec::with_capacity(raw.len() * 2);
    for field in &outer {
        if encrypting && Some(*field) == subject {
            out.extend_from_slice(b"Subject: ...\r\n");
        } else {
            out.extend_from_slice(field);
        }
    }
    if let Some(value) = &p.autocrypt {
        out.extend_from_slice(format!("Autocrypt: {value}\r\n").as_bytes());
    }
    out.extend_from_slice(b"MIME-Version: 1.0\r\n");
    if !encrypting && p.sign.is_none() {
        for field in &content {
            out.extend_from_slice(field);
        }
        out.extend_from_slice(b"\r\n");
        out.extend_from_slice(body);
        return Ok(out);
    }
    // The entity to sign or encrypt: the message's own content headers and body.
    let mut entity = Vec::with_capacity(body.len() + 512);
    for field in &content {
        if encrypting && field.to_ascii_lowercase().starts_with(b"content-type:") {
            // Protected headers (draft-ietf-lamps-header-protection, "v1" as Thunderbird
            // and Enigmail write it): the subject travels inside.
            let trimmed = field
                .strip_suffix(b"\r\n")
                .or_else(|| field.strip_suffix(b"\n"))
                .unwrap_or(field);
            entity.extend_from_slice(trimmed);
            entity.extend_from_slice(b"; protected-headers=\"v1\"\r\n");
        } else {
            entity.extend_from_slice(field);
        }
    }
    if encrypting {
        if let Some(subject) = subject {
            entity.extend_from_slice(subject);
        }
    }
    entity.extend_from_slice(b"\r\n");
    entity.extend_from_slice(body);
    let entity = canonical(&entity);
    let b = boundary();
    if !encrypting {
        let own = p.sign.as_ref().ok_or(WriteProblem::Failed)?;
        let signature = sign_detached(own, &entity)?;
        out.extend_from_slice(
            format!(
                "Content-Type: multipart/signed; micalg=pgp-sha256;\r\n \
                 protocol=\"application/pgp-signature\"; boundary=\"{b}\"\r\n\r\n\
                 This is an OpenPGP/MIME signed message (RFC 4880 and 3156)\r\n--{b}\r\n"
            )
            .as_bytes(),
        );
        out.extend_from_slice(&entity);
        out.extend_from_slice(
            format!(
                "\r\n--{b}\r\nContent-Type: application/pgp-signature; name=\"signature.asc\"\r\n\
                 Content-Description: OpenPGP digital signature\r\n\
                 Content-Disposition: attachment; filename=\"signature.asc\"\r\n\r\n{}\r\n--{b}--\r\n",
                signature.replace('\n', "\r\n").replace("\r\r\n", "\r\n").trim_end()
            )
            .as_bytes(),
        );
        return Ok(out);
    }
    let armored = encrypt_hidden(&entity, &p.encrypt, &p.hidden, p.sign.as_ref())?;
    out.extend_from_slice(
        format!(
            "Content-Type: multipart/encrypted;\r\n protocol=\"application/pgp-encrypted\"; boundary=\"{b}\"\r\n\r\n\
             This is an OpenPGP/MIME encrypted message (RFC 4880 and 3156)\r\n--{b}\r\n\
             Content-Type: application/pgp-encrypted\r\nContent-Description: PGP/MIME version identification\r\n\r\n\
             Version: 1\r\n\r\n--{b}\r\n\
             Content-Type: application/octet-stream; name=\"encrypted.asc\"\r\n\
             Content-Description: OpenPGP encrypted message\r\n\
             Content-Disposition: inline; filename=\"encrypted.asc\"\r\n\r\n{}\r\n--{b}--\r\n",
            armored.replace('\n', "\r\n").replace("\r\r\n", "\r\n").trim_end()
        )
        .as_bytes(),
    );
    Ok(out)
}

/// [`encrypt`], with keys to encrypt to without naming them.
fn encrypt_hidden(
    data: &[u8],
    to: &[SignedPublicKey],
    hidden: &[SignedPublicKey],
    signer: Option<&OwnKey>,
) -> Result<String, WriteProblem> {
    use pgp::composed::MessageBuilder;
    use pgp::crypto::{hash::HashAlgorithm, sym::SymmetricKeyAlgorithm};
    let mut rng = rand::thread_rng();
    let mut builder = MessageBuilder::from_bytes("", data.to_vec())
        .seipd_v1(&mut rng, SymmetricKeyAlgorithm::AES256);
    for key in to {
        match encryption_key(key).ok_or(WriteProblem::Failed)? {
            EncryptTo::Primary(k) => builder.encrypt_to_key(&mut rng, k),
            EncryptTo::Sub(k) => builder.encrypt_to_key(&mut rng, k),
        }
        .map_err(|_| WriteProblem::Failed)?;
    }
    for key in hidden {
        match encryption_key(key).ok_or(WriteProblem::Failed)? {
            EncryptTo::Primary(k) => builder.encrypt_to_key_anonymous(&mut rng, k),
            EncryptTo::Sub(k) => builder.encrypt_to_key_anonymous(&mut rng, k),
        }
        .map_err(|_| WriteProblem::Failed)?;
    }
    if let Some(own) = signer {
        let part = signing_part(&own.key).ok_or(WriteProblem::Failed)?;
        builder.sign(part, clone_password(&own.password), HashAlgorithm::Sha256);
    }
    builder
        .to_armored_string(&mut rng, None.into())
        .map_err(|_| WriteProblem::Failed)
}

fn clone_password(pw: &Password) -> Password {
    match pw {
        Password::Static(b) => Password::from(&b[..]),
        Password::Dynamic(f) => Password::from(&f()[..]),
    }
}

fn no_such_key() -> StoreError {
    StoreError::Invalid(tr!(
        "That key isn't in Telex any more.",
        "Tego klucza nie ma już w Telexie."
    ))
}

// ---- Autocrypt (Level 1) ----

/// An Autocrypt header's parts (autocrypt.org/level1.html, "The Autocrypt Header").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutocryptHeader {
    pub addr: String,
    pub prefer_encrypt: bool,
    pub keydata: Vec<u8>,
}

/// Read one Autocrypt header value; `None` when it isn't valid: no addr or keydata, an
/// unknown attribute that doesn't start with "_", keydata that isn't base64 or is too big.
pub fn parse_autocrypt(value: &str) -> Option<AutocryptHeader> {
    let mut addr = None;
    let mut prefer_encrypt = false;
    let mut keydata = None;
    for attr in value.split(';') {
        let attr = attr.trim();
        if attr.is_empty() {
            continue;
        }
        let (name, val) = attr.split_once('=')?;
        let (name, val) = (name.trim(), val.trim());
        match name {
            "addr" => addr = Some(val.trim_matches(['<', '>']).to_lowercase()),
            "prefer-encrypt" => prefer_encrypt = val == "mutual",
            "keydata" => {
                let clean: String = val.chars().filter(|c| !c.is_whitespace()).collect();
                if clean.len() > MAX_AUTOCRYPT * 4 / 3 + 4 {
                    return None;
                }
                keydata = Some(
                    base64::engine::general_purpose::STANDARD
                        .decode(clean)
                        .ok()?,
                );
            }
            n if n.starts_with('_') => {}
            _ => return None,
        }
    }
    Some(AutocryptHeader {
        addr: addr?,
        prefer_encrypt,
        keydata: keydata?,
    })
}

/// The Autocrypt header a message carries for its sender, if exactly one valid one does
/// (more than one counts as none, as the specification says).
fn autocrypt_of(p: &mail_parser::Message<'_>, from: &str) -> Option<AutocryptHeader> {
    let mut found = p
        .header_values("Autocrypt")
        .filter_map(|v| v.as_text())
        .filter_map(parse_autocrypt)
        .filter(|h| h.addr == from.to_lowercase());
    let first = found.next()?;
    found.next().is_none().then_some(first)
}

/// Learn from an incoming message's Autocrypt header (Level 1's "update peer state"): its
/// sender's key is saved (unconfirmed) and becomes the one to encrypt to when the message
/// is newer than the last one with a header. Mail from the address without a header moves
/// `last_seen` on. Nothing here fails a sync: a bad header is ignored.
pub(crate) fn note_autocrypt(
    db: &Connection,
    raw: &[u8],
    from: &str,
    date: NaiveDateTime,
    now: NaiveDateTime,
) -> Result<(), StoreError> {
    let from = from.trim().to_lowercase();
    if from.is_empty() || !memfind(raw, b"utocrypt") && !peer_exists(db, &from)? {
        return Ok(());
    }
    let Some(p) = MessageParser::default().parse_headers(raw) else {
        return Ok(());
    };
    // A message with more than one sender can't speak for one.
    if p.from().is_some_and(|f| f.iter().count() != 1) {
        return Ok(());
    }
    let header = autocrypt_of(&p, &from);
    let peer: Option<Option<NaiveDateTime>> = db
        .query_row(
            "SELECT autocrypt_seen FROM autocrypt_peers WHERE email = ?1",
            [&from],
            |r| r.get(0),
        )
        .optional()?;
    let Some(header) = header else {
        if peer.is_some() {
            db.execute(
                "UPDATE autocrypt_peers SET last_seen = MAX(last_seen, ?2) WHERE email = ?1",
                params![from, date],
            )?;
        }
        return Ok(());
    };
    if peer.flatten().is_some_and(|seen| seen >= date) {
        return Ok(());
    }
    if header.keydata.len() > MAX_AUTOCRYPT {
        return Ok(());
    }
    let Ok(key) = SignedPublicKey::from_bytes(Cursor::new(&header.keydata[..])) else {
        return Ok(());
    };
    // Autocrypt keys are for encrypting to their address: one that can't, or whose user ids
    // don't hold, isn't kept.
    if encryption_key(&key).is_none() || valid_user_ids(&key).is_empty() {
        return Ok(());
    }
    let (reference, _) = save_key(db, &PublicOrSecret::Public(key), Source::Autocrypt, now)?;
    db.execute(
        "INSERT INTO autocrypt_peers (email, fingerprint, prefer_encrypt, last_seen, autocrypt_seen)
         VALUES (?1, ?2, ?3, ?4, ?4)
         ON CONFLICT (email) DO UPDATE SET fingerprint = excluded.fingerprint,
             prefer_encrypt = excluded.prefer_encrypt,
             last_seen = MAX(autocrypt_peers.last_seen, excluded.last_seen),
             autocrypt_seen = excluded.autocrypt_seen",
        params![from, reference.fingerprint, header.prefer_encrypt, date],
    )?;
    Ok(())
}

fn peer_exists(db: &Connection, email: &str) -> Result<bool, StoreError> {
    Ok(db
        .prepare_cached("SELECT EXISTS (SELECT 1 FROM autocrypt_peers WHERE email = ?1)")?
        .query_row([email], |r| r.get(0))?)
}

fn memfind(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

// ---- Finding OpenPGP in a message ----

/// What OpenPGP a message holds.
#[derive(Debug, PartialEq, Eq)]
enum Found {
    /// multipart/encrypted: the encrypted payload.
    MimeEncrypted { payload: Vec<u8>, partial: bool },
    /// multipart/signed: the signed part as signed (canonical CRLF lines) and the signature.
    MimeSigned {
        signed: Vec<u8>,
        signature: Vec<u8>,
        partial: bool,
    },
    /// "-----BEGIN PGP MESSAGE-----" in the text: the text, and where the block is in it.
    InlineEncrypted { text: String, block: (usize, usize) },
    /// "-----BEGIN PGP SIGNED MESSAGE-----" in the text.
    InlineSigned { text: String, block: (usize, usize) },
}

/// The message is OpenPGP-encrypted (PGP/MIME or an inline block).
pub fn is_encrypted(raw: &[u8]) -> bool {
    matches!(
        find(raw),
        Some(Found::MimeEncrypted { .. } | Found::InlineEncrypted { .. })
    )
}

/// A part's MIME type, lower-case, and an attribute of it.
fn is_type(part: &MessagePart<'_>, ctype: &str, sub: &str) -> bool {
    part.content_type().is_some_and(|ct| {
        ct.ctype().eq_ignore_ascii_case(ctype)
            && ct.subtype().is_some_and(|s| s.eq_ignore_ascii_case(sub))
    })
}

fn protocol_is(part: &MessagePart<'_>, protocol: &str) -> bool {
    part.content_type()
        .and_then(|ct| ct.attribute("protocol"))
        .is_some_and(|p| p.trim().eq_ignore_ascii_case(protocol))
}

/// A part that only carries OpenPGP's machinery (multipart/encrypted's version part and
/// payload, a detached signature): not an attachment to list.
pub(crate) fn is_machinery(p: &mail_parser::Message<'_>, index: usize) -> bool {
    let part = &p.parts[index];
    if is_type(part, "application", "pgp-signature")
        || is_type(part, "application", "pgp-encrypted")
    {
        return true;
    }
    // The payload of multipart/encrypted.
    p.parts.iter().any(|parent| {
        is_type(parent, "multipart", "encrypted")
            && matches!(&parent.body, PartType::Multipart(kids) if kids.iter().any(|&k| k as usize == index))
    })
}

/// The OpenPGP a message holds, if any. Cheap for mail without any.
fn find(raw: &[u8]) -> Option<Found> {
    if !memfind(raw, b"application/pgp") && !memfind(raw, b"-----BEGIN PGP") {
        return None;
    }
    let p = MessageParser::default().parse(raw)?;
    for (index, part) in p.parts.iter().enumerate() {
        let PartType::Multipart(kids) = &part.body else {
            continue;
        };
        if is_type(part, "multipart", "encrypted") && protocol_is(part, "application/pgp-encrypted")
        {
            let payload = kids
                .iter()
                .map(|&k| &p.parts[k as usize])
                .find(|k| is_type(k, "application", "octet-stream"))?;
            return Some(Found::MimeEncrypted {
                payload: payload.contents().to_vec(),
                partial: index != 0,
            });
        }
        if is_type(part, "multipart", "signed") && protocol_is(part, "application/pgp-signature") {
            let boundary = part.content_type()?.attribute("boundary")?;
            let body = raw.get(part.offset_body as usize..part.offset_end as usize)?;
            let signed = first_part(body, boundary.as_bytes())?;
            let signature = kids
                .iter()
                .map(|&k| &p.parts[k as usize])
                .find(|k| is_type(k, "application", "pgp-signature"))?;
            return Some(Found::MimeSigned {
                signed: canonical(signed),
                signature: signature.contents().to_vec(),
                partial: index != 0,
            });
        }
    }
    let text = p.text_bodies().find_map(|part| match &part.body {
        PartType::Text(t) => Some(t.to_string()),
        _ => None,
    })?;
    if let Some(block) = block_in(
        &text,
        "-----BEGIN PGP MESSAGE-----",
        "-----END PGP MESSAGE-----",
    ) {
        return Some(Found::InlineEncrypted { text, block });
    }
    if let Some(block) = block_in(
        &text,
        "-----BEGIN PGP SIGNED MESSAGE-----",
        "-----END PGP SIGNATURE-----",
    ) {
        return Some(Found::InlineSigned { text, block });
    }
    None
}

/// A message's text as stored and searched: encrypted blocks taken out (what they say is
/// read only when the message is opened), a signed block's armor taken out and its text kept.
pub fn index_text(body: &str) -> String {
    if !body.contains("-----BEGIN PGP") {
        return body.to_owned();
    }
    let mut text = body.to_owned();
    for _ in 0..16 {
        let Some((start, stop)) = block_in(
            &text,
            "-----BEGIN PGP MESSAGE-----",
            "-----END PGP MESSAGE-----",
        ) else {
            break;
        };
        text.replace_range(start..stop, "");
    }
    for _ in 0..16 {
        let Some((start, stop)) = block_in(
            &text,
            "-----BEGIN PGP SIGNED MESSAGE-----",
            "-----END PGP SIGNATURE-----",
        ) else {
            break;
        };
        let block = &text[start..stop];
        let Ok((csm, _)) = CleartextSignedMessage::from_string(block) else {
            break;
        };
        let inner = csm.signed_text().replace("\r\n", "\n");
        text.replace_range(start..stop, &inner);
    }
    text
}

/// Where an armored block starts and ends in a text (its BEGIN line at a line's start,
/// through the end of its END line).
fn block_in(text: &str, begin: &str, end: &str) -> Option<(usize, usize)> {
    let start = text
        .match_indices(begin)
        .map(|(i, _)| i)
        .find(|&i| i == 0 || text.as_bytes()[i - 1] == b'\n')?;
    let stop = text[start..].find(end)? + start + end.len();
    let stop = text[stop..].find('\n').map_or(text.len(), |i| stop + i + 1);
    Some((start, stop))
}

/// The first body part of a multipart body as signed (RFC 3156 §5): from after the first
/// delimiter line to before the line break that precedes the next delimiter.
fn first_part<'a>(body: &'a [u8], boundary: &[u8]) -> Option<&'a [u8]> {
    let mut delimiter = b"--".to_vec();
    delimiter.extend_from_slice(boundary);
    let mut at = 0;
    // The first delimiter: at the start or after a line break.
    let start = loop {
        let i = body[at..]
            .windows(delimiter.len())
            .position(|w| w == delimiter.as_slice())?
            + at;
        if i == 0 || body[i - 1] == b'\n' {
            let line_end = body[i..].iter().position(|&b| b == b'\n')? + i + 1;
            break line_end;
        }
        at = i + 1;
    };
    let mut at = start;
    loop {
        let i = body[at..]
            .windows(delimiter.len())
            .position(|w| w == delimiter.as_slice())?
            + at;
        if i > 0 && body[i - 1] == b'\n' {
            let end = if i >= 2 && body[i - 2] == b'\r' {
                i - 2
            } else {
                i - 1
            };
            return body.get(start..end.max(start));
        }
        at = i + 1;
    }
}

/// Lines ending in CRLF, as signed MIME is signed.
fn canonical(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + data.len() / 32);
    for (i, &b) in data.iter().enumerate() {
        if b == b'\n' && (i == 0 || data[i - 1] != b'\r') {
            out.push(b'\r');
        }
        out.push(b);
    }
    out
}

// ---- Checking signatures ----

/// A part of a key that may have made a signature.
enum Signer<'a> {
    Primary(&'a PublicKey),
    Sub(&'a PublicSubkey),
}

/// The ids a signature names its key by, upper-case hex: fingerprints first.
fn issuer_ids(sig: &Signature) -> Vec<String> {
    let mut ids: Vec<String> = sig
        .issuer_fingerprint()
        .iter()
        .map(|fp| hex(fp.as_bytes()))
        .collect();
    ids.extend(sig.issuer_key_id().iter().map(|id| hex(id.as_ref())));
    ids
}

/// The parts of `key` that `ids` name and that may sign: the primary key, or a subkey it
/// binds now (or did when the message was signed).
fn signers<'a>(key: &'a SignedPublicKey, ids: &[String], at: u32) -> Vec<Signer<'a>> {
    let named = |k: &dyn KeyDetailsDyn| ids.contains(&k.fp()) || ids.contains(&k.id());
    let mut out = Vec::new();
    if named(&key.primary_key) {
        out.push(Signer::Primary(&key.primary_key));
    }
    for sub in &key.public_subkeys {
        if named(&sub.key) && bound_subkey(key, sub, at).is_some_and(|b| b.key_flags().sign()) {
            out.push(Signer::Sub(&sub.key));
        }
    }
    out
}

/// Fingerprint and key id of a key part, through one trait object.
trait KeyDetailsDyn {
    fn fp(&self) -> String;
    fn id(&self) -> String;
}

impl KeyDetailsDyn for PublicKey {
    fn fp(&self) -> String {
        fingerprint_of(self)
    }
    fn id(&self) -> String {
        key_id_of(self)
    }
}

impl KeyDetailsDyn for PublicSubkey {
    fn fp(&self) -> String {
        fingerprint_of(self)
    }
    fn id(&self) -> String {
        key_id_of(self)
    }
}

/// The verdict on one signature: `check` says whether it verifies with a key part. The key
/// is looked for among the stored keys and those the message itself carries.
fn judge(
    sig: &Signature,
    stored: &[(SignedPublicKey, bool, Source)],
    offered: &[SignedPublicKey],
    from: &str,
    check: &dyn Fn(&Signer<'_>) -> bool,
) -> SigView {
    let ids = issuer_ids(sig);
    let created = sig.created().map(|t| t.as_secs());
    let at = created.unwrap_or_else(now_secs);
    let key_id = ids
        .iter()
        .find(|id| id.len() == 16)
        .cloned()
        .or_else(|| {
            ids.first()
                .map(|fp| fp[fp.len().saturating_sub(16)..].to_owned())
        })
        .unwrap_or_default();
    let view = |status, key: Option<&SignedPublicKey>| SigView {
        status,
        key: key.map(|k| KeyRef {
            fingerprint: fingerprint_of(&k.primary_key),
            name: valid_user_ids(k).into_iter().next().unwrap_or_default(),
        }),
        key_id: key_id.clone(),
        created: created.map(local_time),
    };
    let candidates = stored
        .iter()
        .map(|(k, verified, source)| (k, *verified || source.confirmed()))
        .chain(offered.iter().map(|k| (k, false)));
    let mut seen_key = None;
    for (key, confirmed) in candidates {
        let parts = signers(key, &ids, at);
        if parts.is_empty() {
            continue;
        }
        seen_key.get_or_insert(key);
        if !parts.iter().any(check) {
            continue;
        }
        if revoked(key) {
            return view(SigStatus::Revoked, Some(key));
        }
        if expires(key).is_some_and(|t| t < at) {
            return view(SigStatus::Expired, Some(key));
        }
        let from = from.trim().to_lowercase();
        let theirs = valid_user_ids(key)
            .iter()
            .filter_map(|u| email_of_user_id(u))
            .any(|e| e == from);
        let status = match (theirs, confirmed) {
            (false, _) => SigStatus::OtherSender,
            (true, true) => SigStatus::Good,
            (true, false) => SigStatus::Unconfirmed,
        };
        return view(status, Some(key));
    }
    match seen_key {
        Some(key) => view(SigStatus::Bad, Some(key)),
        None => view(SigStatus::NoKey, None),
    }
}

/// Of several signatures, the one to show: the best verdict.
fn best(views: Vec<SigView>) -> Option<SigView> {
    let rank = |s: SigStatus| match s {
        SigStatus::Good => 0,
        SigStatus::Unconfirmed => 1,
        SigStatus::OtherSender => 2,
        SigStatus::Expired => 3,
        SigStatus::Revoked => 4,
        SigStatus::NoKey => 5,
        SigStatus::Bad => 6,
    };
    views.into_iter().min_by_key(|v| rank(v.status))
}

// ---- Decrypting ----

/// The decrypted contents, and the signature found inside, if any.
struct Plain {
    data: Vec<u8>,
    signature: Option<SigView>,
}

/// Why decrypting stopped.
enum Stop {
    Problem(Problem),
    Locked(Vec<KeyRef>),
    NotForYou(Vec<String>),
}

/// The key ids an encrypted message names as recipients (upper-case hex).
fn recipients_of(msg: &PgpMessage<'_>) -> Vec<String> {
    let PgpMessage::Encrypted { esk, .. } = msg else {
        return Vec::new();
    };
    esk.iter()
        .filter_map(|e| match e {
            Esk::PublicKeyEncryptedSessionKey(pk) => {
                if let Ok(Some(fp)) = pk.fingerprint() {
                    Some(hex(fp.as_bytes()))
                } else {
                    pk.id().ok().map(|id| hex(id.as_ref()))
                }
            }
            Esk::SymKeyEncryptedSessionKey(_) => None,
        })
        .collect()
}

impl MailStore {
    /// Decrypt an OpenPGP message (armored or binary) with the user's keys.
    fn decrypt(&self, data: &[u8], from: &str, offered: &[SignedPublicKey]) -> Result<Plain, Stop> {
        let parse = || PgpMessage::from_reader(BufReader::new(Cursor::new(data)));
        let (msg, _) = parse().map_err(|_| Stop::Problem(Problem::Failed))?;
        if !msg.is_encrypted() {
            return Err(Stop::Problem(Problem::Failed));
        }
        let recipients = recipients_of(&msg);
        let own = own_keys(&self.db).map_err(|_| Stop::Problem(Problem::Failed))?;
        // Own keys named as recipients (an anonymous recipient, all zeros, could be any).
        let anonymous = recipients.iter().any(|r| r.chars().all(|c| c == '0'));
        let for_us: Vec<&(String, SignedSecretKey)> = own
            .iter()
            .filter(|(_, k)| {
                let public = k.to_public_key();
                let ids = facts(&public).ids;
                anonymous || recipients.iter().any(|r| ids.contains(r))
            })
            .collect();
        if for_us.is_empty() {
            return Err(Stop::NotForYou(recipients));
        }
        let session = self.pgp.borrow();
        let empty = Password::empty();
        let mut keys = Vec::new();
        let mut passwords = Vec::new();
        let mut locked = Vec::new();
        for (fp, key) in &for_us {
            match session.passphrases.get(fp) {
                Some(pw) => {
                    keys.push(key);
                    passwords.push(pw);
                }
                None if !is_protected(key) => {
                    keys.push(key);
                    passwords.push(&empty);
                }
                None => locked.push(KeyRef {
                    fingerprint: fp.clone(),
                    name: valid_user_ids(&key.to_public_key())
                        .into_iter()
                        .next()
                        .unwrap_or_default(),
                }),
            }
        }
        if keys.is_empty() {
            return Err(Stop::Locked(locked));
        }
        let mut plain = msg
            .decrypt_with_keys(passwords, keys)
            .map_err(|e| Stop::Problem(problem_of(&e)))?;
        for _ in 0..4 {
            if !plain.is_compressed() {
                break;
            }
            plain = plain
                .decompress()
                .map_err(|e| Stop::Problem(problem_of(&e)))?;
        }
        let mut data = Vec::new();
        (&mut plain)
            .take(MAX_PLAIN as u64 + 1)
            .read_to_end(&mut data)
            .map_err(|_| Stop::Problem(Problem::Failed))?;
        if data.len() > MAX_PLAIN {
            return Err(Stop::Problem(Problem::TooLarge));
        }
        let signature = if plain.is_signed() {
            self.judge_message(&plain, from, offered)
        } else {
            None
        };
        Ok(Plain { data, signature })
    }

    /// The verdict on the signatures of a signed OpenPGP message read to its end.
    fn judge_message(
        &self,
        msg: &PgpMessage<'_>,
        from: &str,
        offered: &[SignedPublicKey],
    ) -> Option<SigView> {
        let PgpMessage::Signed { reader, .. } = msg else {
            return None;
        };
        let mut views = Vec::new();
        for index in 0..reader.num_signatures().min(MAX_SIGNATURES) {
            let Some(sig) = reader.signature(index) else {
                continue;
            };
            let stored = keys_by_ids(&self.db, &issuer_ids(sig)).unwrap_or_default();
            let check = |signer: &Signer<'_>| match signer {
                Signer::Primary(k) => msg.verify_nested_explicit(index, *k).is_ok(),
                Signer::Sub(k) => msg.verify_nested_explicit(index, *k).is_ok(),
            };
            views.push(judge(sig, &stored, offered, from, &check));
        }
        best(views)
    }

    /// The verdict on a detached signature over `data`.
    fn judge_detached(
        &self,
        signature: &[u8],
        data: &[u8],
        from: &str,
        offered: &[SignedPublicKey],
    ) -> SigView {
        let sigs = read_signatures(signature);
        let views = sigs
            .iter()
            .take(MAX_SIGNATURES)
            .map(|sig| {
                let stored = keys_by_ids(&self.db, &issuer_ids(sig)).unwrap_or_default();
                let check = |signer: &Signer<'_>| match signer {
                    Signer::Primary(k) => sig.verify(*k, data).is_ok(),
                    Signer::Sub(k) => sig.verify(*k, data).is_ok(),
                };
                judge(sig, &stored, offered, from, &check)
            })
            .collect();
        best(views).unwrap_or(SigView {
            status: SigStatus::Bad,
            key: None,
            key_id: String::new(),
            created: None,
        })
    }

    /// OpenPGP of a stored message: what to show and the line above it; `None` when it has
    /// none. Decrypted messages are kept in memory for the next views of it (HTML,
    /// attachments, a reply's quote).
    pub fn pgp_open(&self, message: &Message, raw: &[u8]) -> Option<Arc<Opened>> {
        let key = (message.id, {
            let mut h = DefaultHasher::new();
            raw.hash(&mut h);
            h.finish()
        });
        if let Some((_, opened)) = self.pgp.borrow().opened.iter().find(|(k, _)| *k == key) {
            return Some(opened.clone());
        }
        let opened = Arc::new(self.open_now(message, raw)?);
        let mut session = self.pgp.borrow_mut();
        session.opened.retain(|(k, _)| k.0 != message.id);
        session.opened.insert(0, (key, opened.clone()));
        session.opened.truncate(KEEP_OPENED);
        Some(opened)
    }

    fn open_now(&self, message: &Message, raw: &[u8]) -> Option<Opened> {
        let found = find(raw)?;
        let from = message.from_email.as_str();
        let offered_keys = offered_keys(&self.db, raw, from);
        let offered = |keys: &[SignedPublicKey]| -> Vec<KeyRef> {
            keys.iter()
                .map(|k| KeyRef {
                    fingerprint: fingerprint_of(&k.primary_key),
                    name: valid_user_ids(k).into_iter().next().unwrap_or_default(),
                })
                .collect()
        };
        let mut view = View {
            offered: offered(&offered_keys),
            ..View::default()
        };
        match found {
            Found::MimeSigned {
                signed,
                signature,
                partial,
            } => {
                view.partial = partial;
                view.signature =
                    Some(self.judge_detached(&signature, &signed, from, &offered_keys));
                Some(Opened {
                    view,
                    raw: None,
                    body: None,
                })
            }
            Found::InlineSigned { text, block } => {
                view.partial =
                    !text[..block.0].trim().is_empty() || !text[block.1..].trim().is_empty();
                let armored = &text[block.0..block.1];
                let Ok((csm, _)) = CleartextSignedMessage::from_string(armored) else {
                    view.signature = Some(SigView {
                        status: SigStatus::Bad,
                        key: None,
                        key_id: String::new(),
                        created: None,
                    });
                    return Some(Opened {
                        view,
                        raw: None,
                        body: None,
                    });
                };
                let views = csm
                    .signatures()
                    .iter()
                    .take(MAX_SIGNATURES)
                    .map(|sig| {
                        let stored = keys_by_ids(&self.db, &issuer_ids(sig)).unwrap_or_default();
                        let check = |signer: &Signer<'_>| match signer {
                            Signer::Primary(k) => {
                                sig.verify(*k, csm.signed_text().as_bytes()).is_ok()
                            }
                            Signer::Sub(k) => sig.verify(*k, csm.signed_text().as_bytes()).is_ok(),
                        };
                        judge(sig, &stored, &offered_keys, from, &check)
                    })
                    .collect();
                view.signature = best(views);
                let body = format!(
                    "{}{}{}",
                    &text[..block.0],
                    csm.signed_text(),
                    &text[block.1..]
                );
                Some(Opened {
                    view,
                    raw: None,
                    body: Some(body.replace("\r\n", "\n")),
                })
            }
            Found::InlineEncrypted { text, block } => {
                view.encrypted = true;
                view.partial =
                    !text[..block.0].trim().is_empty() || !text[block.1..].trim().is_empty();
                match self.decrypt(&text.as_bytes()[block.0..block.1], from, &offered_keys) {
                    Ok(plain) => {
                        view.decrypted = true;
                        view.signature = plain.signature;
                        let inner = String::from_utf8_lossy(&plain.data).replace("\r\n", "\n");
                        let body = format!("{}{}{}", &text[..block.0], inner, &text[block.1..]);
                        let readable = with_outer_headers(
                            raw,
                            format!(
                                "Content-Type: text/plain; charset=utf-8\r\n\
                                 Content-Transfer-Encoding: 8bit\r\n\r\n{}",
                                body.replace('\n', "\r\n")
                            )
                            .as_bytes(),
                        );
                        Some(Opened {
                            view,
                            raw: Some(Arc::new(readable)),
                            body: Some(body),
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
            Found::MimeEncrypted { payload, partial } => {
                view.encrypted = true;
                view.partial = partial;
                match self.decrypt(&payload, from, &offered_keys) {
                    Ok(plain) => {
                        view.decrypted = true;
                        let inner = if looks_like_entity(&plain.data) {
                            plain.data
                        } else {
                            let mut wrapped =
                                b"Content-Type: text/plain; charset=utf-8\r\n\r\n".to_vec();
                            wrapped.extend_from_slice(&plain.data);
                            wrapped
                        };
                        let readable = with_outer_headers(raw, &inner);
                        // Signed and then encrypted: the signature is a multipart/signed inside.
                        view.signature = plain.signature.or_else(|| match find(&readable) {
                            Some(Found::MimeSigned {
                                signed, signature, ..
                            }) => {
                                Some(self.judge_detached(&signature, &signed, from, &offered_keys))
                            }
                            _ => None,
                        });
                        Some(Opened {
                            view,
                            raw: Some(Arc::new(readable)),
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

    /// The message to read: decrypted when it was opened and decrypted, else as stored.
    pub fn readable_raw(&self, id: MessageId) -> Result<Option<Vec<u8>>, StoreError> {
        let decrypted = self
            .pgp
            .borrow()
            .opened
            .iter()
            .find(|((m, _), _)| *m == id)
            .and_then(|(_, o)| o.raw.clone());
        match decrypted {
            Some(raw) => Ok(Some(raw.as_ref().clone())),
            None => self.raw_of(id),
        }
    }
}

/// Show the message as OpenPGP read it: the decrypted subject, text and attachments, or
/// inline PGP's text.
pub(crate) fn apply(message: &mut Message, opened: &Opened) {
    if let Some(readable) = &opened.raw {
        let shown = crate::mime::parse_message(readable, &message.folder, Some(message.date));
        message.subject = shown.subject;
        message.body = shown.body;
        message.attachments = shown.attachments;
        message.remote_images = shown.remote_images;
    }
    if let Some(body) = &opened.body {
        message.body = body.clone();
    }
}

impl MailStore {
    /// A message as the user last saw it: decrypted when it was opened and decrypted this
    /// session (for a reply's quote, a forward), else as stored. Decrypts nothing itself.
    pub fn get_readable(&self, id: MessageId) -> Result<Message, StoreError> {
        let mut message = self.get(id)?;
        let opened = self
            .pgp
            .borrow()
            .opened
            .iter()
            .find(|((m, _), _)| *m == id)
            .map(|(_, o)| o.clone());
        if let Some(opened) = opened {
            apply(&mut message, &opened);
        }
        Ok(message)
    }
}

fn stopped(view: &mut View, stop: Stop) {
    match stop {
        Stop::Problem(p) => view.problem = Some(p),
        Stop::Locked(keys) => {
            view.problem = Some(Problem::Locked);
            view.locked = keys;
        }
        Stop::NotForYou(ids) => {
            view.problem = Some(Problem::NotForYou);
            view.recipients = ids;
        }
    }
}

/// rPGP's error, as one of Telex's codes (its text never reaches the window).
fn problem_of(e: &pgp::errors::Error) -> Problem {
    let text = e.to_string().to_lowercase();
    if text.contains("bzip") || text.contains("unsupported") || text.contains("legacy") {
        Problem::Unsupported
    } else {
        Problem::Failed
    }
}

/// The signatures in a detached signature block (armored or binary).
fn read_signatures(data: &[u8]) -> Vec<Signature> {
    if data.len() > MAX_KEY {
        return Vec::new();
    }
    if let Ok((sigs, _)) = DetachedSignature::from_reader_many(Cursor::new(data)) {
        let sigs: Vec<Signature> = sigs
            .take(MAX_SIGNATURES)
            .filter_map(Result::ok)
            .map(|s| s.signature)
            .collect();
        if !sigs.is_empty() {
            return sigs;
        }
    }
    DetachedSignature::from_bytes(Cursor::new(data))
        .map(|s| vec![s.signature])
        .unwrap_or_default()
}

/// The keys a message carries that Telex doesn't have: its Autocrypt header's (for its
/// sender) and attached key files (application/pgp-keys).
fn offered_keys(db: &Connection, raw: &[u8], from: &str) -> Vec<SignedPublicKey> {
    let Some(p) = MessageParser::default().parse(raw) else {
        return Vec::new();
    };
    let mut blocks: Vec<Vec<u8>> = Vec::new();
    if let Some(h) = autocrypt_of(&p, from) {
        blocks.push(h.keydata);
    }
    for part in p.parts.iter() {
        if is_type(part, "application", "pgp-keys") && part.contents().len() <= MAX_KEY {
            blocks.push(part.contents().to_vec());
        }
    }
    let mut keys = Vec::new();
    for block in blocks {
        for key in read_keys(&block).0 {
            let public = public_of(&key);
            let fp = fingerprint_of(&public.primary_key);
            let known: bool = db
                .query_row(
                    "SELECT EXISTS (SELECT 1 FROM pgp_keys WHERE fingerprint = ?1)",
                    [&fp],
                    |r| r.get(0),
                )
                .unwrap_or(false);
            if !known
                && !keys
                    .iter()
                    .any(|k: &SignedPublicKey| fingerprint_of(&k.primary_key) == fp)
            {
                keys.push(public);
            }
        }
        if keys.len() >= MAX_OFFERED {
            break;
        }
    }
    keys.truncate(MAX_OFFERED);
    keys
}

impl MailStore {
    /// Import the keys a message carries (its Autocrypt header's, attached key files): the
    /// user's "Import key" above it. They stay unconfirmed until the user confirms them.
    pub fn import_offered_keys(
        &mut self,
        id: MessageId,
        now: NaiveDateTime,
    ) -> Result<ImportReport, StoreError> {
        let message = self.get(id)?;
        let raw = self
            .readable_raw(id)?
            .ok_or(StoreError::UnknownMessage(id))?;
        let keys = offered_keys(&self.db, &raw, &message.from_email);
        let mut report = ImportReport::default();
        let tx = self.db.transaction()?;
        for key in keys {
            let (reference, saved) =
                save_key(&tx, &PublicOrSecret::Public(key), Source::Message, now)?;
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
}

/// The decrypted data starts with MIME headers (a body part), not with text.
fn looks_like_entity(data: &[u8]) -> bool {
    let head = &data[..data.len().min(2048)];
    let text = String::from_utf8_lossy(head);
    let first = text.lines().next().unwrap_or_default();
    first.split_once(':').is_some_and(|(name, _)| {
        !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    })
}

/// A message made of the outer message's headers (From, To, Date…; not its Content- ones)
/// and a decrypted body part. The part's own Subject (protected headers, the "memory hole"
/// scheme) replaces the outer one, which is often "..." then; its From, To and Date don't:
/// the outer ones are those the list and the signature check go by.
fn with_outer_headers(raw: &[u8], inner: &[u8]) -> Vec<u8> {
    let (outer_head, _) = split_head(raw);
    let (inner_head, inner_body) = split_head(inner);
    let inner_fields = fields(inner_head);
    let inner_subject = inner_fields
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("subject"));
    let mut out = Vec::with_capacity(raw.len().min(64 * 1024) + inner.len());
    for (name, field) in fields(outer_head) {
        let lower = name.to_ascii_lowercase();
        if lower.starts_with("content-") || lower == "mime-version" {
            continue;
        }
        if lower == "subject" && inner_subject.is_some() {
            continue;
        }
        out.extend_from_slice(field);
    }
    if let Some((_, field)) = inner_subject {
        out.extend_from_slice(field);
    }
    out.extend_from_slice(b"MIME-Version: 1.0\r\n");
    for (name, field) in &inner_fields {
        if name.to_ascii_lowercase().starts_with("content-") {
            out.extend_from_slice(field);
        }
    }
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(inner_body);
    out
}

/// A message's header block (with its line breaks) and its body.
fn split_head(raw: &[u8]) -> (&[u8], &[u8]) {
    let mut i = 0;
    while i < raw.len() {
        let line_end = raw[i..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(raw.len(), |p| i + p + 1);
        let line = &raw[i..line_end];
        if line == b"\r\n" || line == b"\n" {
            return (&raw[..i], &raw[line_end..]);
        }
        i = line_end;
    }
    (raw, &[])
}

/// The header fields of a header block: (name, the whole field with its folded lines).
fn fields(head: &[u8]) -> Vec<(String, &[u8])> {
    let mut out: Vec<(String, &[u8])> = Vec::new();
    let mut start = None::<(usize, String)>;
    let mut i = 0;
    while i < head.len() {
        let line_end = head[i..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(head.len(), |p| i + p + 1);
        let line = &head[i..line_end];
        let folded = line.first().is_some_and(|&b| b == b' ' || b == b'\t');
        if !folded {
            if let Some((s, name)) = start.take() {
                out.push((name, &head[s..i]));
            }
            if let Some(colon) = line.iter().position(|&b| b == b':') {
                let name = String::from_utf8_lossy(&line[..colon]).trim().to_owned();
                start = Some((i, name));
            }
        }
        i = line_end;
    }
    if let Some((s, name)) = start {
        out.push((name, &head[s..]));
    }
    out
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod write_tests;
