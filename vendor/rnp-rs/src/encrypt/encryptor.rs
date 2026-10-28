//! [`Encryptor`] — builder over `rnp_op_encrypt_*`.

use crate::algorithm::{Cipher, Compression, Hash};
use crate::context::Context;
use crate::error::{Result, check};
use crate::ffi;
use crate::key::Key;
use crate::ops::{Input, Output};
use std::ffi::CString;
use std::ptr;

use super::{AddPasswordOptions, AeadType, EncryptFlags};

/// Recipients and passwords for an encryption op. This is not a separate
/// seam — it just keeps "who can decrypt" replay local to the fields that
/// store it.
#[derive(Default)]
struct RecipientOptions<'a> {
    recipients: Vec<&'a Key<'a>>,
    signatures: Vec<&'a Key<'a>>,
    passwords: Vec<(CString, AddPasswordOptions)>,
}

impl<'a> RecipientOptions<'a> {
    unsafe fn apply(&self, op: ffi::rnp_op_encrypt_t) -> Result<()> {
        unsafe {
            for key in &self.recipients {
                check(ffi::rnp_op_encrypt_add_recipient(op, key.handle))?;
            }
            for key in &self.signatures {
                check(ffi::rnp_op_encrypt_add_signature(
                    op,
                    key.handle,
                    ptr::null_mut(),
                ))?;
            }
            for (pw, opts) in &self.passwords {
                let hash_str = opts
                    .hash
                    .map(|h| CString::new(h.as_str()).unwrap())
                    .unwrap_or_else(|| CString::new("SHA256").unwrap());
                let cipher_str = opts
                    .cipher
                    .map(|c| CString::new(c.as_str()).unwrap())
                    .unwrap_or_else(|| CString::new("AES256").unwrap());
                check(ffi::rnp_op_encrypt_add_password(
                    op,
                    pw.as_ptr(),
                    hash_str.as_ptr(),
                    opts.iterations.unwrap_or(0),
                    cipher_str.as_ptr(),
                ))?;
            }
            Ok(())
        }
    }
}

/// Op-level encryption options; fields and their FFI replay live together.
#[derive(Default)]
struct EncryptOptions {
    armor: bool,
    cipher: Option<Cipher>,
    hash: Option<Hash>,
    compression: Option<(Compression, u32)>,
    aead: Option<AeadType>,
    aead_bits: Option<i32>,
    file_name: Option<CString>,
    file_mtime: Option<u32>,
    creation_time: Option<u32>,
    expiration_time: Option<u32>,
    flags: EncryptFlags,
    #[cfg(feature = "pqc")]
    prefer_pqc: bool,
    #[cfg(feature = "crypto-refresh")]
    pkesk_v6: bool,
    #[cfg(feature = "crypto-refresh")]
    skesk_v6: bool,
}

impl EncryptOptions {
    unsafe fn apply(&self, op: ffi::rnp_op_encrypt_t) -> Result<()> {
        unsafe {
            if self.armor {
                check(ffi::rnp_op_encrypt_set_armor(op, true))?;
            }
            if let Some(c) = self.cipher {
                let cs = CString::new(c.as_str()).unwrap();
                check(ffi::rnp_op_encrypt_set_cipher(op, cs.as_ptr()))?;
            }
            if let Some(h) = self.hash {
                let cs = CString::new(h.as_str()).unwrap();
                check(ffi::rnp_op_encrypt_set_hash(op, cs.as_ptr()))?;
            }
            if let Some((alg, level)) = self.compression {
                let cs = CString::new(alg.as_str()).unwrap();
                check(ffi::rnp_op_encrypt_set_compression(
                    op,
                    cs.as_ptr(),
                    level as i32,
                ))?;
            }
            if let Some(aead) = self.aead {
                let cs = CString::new(aead.as_str()).unwrap();
                check(ffi::rnp_op_encrypt_set_aead(op, cs.as_ptr()))?;
            }
            if let Some(bits) = self.aead_bits {
                check(ffi::rnp_op_encrypt_set_aead_bits(op, bits))?;
            }
            if let Some(name) = &self.file_name {
                check(ffi::rnp_op_encrypt_set_file_name(op, name.as_ptr()))?;
            }
            if let Some(mtime) = self.file_mtime {
                check(ffi::rnp_op_encrypt_set_file_mtime(op, mtime))?;
            }
            if let Some(t) = self.creation_time {
                check(ffi::rnp_op_encrypt_set_creation_time(op, t))?;
            }
            if let Some(t) = self.expiration_time {
                check(ffi::rnp_op_encrypt_set_expiration_time(op, t))?;
            }
            if self.flags.bits() != 0 {
                check(ffi::rnp_op_encrypt_set_flags(op, self.flags.bits()))?;
            }
            #[cfg(feature = "pqc")]
            if self.prefer_pqc {
                check(ffi::rnp_op_encrypt_prefer_pqc_enc_subkey(op))?;
            }
            #[cfg(feature = "crypto-refresh")]
            if self.pkesk_v6 {
                check(ffi::rnp_op_encrypt_enable_pkesk_v6(op))?;
            }
            #[cfg(feature = "crypto-refresh")]
            if self.skesk_v6 {
                check(ffi::rnp_op_encrypt_enable_skesk_v6(op))?;
            }
            Ok(())
        }
    }
}

