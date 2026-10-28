//! In-memory OpenPGP adapter. Serialized keys cross threads; native RNP handles never do.
//! RNP owns packet validation and Botan owns private-key arithmetic. Neither keys nor
//! decrypted output are written to a temporary file or handed to an external process.
//!
//! Native verification handles must not escape the operation that owns them:
//! ```compile_fail,E0597
//! let ctx = rnp::Context::new().unwrap();
//! let signature;
//! {
//!     let result = rnp::verify(&ctx, b"fixture").unwrap();
//!     signature = result.signature_at(0).unwrap().unwrap();
//! }
//! signature.status_is_valid();
//! ```
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};

use rnp::{Context, ExportFlags, KeyIdentifier, KeyUsage, KeyringFormat, LoadSaveFlags};
use zeroize::Zeroizing;

use super::{KeyKind, MAX_KEY, MAX_PLAIN, MAX_SIGNATURES};

#[derive(Clone, Debug)]
pub struct Primary {
    pub fingerprint: String,
    pub id: String,
    pub created: u32,
    pub algorithm: String,
}
impl Primary {
    pub fn created_at(&self) -> u32 {
        self.created
    }
}

#[derive(Clone)]
pub struct SignedPublicKey {
    pub primary_key: Primary,
    pub user_ids: Vec<String>,
    pub ids: Vec<String>,
    pub expires: Option<u32>,
    pub revoked: bool,
    pub encrypt: bool,
    bytes: Arc<Vec<u8>>,
}
impl std::fmt::Debug for SignedPublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignedPublicKey")
            .field("fingerprint", &self.primary_key.fingerprint)
            .finish_non_exhaustive()
    }
}
#[derive(Clone)]
pub struct SignedSecretKey {
    pub primary_key: Primary,
    pub public: SignedPublicKey,
    bytes: Arc<Zeroizing<Vec<u8>>>,
    pub protected: bool,
}
pub enum PublicOrSecret {
    Public(SignedPublicKey),
    Secret(SignedSecretKey),
}

#[derive(Clone, Default)]
pub struct Password(Zeroizing<String>);
impl std::fmt::Debug for Password {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Password")
    }
}
impl From<&str> for Password {
    fn from(p: &str) -> Self {
        Self(Zeroizing::new(p.to_owned()))
    }
}
impl Password {
    pub fn empty() -> Self {
        Self::default()
    }
    fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

fn context(bytes: &[u8]) -> Result<Context, ()> {
    let ctx = Context::new().map_err(|_| ())?;
    ctx.load_keys(
        KeyringFormat::Gpg,
        bytes,
        LoadSaveFlags::PUBLIC | LoadSaveFlags::SECRET,
    )
    .map_err(|_| ())?;
    Ok(ctx)
}
fn named<'a>(ctx: &'a Context, fp: &str) -> Result<rnp::Key<'a>, ()> {
    ctx.find_key(KeyIdentifier::Fingerprint(fp))
        .map_err(|_| ())?
        .ok_or(())
}

