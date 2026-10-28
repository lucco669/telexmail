//! Streaming I/O: reader-backed [`Input`]s, writer-backed [`Output`]s, the
//! `*_with_input` op constructors, and the error/panic/discard semantics of
//! the callback plumbing (see `src/ops/io/stream.rs`).

mod common;
use common::{encryption_key, signing_key};

use std::io::{ErrorKind, Read, Write};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use rnp::algorithm::Algorithm;
use rnp::{
    Context, Decryptor, EncryptFlags, Encryptor, Error, ExportFlags, Input, KeyBuilder, KeyUsage,
    KeyringFormat, LoadSaveFlags, Mode, Output, Signer, VerifyOp,
};

/// A reader that yields its bytes and then fails with a caller-chosen
/// error. Used to exercise mid-stream read failures.
struct FailingReader {
    bytes: std::io::Cursor<Vec<u8>>,
    error: std::io::Error,
}

impl FailingReader {
    fn new(bytes: Vec<u8>, error: std::io::Error) -> Self {
        FailingReader {
            bytes: std::io::Cursor::new(bytes),
            error,
        }
    }
}

impl Read for FailingReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.bytes.position() as usize >= self.bytes.get_ref().len() {
            return Err(std::mem::replace(
                &mut self.error,
                std::io::Error::other("already failed"),
            ));
        }
        self.bytes.read(buf)
    }
}

/// A writer that records writes, flushes, and failures in shared state.
#[derive(Clone, Default)]
struct RecordingWriter {
    inner: Arc<Mutex<Vec<u8>>>,
    flushed: Arc<AtomicBool>,
    fail_after: Option<Arc<AtomicUsize>>, // when Some(n): fail once n bytes written
}

impl RecordingWriter {
    fn new() -> Self {
        Self::default()
    }

    fn failing_after(n: usize) -> Self {
        RecordingWriter {
            fail_after: Some(Arc::new(AtomicUsize::new(n))),
            ..Self::default()
        }
    }

    fn bytes(&self) -> Vec<u8> {
        self.inner.lock().unwrap().clone()
    }
}

impl Write for RecordingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Some(remaining) = &self.fail_after {
            let left = remaining.load(Ordering::SeqCst);
            if left == 0 {
                return Err(std::io::Error::other("writer failed"));
            }
            let n = buf.len().min(left);
            self.inner.lock().unwrap().extend_from_slice(&buf[..n]);
            remaining.store(left - n, Ordering::SeqCst);
            if n < buf.len() {
                return Err(std::io::Error::other("writer failed"));
            }
            return Ok(n);
        }
        self.inner.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.flushed.store(true, Ordering::SeqCst);
        Ok(())
    }
}

fn assert_io_error(err: &Error, kind: ErrorKind) {
    match err {
        Error::Io { source } => assert_eq!(source.kind(), kind, "wrong io error: {source}"),
        other => panic!("expected Error::Io, got: {other:?}"),
    }
}

// --- Reader/writer plumbing ---------------------------------------------------

#[test]
fn reader_input_pipes_to_writer_output() {
    let payload: Vec<u8> = (0..=255u8).cycle().take(10_000).collect();
    let input = Input::from_reader(std::io::Cursor::new(payload.clone())).expect("reader input");
    let sink = RecordingWriter::new();
    let mut output = Output::to_writer(sink.clone()).expect("writer output");
    output.pipe(&input).expect("pipe");
    drop(output);
    drop(input);
    assert_eq!(sink.bytes(), payload);
    assert!(sink.flushed.load(Ordering::SeqCst), "pipe success flushes");
}

#[test]
fn empty_reader_signals_eof() {
    let input = Input::from_reader(std::io::Cursor::new(Vec::new())).expect("reader input");
    let mut output = Output::to_memory().expect("output");
    output.pipe(&input).expect("pipe of empty input");
    assert_eq!(output.into_bytes().unwrap(), Vec::<u8>::new());
}

#[test]
fn into_reader_reclaims_the_reader() {
    let input = Input::from_reader(std::io::Cursor::new(b"reclaim me".to_vec())).expect("input");
    let (mut reader, err) = input.into_reader().expect("reader-backed input");
    assert!(err.is_none());
    let mut rest = Vec::new();
    reader.read_to_end(&mut rest).expect("read rest");
    assert_eq!(rest, b"reclaim me");

    // Non-reader-backed inputs return None.
    let mem = Input::from_memory(b"x").expect("memory input");
    assert!(mem.into_reader().is_none());
}

// --- Sign / verify through streams -------------------------------------------

