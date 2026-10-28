//! Key identifier enum — how to locate a key in the keyring.

/// How to locate a key within a keyring.
#[derive(Clone, Copy, Debug)]
pub enum KeyIdentifier<'a> {
    Userid(&'a str),
    Keyid(&'a str),
    Fingerprint(&'a str),
    Grip(&'a str),
}

impl<'a> KeyIdentifier<'a> {
    pub(crate) fn type_str(self) -> &'static str {
        match self {
            KeyIdentifier::Userid(_) => "userid",
            KeyIdentifier::Keyid(_) => "keyid",
            KeyIdentifier::Fingerprint(_) => "fingerprint",
            KeyIdentifier::Grip(_) => "grip",
        }
    }

    pub(crate) fn value_str(self) -> &'a str {
        match self {
            KeyIdentifier::Userid(s) => s,
            KeyIdentifier::Keyid(s) => s,
            KeyIdentifier::Fingerprint(s) => s,
            KeyIdentifier::Grip(s) => s,
        }
    }
}
