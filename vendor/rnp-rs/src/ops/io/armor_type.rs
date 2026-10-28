//! [`ArmorType`] — armor stream type for `rnp_enarmor()`.

/// Armor stream type. See `rnp_enarmor()` for the canonical string values.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum ArmorType {
    /// `"message"` — the default.
    Message,
    /// `"public key"`.
    PublicKey,
    /// `"secret key"`.
    SecretKey,
    /// `"signature"`.
    Signature,
    /// `"cleartext signed message"`.
    Cleartext,
}

impl ArmorType {
    pub fn as_str(self) -> &'static str {
        match self {
            ArmorType::Message => "message",
            ArmorType::PublicKey => "public key",
            ArmorType::SecretKey => "secret key",
            ArmorType::Signature => "signature",
            ArmorType::Cleartext => "cleartext signed message",
        }
    }
}
