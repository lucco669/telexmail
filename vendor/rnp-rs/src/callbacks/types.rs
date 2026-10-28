//! `RequestedKeyType` + `KeyRequestOutcome` enums.

/// Why the key-provider is being called.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestedKeyType {
    Keyid,
    Fingerprint,
    Grip,
}

impl RequestedKeyType {
    pub fn as_str(self) -> &'static str {
        match self {
            RequestedKeyType::Keyid => "keyid",
            RequestedKeyType::Fingerprint => "fingerprint",
            RequestedKeyType::Grip => "grip",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "keyid" => Self::Keyid,
            "fingerprint" => Self::Fingerprint,
            "grip" => Self::Grip,
            _ => Self::Keyid,
        }
    }
}

/// What the key-provider returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyRequestOutcome {
    /// The key was loaded into the keyring via `Context::load_keys` (or
    /// similar) inside the callback.
    Found,
    /// The callback couldn't find the key. librnp will try the next
    /// identifier (e.g. fall back from keyid to fingerprint).
    NotFound,
}
