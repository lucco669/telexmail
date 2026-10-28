//! Round-trip coverage for [`rnp::Input`] and [`rnp::Output`].
//!
//! These tests exercise the RAII handles in isolation — no OpenPGP keys or
//! signing ops, just byte-stream plumbing. The integration tests in
//! `tests/sign_verify.rs` cover the same types via real librnp operations.

use rnp::{ArmorType, Input, Output, OutputFileFlags};

/// `Output::write` should append bytes that `into_bytes` returns.
#[test]
fn output_memory_write_and_drain() {
    let mut out = Output::to_memory().expect("memory output");
    out.write(b"hello, ").expect("write 1");
    out.write(b"world").expect("write 2");
    let bytes = out.into_bytes().expect("drain");
    assert_eq!(bytes, b"hello, world");
}

/// `Output::pipe` copies an `Input` through to an `Output` until EOF.
#[test]
fn output_pipe_copies_input_bytes() {
    let payload = b"the quick brown fox jumps over the lazy dog";
    let input = Input::from_memory(payload).expect("input");
    let mut output = Output::to_memory().expect("output");
    output.pipe(&input).expect("pipe");
    let drained = output.into_bytes().expect("drain");
    assert_eq!(drained.as_slice(), payload.as_slice());
}

/// `Output::to_null` accepts writes without complaining and frees on drop.
#[test]
fn output_null_accepts_writes() {
    let mut out = Output::to_null().expect("null output");
    out.write(b"discarded").expect("write to null");
    // `into_bytes` would fail on a null output, so just let it drop here.
    drop(out);
}

/// `Output::to_armor` wraps a base output and produces ASCII-armored output
/// in the base's buffer. The armor wrapper is flushed when dropped; the
/// bytes are drained from the underlying memory output, not the wrapper.
#[test]
fn output_armor_wraps_with_header() {
    let base = Output::to_memory().expect("base output");
    {
        let mut armor = Output::to_armor(&base, ArmorType::Message).expect("armor output");
        armor.write(b"some bytes to armor").expect("write");
        armor.finish().expect("finish armor");
    }
    let armored = base.into_bytes().expect("drain base");
    let s = String::from_utf8(armored).expect("armor is ascii");
    assert!(
        s.starts_with("-----BEGIN PGP MESSAGE-----"),
        "armor output should start with the PGP header, got: {s}"
    );
}

/// `Output::to_path` writes to a real file. Round-trip with stdlib `read`.
#[test]
fn output_path_writes_real_file() {
    let dir = tempdir();
    let path = dir.join("output.bin");
    let path_str = path.to_str().expect("utf-8 path");

    let mut out = Output::to_path(path_str).expect("path output");
    out.write(b"file contents").expect("write");
    out.finish().expect("finish");
    drop(out);

    let on_disk = std::fs::read(&path).expect("read file back");
    assert_eq!(on_disk, b"file contents");
}

/// `Output::to_file` with `OVERWRITE` replaces an existing file.
#[test]
fn output_file_overwrite() {
    let dir = tempdir();
    let path = dir.join("overwrite.bin");
    std::fs::write(&path, b"old contents").expect("seed old file");

    let path_str = path.to_str().expect("utf-8 path");
    let mut out = Output::to_file(path_str, OutputFileFlags::OVERWRITE).expect("file output");
    out.write(b"new").expect("write");
    drop(out);

    let on_disk = std::fs::read(&path).expect("read");
    assert_eq!(on_disk, b"new");
}

/// `Input::from_path` opens a real file for reading.
#[test]
fn input_from_path_reads_back() {
    let dir = tempdir();
    let path = dir.join("input.bin");
    std::fs::write(&path, b"from disk").expect("seed file");

    let input = Input::from_path(path.to_str().expect("utf-8 path")).expect("input");
    let mut output = Output::to_memory().expect("output");
    output.pipe(&input).expect("pipe");
    let drained = output.into_bytes().expect("drain");
    assert_eq!(drained, b"from disk");
}

/// Minimal tempdir helper — avoids pulling the `tempfile` crate dep for a
/// handful of tests.
fn tempdir() -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    p.push(format!("rnp-rs-test-{}-{}", std::process::id(), id));
    std::fs::create_dir_all(&p).expect("create temp dir");
    // Best-effort cleanup when the test process exits. Leaks are harmless
    // (system temp).
    let p_clone = p.clone();
    ctrlc::register_on_drop(p_clone);
    p
}

// No external crate; we just leak the temp dir. Tests above rely on `temp_dir`
// returning a writable location and don't depend on cleanup.
mod ctrlc {
    use std::path::PathBuf;
    pub fn register_on_drop(_p: PathBuf) {}
}