#[test]
fn sign_via_reader_and_verify_via_reader() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "stream-sign <s@example.com>");
    let message: Vec<u8> = (0..=255u8).cycle().take(200_000).collect();

    let input = Input::from_reader(std::io::Cursor::new(message.clone())).expect("input");
    let mut output = Output::to_memory().expect("output");
    Signer::new(&ctx, input, Mode::Detached)
        .add_signer(&key)
        .build(&mut output)
        .expect("streamed sign");
    let signature = output.into_bytes().expect("signature bytes");

    let msg_input = Input::from_reader(std::io::Cursor::new(message)).expect("msg input");
    let sig_input = Input::from_reader(std::io::Cursor::new(signature)).expect("sig input");
    let op = VerifyOp::detached(&ctx, msg_input, sig_input).expect("detached op");
    let result = op.execute().expect("streamed verify");
    assert!(
        result.any_valid().expect("validity"),
        "signature should verify"
    );
}

#[test]
fn inline_verify_via_reader_writes_plaintext_to_writer() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "stream-inl <si@example.com>");
    let message = b"inline signed, streamed verified";

    let signed = Signer::new(&ctx, message, Mode::Inline)
        .add_signer(&key)
        .build_to_memory()
        .expect("inline sign");

    let input = Input::from_reader(std::io::Cursor::new(signed)).expect("input");
    let sink = RecordingWriter::new();
    let output = Output::to_writer(sink.clone()).expect("writer output");
    let op = VerifyOp::inline(&ctx, input, output).expect("inline op");
    let result = op.execute().expect("streamed inline verify");
    assert!(result.any_valid().expect("validity"));
    assert_eq!(sink.bytes(), message);
}

#[test]
fn sign_to_writer_produces_same_bytes_as_memory() {
    let ctx = Context::new().expect("ctx");
    // Inline signatures embed creation time; pin the clock so the two
    // signings below are byte-identical even across a second boundary.
    ctx.set_timestamp(1_700_000_000).expect("set_timestamp");
    let key = signing_key(&ctx, "stream-sign2 <s2@example.com>");
    let message = b"a message signed to a writer";

    let via_memory = Signer::new(&ctx, message, Mode::Inline)
        .add_signer(&key)
        .build_to_memory()
        .expect("memory sign");

    let sink = RecordingWriter::new();
    {
        let mut output = Output::to_writer(sink.clone()).expect("writer output");
        Signer::new(&ctx, message, Mode::Inline)
            .add_signer(&key)
            .build(&mut output)
            .expect("sign to writer");
    }
    assert_eq!(sink.bytes(), via_memory);
    assert!(
        sink.flushed.load(Ordering::SeqCst),
        "writer flushed on success"
    );
}

// --- Encrypt / decrypt through streams ---------------------------------------

#[test]
fn encrypt_via_reader_and_decrypt_via_reader() {
    let ctx = Context::new().expect("ctx");
    let key = encryption_key(&ctx, "stream-enc <e@example.com>");
    let secret: Vec<u8> = (0..=255u8).cycle().take(150_000).collect();

    let sink = RecordingWriter::new();
    {
        let input = Input::from_reader(std::io::Cursor::new(secret.clone())).expect("input");
        let mut output = Output::to_writer(sink.clone()).expect("writer output");
        Encryptor::new(&ctx, input)
            .expect("encryptor")
            .add_recipient(&key)
            .build(&mut output)
            .expect("streamed encrypt");
    }
    let ciphertext = sink.bytes();
    assert!(!ciphertext.is_empty());
    assert_ne!(ciphertext, secret);

    let ct_input = Input::from_reader(std::io::Cursor::new(ciphertext)).expect("ct input");
    let mut plaintext_out = Output::to_memory().expect("plaintext output");
    rnp::decrypt_to(&ctx, ct_input, &mut plaintext_out).expect("streamed decrypt");
    assert_eq!(plaintext_out.into_bytes().unwrap(), secret);
}

#[test]
fn streamed_ciphertext_decrypts_via_decryptor_after_materializing() {
    let ctx = Context::new().expect("ctx");
    let key = encryption_key(&ctx, "stream-enc2 <e2@example.com>");
    let secret = b"small secret for the rich path";

    let mut ct_out = Output::to_memory().expect("ct output");
    Encryptor::new(&ctx, secret)
        .expect("encryptor")
        .add_recipient(&key)
        .build(&mut ct_out)
        .expect("encrypt");
    let ciphertext = ct_out.into_bytes().expect("ct bytes");

    // Materialize the ciphertext from a stream, then use the rich
    // (double-pass) Decryptor on the re-readable copy.
    let ct_input = Input::from_reader(std::io::Cursor::new(ciphertext)).expect("ct input");
    let (mut reader, _) = ct_input.into_reader().expect("reader-backed input");
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).expect("read all");

    let result = Decryptor::new(&ctx, &bytes).build().expect("decryptor");
    assert_eq!(result.plaintext(), secret);
}

