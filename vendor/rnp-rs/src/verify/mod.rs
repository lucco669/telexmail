//! Verify operation: the typed surface over `rnp_op_verify_*`.
//!
//! [`VerifyOp`] owns the verify-operation handle; executing it produces a
//! [`VerifyResult`] that owns the per-signature / per-recipient / per-symenc
//! state for inspection. This is the surface decryption-result inspection
//! needs and the one `signature::verify` reuses.
//!
//! ## Module layout
//!
//! | Sub-module    | Concern                                              |
//! |---------------|------------------------------------------------------|
//! | `op`          | `VerifyOp` builder + `VerifyFlags`                   |
//! | `result`      | `VerifyResult` (owns the op handle post-execute)     |
//! | `signature`   | `VerifySignature` + `SignatureStatus`                |
//! | `recipient`   | `Recipient`                                          |
//! | `symenc`      | `Symenc`                                             |
//! | `file_info`   | `FileInfo`                                           |

mod file_info;
mod op;
mod recipient;
mod result;
mod signature;
mod symenc;

pub use file_info::FileInfo;
pub use op::{VerifyFlags, VerifyOp};
pub use recipient::Recipient;
pub use result::VerifyResult;
pub use signature::{SignatureStatus, VerifySignature};
pub use symenc::Symenc;
