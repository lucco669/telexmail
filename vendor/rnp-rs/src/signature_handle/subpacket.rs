//! [`Subpacket`] handle and [`SubpacketType`] enum — RFC 4880 §5.2.3.1.

use crate::error::{Result, check};
use crate::ffi;
use crate::ffi_safe::call_for_owned_bytes;
use std::ptr;

/// Borrowed handle to a subpacket on a [`super::Signature`].
pub struct Subpacket {
    pub(crate) handle: ffi::rnp_sig_subpacket_t,
}

impl Subpacket {
    pub(crate) fn from_handle(handle: ffi::rnp_sig_subpacket_t) -> Self {
        Subpacket { handle }
    }

    /// Raw subpacket type byte. Prefer [`Self::typ_enum`] for the typed
    /// view.
    pub fn typ_raw(&self) -> Result<u8> {
        let mut t: u8 = 0;
        let mut hashed: bool = false;
        let mut critical: bool = false;
        unsafe {
            check(ffi::rnp_signature_subpacket_info(
                self.handle,
                &mut t,
                &mut hashed,
                &mut critical,
            ))?
        };
        Ok(t)
    }

    /// Subpacket type, mapped to a known variant where possible.
    pub fn typ_enum(&self) -> Result<SubpacketType> {
        Ok(SubpacketType::from_u8(self.typ_raw()?))
    }

    /// Whether the subpacket is in the hashed area (cryptographically
    /// protected) or the unhashed area (informational).
    pub fn is_hashed(&self) -> Result<bool> {
        self.info().map(|(_, hashed, _)| hashed)
    }

    /// Whether the subpacket has the critical bit set.
    pub fn is_critical(&self) -> Result<bool> {
        self.info().map(|(_, _, critical)| critical)
    }

    /// Raw subpacket body bytes.
    pub fn data(&self) -> Result<Vec<u8>> {
        call_for_owned_bytes(|ptr, len| unsafe {
            ffi::rnp_signature_subpacket_data(self.handle, ptr, len)
        })
    }

    // Internal: pull all three info fields at once. Cheaper than three
    // separate calls when the caller wants more than one.
    fn info(&self) -> Result<(u8, bool, bool)> {
        let mut t: u8 = 0;
        let mut hashed: bool = false;
        let mut critical: bool = false;
        unsafe {
            check(ffi::rnp_signature_subpacket_info(
                self.handle,
                &mut t,
                &mut hashed,
                &mut critical,
            ))?
        };
        Ok((t, hashed, critical))
    }
}

impl Drop for Subpacket {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                let _ = ffi::rnp_signature_subpacket_destroy(self.handle);
            }
            self.handle = ptr::null_mut();
        }
    }
}

/// Signature subpacket type per RFC 4880 §5.2.3.1. Covers the commonly-
/// encountered tags; the `Other(u8)` variant preserves forward
/// compatibility with new tags added by future RFCs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
#[non_exhaustive]
pub enum SubpacketType {
    /// Signed signature creation time (tag 2).
    SignatureCreationTime = 2,
    /// Signed signature expiration time (tag 3).
    SignatureExpirationTime = 3,
    /// Exportable certification flag (tag 4).
    ExportableCertification = 4,
    /// Trust signature (tag 5).
    TrustSignature = 5,
    /// Regular expression (tag 6).
    RegularExpression = 6,
    /// Revocable flag (tag 7).
    Revocable = 7,
    /// Key expiration time (tag 9).
    KeyExpirationTime = 9,
    /// Preferred symmetric algorithms (tag 11).
    PreferredSymmetricAlgorithms = 11,
    /// Revocation reason (tag 12).
    RevocationReason = 12,
    /// Signer's key id (tag 16).
    IssuerKeyId = 16,
    /// Preferred hash algorithms (tag 21).
    PreferredHashAlgorithms = 21,
    /// Preferred compression algorithms (tag 22).
    PreferredCompressionAlgorithms = 22,
    /// Key server preferences (tag 23).
    KeyServerPreferences = 23,
    /// Preferred key server (tag 24).
    PreferredKeyServer = 24,
    /// Primary user id flag (tag 25).
    PrimaryUserId = 25,
    /// Policy URI (tag 26).
    PolicyUri = 26,
    /// Key flags (tag 27).
    KeyFlags = 27,
    /// Signer's user id (tag 28).
    SignerUserId = 28,
    /// Reason for revocation (tag 29).
    ReasonForRevocation = 29,
    /// Features bitmask (tag 30).
    Features = 30,
    /// Fingerprint of the signer (tag 33).
    IssuerFingerprint = 33,
    /// Catch-all for tags not yet modelled. Forward-compatible: new RFCs
    /// adding subpacket tags surface here without breaking the enum.
    Other(u8),
}

impl SubpacketType {
    pub fn as_u8(self) -> u8 {
        match self {
            SubpacketType::Other(raw) => raw,
            v => {
                use SubpacketType::*;
                match v {
                    SignatureCreationTime => 2,
                    SignatureExpirationTime => 3,
                    ExportableCertification => 4,
                    TrustSignature => 5,
                    RegularExpression => 6,
                    Revocable => 7,
                    KeyExpirationTime => 9,
                    PreferredSymmetricAlgorithms => 11,
                    RevocationReason => 12,
                    IssuerKeyId => 16,
                    PreferredHashAlgorithms => 21,
                    PreferredCompressionAlgorithms => 22,
                    KeyServerPreferences => 23,
                    PreferredKeyServer => 24,
                    PrimaryUserId => 25,
                    PolicyUri => 26,
                    KeyFlags => 27,
                    SignerUserId => 28,
                    ReasonForRevocation => 29,
                    Features => 30,
                    IssuerFingerprint => 33,
                    Other(_) => unreachable!(),
                }
            }
        }
    }

    /// Map a raw u8 to a known variant, falling back to a typed `Other`
    /// wrapper for unrecognized tags.
    pub fn from_u8(raw: u8) -> Self {
        match raw {
            2 => Self::SignatureCreationTime,
            3 => Self::SignatureExpirationTime,
            4 => Self::ExportableCertification,
            5 => Self::TrustSignature,
            6 => Self::RegularExpression,
            7 => Self::Revocable,
            9 => Self::KeyExpirationTime,
            11 => Self::PreferredSymmetricAlgorithms,
            12 => Self::RevocationReason,
            16 => Self::IssuerKeyId,
            21 => Self::PreferredHashAlgorithms,
            22 => Self::PreferredCompressionAlgorithms,
            23 => Self::KeyServerPreferences,
            24 => Self::PreferredKeyServer,
            25 => Self::PrimaryUserId,
            26 => Self::PolicyUri,
            27 => Self::KeyFlags,
            28 => Self::SignerUserId,
            29 => Self::ReasonForRevocation,
            30 => Self::Features,
            33 => Self::IssuerFingerprint,
            _ => Self::Other(raw),
        }
    }
}

#[allow(dead_code)]
fn _subpacket_type_exhaustive(t: SubpacketType) -> u8 {
    t.as_u8()
}
