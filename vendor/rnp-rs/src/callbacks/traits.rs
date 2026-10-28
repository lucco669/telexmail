//! Password + key provider traits.

use crate::context::Context;
use crate::key::KeyIdentifier;

use super::types::{KeyRequestOutcome, RequestedKeyType};

/// What the library calls when it needs a passphrase to unlock a secret key.
///
/// Implementations return the password as `Cow<str>`; it is copied into the
/// librnp-provided buffer before the callback returns.
pub trait PasswordProvider: Send + Sync {
    fn get_password(
        &self,
        key: Option<&crate::key::Key>,
        context: &str,
    ) -> Option<std::borrow::Cow<'_, str>>;
}

/// Trait implemented by the consumer to feed keys into librnp on demand.
///
/// Called by librnp during verify/decrypt when the keyring doesn't have a
/// key the operation needs. The implementation typically inspects `id`
/// and `kind`, fetches the key bytes from some external store, and calls
/// [`Context::load_keys`](crate::Context::load_keys) on the supplied
/// `ctx`. Returns [`KeyRequestOutcome::Found`] if a key was loaded,
/// [`KeyRequestOutcome::NotFound`] otherwise.
pub trait KeyProvider: Send + Sync {
    fn on_key_request(
        &self,
        ctx: &Context,
        id: KeyIdentifier<'_>,
        kind: RequestedKeyType,
    ) -> KeyRequestOutcome;
}