fn public(key: &rnp::Key<'_>) -> Result<SignedPublicKey, ()> {
    let get = |result: rnp::Result<String>| result.map_err(|_| ());
    let fingerprint = get(key.fingerprint())?.to_uppercase();
    let id = get(key.keyid())?.to_uppercase();
    let created = key.creation().map_err(|_| ())?;
    let algorithm = match get(key.alg())?.as_str() {
        "RSA" => format!("RSA {}", key.bits().map_err(|_| ())?),
        "EDDSA" => "Ed25519".into(),
        "ECDH" => key.curve().ok().flatten().unwrap_or_else(|| "ECDH".into()),
        s => s.to_owned(),
    };
    let user_ids = key
        .uids()
        .map_err(|_| ())?
        .iter()
        .filter(|uid| uid.is_valid().unwrap_or(false))
        .filter_map(|uid| uid.data_string().ok())
        .collect::<Vec<_>>();
    let mut ids = vec![fingerprint.clone(), id.clone()];
    for sub in key.subkeys().map_err(|_| ())? {
        ids.push(get(sub.fingerprint())?.to_uppercase());
        ids.push(get(sub.keyid())?.to_uppercase());
    }
    let lifetime = key.expiration().map_err(|_| ())?;
    let revoked = key.is_revoked().map_err(|_| ())?;
    let encrypt = !revoked
        && !key.is_expired().map_err(|_| ())?
        && key
            .default_key_for(KeyUsage::EncryptComms)
            .ok()
            .flatten()
            .is_some();
    Ok(SignedPublicKey {
        primary_key: Primary {
            fingerprint,
            id,
            created,
            algorithm,
        },
        user_ids,
        ids,
        expires: (lifetime != 0).then(|| created.saturating_add(lifetime)),
        revoked,
        encrypt,
        bytes: Arc::new(
            key.export(ExportFlags::PUBLIC | ExportFlags::SUBKEYS)
                .map_err(|_| ())?,
        ),
    })
}

pub fn read_keys(bytes: &[u8]) -> (Vec<PublicOrSecret>, usize) {
    if bytes.len() > MAX_KEY * 4 {
        return (vec![], 1);
    }
    let Ok(ctx) = context(bytes) else {
        return (vec![], 1);
    };
    let Ok(ids) = ctx.identifiers(rnp::keyring::IdentifierKind::Fingerprint) else {
        return (vec![], 1);
    };
    let mut keys = vec![];
    let mut skipped = 0;
    for fp in ids.take(1000) {
        let Ok(key) = named(&ctx, &fp) else {
            skipped += 1;
            continue;
        };
        if !key.is_primary().unwrap_or(false) {
            continue;
        }
        let Ok(public) = public(&key) else {
            skipped += 1;
            continue;
        };
        if public.user_ids.is_empty() {
            skipped += 1;
            continue;
        }
        let subkeys = key.subkeys().unwrap_or_default();
        let secret = key.have_secret().unwrap_or(false)
            || subkeys.iter().any(|k| k.have_secret().unwrap_or(false));
        if secret {
            let Ok(bytes) = key.export(ExportFlags::SECRET | ExportFlags::SUBKEYS) else {
                skipped += 1;
                continue;
            };
            let protected = key.is_protected().unwrap_or(true)
                || subkeys
                    .iter()
                    .any(|k| k.have_secret().unwrap_or(false) && k.is_protected().unwrap_or(true));
            keys.push(PublicOrSecret::Secret(SignedSecretKey {
                primary_key: public.primary_key.clone(),
                public,
                bytes: Arc::new(Zeroizing::new(bytes)),
                protected,
            }));
        } else {
            keys.push(PublicOrSecret::Public(public));
        }
    }
    (keys, skipped)
}

