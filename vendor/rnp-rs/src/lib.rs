//! Idiomatic Rust binding to the RNP OpenPGP C library (`librnp`).
//!
//! RNP is the OpenPGP implementation that powers Mozilla Thunderbird. This
//! crate provides a thin, idiomatic Rust wrapper over the public C FFI
//! declared in `<rnp/rnp.h>`.

//!
//! ## Quick start
//!
//! Generate a keypair, sign a message, verify the signature:
//!
//! ```no_run
//! use rnp::{Algorithm, Context, Hash, KeyBuilder, KeyUsage};
//!
//! let ctx = Context::new()?;
//! let key = KeyBuilder::new(Algorithm::Rsa)
//!     .bits(2048)
//!     .userid("alice <alice@example.com>")
//!     .hash(Hash::Sha256)
//!     .add_usage(KeyUsage::Sign)
//!     .add_usage(KeyUsage::Certify)
//!     .build(&ctx)?;
//!
//! let message = b"hello, world";
//! let signed = rnp::sign(&ctx, message, &key)?;
//! let result = rnp::verify(&ctx, &signed)?;
//! assert!(result.any_valid()?);
//! # Ok::<(), rnp::Error>(())
//! ```
//!
//! Encrypt and decrypt with a recipient key:
//!
//! ```no_run
//! use rnp::{Algorithm, Context, Decryptor, Encryptor, KeyBuilder, KeyUsage, Output};
//!
//! let ctx = Context::new()?;
//! let key = KeyBuilder::new(Algorithm::Rsa)
//!     .bits(2048)
//!     .userid("enc <enc@example.com>")
//!     .add_usage(KeyUsage::EncryptComms)
//!     .build(&ctx)?;
//!
//! let mut ct = Output::to_memory()?;
//! Encryptor::new(&ctx, b"secret")?
//!     .add_recipient(&key)
//!     .build(&mut ct)?;
//! let ciphertext = ct.into_bytes()?;
//!
//! let result = Decryptor::new(&ctx, &ciphertext).build()?;
//! assert_eq!(result.plaintext(), b"secret");
//! # Ok::<(), rnp::Error>(())
//! ```
//!
//! Multi-signer detached signature via the [`Signer`] builder:
//!
//! ```no_run
//! use rnp::{Algorithm, Context, Hash, KeyBuilder, KeyUsage, Mode, Signer};
//!
//! let ctx = Context::new()?;
//! let k1 = KeyBuilder::new(Algorithm::Rsa).bits(2048).userid("a")
//!     .add_usage(KeyUsage::Sign).build(&ctx)?;
//! let k2 = KeyBuilder::new(Algorithm::Rsa).bits(2048).userid("b")
//!     .add_usage(KeyUsage::Sign).build(&ctx)?;
//!
//! let sig = Signer::new(&ctx, b"doc", Mode::Detached)
//!     .add_signer(&k1)
//!     .add_signer_with_hash(&k2, Hash::Sha384)
//!     .armor(true)
//!     .build_to_memory()?;
//! # Ok::<(), rnp::Error>(())
//! ```
//!
//! ## Streaming
//!
//! Any [`std::io::Read`] plugs in via [`Input::from_reader`] and any
//! [`std::io::Write`] via [`Output::to_writer`], so large or non-seekable
//! data is processed without buffering it in memory:
//!
//! ```no_run
//! # use rnp::{Context, Decryptor, Encryptor, Input, KeyBuilder, KeyUsage, Output, algorithm::Algorithm};
//! # let ctx = Context::new()?;
//! # let key = KeyBuilder::new(Algorithm::Rsa).bits(2048)
//! #     .userid("enc <enc@example.com>").add_usage(KeyUsage::EncryptComms)
//! #     .build(&ctx)?;
//! let input = Input::from_reader(network_stream())?;
//! let mut output = Output::to_writer(tcp_sink())?;
//! Encryptor::new_with_input(&ctx, input)?.add_recipient(&key).build(&mut output)?;
//! # fn network_stream() -> impl std::io::Read { std::io::empty() }
//! # fn tcp_sink() -> impl std::io::Write { std::io::sink() }
//! # Ok::<(), rnp::Error>(())
//! ```
//!
//! Readers and writers are read and written lazily while the operation
//! runs; a failed stream surfaces its original [`std::io::Error`].
//!
//! ## Cargo features
//!
//! | Feature | Description |
//! |---------|-------------|
//! | `vendored` | Compile librnp + Botan + json-c + zlib + bzip2 from source via the `rnp-src` crate and statically link the result. |
//! | `pqc` | Expose PQC algorithm constants and `Encryptor::prefer_pqc_enc_subkey`. Requires librnp built with `ENABLE_PQC=ON`. |
//! | `crypto-refresh` | Expose v6 keys, crypto-refresh algorithms, and v6 PKESK/SKESK. Requires librnp built with `ENABLE_CRYPTO_REFRESH=ON`. |
//! | `logging` | Gate `Context::set_log_fd` / `set_log_file`. |
//!
//! ## Status
//!
//! This crate exercises **all 293 public functions** librnp 0.18.1
//! declares in `rnp.h` (4 as documented equivalents; see
//! [`docs/PARITY.md`](https://github.com/rnpgp/rnp-rs/blob/main/docs/PARITY.md)
//! for the per-function mapping and the audit that keeps it true). The
//! per-phase TODO files in
//! [`TODO.roadmap`](https://github.com/rnpgp/rnp-rs/tree/main/TODO.roadmap)
//! remain the build history.
//!
//! ## Linking
//!
//! By default the crate links against a system-installed `librnp`
//! (`-lrnp`). Install via `brew install rnp` (macOS),
//! `dnf install librnp-devel` (Fedora), or build from source. To point
//! at a non-standard install location set `RNP_INCLUDE_DIR` and
//! `RNP_LIB_DIR`. Use `--features vendored` to build librnp from source.

