# Signed and encrypted mail in Telex

Telex reads and writes OpenPGP mail: signed and encrypted messages that Thunderbird, GnuPG
(with Mutt, Evolution, KMail, Claws), The Bat!, Proton, OpenKeychain, Delta Chat and other
OpenPGP programs send and read. And S/MIME mail, signed and encrypted with X.509 certificates,
as Outlook, Apple Mail, Thunderbird and most company mail write it (see "S/MIME" below).
Everything happens on the user's PC; no key server, no account, no network beyond the mail
servers.

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
are offered above it: Import. A key that comes by itself (Autocrypt, a message) never replaces a key Telex
already has, since anyone can send a copy with its revocation left out; a key file the user
imports does.

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

## S/MIME

### The libraries

RustCrypto's `cms` (CMS, RFC 5652, with its builder for signing and encrypting), `x509-cert`,
`pkcs12` (PKCS #12's structures and key derivation), `pkcs5` and `pkcs8` (PBES2), and the
ciphers `aes`, `des` (3DES) and `rc2` (old .p12 files), all MIT or Apache-2.0, of the same
generation (`der` 0.7) as the `rsa` crate rPGP already brings, so RSA is one implementation for
both. Chains off Windows: `rustls-webpki` (ISC) and `rustls-native-certs`, both in the build
for TLS already. Nothing GPL or LGPL. `crates/telex-core/src/smime.rs` decides what the
structures mean; `smime/ber.rs` makes BER into DER (below), `smime/p12.rs` reads .p12 and .pfx
files, `smime/trust.rs` asks the system about chains.

### Reading

- `multipart/signed` with `application/pkcs7-signature` (or `x-pkcs7-signature`): the first
  part is checked against the detached signature (SignedData).
- `application/pkcs7-mime` (or `x-pkcs7-mime`, or an `application/octet-stream` named
  `.p7m`): `signed-data` carries the message inside its signature (opaque signing), shown from
  there; `enveloped-data` is encrypted, decrypted on open. Signed inside encrypted (either way of
  signing) is checked too. A part that doesn't read as CMS counts as what its `smime-type` says,
  so the line can say it is damaged.

The line above the message is OpenPGP's (`View`, with `smime: true`), worded for certificates:

| Signature        | When                                                                     |
|------------------|--------------------------------------------------------------------------|
| good             | Valid, within the certificate's dates at the signing time (its signed signing-time, else now), the From address in its subjectAltName (or the subject's emailAddress), and either a chain Windows trusts for email protection or a certificate the user confirmed or owns. The line names the issuing authority |
| unconfirmed      | Valid and the sender's, but no trusted chain (a company's own authority, self-signed): its SHA-256 fingerprint and Confirm certificate |
| other sender     | Valid, but the certificate names another address                         |
| no key           | The signing certificate isn't in the message or the mail base (its serial number) |
| bad              | The signature, or the content's digest in its signed attributes, doesn't match |
| expired          | Outside the certificate's dates at the signing time                      |
| revoked          | Windows already knows it was revoked (nothing is fetched to find out)    |
| unsupported      | RSASSA-PSS, MD5 or an algorithm Telex doesn't check                      |

Signatures: RSA PKCS #1 v1.5 (SHA-1 to SHA-512) and ECDSA P-256 and P-384. RSA keys up to 8,192
bits.

**Chain trust** is the system's. On Windows, CryptoAPI's chain engine (`CertGetCertificateChain`)
with the message's certificates as possible issuers, for the email-protection usage, at the
signing time: the user's and the machine's stores, enterprise policy and the distrusted list
count. It runs with `CERT_CHAIN_CACHE_ONLY_URL_RETRIEVAL` and
`CERT_CHAIN_DISABLE_AUTH_ROOT_AUTO_UPDATE`: nothing is fetched, not issuer certificates from the
address a certificate names (its sender would learn when the mail was opened), not revocation
lists, not root updates. A root Windows hasn't downloaded yet therefore counts as unknown, and the
signature stays unconfirmed until the user confirms the certificate. Elsewhere rustls-webpki
builds the chain to the system's authorities (`rustls-native-certs`: /etc/ssl/certs and the
like); with none, every chain is unconfirmed.

**Decrypting**: key transport recipients (`KeyTransRecipientInfo`) named by issuer and serial or
subject key identifier, with an own certificate's RSA key: PKCS #1 v1.5 or RSA-OAEP (SHA-1 and
SHA-256 as hash and MGF1); content in AES-128/192/256-CBC or 3DES-CBC. A locked key asks for its
password in the line (Remember: the system's password store, "smime:<fingerprint>").
authEnveloped-data (AES-GCM) says it is in a format Telex doesn't read; key agreement (EC
certificates) isn't read, so such mail says it isn't for this PC's certificates. The decrypted
message goes the way OpenPGP's does: under the outer headers, in the same memory (the last 8
opened), through the same sanitizer; nothing decrypted reaches the mail base or the search index.

