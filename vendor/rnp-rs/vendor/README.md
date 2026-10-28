# vendor/

When the `vendored` Cargo feature is enabled, `build.rs` invokes CMake on
`vendor/rnp/` to build librnp statically and link it into the crate.

The `rnp/` submodule is **not** initialized by default — initialize it
explicitly:

```sh
git submodule add https://github.com/rnpgp/rnp.git vendor/rnp
git -C vendor/rnp checkout v0.18.1   # or whichever release you want
git -C vendor/rnp submodule update --init --recursive
```

## Required system libraries

librnp depends on:

- **Botan** ≥ 2.14 (default backend) or **Botan 3.x** if you want PQC /
  crypto-refresh support. Set `RNP_VENDOR_BACKEND=botan` (default) or
  `openssl`.
- **JSON-C** ≥ 0.11
- **zlib**
- **bzip2** (optional, auto-detected)
- **sexpp** — bundled as `vendor/rnp/src/libsexpp` (initialized by the
  recursive submodule update above)

Install via Homebrew on macOS (`brew install botan json-c zlib`) or your
distro's package manager.

## Optional: enable PQC / crypto-refresh

```sh
cargo build --features vendored,pqc,crypto-refresh
```

This passes `-DENABLE_PQC=ON -DENABLE_CRYPTO_REFRESH=ON` to the librnp
CMake build. Requires Botan 3.6+ built with the corresponding modules.

## Passing extra CMake args

Set `RNP_VENDOR_CMAKE_ARGS` to a whitespace-separated list of `KEY=VALUE`
pairs:

```sh
RNP_VENDOR_CMAKE_ARGS="ENABLE_SM2=ON ENABLE_BRAINPOOL=ON" \
    cargo build --features vendored
```
