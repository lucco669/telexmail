# Signed and encrypted mail in Telex

Telex reads and writes OpenPGP mail: signed and encrypted messages that Thunderbird, GnuPG
(with Mutt, Evolution, KMail, Claws), The Bat!, Proton, OpenKeychain, Delta Chat and other
OpenPGP programs send and read. Everything happens on the user's PC; no key server, no account,
no network beyond the mail servers.

S/MIME (certificates from a .p12/.pfx file) is not done yet: see "Not yet" at the end.

## The library

[rPGP](https://github.com/rpgp/rpgp) (the `pgp` crate, 0.21): OpenPGP in pure Rust, MIT or
Apache-2.0, audited twice (2019, 2024 by Radically Open Security; the findings were fixed), and
used in Delta Chat, which sends millions of encrypted messages. Telex is freeware with all rights
reserved, so LGPL or GPL code can't be in it: that rules out Sequoia (LGPL-2.0) and GnuPG's GPGME.
rPGP handles packets, keys, signatures and encryption (RFC 9580, and RFC 4880 for older mail);
`crates/telex-core/src/openpgp.rs` decides what they mean: which key a signature is from and
whether that key is the sender's, which subkey of a key may encrypt, which of the user's keys can
decrypt a message.

BZip2 compression (rare; GnuPG uses ZIP and ZLIB) is left out of the build; such a message says
it is in a format Telex doesn't read. Every crate rPGP brings is MIT, Apache-2.0 or BSD
(THIRD-PARTY-NOTICES.txt).

## Reading

What Telex recognizes:

- **PGP/MIME** (RFC 3156): `multipart/signed` with `application/pgp-signature`, and
  `multipart/encrypted` with `application/pgp-encrypted`, at the top of the message or inside it
  (a mailing list's footer around it).
- **Inline PGP**: a `-----BEGIN PGP SIGNED MESSAGE-----` block (clearsigned text) or a
  `-----BEGIN PGP MESSAGE-----` block in the plain text.

Above the message a line says what was found:

| Signature        | Shown as                                                                 |
|------------------|--------------------------------------------------------------------------|
| good             | Signed by *name*: a valid signature from a key the user confirmed (their own, one imported from a file, or one they confirmed), and the key's addresses include the From address |
| unconfirmed      | Valid, from a key of the From address that came with mail (Autocrypt, an attached key): its fingerprint and a Confirm key button |
| other sender     | Valid, but the key belongs to another address than From                    |
| no key           | Signed with a key Telex doesn't have (its key id)                          |
| bad              | The signature doesn't match: changed after signing, or broken              |
| revoked, expired | The key was revoked by its owner, or had expired when the message was signed |

"Only part of the message is signed" is added when text outside the signed part could have been
added (a list's footer, text around an inline block). A key is valid for a signature only when its
primary key binds it (subkey binding signatures are checked, revoked or expired subkeys don't
count) and the subkey may sign.

Encrypted mail is decrypted when it is opened, with the user's keys whose passphrase is known.
When one is needed, the line asks for it (with "Remember it on this PC"); a wrong one says so. Mail
encrypted to someone else's key, damaged mail (the integrity check, MDC or AEAD, fails), mail that
would decrypt to more than 64 MB, and old formats without integrity protection (SED packets) are
named as such and show nothing: a message is never shown half-decrypted.

The decrypted message is a MIME part: Telex puts it under the outer message's headers and shows it
like any other mail, through the same sanitizer for HTML (`html::render`, ammonia), the same
blocked remote images and the same attachment rules. Its own Subject (protected headers, as
Thunderbird writes them) replaces the outer "...", but never its From, To or Date: the outer From
is the one the list shows and the signature check goes by.

## What is kept where

- **Encrypted mail is stored as it arrived**, encrypted. Its text is not stored as text, so it is
  not in the search index (FTS5) and search finds it by sender and subject only; its attachments
  aren't listed until it is opened. Rules and the spam filter see it encrypted too.
- **Decrypted text lives in memory only**: the last 8 opened messages (`KEEP_OPENED`), so their
  HTML, attachments, a reply's quote, Copy as Markdown and the message's own window work. Keys
  added, removed or unlocked clear that memory. It reaches the disk only when the user opens or
  saves one of its attachments (the user asked for that file), and in what the user writes: a reply
  quotes the text, so a reply to encrypted mail starts encrypted.
- **Mail the user writes encrypted** goes out encrypted to every recipient and to the sender's own
  key, and that same message is the copy kept in the Outbox and Sent, so it is stored encrypted
  too. Its list line keeps the subject the user typed (on this PC only; it says "..." on the wire).
  An encrypted draft is kept encrypted to the sender's own key, with no text in the mail base.
- **Keys live in the mail base** (`pgp_keys`, schema 15), so backups carry them, and a key's
  secret part is stored as the key came: protected by its passphrase (OpenPGP's own S2K
  encryption; keys made in Telex use iterated and salted SHA-256 with AES). Telex never stores a
  secret key without the protection it came with, and never asks rPGP for an unprotected copy.
- **Passphrases** are kept in memory until Telex closes (or the user clicks Forget passphrase).
  With "Remember it on this PC" one is also saved in the system's password store, like account
  passwords: Windows Credential Manager, the Secret Service on Linux (service "Telex", entry
  `openpgp:<fingerprint>`); Telex tries it once per session when a message needs it.
- **Backups**: `backup_secret_keys` (Settings › Backups) says whether the secret parts go into
  backups. On (the default), a backup restored on a new PC reads old encrypted mail with the
  passphrase; off, backups carry only public keys (the secret parts are cleared with SQLite's
  `secure_delete` and compacted away), and the user backs up their secret key themselves
  (Settings › Keys › Back up secret key, a file locked with the passphrase).
- **Errors never carry key material or rPGP's own messages.** Each failure is a code (locked,
  not_for_you, failed, too_large, unsupported; no_own_key, locked, no_key_for) that the window
  words; rPGP logs through the `log` crate, and Telex installs no logger.

## Keys

Settings › Keys lists the user's own keys and other people's, each with its fingerprint in groups
of four, algorithm, dates and where it came from. Own keys: Export public key, Copy public key,
Back up secret key (armored, still protected), Forget passphrase, Delete. Other keys: Confirm key
(after comparing the fingerprint with its owner), Delete. Import key file (`.asc`, `.gpg`, `.pgp`,
`.key`; several keys in one file are fine) and Paste a key.

Make a key: for an identity's address, **Ed25519** (signing, with an X25519 subkey for
encryption, in the version 4 format every OpenPGP program of the last decade reads; the default)
or **RSA 3072** for old programs. A passphrase is required: without it, mail encrypted to the key
couldn't be protected at rest.

**Autocrypt (Level 1)**: incoming mail (not Spam, Trash or the user's own folders) with an
`Autocrypt:` header for its From address gives Telex that sender's key, unconfirmed; a newer
message replaces it, two headers count as none, unknown attributes make a header invalid, keys
over 64 KB or that can't encrypt are ignored. Keys attached to a message (`application/pgp-keys`)
are offered above it: Import.

## Writing

Compose shows Sign, Encrypt and My key when the sender's identity has a key:

- **Sign**: the message goes out as `multipart/signed` (RFC 3156 §5) with a SHA-256 signature. Its
  text parts are quoted-printable, so no line ends in a space for a server to trim (the "-- "
  before a signature has one). Signing needs the passphrase: Send asks for it in the line above
  the buttons.
- **Encrypt**: to every recipient's key and the sender's own, as `multipart/encrypted` (§4), signed
  inside when Sign is on (one OpenPGP message, §6.2), AES-256 (SEIPD version 1, which every
  program reads). Bcc recipients are encrypted to as anonymous recipients (no key id), so the
  others can't tell who else got it. The real subject goes inside; outside it says "...".
- **Refusing**: Encrypt with a recipient who has no key (or none that can encrypt now) says whom,
  and Send doesn't send until their key is imported or Encrypt is off.
- **My key**: attaches the sender's public key.
- **Autocrypt header**: every message from an address with a key carries its public key (minimal:
  the address's user id and the encryption subkey) in an `Autocrypt:` header
  (`pgp_autocrypt`, Settings › Keys), so other programs can encrypt replies.

Where compose starts: Sign is on with `pgp_sign` (Settings › Keys › Sign new messages); a reply to
encrypted mail starts encrypted and signed; otherwise what was chosen last time for all of these
recipients (`pgp_prefs`) counts.

## Interoperability

The tests (`openpgp/tests.rs`, `openpgp/write_tests.rs`) use keys and messages made in the
tests, and vectors made with **GnuPG 2.4.9** (`openpgp/fixtures/`): a PGP/MIME signed message,
a signed and encrypted PGP/MIME message with a protected subject and a PDF, clearsigned text and
an inline encrypted text. The other way round, what Telex sends (`write_mail_for_gnupg`) was
checked with GnuPG: `gpg --verify` says "Good signature" for Telex's detached signature, and
`gpg --decrypt` decrypts Telex's encrypted mail and verifies the signature inside.

## Not yet

- **S/MIME** (certificates from .p12/.pfx, verify, decrypt, sign): the RustCrypto `cms` crate
  (MIT/Apache) can parse it, but the certificate chain and trust work is a phase of its own.
- Autocrypt Level 1's `prefer-encrypt=mutual` from the user's side (Telex sends no preference),
  Autocrypt Setup Messages and gossip headers.
- Search inside encrypted mail (would mean storing its text), key servers and WKD lookups (would
  mean asking a server about each address), hardware keys (OpenPGP cards), key expiry and revocation
  made from Telex.
