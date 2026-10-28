//! [`KeyBuilder`] — builder over `rnp_op_generate_*` for primary keys.

use crate::algorithm::{Algorithm, Cipher, Compression, Curve, Hash, KeyUsage};
use crate::context::Context;
use crate::error::{self, Result, check};
use crate::ffi;
use crate::key::Key;
use std::ffi::CString;
use std::ptr;

#[cfg(feature = "pqc")]
use super::PqcAlgorithm;
use super::ProtectConfig;
use super::SubkeyBuilder;

/// Builder over `rnp_op_generate_*` for primary keys.
///
/// Terminal method [`KeyBuilder::build`] consumes `self`, executes the
/// generation, and returns the resulting [`Key`].
///
/// ```
/// # use rnp::{Algorithm, Context, Hash, KeyBuilder, KeyUsage};
/// # let ctx = Context::new().unwrap();
/// let key = KeyBuilder::new(Algorithm::Rsa)
///     .bits(2048)
///     .userid("alice <alice@example.com>")
///     .hash(Hash::Sha256)
///     .add_usage(KeyUsage::Sign)
///     .add_usage(KeyUsage::Certify)
///     .build(&ctx)
///     .unwrap();
/// ```
pub struct KeyBuilder {
    pub(crate) alg: Algorithm,
    pub(crate) bits: Option<u32>,
    pub(crate) hash: Option<Hash>,
    pub(crate) dsa_qbits: Option<u32>,
    pub(crate) curve: Option<Curve>,
    pub(crate) userid: Option<String>,
    pub(crate) expiration: Option<u32>,
    pub(crate) usages: Vec<KeyUsage>,
    pub(crate) pref_hashes: Vec<Hash>,
    pub(crate) pref_ciphers: Vec<Cipher>,
    pub(crate) pref_compressions: Vec<Compression>,
    pub(crate) pref_keyserver: Option<String>,
    pub(crate) protection: Option<ProtectConfig>,
    pub(crate) request_password: bool,
    pub(crate) subkeys: Vec<SubkeyBuilder>,
    #[cfg(feature = "crypto-refresh")]
    pub(crate) v6: bool,
}

impl KeyBuilder {
    pub fn new(alg: Algorithm) -> Self {
        KeyBuilder {
            alg,
            bits: None,
            hash: None,
            dsa_qbits: None,
            curve: None,
            userid: None,
            expiration: None,
            usages: Vec::new(),
            pref_hashes: Vec::new(),
            pref_ciphers: Vec::new(),
            pref_compressions: Vec::new(),
            pref_keyserver: None,
            protection: None,
            request_password: false,
            subkeys: Vec::new(),
            #[cfg(feature = "crypto-refresh")]
            v6: false,
        }
    }

    pub fn bits(mut self, n: u32) -> Self {
        self.bits = Some(n);
        self
    }

    pub fn hash(mut self, h: Hash) -> Self {
        self.hash = Some(h);
        self
    }

    pub fn dsa_qbits(mut self, n: u32) -> Self {
        self.dsa_qbits = Some(n);
        self
    }

    pub fn curve(mut self, c: Curve) -> Self {
        self.curve = Some(c);
        self
    }

    pub fn userid(mut self, uid: impl Into<String>) -> Self {
        self.userid = Some(uid.into());
        self
    }

    pub fn expiration(mut self, seconds: u32) -> Self {
        self.expiration = Some(seconds);
        self
    }

    pub fn add_usage(mut self, u: KeyUsage) -> Self {
        self.usages.push(u);
        self
    }

    pub fn add_pref_hash(mut self, h: Hash) -> Self {
        self.pref_hashes.push(h);
        self
    }

    pub fn add_pref_cipher(mut self, c: Cipher) -> Self {
        self.pref_ciphers.push(c);
        self
    }

    pub fn add_pref_compression(mut self, c: Compression) -> Self {
        self.pref_compressions.push(c);
        self
    }

    /// Clear all key usage flags. Equivalent to
    /// `rnp_op_generate_clear_usage`: the builder defers every setter to
    /// build time, so clearing the local vector is exactly the C-side
    /// clear.
    pub fn clear_usage(mut self) -> Self {
        self.usages.clear();
        self
    }

    /// Clear all preferred hashes. Equivalent to
    /// `rnp_op_generate_clear_pref_hash` (see [`Self::clear_usage`] for why
    /// the local vector is cleared instead of the C-side state).
    pub fn clear_pref_hash(mut self) -> Self {
        self.pref_hashes.clear();
        self
    }

    /// Clear all preferred ciphers. Equivalent to
    /// `rnp_op_generate_clear_pref_cipher` (see [`Self::clear_usage`]).
    pub fn clear_pref_cipher(mut self) -> Self {
        self.pref_ciphers.clear();
        self
    }

    /// Clear all preferred compressions.
    /// Equivalent to `rnp_op_generate_clear_pref_compression`
    /// (see [`Self::clear_usage`]).
    pub fn clear_pref_compression(mut self) -> Self {
        self.pref_compressions.clear();
        self
    }

    pub fn pref_keyserver(mut self, s: impl Into<String>) -> Self {
        self.pref_keyserver = Some(s.into());
        self
    }

    /// Apply protection (encryption of secret material) at generation
    /// time. Takes the same [`ProtectOptions`] type used by
    /// [`Key::protect`](crate::Key::protect) — single canonical config.
    ///
    /// [`ProtectOptions`]: crate::key::ProtectOptions
    pub fn protection(mut self, opts: &crate::key::ProtectOptions) -> Self {
        self.protection = Some(ProtectConfig::from_options(opts));
        self
    }

