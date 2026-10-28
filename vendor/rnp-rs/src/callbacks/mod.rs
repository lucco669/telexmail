//! Callback installation for [`Context`](crate::Context).
//!
//! All callback plumbing — trait definitions, holder types, C thunks, and the
//! registry that owns the holders — lives in this directory module.
//! [`Context`](crate::Context) delegates to the internal `Callbacks` registry
//! via thin `set_*` methods.
//!
//! ## Borrowed view
//!
//! The C-side key-provider thunk receives a raw `rnp_ffi_t` from librnp.
//! To give the user's trait implementation a usable `&Context`, the thunk
//! constructs a transient view via `ContextBorrow` (a `ManuallyDrop`-
//! backed wrapper that does **not** destroy the ffi). This avoids the
//! older `borrowed: bool` flag on `Context` itself — ownership semantics
//! are expressed in the type, not in runtime state.
//!
//! ## Module layout
//!
//! | Sub-module  | Concern                                                |
//! |-------------|--------------------------------------------------------|
//! | `traits`    | `PasswordProvider` + `KeyProvider` traits              |
//! | `types`     | `RequestedKeyType` + `KeyRequestOutcome` enums         |
//! | `registry`  | `Callbacks` + `PasswordHolder` + `KeyProviderHolder`   |
//! | `thunks`    | `password_thunk` + `key_provider_thunk` + `ContextBorrow` |
//! | `logging`   | `Context::set_log_fd`/`set_log_file` (feature-gated)  |

mod logging;
mod registry;
mod thunks;
mod traits;
mod types;

#[allow(unused_imports)]
pub(crate) use registry::Callbacks;
#[allow(unused_imports)]
pub(crate) use thunks::ContextBorrow;
pub use traits::{KeyProvider, PasswordProvider};
pub use types::{KeyRequestOutcome, RequestedKeyType};