pub mod algorithm;
pub mod armor;
pub mod callbacks;
pub mod context;
pub mod dump;
pub mod encrypt;
pub mod error;
pub mod ffi;
pub mod ffi_safe;
pub mod key;
pub mod key_signature_builder;
pub mod keygen;
pub mod keyring;
pub mod ops;
pub mod secret;
pub mod security;
pub mod signature;
pub mod signature_handle;
pub mod strconv;
pub mod subkey;
pub mod uid;
pub mod verify;
pub mod version;

pub use algorithm::{Algorithm, Cipher, Compression, Curve, Hash, KeyUsage};
#[cfg(feature = "pqc")]
pub use algorithm::{PqcAlgorithm, librnp_supports_pqc};
pub use armor::{ContentType, armor_bytes, dearmor, dearmor_bytes, enarmor, guess_contents};
pub use callbacks::{KeyProvider, KeyRequestOutcome, PasswordProvider, RequestedKeyType};
pub use context::{Context, KeyringFormat};
pub use dump::{
    DumpFlags, JsonDumpFlags, JsonFlags, dump_packets_bytes_to_json, dump_packets_to_json,
    dump_packets_to_output,
};
#[allow(deprecated)]
pub use encrypt::{
    AddPasswordOptions, AeadType, DecryptResult, Decryptor, EncryptFlags, Encryptor, decrypt,
    decrypt_from_input, decrypt_to,
};
pub use error::{Error, ErrorKind, Result, from_rnp_code, unknown_variant};
pub use key::{AddUidOptions, ProtectOptions, RevocationCode, RevocationReason};
pub use key::{
    ExportFlags, Key, KeyIdentifier, LoadSaveFlags, RemoveFlags, RemoveSignaturesFlags, UnloadFlags,
};
pub use key_signature_builder::{
    CertificationBuilder, ConfiguredBuilder, DirectSignatureBuilder, RevocationSignatureBuilder,
    SignatureSetterOps,
};
pub use keygen::{
    KeyBuilder, SubkeyBuilder, generate_key_25519, generate_key_dsa_eg, generate_key_ec,
    generate_key_ex, generate_key_json, generate_key_rsa, generate_key_sm2,
};
pub use keyring::{IdentifierIterator, IdentifierKind};
pub use ops::{
    ArmorType, Input, MessageSource, Output, OutputFileFlags, WriterOutcome,
    call_for_optional_string, call_for_string, cstr_to_optional_string, cstr_to_string,
};
pub use secret::{SecretString, zero_string_bytes};
pub use security::{
    FeatureType, SecurityFlags, SecurityLevel, SecurityRule, calculate_iterations,
    request_password, supported_features, supports_feature,
};
pub use signature::{
    Mode, Signer, generate_revocation_certificate, generate_revocation_certificate_with, sign,
    sign_cleartext, sign_detached, verify, verify_detached,
};
pub use signature_handle::{Signature, SignatureType, Subpacket, SubpacketType};
pub use subkey::Subkey;
pub use uid::{Uid, UidType};
pub use verify::{
    FileInfo, Recipient, SignatureStatus, Symenc, VerifyFlags, VerifyOp, VerifyResult,
    VerifySignature,
};

/// librnp version string, e.g. `"0.18.1"`.
pub fn version_string() -> String {
    crate::version::version_string()
}
