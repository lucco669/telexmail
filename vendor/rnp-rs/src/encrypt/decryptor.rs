//! [`Decryptor`] builder + [`DecryptResult`] + `decrypt`/`decrypt_to` free
//! functions.

use crate::context::Context;
use crate::error::{Result, check};
use crate::ffi;
use crate::ops::{Input, Output};

/// Decrypt `ciphertext` (anything message-shaped: a byte slice or a
/// caller-built [`Input`]) and return the plaintext bytes. Requires that
/// the keyring contain the matching secret key (unlocked) or that a
/// password provider returns the right password.
pub fn decrypt<'s>(
    ctx: &Context,
    ciphertext: impl Into<crate::ops::MessageSource<'s>>,
) -> Result<Vec<u8>> {
    let mut output = Output::to_memory()?;
    decrypt_into(ctx, ciphertext, &mut output)?;
    output.into_bytes()
}

/// Decrypt `ciphertext` (anything message-shaped: a byte slice or a
/// caller-built [`Input`]) and write the plaintext to `output`.
pub fn decrypt_to<'s>(
    ctx: &Context,
    ciphertext: impl Into<crate::ops::MessageSource<'s>>,
    output: &mut Output,
) -> Result<()> {
    decrypt_into(ctx, ciphertext, output).map(|_| ())
}

fn decrypt_into<'s>(
    ctx: &Context,
    ciphertext: impl Into<crate::ops::MessageSource<'s>>,
    output: &mut Output,
) -> Result<()> {
    let mut source = ciphertext.into();
    let input = source.0.take()?;
    unsafe { check(ffi::rnp_decrypt(ctx.ffi, input.as_ptr(), output.as_ptr())) }
}

