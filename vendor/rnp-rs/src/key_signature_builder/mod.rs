//! Signature-creation builders: certification, direct, and revocation
//! signatures.
//!
//! The three builders share a setter chain via the [`SignatureSetterOps`]
//! trait (DRY) — adding a new setter means extending the trait + the
//! blanket impl, not editing three parallel surfaces (OCP).
//!
//! ## Module layout
//!
//! | Sub-module       | Concern                                          |
//! |------------------|--------------------------------------------------|
//! | `inner`          | `SignatureBuilderInner` + `SignatureSetterOps`   |
//! | `certification`  | `CertificationBuilder`                           |
//! | `direct`         | `DirectSignatureBuilder`                         |
//! | `revocation`     | `RevocationSignatureBuilder`                     |
//! | `configured`     | `ConfiguredBuilder` (post-setter chain state)    |

mod certification;
mod configured;
mod direct;
mod inner;
mod revocation;

pub use certification::CertificationBuilder;
pub use configured::ConfiguredBuilder;
pub use direct::DirectSignatureBuilder;
pub use inner::SignatureSetterOps;
pub use revocation::RevocationSignatureBuilder;
