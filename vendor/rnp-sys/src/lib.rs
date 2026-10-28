//! Raw FFI bindings to the `librnp` C API (`rnp/rnp.h`), generated at build
//! time by bindgen — or taken from the pregenerated `bindings/` file when
//! the headers are known to match, so cross builds don't need a working
//! host-side libclang.
//!
//! The contents of this crate are entirely machine-generated and mirror the
//! upstream C API verbatim, including its naming conventions. The lint
//! suppressions below acknowledge that: C identifiers are not Rust-idiomatic
//! and we do not want to fight the compiler over them.
//!
//! Safe, idiomatic wrappers live in the `rnp` crate (`rnp-rs`); most users
//! want that instead.
//!
//! The upstream doc comments copied by bindgen contain fenced C/JSON
//! fragments that rustdoc misreads as Rust code blocks; that is noise from
//! the C source, not something we can fix here.

#![allow(
    non_camel_case_types,
    non_snake_case,
    dead_code,
    non_upper_case_globals
)]
#![allow(rustdoc::invalid_rust_codeblocks)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