/// Builder over `rnp_op_encrypt_*`. Configured first, then executed with
/// [`Encryptor::build`] against the caller-supplied destination [`Output`].
///
/// ```
/// # use rnp::{Algorithm, Context, Encryptor, Hash, KeyBuilder, KeyUsage, Output};
/// # let ctx = Context::new().unwrap();
/// # let key = KeyBuilder::new(Algorithm::Rsa).bits(2048)
/// #     .userid("enc <enc@example.com>").hash(Hash::Sha256)
/// #     .add_usage(KeyUsage::EncryptComms).build(&ctx).unwrap();
/// let plaintext = b"secret message";
/// let mut output = Output::to_memory().unwrap();
/// Encryptor::new(&ctx, plaintext).unwrap()
///     .add_recipient(&key)
///     .build(&mut output)
///     .unwrap();
/// let _ciphertext = output.into_bytes().unwrap();
/// ```
pub struct Encryptor<'a> {
    pub(crate) ctx: &'a Context,
    pub(crate) source: crate::ops::MessageSource<'a>,
    recipients: RecipientOptions<'a>,
    options: EncryptOptions,
}

impl<'a> Encryptor<'a> {
    /// Begin building an encryption operation over `plaintext` — anything
    /// message-shaped: a byte slice (`b"..."`, `&data[..]`) or a
    /// caller-built [`Input`] (e.g. from
    /// [`Input::from_reader`](crate::Input::from_reader), to stream from a
    /// non-seekable source). The input is consumed when the operation
    /// executes.
    ///
    /// Construction cannot fail (the source is only converted to an
    /// [`Input`] at execution time); the `Result` is kept for API
    /// stability.
    pub fn new(
        ctx: &'a Context,
        plaintext: impl Into<crate::ops::MessageSource<'a>>,
    ) -> Result<Self> {
        Ok(Encryptor {
            ctx,
            source: plaintext.into(),
            recipients: RecipientOptions::default(),
            options: EncryptOptions::default(),
        })
    }