**Hostile input**: mail programs write CMS in BER (Thunderbird streams it with indefinite lengths;
Windows' .pfx files too), which `der` doesn't read, so `ber.rs` rewrites it as DER first: at most
40 levels deep and 200,000 elements, lengths checked against the data. CMS structures over 48 MB
aren't read, content that would decrypt to more than 64 MB isn't decrypted, and only the first 8
signatures, 32 certificates and 256 recipients of a message are looked at.

### Certificates

Settings › Keys › Certificates (S/MIME) lists the user's own certificates and other people's, each
with its SHA-256 fingerprint in groups of four, algorithm, issuer, dates and where it came from,
and whether a trusted authority vouches for it. Import certificate… takes a `.p12` or `.pfx`
(asks its password once, right there) or other people's certificates (`.cer`, `.crt`, `.pem`,
`.der`, `.p7b`; imported as confirmed, authorities' own certificates skipped). Export
certificate… saves PEM (never the key). Forget password, Confirm certificate, Delete.

**.p12 and .pfx**: OpenSSL 3's default (PBES2: PBKDF2 and AES-256-CBC, a SHA-256 MAC), recent
Windows exports (AES-256) and older ones (OpenSSL's `-legacy`, Windows' "TripleDES-SHA1": 3DES for
the key, 40-bit RC2 for the certificates, a SHA-1 MAC). The MAC says whether the password is
right before anything is decrypted; key derivations over 2,000,000 iterations are refused. The
certificate is the one whose public key the private key has; the others are kept as its chain and
sent along with signatures.

**What is kept**: the private key is sealed again as PKCS #8 (PBES2: PBKDF2-SHA256 with 210,000
iterations and AES-256-CBC) under the file's password and kept in the mail base (`smime_certs`,
schema 16), so backups carry it, locked; `backup_secret_keys` off removes sealed keys from
backups as it does OpenPGP's secret keys. Opened, it stays in memory until Telex closes.

**People's certificates** come with their signed mail: the certificate of a valid signature whose
addresses include the From address, within its dates, is kept when the message arrives (not in
Spam, Trash or the user's own folders) or is opened, with the chain the message carried. A
certificate Telex has stays as it is.

### Writing

Compose's Sign and Encrypt work with the sender's certificate when the sender has one; with both a
key and a certificate, a switch says which (it starts on S/MIME when the message answers S/MIME
mail, or when every recipient has a certificate but not a key).

- **Sign**: `multipart/signed; protocol="application/pkcs7-signature"; micalg=sha-256`, the text
  in quoted-printable (as for OpenPGP), a detached SignedData with SHA-256, the signing time, the
  S/MIME capabilities (AES-256, AES-192, AES-128, so the answer comes in AES) and the certificate
  with its chain. RSA keys sign with PKCS #1 v1.5; ECDSA P-256 and P-384 keys sign too.
- **Encrypt**: `application/pkcs7-mime; smime-type=enveloped-data` (signed first when Sign is on,
  as Outlook and Thunderbird do), AES-256-CBC, the content key for each recipient's certificate and
  the sender's own with RSA PKCS #1 v1.5 (what every program reads). A recipient's certificate
  counts when it can encrypt (RSA, key encipherment), is within its dates, and the user confirmed
  it (imported, Confirm certificate) or a trusted authority vouches for it. Send refuses with the
  addresses that have none.
- **The subject stays outside**: S/MIME programs show the outer one (no protected headers here).
- **Bcc**: S/MIME names every recipient in the message, so Encrypt with blind copies is refused
  ("send them separately") rather than telling everyone who else got it.
- **Drafts** with Encrypt on are sealed to the sender's own certificate.

### Interoperability

`smime/tests.rs` uses certificates made in the tests (an authority and leaf certificates built
with `x509-cert`: dates, addresses, chains) and vectors made with **OpenSSL 3.5.5**
(`smime/fixtures/`, see its README): modern and legacy .p12 files, detached and opaque signatures
(also streamed BER), encryption with AES-256, AES-256 streamed, 3DES and RSA-OAEP, and signed then
encrypted mail. The other way round, `write_mail_for_openssl` writes Telex's signed and encrypted
mail, and `openssl smime -verify` and `-decrypt` say "Verification successful" for both.

## Not yet

- S/MIME: authEnveloped-data (AES-GCM) and key agreement (EC recipients), RSASSA-PSS signatures,
  revocation checks (CRL and OCSP would mean asking the authority about each signature), header
  protection, and certificates looked up in a directory (LDAP).
- Autocrypt Level 1's `prefer-encrypt=mutual` from the user's side (Telex sends no preference),
  Autocrypt Setup Messages and gossip headers.
- Search inside encrypted mail (would mean storing its text), key servers and WKD lookups (would
  mean asking a server about each address), hardware keys (OpenPGP cards), key expiry and revocation
  made from Telex.