impl SignedPublicKey {
    #[cfg(test)]
    pub fn without_subkeys(&self) -> Result<Self, ()> {
        let ctx = context(&self.bytes)?;
        let key = named(&ctx, &self.primary_key.fingerprint)?;
        Self::from_bytes(key.export(ExportFlags::PUBLIC).map_err(|_| ())?.as_slice())
    }
    pub fn from_bytes(mut reader: impl Read) -> Result<Self, ()> {
        let mut bytes = vec![];
        reader
            .by_ref()
            .take((MAX_KEY * 4 + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| ())?;
        read_keys(&bytes)
            .0
            .into_iter()
            .next()
            .map(|k| match k {
                PublicOrSecret::Public(p) => p,
                PublicOrSecret::Secret(s) => s.public,
            })
            .ok_or(())
    }
    pub fn to_bytes(&self) -> Result<Vec<u8>, ()> {
        Ok(self.bytes.as_ref().clone())
    }
    pub fn to_armored_string(&self, _: Option<()>) -> Result<String, ()> {
        let ctx = context(&self.bytes)?;
        let key = named(&ctx, &self.primary_key.fingerprint)?;
        String::from_utf8(
            key.export(ExportFlags::PUBLIC | ExportFlags::SUBKEYS | ExportFlags::ARMORED)
                .map_err(|_| ())?,
        )
        .map_err(|_| ())
    }
    pub fn autocrypt(&self, email: &str) -> Result<Vec<u8>, ()> {
        let uid = self
            .user_ids
            .iter()
            .find(|uid| super::email_of_user_id(uid).as_deref() == Some(email))
            .ok_or(())?;
        let ctx = context(&self.bytes)?;
        let key = named(&ctx, &self.primary_key.fingerprint)?;
        let sub = key
            .default_key_for(KeyUsage::EncryptComms)
            .map_err(|_| ())?
            .ok_or(())?;
        key.export_autocrypt(Some(&sub), Some(uid), ExportFlags::default())
            .map_err(|_| ())
    }
}
impl SignedSecretKey {
    pub fn from_bytes(mut reader: impl Read) -> Result<Self, ()> {
        let mut bytes = vec![];
        reader
            .by_ref()
            .take((MAX_KEY * 4 + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| ())?;
        read_keys(&bytes)
            .0
            .into_iter()
            .find_map(|k| match k {
                PublicOrSecret::Secret(s) => Some(s),
                _ => None,
            })
            .ok_or(())
    }
    pub fn to_public_key(&self) -> SignedPublicKey {
        self.public.clone()
    }
    pub fn to_bytes(&self) -> Result<Vec<u8>, ()> {
        Ok(self.bytes.as_slice().to_vec())
    }
    pub fn to_armored_string(&self, _: Option<()>) -> Result<String, ()> {
        let ctx = context(&self.bytes)?;
        let key = named(&ctx, &self.primary_key.fingerprint)?;
        String::from_utf8(
            key.export(ExportFlags::SECRET | ExportFlags::SUBKEYS | ExportFlags::ARMORED)
                .map_err(|_| ())?,
        )
        .map_err(|_| ())
    }
    pub fn opens(&self, password: &Password) -> bool {
        let Ok(ctx) = context(&self.bytes) else {
            return false;
        };
        let Ok(key) = named(&ctx, &self.primary_key.fingerprint) else {
            return false;
        };
        key.unlock(Some(password.as_str())).is_ok()
            || key.subkeys().unwrap_or_default().iter().any(|k| {
                k.have_secret().unwrap_or(false) && k.unlock(Some(password.as_str())).is_ok()
            })
    }
}

struct Provider(Password);
impl rnp::callbacks::PasswordProvider for Provider {
    fn get_password(&self, _: Option<&rnp::Key<'_>>, _: &str) -> Option<std::borrow::Cow<'_, str>> {
        Some(std::borrow::Cow::Borrowed(self.0.as_str()))
    }
}

pub fn generate(uid: &str, password: &str, kind: KeyKind) -> Result<SignedSecretKey, ()> {
    use rnp::{
        Algorithm, Cipher, Compression, Curve, Hash, KeyBuilder, ProtectOptions, SubkeyBuilder,
    };
    let mut ctx = Context::new().map_err(|_| ())?;
    ctx.set_password_provider(Box::new(Provider(Password::from(password))));
    let (alg, subalg) = match kind {
        KeyKind::Ed25519 => (Algorithm::Eddsa, Algorithm::Ecdh),
        KeyKind::Rsa3072 => (Algorithm::Rsa, Algorithm::Rsa),
    };
    let protection = ProtectOptions::new()
        .password(password)
        .cipher(Cipher::Aes256)
        .hash(Hash::Sha256)
        .iterations(3_000_000);
    let mut primary = KeyBuilder::new(alg)
        .userid(uid)
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Certify)
        .add_usage(KeyUsage::Sign)
        .add_pref_cipher(Cipher::Aes256)
        .add_pref_cipher(Cipher::Aes128)
        .add_pref_hash(Hash::Sha512)
        .add_pref_hash(Hash::Sha256)
        .add_pref_compression(Compression::Zlib)
        .add_pref_compression(Compression::Zip)
        .protection(&protection);
    let mut sub = SubkeyBuilder::new(subalg)
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::EncryptComms)
        .add_usage(KeyUsage::EncryptStorage);
    match kind {
        KeyKind::Ed25519 => {
            sub = sub.curve(Curve::Curve25519);
        }
        KeyKind::Rsa3072 => {
            primary = primary.bits(3072);
            sub = sub.bits(3072);
        }
    }
    let key = primary.build(&ctx).map_err(|_| ())?;
    key.unlock(Some(password)).map_err(|_| ())?;
    let sk = sub.build(&ctx, &key).map_err(|_| ())?;
    sk.protect(&protection).map_err(|_| ())?;
    let bytes = Zeroizing::new(
        key.export(ExportFlags::SECRET | ExportFlags::SUBKEYS)
            .map_err(|_| ())?,
    );
    SignedSecretKey::from_bytes(bytes.as_slice())
}

pub fn sign_detached(
    key: &SignedSecretKey,
    password: &Password,
    data: &[u8],
) -> Result<String, ()> {
    let mut ctx = context(&key.bytes)?;
    ctx.set_password_provider(Box::new(Provider(password.clone())));
    let primary = named(&ctx, &key.primary_key.fingerprint)?;
    let signer = primary
        .default_key_for(KeyUsage::Sign)
        .map_err(|_| ())?
        .ok_or(())?;
    let bytes = rnp::Signer::new(&ctx, data, rnp::Mode::Detached)
        .add_signer(&signer)
        .armor(true)
        .hash(rnp::Hash::Sha256)
        .build_to_memory()
        .map_err(|_| ())?;
    String::from_utf8(bytes).map_err(|_| ())
}

pub fn encrypt(
    data: &[u8],
    to: &[SignedPublicKey],
    hidden: &[SignedPublicKey],
    signer: Option<(&SignedSecretKey, &Password)>,
) -> Result<String, ()> {
    let mut ctx = Context::new().map_err(|_| ())?;
    for key in to.iter().chain(hidden) {
        ctx.load_keys(
            KeyringFormat::Gpg,
            key.bytes.as_slice(),
            LoadSaveFlags::PUBLIC,
        )
        .map_err(|_| ())?;
    }
    if let Some((key, password)) = signer {
        ctx.load_keys(
            KeyringFormat::Gpg,
            key.bytes.as_slice(),
            LoadSaveFlags::PUBLIC | LoadSaveFlags::SECRET,
        )
        .map_err(|_| ())?;
        ctx.set_password_provider(Box::new(Provider(password.clone())));
    }
    let primary = to
        .iter()
        .chain(hidden)
        .map(|k| named(&ctx, &k.primary_key.fingerprint))
        .collect::<Result<Vec<_>, _>>()?;
    let recipients = primary
        .iter()
        .map(|k| {
            k.default_key_for(KeyUsage::EncryptComms)
                .map_err(|_| ())?
                .ok_or(())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let hidden_ids = recipients
        .iter()
        .skip(to.len())
        .map(|k| k.keyid().map(|s| s.to_uppercase()).map_err(|_| ()))
        .collect::<Result<Vec<_>, _>>()?;
    let own = signer
        .map(|(k, _)| named(&ctx, &k.primary_key.fingerprint))
        .transpose()?;
    let signing = own
        .as_ref()
        .map(|k| k.default_key_for(KeyUsage::Sign).map_err(|_| ())?.ok_or(()))
        .transpose()?;
    let mut builder = rnp::Encryptor::new(&ctx, data)
        .map_err(|_| ())?
        .cipher(rnp::Cipher::Aes256)
        .compression(rnp::Compression::Zlib, 6);
    for k in &recipients {
        builder = builder.add_recipient(k);
    }
    if let Some(k) = &signing {
        builder = builder.add_signature(k);
    }
    let mut output = rnp::Output::to_memory().map_err(|_| ())?;
    builder.build(&mut output).map_err(|_| ())?;
    let mut bytes = output.into_bytes().map_err(|_| ())?;
    hide_recipients(&mut bytes, &hidden_ids)?;
    String::from_utf8(rnp::armor_bytes(&bytes, rnp::ArmorType::Message).map_err(|_| ())?)
        .map_err(|_| ())
}

/// PKESK recipient identifiers aren't authenticated by OpenPGP. Zero only the hidden
/// recipients' v3 identifiers, before armoring; the encrypted session keys are untouched.
/// RNP decrypts these using its explicit ALLOW_HIDDEN_RECIPIENT option.
fn hide_recipients(bytes: &mut [u8], hidden: &[String]) -> Result<(), ()> {
    let mut remaining = hidden.to_vec();
    let mut offset = 0usize;
    while !remaining.is_empty() && offset < bytes.len() {
        let header = bytes[offset];
        offset += 1;
        if header & 0xc0 != 0xc0 {
            return Err(());
        }
        let tag = header & 63;
        if tag != 1 {
            break;
        }
        let first = *bytes.get(offset).ok_or(())?;
        offset += 1;
        let len = match first {
            0..=191 => first as usize,
            192..=223 => {
                let b = *bytes.get(offset).ok_or(())?;
                offset += 1;
                ((first as usize - 192) << 8) + b as usize + 192
            }
            255 => {
                let n = u32::from_be_bytes(
                    bytes
                        .get(offset..offset + 4)
                        .ok_or(())?
                        .try_into()
                        .map_err(|_| ())?,
                );
                offset += 4;
                n as usize
            }
            _ => return Err(()),
        };
        let end = offset.checked_add(len).ok_or(())?;
        let body = bytes.get_mut(offset..end).ok_or(())?;
        if body.len() < 10 || body[0] != 3 {
            return Err(());
        }
        let id = super::hex(&body[1..9]);
        if hidden.contains(&id) {
            body[1..9].fill(0);
            remaining.retain(|s| s != &id);
        }
        offset = end;
    }
    if remaining.is_empty() {
        Ok(())
    } else {
        Err(())
    }
}

#[derive(Clone)]
pub struct Checked {
    pub code: u32,
    pub key_id: String,
    pub primary: Option<String>,
    pub created: Option<u32>,
    pub packet: Vec<u8>,
}
fn signatures(result: &rnp::VerifyResult<'_>) -> Vec<Checked> {
    result
        .iter_signatures()
        .take(MAX_SIGNATURES)
        .map(|s| {
            let primary = s.key().ok().flatten().and_then(|k| {
                k.primary_fprint()
                    .ok()
                    .flatten()
                    .or_else(|| k.fingerprint().ok())
            });
            let packet = s
                .handle()
                .ok()
                .and_then(|h| h.export(ExportFlags::default()).ok())
                .unwrap_or_default();
            Checked {
                code: s.status_code(),
                key_id: s.keyid().unwrap_or_default().to_uppercase(),
                primary: primary.map(|s| s.to_uppercase()),
                created: s.times().ok().map(|(created, _)| created),
                packet,
            }
        })
        .collect()
}
pub fn check_detached(
    signature: &[u8],
    data: &[u8],
    keys: &[SignedPublicKey],
) -> Result<Vec<Checked>, ()> {
    if signature.len() > MAX_KEY {
        return Err(());
    }
    let ctx = Context::new().map_err(|_| ())?;
    for k in keys {
        ctx.load_keys(
            KeyringFormat::Gpg,
            k.bytes.as_slice(),
            LoadSaveFlags::PUBLIC,
        )
        .map_err(|_| ())?;
    }
    let op = rnp::VerifyOp::detached(&ctx, data, signature).map_err(|_| ())?;
    let (result, _) = op.execute_inspect();
    Ok(signatures(&result))
}

#[derive(Default)]
struct Buffer {
    data: Vec<u8>,
    full: bool,
}
struct Bounded(Arc<Mutex<Buffer>>);
impl Write for Bounded {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut buffer = self.0.lock().map_err(|_| io::Error::other("buffer lock"))?;
        if bytes.len() > MAX_PLAIN.saturating_sub(buffer.data.len()) {
            buffer.full = true;
            return Err(io::Error::other("plaintext limit"));
        }
        buffer.data.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub struct Plain {
    pub data: Vec<u8>,
    pub signatures: Vec<Checked>,
}
pub fn open(
    data: &[u8],
    keys: &[(&SignedSecretKey, &Password)],
    encrypted: bool,
) -> Result<Plain, super::Problem> {
    fn failed<E>(_: E) -> super::Problem {
        super::Problem::Failed
    }
    let ctx = Context::new().map_err(failed)?;
    for (key, password) in keys {
        ctx.load_keys(
            KeyringFormat::Gpg,
            key.bytes.as_slice(),
            LoadSaveFlags::PUBLIC | LoadSaveFlags::SECRET,
        )
        .map_err(failed)?;
        let primary = named(&ctx, &key.primary_key.fingerprint).map_err(failed)?;
        let _ = primary.unlock(Some(password.as_str()));
        for sub in primary.subkeys().map_err(failed)? {
            if sub.have_secret().unwrap_or(false) {
                let _ = sub.unlock(Some(password.as_str()));
            }
        }
    }
    let buffer = Arc::new(Mutex::new(Buffer::default()));
    let output = rnp::Output::to_writer(Bounded(buffer.clone())).map_err(failed)?;
    let mut op = rnp::VerifyOp::inline(&ctx, data, output).map_err(failed)?;
    op.set_flags(
        rnp::VerifyFlags::ALLOW_HIDDEN_RECIPIENT | rnp::VerifyFlags::IGNORE_SIGS_ON_DECRYPT,
    )
    .map_err(failed)?;
    let (result, status) = op.execute_inspect();
    if buffer.lock().map_err(failed)?.full {
        return Err(super::Problem::TooLarge);
    }
    if encrypted {
        let (mode, _, valid) = result.protection_info().map_err(failed)?;
        if mode == "cfb" || mode == "none" {
            return Err(super::Problem::Unsupported);
        }
        if !valid || status.is_err() {
            return Err(super::Problem::Failed);
        }
    } else if status.is_err() && result.signature_count().unwrap_or(0) == 0 {
        return Err(super::Problem::Failed);
    }
    let signatures = signatures(&result);
    let mut data = std::mem::take(&mut buffer.lock().map_err(failed)?.data);
    // RFC 4880 cleartext signatures exclude the separator's final line ending.
    if !encrypted {
        if data.ends_with(b"\r\n") {
            data.truncate(data.len() - 2);
        } else if data.ends_with(b"\n") {
            data.truncate(data.len() - 1);
        }
    }
    Ok(Plain { data, signatures })
}

pub fn recipients(data: &[u8]) -> Result<Vec<String>, super::Problem> {
    fn failed<E>(_: E) -> super::Problem {
        super::Problem::Failed
    }
    let ctx = Context::new().map_err(failed)?;
    let output = rnp::Output::to_null().map_err(failed)?;
    let op = rnp::VerifyOp::inline(&ctx, data, output).map_err(failed)?;
    let (result, _) = op.execute_inspect();
    let ids: Vec<String> = result
        .recipients()
        .map_err(failed)?
        .iter()
        .filter_map(|r| r.keyid().ok())
        .map(|s| s.to_uppercase())
        .take(256)
        .collect();
    if ids.is_empty() {
        return Err(if result.symenc_count().unwrap_or(0) > 0 {
            super::Problem::Unsupported
        } else {
            super::Problem::Failed
        });
    }
    Ok(ids)
}
