# rnp-rs

Idiomatic Rust binding to the [RNP](https://github.com/rnpgp/rnp) OpenPGP
(RFC 9580) C library.

RNP is the C++ OpenPGP implementation that powers Mozilla Thunderbird.
This crate provides a thin, safe Rust wrapper over its public C FFI
(declared in `include/rnp/rnp.h`) — with **full surface parity**: all 293
public functions of librnp 0.18.1 are covered (see
[`docs/PARITY.md`](docs/PARITY.md)).

The workspace has three crates, mirroring the `botan-rs` layout:

| Crate | Purpose | Docs |
|-------|---------|------|
| `rnp` (this crate) | Safe, idiomatic wrappers | [docs.rs/rnp-rs](https://docs.rs/rnp-rs) |
| `rnp-sys` | Raw FFI declarations (`links = "rnp"`) | [docs.rs/rnp-sys](https://docs.rs/rnp-sys) |
| `rnp-src` | Build-time librnp + Botan compilation | [docs.rs/rnp-src](https://docs.rs/rnp-src) |

## Getting started

### 1. Install build prerequisites

The only Rust toolchain requirement is **Rust ≥ 1.88** (`rustup install stable`).
Beyond that, the prerequisites depend on whether you use the system `librnp` or
compile from source:

#### Option A — vendored (recommended for new projects)

Compiles librnp + Botan + json-c + zlib + bzip2 from source. No system crypto
libraries needed.

| Platform | Prerequisites |
|---|---|
| **Linux** | `sudo apt install build-essential cmake python3` (Debian/Ubuntu) or `sudo dnf install gcc gcc-c++ cmake python3` (Fedora) |
| **macOS** | `xcode-select --install && brew install cmake python3` |
| **Windows** | Install [MSYS2](https://www.msys2.org/), then in an **UCRT64** shell: `pacman -S mingw-w64-ucrt-x86_64-gcc mingw-w64-ucrt-x86_64-cmake mingw-w64-ucrt-x86_64-make mingw-w64-ucrt-x86_64-python3 make` |

First build takes ~10 min (Botan dominates); cached in `OUT_DIR` after that.

#### Option B — system librnp (smaller builds, faster iteration)

| Platform | Install command |
|---|---|
| **macOS** | `brew install rnp` |
| **Fedora** | `sudo dnf install librnp-devel` |
| **Debian/Ubuntu** | `sudo apt install librnp-dev` (if packaged) |
| **Custom build** | Set `RNP_INCLUDE_DIR` + `RNP_LIB_DIR` to point at your own librnp build |

### 2. Add the dependency

```toml
# Cargo.toml

# Option A — vendored (self-contained, no system libs):
[dependencies]
rnp = { version = "0.1", features = ["vendored"] }

# Option A + PQC (post-quantum algorithms):
rnp = { version = "0.1", features = ["vendored", "pqc", "crypto-refresh"] }

# Option B — system librnp (requires `brew install rnp` etc.):
[dependencies]
rnp = "0.1"
```

### 3. Build and test

```sh
cargo build                    # compiles Botan + librnp on first run (vendored)
cargo test                     # runs the integration test suite
```

On Windows, run from an **MSYS2 UCRT64** shell so cargo finds the mingw toolchain.

### 4. Write your first program

```rust
use rnp::{Algorithm, Context, Hash, KeyBuilder, KeyUsage};

fn main() -> rnp::Result<()> {
    let ctx = Context::new()?;

    // Generate a signing key
    let key = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("alice <alice@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Sign)
        .add_usage(KeyUsage::Certify)
        .build(&ctx)?;

    // Sign and verify a message
    let message = b"hello, world";
    let signed = rnp::sign(&ctx, message, &key)?;
    let result = rnp::verify(&ctx, &signed)?;
    assert!(result.any_valid()?);

    println!("Signature verified!");
    Ok(())
}
```

See `examples/` for encrypt/decrypt, keygen, and multi-signer demos.

## Status

The crate wraps ~250 of librnp's ~309 public functions across all major
OpenPGP concerns: signing, verification, encryption, decryption, key
generation, keyring management, ASCII armor, packet dumps, security
profile, and feature queries. PQC and crypto-refresh (RFC 9580 v6)
support is feature-gated. MSRV is **Rust 1.88** (let-chains in
`build.rs`).

## Cargo features

| Feature          | Default | Description                                                                                                                                                          |
|------------------|---------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `vendored`       | off     | Compile librnp + Botan + json-c + zlib + bzip2 from source via the `rnp-src` crate and statically link. Fully self-contained — no system libraries required. First build ~10 min (Botan); cached in `OUT_DIR` after that. |
| `vendored-minimal` | off   | Implies `vendored`. (Future: select a minimal Botan module set for smaller binaries.) |
| `pqc`            | off     | Build with `ENABLE_PQC=ON` and expose PQC algorithm constants (ML-KEM, ML-DSA, SLH-DSA) + `Encryptor::prefer_pqc_enc_subkey`. Requires `vendored` (system librnp rarely has PQC). |
| `crypto-refresh` | off     | Expose v6 keys, crypto-refresh algorithm names, and v6 PKESK/SKESK. Requires `vendored`. |
| `logging`        | off     | Gate `Context::set_log_fd` / `set_log_file` for directing librnp's diagnostic output.                                                                                |

## Quick start

```rust
use rnp::{Algorithm, Context, Hash, KeyBuilder, KeyUsage};

let ctx = Context::new()?;
let key = KeyBuilder::new(Algorithm::Rsa)
    .bits(2048)
    .userid("alice <alice@example.com>")
    .hash(Hash::Sha256)
    .add_usage(KeyUsage::Sign)
    .add_usage(KeyUsage::Certify)
    .build(&ctx)?;

let message = b"hello, world";
let signed = rnp::sign(&ctx, message, &key)?;
let result = rnp::verify(&ctx, &signed)?;
assert!(result.any_valid()?);
# Ok::<(), rnp::Error>(())
```

See `examples/` for end-to-end demos of sign/verify, encrypt/decrypt,
key generation, and multi-signer signatures.

## Feature coverage

Each subsection below lists the Rust surface and the librnp C functions
it wraps.

### Context, password provider, and key provider

`Context::new()` is the entry point — it owns an `rnp_ffi_t` and ties
the lifetimes of every derived handle to it. A `Context` accepts a
`PasswordProvider` trait object (used by `rnp_request_password` and any
operation that needs to unlock a key) and a `KeyProvider` trait object
(used by `rnp_ffi_set_key_provider` for verify/decrypt key lookup).

```rust
use rnp::{Context, KeyIdentifier, PasswordProvider, RequestedKeyType};

struct MyPw;
impl PasswordProvider for MyPw {
    fn request(&mut self, _keyid: Option<&[u8]>, _ctx: &Context) -> rnp::Result<Vec<u8>> {
        Ok(b"hunter2".to_vec())
    }
}

let mut ctx = Context::new()?;
ctx.set_password_provider(MyPw);
```

| Rust                                          | C function                                  |
|-----------------------------------------------|---------------------------------------------|
| `Context::new` / `Drop`                       | `rnp_ffi_create` / `rnp_ffi_destroy`        |
| `Context::set_password_provider`              | `rnp_ffi_set_password_provider`             |
| `Context::set_key_provider`                   | `rnp_ffi_set_key_provider`                  |
| `Context::borrow_ffi`                         | (re-entrant thunk plumbing)                 |
| `Context::set_log_file` / `set_log_fd`        | `rnp_ffi_set_log_fd` / `set_log_file`       |
| `Context::load_keys` / `save_keys`            | `rnp_load_keys` / `rnp_save_keys`           |
| `Context::unload_keys`                        | `rnp_unload_keys`                           |
| `Context::import_keys` / `import_signatures`  | `rnp_import_keys` / `rnp_import_signatures` |
| `Context::find_key`                           | `rnp_locate_key`                            |
| `Context::default_homedir` / `detect_homedir` | `rnp_get_default_homedir` / `detect_*`      |

### Key generation

`KeyBuilder` is the primary surface; `SubkeyBuilder` builds subkeys off
a primary in one composite call. Both wrap `rnp_op_generate_*`.

```rust
use rnp::{Algorithm, Context, Hash, KeyBuilder, KeyUsage, SubkeyBuilder};

let ctx = Context::new()?;
let key = KeyBuilder::new(Algorithm::Rsa)
    .bits(3072)
    .userid("alice <alice@example.com>")
    .hash(Hash::Sha256)
    .add_usage(KeyUsage::Sign)
    .add_usage(KeyUsage::Certify)
    .add_pref_hash(Hash::Sha512)
    .clear_pref_hash()                  // reset and add a fresh set
    .add_pref_hash(Hash::Sha256)
    .add_subkey(
        SubkeyBuilder::new(Algorithm::Rsa)
            .bits(2048)
            .add_usage(KeyUsage::EncryptComms),
    )
    .build(&ctx)?;
```

| Rust                                            | C function                                |
|-------------------------------------------------|-------------------------------------------|
| `KeyBuilder::new` / `build`                     | `rnp_op_generate_create` / `execute`      |
| `KeyBuilder::bits` / `hash` / `dsa_qbits`       | `rnp_op_generate_set_{bits,hash,dsa_qbits}` |
| `KeyBuilder::curve`                             | `rnp_op_generate_set_curve`               |
| `KeyBuilder::userid`                            | `rnp_op_generate_set_userid`              |
| `KeyBuilder::expiration`                        | `rnp_op_generate_set_expiration`          |
| `KeyBuilder::add_usage` / `clear_usage`         | `rnp_op_generate_{add,clear}_usage`       |
| `KeyBuilder::add_pref_hash` / `clear_pref_hash` | `rnp_op_generate_{add,clear}_pref_hash`   |
| `KeyBuilder::add_pref_cipher` / `clear_pref_cipher`         | `rnp_op_generate_{add,clear}_pref_cipher`            |
| `KeyBuilder::add_pref_compression` / `clear_pref_compression` | `rnp_op_generate_{add,clear}_pref_compression`     |
| `KeyBuilder::pref_keyserver`                    | `rnp_op_generate_set_pref_keyserver`      |
| `KeyBuilder::protection`                        | `rnp_op_generate_set_protection_*`        |
| `KeyBuilder::request_password`                  | `rnp_op_generate_set_request_password`    |
| `KeyBuilder::v6` (crypto-refresh)               | `rnp_op_generate_set_v6_key`              |
| `SubkeyBuilder::build_with_ffi`                 | `rnp_op_generate_subkey_create`           |
| `generate_key_json`                             | `rnp_generate_key_json`                   |

Algorithm coverage: RSA, DSA, ElGamal, ECDSA/ECDH (NIST/Brainpool
curves), EdDSA, X25519, SM2, plus the PQC composites (`ML-KEM`,
`ML-DSA`, `SLH-DSA`) when built with the `pqc` feature.

### Key inspection

`Key<'ctx>` is a borrowed handle. All getters return `Result<T>` so a
deleted/expired/revoked underlying key surfaces as an error rather than
a panic.

```rust
# use rnp::*;
# let ctx = Context::new()?;
# let key = KeyBuilder::new(Algorithm::Rsa).bits(2048)
#     .userid("alice <a@example.com>").build(&ctx)?;
println!("fp: {}", key.fingerprint()?);
println!("alg: {:?}", key.alg()?);
println!("bits: {:?}", key.bits()?);
println!("subkeys: {}", key.subkey_count()?);
for uid in key.uids()? {
    println!("uid: {}", uid.data_string()?);
}
# Ok::<(), rnp::Error>(())
```

| Rust                                     | C function                                  |
|------------------------------------------|---------------------------------------------|
| `Key::alg` / `bits` / `curve` / `version`| `rnp_key_get_alg` / `get_bits` / `get_curve` / `get_version` |
| `Key::keyid` / `fingerprint` / `grip`    | `rnp_key_get_keyid` / `get_fprint` / `get_grip`             |
| `Key::primary_fprint` / `primary_grip`   | `rnp_key_get_primary_fprint` / `get_primary_grip`           |
| `Key::creation` / `expiration`           | `rnp_key_get_creation` / `get_expiration`                   |
| `Key::is_{locked,protected,primary,sub,valid,revoked,compromised,expired,superseded,retired}` | matching `rnp_key_is_*` |
| `Key::have_public` / `have_secret`       | `rnp_key_have_public` / `rnp_key_have_secret`               |
| `Key::protection_{mode,type,cipher,hash,iterations}` | `rnp_key_get_protection_*`                    |
| `Key::valid_till` / `valid_till_64`      | `rnp_key_valid_till` / `rnp_key_valid_till64`               |
| `Key::uid_count` / `uids`                | `rnp_key_get_uid_count` / `get_uid_handle_at`               |
| `Key::subkey_count` / `subkeys` / `subkey_at` | `rnp_key_get_subkey_count` / `get_subkey_at`            |
| `Key::signature_count` / `signatures`    | `rnp_key_get_signature_count` / `get_signature_at`          |
| `Key::default_key_for(usage)`            | `rnp_key_default_key_for`                                  |

### UID, Subkey, Signature handles

`Uid<'key>`, `Subkey<'key>`, and `Signature<'key>` borrow the parent
`Key`. They each carry a `Drop` that calls the matching
`rnp_*_handle_destroy`.

```rust
# use rnp::*;
# let ctx = Context::new()?;
# let key = KeyBuilder::new(Algorithm::Rsa).bits(2048)
#     .userid("alice <a@example.com>").build(&ctx)?;
let uid = key.uids()?.into_iter().next().unwrap();
println!("primary? {}", uid.is_primary()?);
println!("signatures on uid: {}", uid.signature_count()?);

let sig = key.signatures()?.into_iter().next().unwrap();
for sp in sig.subpackets()? {
    println!("subpacket: {:?}", sp.subpacket_type());
}
# Ok::<(), rnp::Error>(())
```

| Rust                                              | C function                                                |
|---------------------------------------------------|-----------------------------------------------------------|
| `Uid::uid_type` / `data` / `data_string`          | `rnp_uid_get_type` / `rnp_uid_get_data`                   |
| `Uid::is_primary` / `is_valid` / `is_revoked`     | `rnp_uid_is_primary` / `is_valid` / `is_revoked`          |
| `Uid::signature_count` / `signatures` / `signature_at` | `rnp_uid_get_signature_count` / `get_signature_at`   |
| `Uid::revocation_signature`                       | `rnp_uid_get_revocation_signature`                        |
| `Uid::remove(&key)`                               | `rnp_uid_remove`                                          |
| `Subkey` (newtype over `Key`)                     | `rnp_key_get_subkey_at`                                   |
| `Signature::sig_type` / `sig_type_enum`           | `rnp_signature_get_type`                                  |
| `Signature::alg` / `hash` / `keyid` / `creation`  | `rnp_signature_get_*`                                     |
| `Signature::signer_key`                           | `rnp_signature_get_signer`                                |
| `Signature::subpacket_count` / `subpackets` / `find_subpacket` | `rnp_signature_subpacket_{count,at,find}`    |
| `Signature::export` / `remove_from_key`           | `rnp_signature_export` / `rnp_signature_remove`           |
| `Subpacket` accessors                             | `rnp_signature_subpacket_info` / `_data`                  |

### Key mutation

```rust
# use rnp::*;
# let ctx = Context::new()?;
# let key = KeyBuilder::new(Algorithm::Rsa).bits(2048)
#     .userid("alice <a@example.com>").build(&ctx)?;
key.add_uid("alias <a@example.com>", &AddUidOptions::new().hash(Hash::Sha256))?;
key.protect(&ProtectOptions::new().password("hunter2"))?;
key.lock()?;
key.unlock(|_| Ok(b"hunter2".to_vec()))?;
# Ok::<(), rnp::Error>(())
```

| Rust                                              | C function                                |
|---------------------------------------------------|-------------------------------------------|
| `Key::protect` / `unprotect` / `lock` / `unlock`  | `rnp_key_{protect,unprotect,lock,unlock}` |
| `Key::add_uid`                                    | `rnp_key_add_uid`                         |
| `Key::revoke`                                     | `rnp_key_revoke`                          |
| `Key::set_expiration`                             | `rnp_key_set_expiration`                  |
| `Key::remove`                                     | `rnp_key_remove`                          |
| `Key::remove_signatures`                          | `rnp_key_remove_signatures`               |
| `Key::export_revocation`                          | `rnp_key_export_revocation`               |
| `Key::export_autocrypt`                           | `rnp_key_export_autocrypt`                |
| `Key::export`                                     | `rnp_key_export`                          |
| `CertificationBuilder` / `DirectSignatureBuilder` / `RevocationSignatureBuilder` | `rnp_key_{certification,direct_signature,revocation_signature}_create` |

The three signature-creation builders share a single
`SignatureSetterOps` trait — adding a new subpacket setter adds one
method to the trait and a delegating impl per concrete builder.

### Sign and verify

Three signing modes (inline / detached / cleartext) all flow through
one `Signer` builder, so new output formats are a `Mode` variant rather
than a new top-level function.

```rust
# use rnp::*;
# let ctx = Context::new()?;
# let k1 = KeyBuilder::new(Algorithm::Rsa).bits(2048).userid("a")
#     .add_usage(KeyUsage::Sign).build(&ctx)?;
# let k2 = KeyBuilder::new(Algorithm::Rsa).bits(2048).userid("b")
#     .add_usage(KeyUsage::Sign).build(&ctx)?;
let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as u32;
let sig = Signer::new(&ctx, b"doc", Mode::Detached)
    .add_signer(&k1)
    .add_signer_with_options(&k2, Hash::Sha384, Some(now), Some(604800))
    .armor(true)
    .build_to_memory()?;
# Ok::<(), rnp::Error>(())
```

| Rust                                          | C function                                |
|-----------------------------------------------|-------------------------------------------|
| `sign` / `sign_detached` / `sign_cleartext`   | `rnp_op_sign_*_create` + `execute`        |
| `Signer::add_signer` / `_with_hash` / `_with_options` | `rnp_op_sign_add_signature`        |
| `Signer::hash` (default) / per-signer         | `rnp_op_sign_set_hash` / `_signature_set_hash` |
| `Signer::armor`                               | `rnp_op_sign_set_armor`                   |
| `add_signer_with_options` (creation/expiration) | `rnp_op_sign_signature_set_creation_time` / `_set_expiration_time` |
| `verify` / `verify_detached`                  | `rnp_op_verify_*_create` + `execute`      |
| `VerifyResult::signature_count` / `signature_at` | `rnp_op_verify_get_signature_count` / `_at` |
| `VerifySignature::status` / `key` / `keyid`   | `rnp_signature_get_status` etc.           |
| `VerifyOp::flags`                             | `rnp_op_verify_set_flags`                 |
| `generate_revocation_certificate[_with]`      | `rnp_key_export_revocation`               |

### Encrypt and decrypt

```rust
# use rnp::*;
# let ctx = Context::new()?;
# let key = KeyBuilder::new(Algorithm::Rsa).bits(2048)
#     .userid("enc <enc@example.com>")
#     .add_usage(KeyUsage::EncryptComms).build(&ctx)?;
let mut ct = Output::to_memory()?;
Encryptor::new(&ctx, b"secret")?
    .add_recipient(&key)
    .armor(true)
    .aead(AeadType::Ocb)
    .build(&mut ct)?;
let ciphertext = ct.into_bytes()?;

let result = Decryptor::new(&ctx, &ciphertext).build()?;
assert_eq!(result.plaintext(), b"secret");
# Ok::<(), rnp::Error>(())
```

| Rust                                              | C function                                |
|---------------------------------------------------|-------------------------------------------|
| `Encryptor::new` / `build`                        | `rnp_op_encrypt_create` + `execute`       |
| `Encryptor::add_recipient` / `add_password` / `add_signature` | `rnp_op_encrypt_add_recipient` / `_add_password` / `_add_signature` |
| `Encryptor::{cipher,hash,compression,aead,aead_bits,armor,file_name,file_mtime,creation_time,expiration_time,flags}` | matching `rnp_op_encrypt_set_*` |
| `Encryptor::prefer_pqc_enc_subkey` (pqc)          | `rnp_op_encrypt_prefer_pqc_enc_subkey`    |
| `Encryptor::enable_pkesk_v6` / `enable_skesk_v6`  | `rnp_op_encrypt_set_*` v6 PKESK/SKESK     |
| `Decryptor::new` / `build`                        | `rnp_decrypt` (and verify-op plumbing)    |
| `DecryptResult::{recipient,symenc,protection_info,file_info,format}` | `rnp_decrypt_get_*`            |
| `Recipient` / `Symenc` handles                    | `rnp_recipient_*` / `rnp_symenc_*`        |

### ASCII armor, packet dumps, JSON serialization

```rust
# use rnp::*;
let armored = enarmor(b"\x88\x01...", ContentType::Binary)?;
let raw = dearmor(&armored)?;
let json = dump_packets_to_json(&ctx, &raw, JsonDumpFlags::empty())?;
let key_json = key.to_json(JsonFlags::empty())?;
# Ok::<(), rnp::Error>(())
```

| Rust                                              | C function                                  |
|---------------------------------------------------|---------------------------------------------|
| `enarmor` / `dearmor` / `dearmor_bytes` / `armor_bytes` | `rnp_armor_*` / `rnp_dearmor_*`       |
| `guess_contents`                                  | `rnp_guess_contents`                        |
| `dump_packets_to_output` / `_to_json` / `_bytes_to_json` | `rnp_dump_packets_to_*`              |
| `Key::to_json` / `Signature::to_json`             | `rnp_key_to_json` / `rnp_signature_packet_to_json` |
| `DumpFlags` / `JsonDumpFlags` / `JsonFlags`       | `RNP_DUMP_*` / `RNP_JSON_*` bit constants   |

### Security profile and feature queries

```rust
# use rnp::*;
let ctx = Context::new()?;
let rule = SecurityRule::new(
    SecurityLevel::Default,
    SecurityFlags::empty(),
    FeatureType::PublicKey,
    "RSA",
)?;
ctx.add_security_rule(&rule)?;
let iters = calculate_iterations(Hash::Sha256, 100_000_000)?;
let features = supported_features()?;
assert!(supports_feature(FeatureType::HashAlgorithm, "SHA256")?);
# Ok::<(), rnp::Error>(())
```

| Rust                                              | C function                                |
|---------------------------------------------------|-------------------------------------------|
| `Context::add_security_rule` / `get_security_rule` / `remove_security_rule` | `rnp_add_security_rule` / `rnp_get_security_rule` / `rnp_remove_security_rule` |
| `SecurityLevel` / `SecurityFlags`                 | `RNP_SECURITY_*` constants                |
| `supports_feature` / `supported_features`         | `rnp_supports_feature` / `rnp_supported_features` |
| `calculate_iterations`                            | `rnp_calculate_iterations`                |
| `request_password`                                | `rnp_request_password` (returns `SecretString`) |
| `Context::set_timestamp`                          | `rnp_set_timestamp`                       |

### Secret-string hygiene

`SecretString` is a `Vec<u8>` wrapper that zeros its bytes on `Drop`
via `rnp_buffer_clear`. `request_password` returns one; user-supplied
passwords that you want to scrub should also live in a `SecretString`.

### PQC and crypto-refresh

Gated on the `pqc` and `crypto-refresh` Cargo features. The crate
probes the linked librnp at runtime (`librnp_supports_pqc`) so a binary
that enables PQC still degrades cleanly on a non-PQC librnp.

```rust
#[cfg(feature = "pqc")]
{
    if rnp::librnp_supports_pqc()? {
        let key = KeyBuilder::new(Algorithm::Rsa)
            .bits(2048)
            .userid("alice")
            .add_pqc_encryption_subkey(PqcAlgorithm::MlKem768X25519)
            .add_pqc_signing_subkey(PqcAlgorithm::MlDsa65Ed25519)
            .build(&ctx)?;
    }
}
```

### Version and debug

```rust
println!("librnp {}", rnp::version_string());
let (maj, min, patch) = rnp::version::decompose();
println!("backend: {}", rnp::version::backend_string());

// Emit librnp diagnostic output to stderr (or a named file).
rnp::version::enable_debug("stderr");
rnp::version::disable_debug();
```

| Rust                                       | C function                            |
|--------------------------------------------|---------------------------------------|
| `version` / `decompose`                    | `rnp_version` / `_major` / `_minor` / `_patch` |
| `version_string` / `version_string_full`   | `rnp_version_string` / `_full`        |
| `version_for` / `version_commit_timestamp` | `rnp_version_for` / `_commit_timestamp` |
| `backend_string` / `backend_version`       | `rnp_backend_string` / `_version`     |
| `enable_debug` / `disable_debug`           | `rnp_enable_debug` / `rnp_disable_debug` |

## Linking

`rnp-rs` links against a **system-installed** `librnp` (`-lrnp`) by
default. Use `--features vendored` to skip that requirement entirely.

### macOS (Homebrew)

```sh
brew install rnp
```

### Linux

```sh
# Fedora
sudo dnf install librnp-devel

# Debian/Ubuntu (if packaged)
sudo apt install librnp-dev
```

### Pointing at a source checkout

```sh
RNP_INCLUDE_DIR=/path/to/rnp/install/include \
RNP_LIB_DIR=/path/to/rnp/install/lib \
cargo build
```

### Vendored (compile everything from source)

```toml
# Default: librnp 0.18.1 + Botan 3.12 + all deps, statically linked
[dependencies]
rnp = { version = "0.1", features = ["vendored"] }

# PQC + crypto-refresh: librnp HEAD + Botan with PQC modules
rnp = { version = "0.1", features = ["vendored", "pqc", "crypto-refresh"] }
```

No system librnp, Botan, json-c, zlib, or bzip2 required — the `rnp-src`
crate downloads and compiles them all. Source downloads are pure Rust
(ureq + flate2 + tar), so `curl` and `tar` are not needed on the host.

Build requirements: **C/C++ compiler**, **cmake**, **python3** (for
Botan's `configure.py`), and **patch** (applies the librnp 0.18.1
backport). On Windows, use **MSYS2 UCRT64** (`mingw-w64-ucrt-x86_64-gcc`,
`cmake`, `make`, `python3`, `patch`).

| Feature combo | librnp | Botan | When to use |
|---|---|---|---|
| `vendored` | 0.18.1 (stable tarball + backports) | 3.13.0 (full) | Default — all RFC 9580 algorithms |
| `vendored` + `pqc` | `main` @ `470695b9` (pinned clone) | 3.13.0 (PQC modules enabled) | ML-KEM / ML-DSA / SLH-DSA signing + encryption |
| `vendored` + `crypto-refresh` | `main` @ `470695b9` (pinned clone) | 3.13.0 (full) | v6 keys, crypto-refresh algorithm names |

The HEAD flavor builds from a **pinned** upstream commit (see
`RNP_HEAD_REF` in `rnp-src/src/lib.rs`) that is fetched and checked out on
every build — cached clones track the pin. Unlike the 0.18.1 flavor, it
does not compile json-c: upstream `main` vendors nlohmann/json as a single
header (rnpgp/rnp@4f5c4e6e).

The 0.18.1 tarball carries one backport:
`patches/rsa-short-mpi-botan-3.13.patch` in `rnp-src` pads short
RSA MPIs to the modulus length — without it, Botan 3.13's strict length
checks reject ~1/256 of random RSA ciphertexts/signatures
([rnpgp/rnp#2465](https://github.com/rnpgp/rnp/issues/2465); fixed
upstream by rnpgp/rnp@82283888, backported here until a librnp release
carries it).

PQC/crypto-refresh use librnp HEAD because 0.18.1's PQC code paths are
incompatible with Botan 3.12's opaque EC types.

### Pregenerated bindings (cross builds)

Running bindgen requires a working libclang on the build host — which
minimal cross containers often lack (the common failure is libclang unable
to find `stdbool.h`). The crate therefore ships pregenerated bindings
(`bindings/bindings-<librnp-version>.rs`) and uses them automatically
whenever the headers are known to match — i.e. `vendored` builds against
librnp 0.18.1. The file is target-independent (rnp.h is opaque handles +
primitives; C types render as per-target `std::os::raw` aliases).

| Env var | Effect |
|---|---|
| `RNP_BINDINGS_RUNTIME=1` | Force runtime bindgen (skip the shipped file) |
| `RNP_BINDINGS_PREGENERATED=1` | Force the shipped file — the escape hatch for cross builds in system/explicit mode |
| `RNP_BINDINGS_EXPERIMENTAL=1` | Add the PQC/crypto-refresh defines at bindgen time (regeneration only) |
| `RNP_BINDINGS_REGENERATE=1` | Copy freshly generated bindings back into `bindings/` |

Regenerate after a librnp version bump:

```sh
scripts/regenerate-bindings.sh
```

### Using rnp-rs alongside the `botan` crate

If your project depends on both `rnp` and the
[`botan`](https://crates.io/crates/botan) crate, be careful with vendored
mode: `botan[vendored]` compiles Botan via `botan-sys`, while
`rnp[vendored]` compiles its own Botan via `rnp-src`. With mismatched Botan
versions (cargo treats `0.x` minors as incompatible, so both `botan-src`
versions coexist), the final binary ends up with **two Botan builds and
order-dependent linking** — potentially a silent ABI mismatch.

Recommendations, in order of preference:

1. **System Botan for both** — `rnp` (default, no `vendored`) + `botan`
   (default). One shared `libbotan`, zero duplication.
2. **Aligned vendored versions** — both vendored paths now build Botan
   3.13.0, so mixed graphs compile the same Botan twice (wasteful,
   ~10 min extra, but correct).

To get a loud warning when a duplicate vendored Botan is detected, enable
the diagnostic feature on `rnp-sys` from your project:

```toml
rnp = { version = "0.1", features = ["vendored"] }
rnp-sys = { version = "0.1", features = ["botan-sys-detect"] }
```

(This requires a system Botan or a vendored `botan` elsewhere in the
graph, since botan-sys's non-vendored build probes for a system install.)

## Architecture

A three-crate workspace, mirroring the botan-rs layout
(`botan-src` : `botan-sys` : `botan`):

- **`rnp-src`** — build-time library that compiles librnp + Botan + json-c
  + zlib + bzip2 from source; no build script of its own, declares no
  `links`. Its `rnp_src::build()` is called from rnp-sys's build script,
  so the compilation lands in the caller's `OUT_DIR`.
- **`rnp-sys`** — raw FFI crate: `links = "rnp"`, owns the build script
  (link-mode dispatch, pregenerated bindings + bindgen fallback, link
  directives). Raw-FFI consumers can depend on it directly.
- **`rnp`** (this crate) — safe, idiomatic wrappers. No build script.

Safe-wrapper internals, top to bottom:

1. **`src/ffi.rs`** — re-export of `rnp_sys::*`, keeping the safe wrappers'
   `crate::ffi` paths stable. The bindings themselves are generated by
   rnp-sys (bindgen at build time, or the pregenerated file when the
   headers match); the module mirrors the C API verbatim.
2. **`src/error.rs`** — `Error` (snafu), `Result<T>`, and `check(rnp_result_t)` that maps non-zero to `Error::Rnp { code, message }` via `rnp_result_to_string`.
3. **Safe wrappers** — one directory module per OpenPGP concern:
   - `context.rs`, `key/`, `uid.rs`, `subkey.rs`, `keygen/`, `signature.rs`, `encrypt/`, `verify/`, `armor.rs`, `dump.rs`, `security.rs`, `signature_handle/`, `key_signature_builder/`, `callbacks/`, `secret.rs`, `ops/` (IO + string-conversion helpers).

Every handle type borrows its parent (via `PhantomData`) and owns a
`Drop` that calls the matching `rnp_*_handle_destroy`. Never add
`unsafe impl Send` — librnp handles are not thread-safe.

## License

BSD-2-Clause, matching the upstream RNP project. See `LICENSE.md`.