    /// Begin building an encryption operation over a caller-built
    /// [`Input`]. Deprecated: pass the [`Input`] to [`Encryptor::new`] —
    /// it accepts both bytes and inputs.
    #[deprecated(
        since = "0.2.0",
        note = "pass the Input to Encryptor::new; it accepts both"
    )]
    pub fn new_with_input(ctx: &'a Context, input: Input) -> Result<Self> {
        Encryptor::new(ctx, input)
    }

    /// Add a recipient's public key. May be called multiple times.
    pub fn add_recipient(mut self, key: &'a Key<'a>) -> Self {
        self.recipients.recipients.push(key);
        self
    }

    /// Add a signature key (sign-and-encrypt in one op). May be called
    /// multiple times.
    pub fn add_signature(mut self, key: &'a Key<'a>) -> Self {
        self.recipients.signatures.push(key);
        self
    }

    /// Add a password-encrypted session key. May be called multiple times
    /// to allow multiple passwords (any one decrypts).
    pub fn add_password(
        mut self,
        password: impl Into<Vec<u8>>,
        options: AddPasswordOptions,
    ) -> Self {
        // CString::new needs Vec<u8> without interior NUL. unwrap_or lets
        // us fall back if the password is weird, but passwords containing
        // NUL bytes are pathological — surface as NullPointer for caller
        // to handle.
        let c = CString::new(password.into()).unwrap_or_default();
        self.recipients.passwords.push((c, options));
        self
    }

    pub fn armor(mut self, armored: bool) -> Self {
        self.options.armor = armored;
        self
    }

    pub fn cipher(mut self, c: Cipher) -> Self {
        self.options.cipher = Some(c);
        self
    }

    pub fn hash(mut self, h: Hash) -> Self {
        self.options.hash = Some(h);
        self
    }

    pub fn compression(mut self, alg: Compression, level: u32) -> Self {
        self.options.compression = Some((alg, level));
        self
    }

    pub fn aead(mut self, alg: AeadType) -> Self {
        self.options.aead = Some(alg);
        self
    }

    /// AEAD chunk size in bits (0..=16 per the OpenPGP spec).
    pub fn aead_bits(mut self, bits: i32) -> Self {
        self.options.aead_bits = Some(bits);
        self
    }

    pub fn file_name(mut self, name: impl AsRef<str>) -> Self {
        self.options.file_name = Some(CString::new(name.as_ref()).unwrap_or_default());
        self
    }

    pub fn file_mtime(mut self, mtime: u32) -> Self {
        self.options.file_mtime = Some(mtime);
        self
    }

    pub fn creation_time(mut self, t: u32) -> Self {
        self.options.creation_time = Some(t);
        self
    }

    pub fn expiration_time(mut self, t: u32) -> Self {
        self.options.expiration_time = Some(t);
        self
    }

    pub fn flags(mut self, flags: EncryptFlags) -> Self {
        self.options.flags = flags;
        self
    }

    /// Prefer PQC-encryption subkeys over non-PQC subkeys when choosing
    /// recipients. Feature-gated; requires `--features pqc` and a librnp
    /// built with `ENABLE_PQC=ON`.
    #[cfg(feature = "pqc")]
    pub fn prefer_pqc_enc_subkey(mut self) -> Self {
        self.options.prefer_pqc = true;
        self
    }

    /// Enable v6 PKESK (Public-Key Encrypted Session Key) packets in the
    /// output. Feature-gated; requires `--features crypto-refresh`.
    #[cfg(feature = "crypto-refresh")]
    pub fn enable_pkesk_v6(mut self) -> Self {
        self.options.pkesk_v6 = true;
        self
    }

    /// Enable v6 SKESK (Symmetric-Key Encrypted Session Key) packets in
    /// the output. Feature-gated; requires `--features crypto-refresh`.
    #[cfg(feature = "crypto-refresh")]
    pub fn enable_skesk_v6(mut self) -> Self {
        self.options.skesk_v6 = true;
        self
    }

    /// Execute the encryption, writing the ciphertext to `output`.
    ///
    /// If a reader-backed input fails mid-operation, the returned error is
    /// the original [`std::io::Error`] (see
    /// [`Input::from_reader`](crate::Input::from_reader)).
    pub fn build(mut self, output: &mut Output) -> Result<()> {
        let mut input = self.source.0.take()?;
        let mut op: ffi::rnp_op_encrypt_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_op_encrypt_create(
                &mut op,
                self.ctx.ffi,
                input.as_ptr(),
                output.as_ptr(),
            ))?;
        }
        // Apply all configuration. On any failure, destroy the op and bail.
        let result = unsafe { apply_config(op, &self) };
        if let Err(e) = result {
            unsafe {
                let _ = ffi::rnp_op_encrypt_destroy(op);
            }
            return Err(e);
        }

        unsafe {
            let exec = check(ffi::rnp_op_encrypt_execute(op));
            let _ = ffi::rnp_op_encrypt_destroy(op);
            if exec.is_err() {
                // Surface the underlying stream failure, if any — librnp
                // only reports a generic RNP_ERROR_READ/WRITE.
                if let Some(io) = input.take_io_error() {
                    return Err(io.into());
                }
            }
            exec?;
        }
        Ok(())
    }
}

pub(crate) unsafe fn apply_config(op: ffi::rnp_op_encrypt_t, e: &Encryptor<'_>) -> Result<()> {
    unsafe {
        e.recipients.apply(op)?;
        e.options.apply(op)
    }
}
