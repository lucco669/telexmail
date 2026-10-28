# rnp-src

Build scripts for compiling [librnp](https://github.com/rnpgp/rnp) and all its
C/C++ dependencies from source.

## What it does

When the `vendored` Cargo feature is enabled on the parent [`rnp-rs`](https://crates.io/crates/rnp-rs)
crate, `rnp-src`'s build script downloads and compiles:

| Dependency | Version | Purpose |
|---|---|---|
| **Botan** | 3.12.0 (via [`botan-src`](https://crates.io/crates/botan-src)) | Cryptographic backend |
| **json-c** | 0.17 | JSON parsing (librnp dependency) |
| **zlib** | 1.3.1 | Compression |
| **bzip2** | 1.0.8 | Compression (with `bz_internal_error` shim) |
| **librnp** | 0.18.1 (or HEAD — see below) | OpenPGP implementation (the thing we actually wrap) |

All five are statically linked into the final Rust binary. No system libraries required.

Source tarballs are downloaded at build time via pure-Rust HTTP + decompression
(ureq + flate2 + tar), so `curl` and `tar` are **not** required on the host.

## Usage

You don't use this crate directly. It's pulled in automatically when you enable
the `vendored` feature on `rnp-rs`:

```toml
[dependencies]
rnp = { version = "0.1", features = ["vendored"] }
```

## Cargo features

| Feature | Description |
|---|---|
| `pqc` | Builds librnp with `ENABLE_PQC=ON` and Botan with PQC modules (ML-KEM, ML-DSA, SLH-DSA). Clones librnp HEAD instead of 0.18.1 (PQC code paths in 0.18.1 are incompatible with Botan 3.12's opaque EC types). |
| `crypto-refresh` | Builds librnp with `ENABLE_CRYPTO_REFRESH=ON`. Same HEAD-vs-0.18.1 reasoning as `pqc`. |

When **neither** feature is on, rnp-src uses the librnp **0.18.1** release
tarball. When **either** is on, rnp-src clones librnp **HEAD** (pinned to
`main`) and patches it for Botan 3.12 compatibility.

## Platform support

| Platform | Status | Toolchain |
|---|---|---|
| Linux x86_64 | Tested in CI | gcc/g++ |
| Linux arm64 | Tested in CI | gcc/g++ |
| macOS Intel | Tested in CI | clang/clang++ |
| macOS ARM (Apple Silicon) | Tested in CI | clang/clang++ |
| Windows x86_64 | Tested in CI (MSYS2 UCRT64) | mingw-w64 gcc/g++ |

## Build requirements

- **C/C++ compiler**: gcc/g++ (Linux), clang/clang++ (macOS), or mingw-w64-ucrt-x86_64-gcc/g++ (Windows MSYS2 UCRT64)
- **cmake** >= 3.5
- **python3** (for Botan's `configure.py`)
- **make** (for bzip2 and Botan; not needed on platforms with Ninja-only cmake)

No `curl`, `tar`, or `bsdtar` needed — source downloads are pure Rust
(ureq + flate2 + tar). `git` is needed only when the `pqc` or
`crypto-refresh` feature is on (librnp HEAD is cloned via git).

First build takes ~5-10 min (Botan dominates); cached in `OUT_DIR` after that.

### Steering the compilers

rnp-src picks the compilers for the librnp (and json-c/zlib/bzip2) builds
with one precedence:

1. **`CC` / `CXX` environment variables** — set both when the two differ
   in name. This is how you build on compiler-toolchain-pure environments
   such as MSYS2 **clangarm64** (aarch64 Windows, LLVM only — no gcc
   exists there): `export CC=clang CXX=clang++`.
2. **Platform defaults**: `/usr/bin/clang`/`/usr/bin/clang++` on a macOS
   target, `gcc`/`g++` everywhere else.

A full CMake toolchain file overrides both: set
`RNP_CMAKE_TOOLCHAIN=/path/to/toolchain.cmake`. Individual extra flags
can be passed with `RNP_CMAKE_ARGS` (split on whitespace).

### Windows setup (MSYS2 UCRT64)

```sh
# In an MSYS2 UCRT64 shell:
pacman -S --needed \
  mingw-w64-ucrt-x86_64-gcc \
  mingw-w64-ucrt-x86_64-cmake \
  mingw-w64-ucrt-x86_64-make \
  mingw-w64-ucrt-x86_64-python3 \
  make
```

## License

BSD-2-Clause, matching [RNP](https://github.com/rnpgp/rnp).
