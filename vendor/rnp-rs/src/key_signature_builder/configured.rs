//! [`ConfiguredBuilder`] — the post-setter-chain state. Terminal is
//! `SignatureSetterOps::sign`.

use crate::error::Result;
use crate::key::{Key, RevocationReason};
use crate::keygen::{Cipher, Compression, Hash};
use crate::signature_handle::Signature;
use std::marker::PhantomData;

use super::inner::{SignatureBuilderInner, SignatureSetterOps};

/// Configured builder: holds the in-progress signature after the shared
/// setter chain has been applied. Terminal is [`Self::sign`].
pub struct ConfiguredBuilder<'a> {
    pub(crate) inner: SignatureBuilderInner,
    pub(crate) _phantom: PhantomData<&'a ()>,
}

impl<'a> SignatureSetterOps for ConfiguredBuilder<'a> {
    fn hash(self, h: Hash) -> Self {
        ConfiguredBuilder {
            inner: self.inner.hash(h),
            _phantom: PhantomData,
        }
    }
    fn creation(self, t: u32) -> Self {
        ConfiguredBuilder {
            inner: self.inner.creation(t),
            _phantom: PhantomData,
        }
    }
    fn key_flags(self, flags: u32) -> Self {
        ConfiguredBuilder {
            inner: self.inner.key_flags(flags),
            _phantom: PhantomData,
        }
    }
    fn key_expiration(self, seconds: u32) -> Self {
        ConfiguredBuilder {
            inner: self.inner.key_expiration(seconds),
            _phantom: PhantomData,
        }
    }
    fn features(self, bits: u32) -> Self {
        ConfiguredBuilder {
            inner: self.inner.features(bits),
            _phantom: PhantomData,
        }
    }
    fn add_preferred_cipher(self, c: Cipher) -> Self {
        ConfiguredBuilder {
            inner: self.inner.add_preferred_cipher(c),
            _phantom: PhantomData,
        }
    }
    fn add_preferred_hash(self, h: Hash) -> Self {
        ConfiguredBuilder {
            inner: self.inner.add_preferred_hash(h),
            _phantom: PhantomData,
        }
    }
    fn add_preferred_compression(self, c: Compression) -> Self {
        ConfiguredBuilder {
            inner: self.inner.add_preferred_compression(c),
            _phantom: PhantomData,
        }
    }
    fn primary_uid(self, primary: bool) -> Self {
        ConfiguredBuilder {
            inner: self.inner.primary_uid(primary),
            _phantom: PhantomData,
        }
    }
    fn key_server(self, url: impl Into<String>) -> Self {
        ConfiguredBuilder {
            inner: self.inner.key_server(url),
            _phantom: PhantomData,
        }
    }
    fn key_server_prefs(self, prefs: u8) -> Self {
        ConfiguredBuilder {
            inner: self.inner.key_server_prefs(prefs),
            _phantom: PhantomData,
        }
    }
    fn revocation_reason(self, reason: RevocationReason) -> Self {
        ConfiguredBuilder {
            inner: self.inner.revocation_reason(reason),
            _phantom: PhantomData,
        }
    }
    fn revoker(self, revoker: &Key<'_>) -> Self {
        ConfiguredBuilder {
            inner: self.inner.revoker(revoker),
            _phantom: PhantomData,
        }
    }
    fn trust_level(self, level: u8, amount: u8) -> Self {
        ConfiguredBuilder {
            inner: self.inner.trust_level(level, amount),
            _phantom: PhantomData,
        }
    }
    fn sign(self) -> Result<Signature<'static>> {
        self.inner.sign()
    }
}
