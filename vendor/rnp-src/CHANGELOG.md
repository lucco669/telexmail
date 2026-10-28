# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- Honor CC/CXX for the librnp cmake build (rnp-rs#120) by @[object]

### Fixed

- Honor crt-static on MSVC — compile every staged C/C++ dep /MT by @[object]

### Fixed

- Alias MSVC zlibstatic.lib to z.lib; platform-aware rebuild guards by @[object]

### Fixed

- Stage MSVC compat dirent/getopt headers for librnp by @[object]

### Fixed

- /FI cstring on MSVC — cl treats -include's operand as a source file by @[object]
- MSVC cross path for the cmake deps and bzip2 by @[object]

### Fixed

- Pin librnp's Botan discovery to the staged prefix by @[object]

### Other

- Respect a caller-provided BOTAN_CONFIGURE_CC on windows (the unconditional gcc stomp made MSVC cross targets unbuildable) by @[object]

### Added

- Sync vendored HEAD flavor to librnp main (pin, clone-sync, drop json-c) by @[object]

### Other

- Release by @[object]

### Other

- Release by @[object]
- Collapse nested if flagged by clippy under -D warnings by @[object]
- Flavor is the single source of truth for the vendored build by @[object]

### Other

- Collapse nested if flagged by clippy under -D warnings by @[object]
- Flavor is the single source of truth for the vendored build by @[object]

### Fixed

- Backport RSA short-MPI padding; unpin Botan to 3.13.0 by @[object]

### Other

- Make docs.rs build work and clean every rustdoc warning by @[object]

### Added

- Botan 3.13.0 + mixed-graph duplicate-Botan detection (post-split port) by @[object]
- Pregenerated bindings — cross builds without host libclang ([#69](https://github.com/rnpgp/rnp-rs/pull/69)) by @[object]

### Fixed

- Hold Botan at 3.12.0 — 3.13.0 transient PKESK decrypt failure (rnp-rs#79, rnpgp/rnp#2465) by @[object]
- Cross-build hardening — toolchain passthrough, no CLI build, no json-c apps by @[object]

### Other

- Split workspace into rnp-src / rnp-sys / rnp (botan-rs pattern) by @[object]
- Collapse nested if into let-chain (clippy collapsible_if) by @[object]
- Cargo fmt by @[object]
- Fix vendored setup guide and add step-by-step tutorial by @[object]

### Fixed

- Remove needless borrow flagged by clippy by @[object]
- Alias zlib's libzlibstatic.a to libz.a on MinGW by @[object]
- Use CMAKE_CXX_STANDARD_LIBRARIES for ws2_32 + crypt32 by @[object]

### Other

- Add README.md for rnp-src crate ([#61](https://github.com/rnpgp/rnp-rs/pull/61)) by @[object]

### Fixed

- Remove is_packaging() — always compile, no heuristics by @[object]
- Is_packaging() via OUT_DIR path, not Cargo.toml.orig by @[object]
- Declare ws2_32 + crypt32 as Botan's INTERFACE_LINK_LIBRARIES by @[object]