/// Decrypt a caller-built [`Input`] (e.g. from
/// [`Input::from_reader`](crate::Input::from_reader)) and write the
/// plaintext to `output` — the streaming counterpart of [`decrypt_to`].
///
/// Unlike [`Decryptor`], this is single-pass: the input is consumed exactly
/// once, which is what a non-seekable reader can support. If the reader
/// fails mid-operation, the original [`std::io::Error`] is available via
/// [`Input::io_error`](crate::Input::io_error).
///
/// Deprecated: pass the [`Input`] to [`decrypt_to`] — it accepts both
/// bytes and inputs.
#[deprecated(
    since = "0.2.0",
    note = "pass the Input to decrypt_to; it accepts both"
)]
pub fn decrypt_from_input(ctx: &Context, ciphertext: &Input, output: &mut Output) -> Result<()> {
    unsafe {
        check(ffi::rnp_decrypt(
            ctx.ffi,
            ciphertext.as_ptr(),
            output.as_ptr(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Decryptor builder + DecryptResult
// ---------------------------------------------------------------------------

/// Builder over a decryption operation. Mirrors
/// [`Encryptor`](super::Encryptor) for symmetry.
///
/// Returns a [`DecryptResult`] that exposes both the plaintext bytes and
/// the rich metadata surface (recipient / symenc / file info / protection
/// info / format) by delegating to the underlying verify-op result.
///
/// For the simple "just give me the plaintext" case, prefer
/// [`decrypt`] — it's a thinner wrapper over `rnp_decrypt` and avoids the
/// verify-op overhead.
///
/// `build` consumes the ciphertext twice (see below), so `Decryptor` only
/// accepts byte slices. To decrypt from a one-shot stream (a reader that
/// cannot be rewound), use [`decrypt_from_input`] instead.
///
/// ```
/// # use rnp::{Algorithm, Context, Decryptor, Encryptor, KeyBuilder, KeyUsage};
/// # let ctx = Context::new().unwrap();
/// # let key = KeyBuilder::new(Algorithm::Rsa).bits(2048)
/// #     .userid("enc <enc@example.com>").add_usage(KeyUsage::EncryptComms)
/// #     .build(&ctx).unwrap();
/// # let mut ct = rnp::Output::to_memory().unwrap();
/// # Encryptor::new(&ctx, b"secret").unwrap().add_recipient(&key).build(&mut ct).unwrap();
/// # let ciphertext = ct.into_bytes().unwrap();
/// let result = Decryptor::new(&ctx, &ciphertext).build().unwrap();
/// let _plaintext = result.plaintext();
/// let _recipients = result.recipient_count().unwrap();
/// ```
pub struct Decryptor<'a, 'ctx> {
    ctx: &'ctx Context,
    ciphertext: &'a [u8],
}

impl<'a, 'ctx> Decryptor<'a, 'ctx> {
    pub fn new(ctx: &'ctx Context, ciphertext: &'a [u8]) -> Self {
        Decryptor { ctx, ciphertext }
    }

    /// Execute the decryption, returning the plaintext and rich metadata.
    ///
    /// This runs the decryption pipeline twice: once via `rnp_decrypt` to
    /// extract the plaintext bytes, and once via `rnp_op_verify_*` to
    /// collect recipient/symenc/file-info metadata. The double pass is
    /// necessary because the verify-op surface doesn't expose its output
    /// bytes after `execute` (the output is destroyed with the op).
    pub fn build(self) -> Result<DecryptResult<'ctx>> {
        // Pass 1: plaintext via rnp_decrypt.
        let plaintext = {
            let input = Input::from_memory(self.ciphertext)?;
            let output = Output::to_memory()?;
            unsafe {
                check(ffi::rnp_decrypt(
                    self.ctx.ffi,
                    input.as_ptr(),
                    output.as_ptr(),
                ))?;
            }
            output.into_bytes()?
        };

        // Pass 2: metadata via rnp_op_verify. Output goes to /dev/null.
        let null_out = Output::to_null()?;
        let op = crate::verify::VerifyOp::inline(self.ctx, self.ciphertext, null_out)?;
        let verify_result = op.execute()?;

        Ok(DecryptResult {
            verify_result,
            plaintext,
        })
    }
}

/// Result of a decryption operation. Exposes the plaintext bytes plus the
/// underlying [`VerifyResult`](crate::VerifyResult) (which carries
/// recipient / symenc / file info / protection info / format metadata).
pub struct DecryptResult<'ctx> {
    verify_result: crate::verify::VerifyResult<'ctx>,
    plaintext: Vec<u8>,
}

impl<'ctx> DecryptResult<'ctx> {
    /// The decrypted plaintext bytes.
    pub fn plaintext(&self) -> &[u8] {
        &self.plaintext
    }

    /// Consume the result and return the owned plaintext bytes.
    pub fn into_plaintext(self) -> Vec<u8> {
        self.plaintext
    }

    /// Borrow the underlying verify result for full metadata access
    /// (per-signature status, etc.).
    pub fn verify_result(&self) -> &crate::verify::VerifyResult<'ctx> {
        &self.verify_result
    }

    /// Number of public-key recipients on the encrypted message.
    pub fn recipient_count(&self) -> Result<usize> {
        self.verify_result.recipient_count()
    }

    /// Borrow the recipient at `idx`.
    pub fn recipient_at(&self, idx: usize) -> Result<Option<crate::verify::Recipient<'_>>> {
        self.verify_result.recipient_at(idx)
    }

    /// The recipient that actually decrypted the message (i.e. matched
    /// a key in the keyring). None if decrypted via a password.
    pub fn used_recipient(&self) -> Result<Option<crate::verify::Recipient<'_>>> {
        self.verify_result.used_recipient()
    }

    /// Number of password-based (symmetric) recipients.
    pub fn symenc_count(&self) -> Result<usize> {
        self.verify_result.symenc_count()
    }

    /// Borrow the symenc at `idx`.
    pub fn symenc_at(&self, idx: usize) -> Result<Option<crate::verify::Symenc<'_>>> {
        self.verify_result.symenc_at(idx)
    }

    /// The symenc that actually decrypted the message (matched a supplied
    /// password). None if decrypted via a public-key recipient.
    pub fn used_symenc(&self) -> Result<Option<crate::verify::Symenc<'_>>> {
        self.verify_result.used_symenc()
    }

    /// File metadata (name + mtime) from the literal-data packet.
    pub fn file_info(&self) -> Result<Option<crate::verify::FileInfo>> {
        self.verify_result.file_info()
    }

    /// `(mode, cipher, valid)` — protection info of the encrypted stream.
    /// `mode` is one of `"none"`, `"cfb"`, `"cfb-mdc"`, `"aead-ocb"`,
    /// `"aead-eax"`. `valid` is true iff MDC/AEAD integrity was verified.
    pub fn protection_info(&self) -> Result<(String, String, bool)> {
        self.verify_result.protection_info()
    }

    /// Format character of the literal data (`'b'`, `'t'`, `'u'`, etc.).
    pub fn format(&self) -> Result<char> {
        self.verify_result.format()
    }
}
