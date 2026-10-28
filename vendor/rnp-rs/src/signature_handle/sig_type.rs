//! [`SignatureType`] — typed view of RFC 4880 §5.2.1 signature types.

/// Signature type per RFC 4880 §5.2.1. The C side returns these as
/// human-readable strings; this enum lets callers `match` instead of
/// string-compare.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SignatureType {
    Binary,
    Text,
    Standalone,
    CertificationGeneric,
    CertificationPersona,
    CertificationCasual,
    CertificationPositive,
    SubkeyBinding,
    PrimaryKeyBinding,
    Direct,
    KeyRevocation,
    SubkeyRevocation,
    CertificationRevocation,
    Timestamp,
    ThirdParty,
    Unknown(u32),
}

impl SignatureType {
    pub fn parse(s: &str) -> Self {
        match s {
            "binary" => Self::Binary,
            "text" => Self::Text,
            "standalone" => Self::Standalone,
            "certification (generic)" => Self::CertificationGeneric,
            "certification (persona)" => Self::CertificationPersona,
            "certification (casual)" => Self::CertificationCasual,
            "certification (positive)" => Self::CertificationPositive,
            "subkey binding" => Self::SubkeyBinding,
            "primary key binding" => Self::PrimaryKeyBinding,
            "direct" => Self::Direct,
            "key revocation" => Self::KeyRevocation,
            "subkey revocation" => Self::SubkeyRevocation,
            "certification revocation" => Self::CertificationRevocation,
            "timestamp" => Self::Timestamp,
            "third-party" => Self::ThirdParty,
            other => {
                if let Some(rest) = other.strip_prefix("unknown: ")
                    && let Ok(n) = rest.trim().parse::<u32>()
                {
                    return Self::Unknown(n);
                }
                Self::Unknown(0)
            }
        }
    }

    pub fn as_str(&self) -> String {
        match self {
            Self::Binary => "binary".to_string(),
            Self::Text => "text".to_string(),
            Self::Standalone => "standalone".to_string(),
            Self::CertificationGeneric => "certification (generic)".to_string(),
            Self::CertificationPersona => "certification (persona)".to_string(),
            Self::CertificationCasual => "certification (casual)".to_string(),
            Self::CertificationPositive => "certification (positive)".to_string(),
            Self::SubkeyBinding => "subkey binding".to_string(),
            Self::PrimaryKeyBinding => "primary key binding".to_string(),
            Self::Direct => "direct".to_string(),
            Self::KeyRevocation => "key revocation".to_string(),
            Self::SubkeyRevocation => "subkey revocation".to_string(),
            Self::CertificationRevocation => "certification revocation".to_string(),
            Self::Timestamp => "timestamp".to_string(),
            Self::ThirdParty => "third-party".to_string(),
            Self::Unknown(n) => format!("unknown: {n}"),
        }
    }
}
