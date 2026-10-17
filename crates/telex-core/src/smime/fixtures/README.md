# S/MIME test vectors

Made with OpenSSL 3.5.5 (Git for Windows' build) on 17 Oct 2026, for `smime/tests.rs`. Test keys
only: nothing here protects anything.

- `ca.key`, `ca.pem`: "Telex Test CA", ECDSA P-256, self-signed, 20 years.
- `alice.key`, `alice.pem`, `bob.key`, `bob.pem`: RSA 2048 certificates issued by it, for
  alice@example.com and bob@example.com (subjectAltName), key usage digitalSignature and
  keyEncipherment, extended key usage emailProtection.
- `alice.p12`: OpenSSL 3's default (PBES2, PBKDF2-SHA256, AES-256-CBC, SHA-256 MAC), password
  `zażółć` (given as UTF-8 through a file). `alice-legacy.p12`: `-legacy` (3DES key, 40-bit RC2
  certificates, SHA-1 MAC), password `test1234`. `bob.p12`: default, password `bobpass`.
- Messages from alice@example.com to bob@example.com, subject `S/MIME-test`, all of the same
  text (`Cześć Bob,` …):
  - `signed.eml`: `openssl smime -sign -md sha256 -certfile ca.pem` (multipart/signed).
  - `opaque.eml`: `-sign -nodetach` (application/pkcs7-mime, signed-data); `opaque-ber.eml` the
    same with `-stream` (indefinite-length BER, as Thunderbird writes).
  - `encrypted.eml`: `-encrypt -aes256 bob.pem`; `encrypted-ber.eml` with `-stream`;
    `encrypted-3des.eml` with `-des3`; `encrypted-oaep.eml`: `openssl cms -encrypt -aes128
    -recip bob.pem -keyopt rsa_padding_mode:oaep`.
  - `signed-encrypted.eml`: signed as `signed.eml`, then encrypted to Bob and Alice.

Git for Windows' OpenSSL writes text output with CR CR LF line ends; they were made CR LF
afterwards (`.gitattributes` keeps these files byte for byte).

The other way round: `cargo test -p telex-core write_mail_for_openssl -- --ignored` writes
Telex's signed and encrypted messages into `out/`, and

```
openssl smime -verify -in out/telex-signed.eml -CAfile ca.pem
openssl smime -decrypt -binary -in out/telex-encrypted.eml -recip bob.pem -inkey bob.key -out out/inner.eml
openssl smime -verify -in out/inner.eml -CAfile ca.pem
```

say "Verification successful" for both (checked 17 Oct 2026).
