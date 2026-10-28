# 0.9.23 dependency migration

OpenPGP uses RNP 0.18.1 and Botan 3.13.0; S/MIME uses the same Botan build for RSA and
ECDSA operations. This removes `rsa` and `crypto-bigint`, including their timing advisories,
from both Cargo lockfiles. CMS/X.509/PKCS #12 parsing remains in the existing Rust ASN.1 code.
Native operations run in memory inside the application. Serialized key bytes cross threads;
native handles and borrowed signature/recipient handles remain bound to their owner.

The migration preserves stored version 4 keys, encrypted messages, passphrase protection,
certificate fingerprints and OS credential names. RNP 0.18.1 does not support version 6
OpenPGP keys or post-quantum algorithms. Telex's generated keys continue to use version 4.
No database conversion decrypts keys or mail. Upgrading the mail database for today's arrival
markers is separate; back up before updating to 0.9.23.

## Maintained Linux and credential dependencies

Tauri 2 still uses GTK 0.18. Its macro crates now use maintained proc-macro-error3 with syn 2
support. Tauri's opener, notification and single-instance plugins retain their public APIs,
with their Linux IPC migrated to zbus 6.1. The notify-rust fallback uses its libdbus backend.
These changes remove proc-macro-error 1, zbus_names 3/4 and zvariant/derive 4/5.

The old keyring facade is replaced by keyring-core and maintained platform stores. Windows
retains the `user.service` target and UTF-16 secret encoding. Linux retains the service/user
Secret Service attributes. Credentials are opened per operation, allowing reconnection after
a keyring failure. The Windows regression writes a unique synthetic legacy credential and
reads it with the new driver; it never searches or reads real account credentials.

## Source provenance and native build

`scripts/security/vendor-sources.json` records original crate versions, registry archive
checksums, upstream revisions, changes and the complete SHA-256 inventory of source overrides.
`verify-vendor.mjs` rejects extra, missing, modified or symbolic-link files. GLib's existing
iterator backport retains its separate verifier and optimized regression test.

Native archives are bundled, checksum verified and built offline: Botan 3.13.0, RNP 0.18.1,
json-c 0.18, zlib 1.3.2 and bzip2 1.0.8. The RNP source crate's upstream RSA short-MPI
compatibility patch is applied exactly with a Rust patch parser. Both adapters link one static
Botan archive. Ninja/MSVC bootstrapping and target metadata allow the Windows build without
requiring a separate globally installed C++ environment. Linux needs CMake, Ninja, Python 3
and a C++ compiler. Native cache paths include the build configuration's source hash.

RNP and Botan use BSD-2-Clause, Botan's Rust bindings MIT, and bundled components retain
their own permissive licenses. `native-components.json` and `vendor/native-licenses/` supply
the native component versions and full licenses to the generated installer notices.

## Release verification

`npm run security` verifies sources, then scans application/fuzz lockfiles and a supplemental
registry-identity lockfile for the verified Rust overrides. The supplemental file keeps the
actual migrated dependency edges; it makes upstream advisories visible for path overrides.
Vulnerability, unsoundness and maintenance findings fail. The scanner rejects advisory
exclusions. This is not an independent security audit.

Run:

```
node scripts/security/verify-vendor.mjs --audit-lock
npm run security
cargo test -p telex-core --locked
cargo test -p telex --lib secrets::tests --locked
cargo clippy -p telex-core -p telex --all-targets -- -D warnings
```

On Linux, also run the three migrated plugin libraries' tests and the optimized GLib test.
OpenPGP regressions cover imported GnuPG keys/messages, trust, bad signatures, protected keys,
hidden recipients, bounded decompression and repeated RSA payload lengths. S/MIME regressions
cover OpenSSL messages, PKCS #12 variants, RSA/OAEP/ECDSA, certificate identity and trust.
The release gate additionally exercises hostile mail in both native webviews and bounded
MIME/HTML/PST fuzzing. Review the completed run for the exact release commit.