    /// Have librnp ask the configured password provider for the protection
    /// password at execution time (instead of supplying it inline via
    /// [`Self::protection`]).
    pub fn request_password(mut self) -> Self {
        self.request_password = true;
        self
    }

    /// Generate a v6 (RFC 9580) primary key instead of v4. Requires the
    /// `crypto-refresh` Cargo feature and a librnp built with
    /// `ENABLE_CRYPTO_REFRESH=ON`.
    #[cfg(feature = "crypto-refresh")]
    pub fn v6(mut self) -> Self {
        self.v6 = true;
        self
    }

    /// Add a subkey to be generated alongside the primary in
    /// [`KeyBuilder::build`].
    ///
    /// The subkey is created after the primary key, with a binding
    /// signature. This is the idiomatic way to build a composite keypair
    /// (primary + signing subkey + encryption subkey) in one call.
    ///
    /// ```
    /// # use rnp::{Algorithm, KeyBuilder, KeyUsage, SubkeyBuilder};
    /// KeyBuilder::new(Algorithm::Rsa)
    ///     .bits(2048)
    ///     .userid("alice")
    ///     .add_subkey(SubkeyBuilder::new(Algorithm::Rsa)
    ///         .bits(2048)
    ///         .add_usage(KeyUsage::EncryptComms))
    ///     .add_subkey(SubkeyBuilder::new(Algorithm::Eddsa)
    ///         .add_usage(KeyUsage::Sign));
    /// ```
    pub fn add_subkey(mut self, sub: SubkeyBuilder) -> Self {
        self.subkeys.push(sub);
        self
    }

    /// Add a PQC encryption subkey (e.g. ML-KEM-768+X25519).
    /// Requires the `pqc` Cargo feature.
    #[cfg(feature = "pqc")]
    pub fn add_pqc_encryption_subkey(self, alg: PqcAlgorithm) -> Self {
        self.add_subkey(SubkeyBuilder::new_str(alg.as_str()).add_usage(KeyUsage::EncryptComms))
    }

    /// Add a PQC signing subkey (e.g. ML-DSA-65+ED25519).
    /// Requires the `pqc` Cargo feature.
    #[cfg(feature = "pqc")]
    pub fn add_pqc_signing_subkey(self, alg: PqcAlgorithm) -> Self {
        self.add_subkey(SubkeyBuilder::new_str(alg.as_str()).add_usage(KeyUsage::Sign))
    }

    /// Execute the generation. The returned [`Key`] borrows `ctx` for its
    /// lifetime (matching the rest of the crate's lifetime discipline).
    pub fn build<'ctx>(self, ctx: &'ctx Context) -> Result<Key<'ctx>> {
        let alg_c = CString::new(self.alg.as_str()).unwrap();
        let mut op: ffi::rnp_op_generate_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_op_generate_create(
                &mut op,
                ctx.ffi,
                alg_c.as_ptr(),
            ))?;
        }

        let setters = unsafe { apply_setters(op, &self) };
        if let Err(e) = setters {
            unsafe {
                let _ = ffi::rnp_op_generate_destroy(op);
            }
            return Err(e);
        }

        let primary_handle = unsafe {
            check(ffi::rnp_op_generate_execute(op))?;
            let mut handle: ffi::rnp_key_handle_t = ptr::null_mut();
            check(ffi::rnp_op_generate_get_key(op, &mut handle))?;
            let _ = ffi::rnp_op_generate_destroy(op);
            if handle.is_null() {
                return Err(error::Error::NullPointer);
            }
            handle
        };

        // Create subkeys inline.
        for sub_spec in self.subkeys {
            sub_spec.build_with_ffi(ctx, primary_handle)?;
        }

        Ok(Key::from_handle(primary_handle))
    }
}

/// Apply all setters from `b` to the op handle.
unsafe fn apply_setters(op: ffi::rnp_op_generate_t, b: &KeyBuilder) -> Result<()> {
    unsafe {
        super::apply_generate_common(
            op,
            super::GenerateCommon {
                bits: b.bits,
                hash: b.hash,
                dsa_qbits: b.dsa_qbits,
                curve: b.curve,
                expiration: b.expiration,
                usages: &b.usages,
                protection: b.protection.as_ref(),
                request_password: b.request_password,
            },
        )?;
        if let Some(uid) = &b.userid {
            let c = CString::new(uid.as_str()).map_err(|_| error::Error::NulByte)?;
            check(ffi::rnp_op_generate_set_userid(op, c.as_ptr()))?;
        }
        for h in &b.pref_hashes {
            let c = CString::new(h.as_str()).unwrap();
            check(ffi::rnp_op_generate_add_pref_hash(op, c.as_ptr()))?;
        }
        for c2 in &b.pref_ciphers {
            let c = CString::new(c2.as_str()).unwrap();
            check(ffi::rnp_op_generate_add_pref_cipher(op, c.as_ptr()))?;
        }
        for cz in &b.pref_compressions {
            let c = CString::new(cz.as_str()).unwrap();
            check(ffi::rnp_op_generate_add_pref_compression(op, c.as_ptr()))?;
        }
        if let Some(ks) = &b.pref_keyserver {
            let c = CString::new(ks.as_str()).map_err(|_| error::Error::NulByte)?;
            check(ffi::rnp_op_generate_set_pref_keyserver(op, c.as_ptr()))?;
        }
        #[cfg(feature = "crypto-refresh")]
        if b.v6 {
            check(ffi::rnp_op_generate_set_v6_key(op))?;
        }
        Ok(())
    }
}
