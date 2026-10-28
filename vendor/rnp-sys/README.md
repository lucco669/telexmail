# rnp-sys

Raw FFI bindings to [librnp](https://github.com/rnpgp/rnp) (the OpenPGP C
library that powers Mozilla Thunderbird).

The contents of this crate are entirely machine-generated and mirror the
upstream C API verbatim, including its naming conventions. There are no
safe wrappers here — every function returns raw `rnp_result_t` codes,
every string is `*mut c_char`, every handle is an opaque pointer you
must destroy with the matching `_destroy` function. **You are on your
own for resource management.**

## When to use this crate

| Use `rnp-sys` | Use [`rnp`](https://crates.io/crates/rnp-rs) |
|---|---|
| You need direct FFI access — calling exactly the upstream C function signatures, no abstraction layer in between | You want idiomatic Rust: `Result`, `Drop`, borrow-checked lifetimes, typed algorithm enums |
| You're wrapping librnp into another language binding | You're building an OpenPGP application or plugin |
| You have specific resource-management semantics the safe crate doesn't expose | You want a normal Rust API with cross-validation against [sequoia-openpgp](https://github.com/rsas/sequoia) |

If you're unsure, start with `rnp` (the safe wrapper). You can drop down
to `rnp-sys` later for any function the safe wrapper doesn't expose.

## Quick start

```toml
[dependencies]
]
rnp-sys = "0.1"
```

Then call librnp directly:

```rust
use rnp_sys::{rnp_ffi_create, rnp_ffi_destroy, rnp_version_string,
              rnp_result_t_RNP_SUCCESS};

unsafe {
    // Every call goes through the raw FFI. You are responsible for
    // every error check, every handle, and every string lifetime.
    let mut ffi = std::ptr::null_mut();
    let rc = rnp_ffi_create(&mut ffi, std::ptr::null(), std::ptr::null(),
                            std::ptr::null());
    if rc != rnp_result_t_RNP_SUCCESS {
        panic!("rnp_ffi_create failed: {rc}");
    }

    let version = rnp_version_string();
    println!("librnp {}", std::ffi::CStr::from_ptr(version));

    // Returning a borrowed pointer — do NOT free `version`. The string
    // is owned by librnp and lives for the lifetime of the process.
    // (Check rnp/rnp.h upstream for the exact lifetime contract of
    // any string-returning function you call.)

    rnp_ffi_destroy(ffi);
}
```

For more involved usage, see the upstream
[`include/rnp/rnp.h`](https://github.com/rnpgp/rnp/blob/main/include/rnp/rnp.h)
documentation — every Rust item here corresponds 1:1 to a C function,
type, or constant.

## Linking

By default `rnp-sys` links against a system-installed `librnp`. Use the
`vendored` feature to skip that requirement entirely — the
[`rnp-src`](https://crates.io/crates/rnp-src) crate compiles librnp + all
dependencies from source.

### Vendored (no system librnp)

```toml
[dependencies]
rnp-sys = { version = "0.1", features = ["vendored"] }
```

Build requirements: **C/C++ compiler**,**cmake**, **python3** (for
Botan's `configure.py`). On Windows, use **MSYS2 UCRT64**
(`mingw-w64-ucrt-x86_64-gcc`, `cmake`, `make`, `python3`). First build
takes ~10 min (Botan dominates); cached in `OUT_DIR` after that.

### System librnp

| Platform | Install command |
|---|---|
| **macOS** | `brew install rnp` |
| **Fedora** | `sudo dnf install librnp-devel` |
| **Debian/Ubuntu** | `sudo apt install librnp-dev` |
| **Custom build** | Set `RNP_INCLUDE_DIR` + `RNP_LIB_DIR` to point at your own librnp build |

### PQC and crypto-refresh

Both are gated behind their respective features and require librnp built
with the matching upstream options:

```toml
rnp-sys = { version = "0.1", features = ["vendored", "pqc", "crypto-refresh"] }
```

## Raw-FFI consumers and the `links = "rnp"` constraint

This crate declares `links = "rnp"` so that any crate depending on it
sees `DEP_RNP_LIBRNP_VERSION` (and related) in its own build script. If
you write a downstream binding, depend on `rnp-sys` directly — do not
depend on `rnp` for raw FFI access. `rnp` re-exports everything for
backward compatibility, but the `links` contract lives here.

## Cross builds without a working host libclang

`rnp-sys` ships a pregenerated `bindings/bindings-<librnp-version>.rs`
and uses it automatically whenever the headers are known to match —
i.e. `vendored` builds against librnp 0.18.1. The file is
target-independent (rnp.h is opaque handles + primitives; C types
render as per-target `std::os::raw` aliases). Verified to compile for
`x86_64-unknown-linux-gnu`, `x86_64-unknown-linux-musl`, macOS, and
Windows MSYS2 from the same artifact.

If you're using a different librnp version (system-installed, or you've
cross-compiled your own), set `RNP_BINDINGS_RUNTIME=1` to force
runtime bindgen instead.

## Regenerating the pregenerated bindings

After a librnp version bump:

```sh
scripts/regenerate-bindings.sh
```

That runs a vendored build with bindgen forced on and the experimental
PQC + crypto-refresh defines included, then writes the result back into
`bindings/`. Commit the updated file alongside the version bump.

## License

BSD-2-Clause, matching upstream RNP.