// --- Key material through a stream -------------------------------------------

#[test]
fn keys_roundtrip_through_reader_input() {
    let ctx = Context::new().expect("ctx");
    let key = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("export-me <exp@example.com>")
        .add_usage(KeyUsage::Sign)
        .add_usage(KeyUsage::Certify)
        .build(&ctx)
        .expect("key");

    let exported = key
        .export(ExportFlags::PUBLIC | ExportFlags::ARMORED)
        .expect("export");

    let input = Input::from_reader(std::io::Cursor::new(exported)).expect("reader input");
    ctx.load_keys(KeyringFormat::Gpg, input, LoadSaveFlags::PUBLIC)
        .expect("load from reader");
    assert!(ctx.public_key_count().expect("count") >= 1);
}

#[test]
fn import_keys_from_input_returns_status() {
    let ctx = Context::new().expect("ctx");
    let key = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("import-me <imp@example.com>")
        .add_usage(KeyUsage::Sign)
        .add_usage(KeyUsage::Certify)
        .build(&ctx)
        .expect("key");
    let exported = key
        .export(ExportFlags::PUBLIC | ExportFlags::ARMORED)
        .expect("export");

    // Import into a fresh context so the key counts as new there.
    let other = Context::new().expect("fresh ctx");
    let input = Input::from_reader(std::io::Cursor::new(exported)).expect("reader input");
    let status = other
        .import_keys(input, LoadSaveFlags::PUBLIC)
        .expect("import status");
    assert!(
        status.contains("\"public\":\"new\""),
        "unexpected status JSON: {status}"
    );
}

// --- Failure semantics --------------------------------------------------------

#[test]
fn reader_io_error_is_surfaced_by_the_op() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "fail-read <fr@example.com>");

    let reader = FailingReader::new(
        b"partial data".to_vec(),
        std::io::Error::new(ErrorKind::PermissionDenied, "read denied"),
    );
    let input = Input::from_reader(reader).expect("input");

    let mut output = Output::to_memory().expect("output");
    let err = Signer::new(&ctx, input, Mode::Detached)
        .add_signer(&key)
        .build(&mut output)
        .expect_err("op must fail when the reader fails");
    assert_io_error(&err, ErrorKind::PermissionDenied);
}

#[test]
fn verify_reader_io_error_is_surfaced_by_the_op() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "fail-verify <fv@example.com>");

    // Produce a real detached signature so the failure can only come from
    // the denied message reader.
    let sig_bytes = Signer::new(&ctx, b"message", Mode::Detached)
        .add_signer(&key)
        .build_to_memory()
        .expect("sign");

    struct DenyingReader;
    impl Read for DenyingReader {
        fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(ErrorKind::BrokenPipe, "nope"))
        }
    }

    let msg = Input::from_reader(DenyingReader).expect("msg input");
    let sig = Input::from_memory(&sig_bytes).expect("sig input");
    let op = VerifyOp::detached(&ctx, msg, sig).expect("op");
    match op.execute() {
        Err(err) => assert_io_error(&err, ErrorKind::BrokenPipe),
        Ok(_) => panic!("verifying a denied reader must fail"),
    }
}

#[allow(deprecated)]
#[test]
fn decrypt_reader_io_error_is_inspectable() {
    // The deprecated decrypt_from_input is deliberately kept for exactly
    // this shape: it borrows the input, so the caller retains access to
    // the recorded io error via Input::io_error() afterwards.
    let ctx = Context::new().expect("ctx");

    let reader = FailingReader::new(
        b"\xc1\x03".to_vec(), // plausible packet prefix, then EOF
        std::io::Error::new(ErrorKind::UnexpectedEof, "truncated"),
    );
    let input = Input::from_reader(reader).expect("input");
    let mut output = Output::to_memory().expect("output");
    let res = rnp::decrypt_from_input(&ctx, &input, &mut output);
    if res.is_err() {
        let io = input
            .io_error()
            .expect("io error recorded on the borrowed input");
        assert_eq!(io.kind(), ErrorKind::UnexpectedEof);
    } else {
        panic!("decrypting a truncated stream must fail");
    }
}

#[test]
fn writer_io_error_is_retrievable_and_discarded() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "fail-write <fw@example.com>");
    let message = vec![0x41u8; 100_000]; // large enough to cross chunk writes

    let sink = RecordingWriter::failing_after(10);
    let mut output = Output::to_writer(sink.clone()).expect("writer output");
    let res = Signer::new(&ctx, &message, Mode::Inline)
        .add_signer(&key)
        .build(&mut output);
    assert!(res.is_err(), "op must fail when the writer fails");

    let outcome = output.into_writer().expect("writer-backed output");
    let io_err = outcome.io_error.expect("io error recorded");
    assert_eq!(io_err.to_string(), "writer failed");
    assert!(
        outcome.discarded,
        "librnp should request a discard on failure"
    );
    assert!(!sink.flushed.load(Ordering::SeqCst), "no flush on discard");
}

#[test]
fn writer_outcome_on_success_is_flushed_and_kept() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "ok-write <ow@example.com>");

    let sink = RecordingWriter::new();
    let output = {
        let mut output = Output::to_writer(sink.clone()).expect("writer output");
        Signer::new(&ctx, b"flush me", Mode::Detached)
            .add_signer(&key)
            .build(&mut output)
            .expect("sign");
        output
    };
    let outcome = output.into_writer().expect("writer-backed output");
    assert!(outcome.io_error.is_none());
    assert!(!outcome.discarded);
    assert!(sink.flushed.load(Ordering::SeqCst));
    assert!(!sink.bytes().is_empty());
}

#[test]
fn reader_panic_propagates_on_drop() {
    struct PanickingReader;
    impl Read for PanickingReader {
        fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            panic!("reader exploded");
        }
    }

    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(|| {
        let ctx = Context::new().expect("ctx");
        let key = signing_key(&ctx, "panic-read <pr@example.com>");
        let mut output = Output::to_memory().expect("output");
        let _ = Signer::new(
            &ctx,
            Input::from_reader(PanickingReader).expect("input"),
            Mode::Detached,
        )
        .add_signer(&key)
        .build(&mut output);
        // `input` was consumed by the builder; it drops inside `build`,
        // resuming the stashed panic there — so build() itself unwinds.
    });
    std::panic::set_hook(prev_hook);
    assert!(result.is_err(), "panics must propagate, not vanish");
}

#[test]
fn writer_panic_propagates_on_into_writer() {
    struct PanickingWriter;
    impl Write for PanickingWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            panic!("writer exploded");
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(|| {
        let ctx = Context::new().expect("ctx");
        let key = signing_key(&ctx, "panic-write <pw@example.com>");
        let mut output = Output::to_writer(PanickingWriter).expect("writer output");
        let _ = Signer::new(&ctx, b"data", Mode::Inline)
            .add_signer(&key)
            .build(&mut output);
        // The op failed; reclaiming the writer resumes the stashed panic.
        let _ = output.into_writer();
    });
    std::panic::set_hook(prev_hook);
    assert!(result.is_err(), "panics must propagate, not vanish");
}

// --- Feature-gated surfaces stay coherent ------------------------------------

#[test]
fn encryptor_flags_still_apply_on_streamed_input() {
    let ctx = Context::new().expect("ctx");
    let key = encryption_key(&ctx, "stream-fl <sf@example.com>");

    let input = Input::from_reader(std::io::Cursor::new(b"flagged".to_vec())).expect("input");
    let mut output = Output::to_memory().expect("output");
    Encryptor::new(&ctx, input)
        .expect("encryptor")
        .add_recipient(&key)
        .flags(EncryptFlags::default())
        .armor(true)
        .build(&mut output)
        .expect("encrypt with flags");
    let bytes = output.into_bytes().expect("ct bytes");
    assert!(
        bytes.starts_with(b"-----BEGIN PGP MESSAGE-----"),
        "armored output expected"
    );
}

// The deprecated twin constructors remain as thin shims over the generic
// MessageSource constructors; keep them exercised so they cannot rot.
#[allow(deprecated)]
#[test]
fn deprecated_input_shims_still_work() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "shims <shims@example.com>");

    let signed =
        Signer::new_with_input(&ctx, Input::from_memory(b"via shim").unwrap(), Mode::Inline)
            .add_signer(&key)
            .build_to_memory()
            .expect("sign via shim");

    let op = VerifyOp::inline_with_input(
        &ctx,
        Input::from_memory(&signed).unwrap(),
        Output::to_memory().unwrap(),
    )
    .expect("inline shim");
    let result = op.execute().expect("verify via shim");
    assert!(result.any_valid().expect("validity"));

    let mut out = Output::to_memory().unwrap();
    let ct = {
        let enc_key = encryption_key(&ctx, "shims-enc <se@example.com>");
        let mut ct_out = Output::to_memory().unwrap();
        Encryptor::new_with_input(&ctx, Input::from_memory(b"secret").unwrap())
            .expect("encryptor shim")
            .add_recipient(&enc_key)
            .build(&mut ct_out)
            .expect("encrypt via shim");
        ct_out.into_bytes().unwrap()
    };
    rnp::decrypt_from_input(&ctx, &Input::from_memory(&ct).unwrap(), &mut out)
        .expect("decrypt via shim");
    assert_eq!(out.into_bytes().unwrap(), b"secret");
}
