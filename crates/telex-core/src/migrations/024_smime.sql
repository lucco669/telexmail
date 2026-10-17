-- Version 16: S/MIME (smime.rs): the certificates Telex knows, the user's own with their
-- private keys. Decrypted mail is never stored: it lives in memory while open.

CREATE TABLE smime_certs (
    fingerprint   TEXT PRIMARY KEY,              -- SHA-256 of the certificate (DER), upper-case hex
    cert          BLOB NOT NULL,                 -- the certificate, DER
    secret        BLOB,                          -- own: the private key, PKCS #8 encrypted with
                                                 -- PBES2 (PBKDF2-SHA256, AES-256-CBC) under the
                                                 -- password its .p12 had
    protected     INTEGER NOT NULL DEFAULT 0,    -- that password isn't empty
    chain         BLOB,                          -- its issuers' certificates as they came (the .p12,
                                                 -- the signed message), DER one after another
    name          TEXT NOT NULL DEFAULT '',      -- "Anna Kowalska <anna@studio.pl>"
    emails        TEXT NOT NULL DEFAULT '',      -- lower-case addresses it names, space-separated
    issuer        TEXT NOT NULL DEFAULT '',      -- the issuer's common name
    serial        TEXT NOT NULL,                 -- hex
    issuer_serial TEXT NOT NULL,                 -- how CMS names it: SHA-256 of the issuer, ':', serial
    ski           TEXT,                          -- subject key identifier, hex
    not_before    TEXT NOT NULL,
    not_after     TEXT NOT NULL,
    source        TEXT NOT NULL,                 -- own (a .p12), file (imported), message (signed mail)
    verified      INTEGER NOT NULL DEFAULT 0,    -- the user said it is the person's
    added         TEXT NOT NULL
) WITHOUT ROWID;
CREATE INDEX smime_certs_issuer_serial ON smime_certs(issuer_serial);
CREATE INDEX smime_certs_ski ON smime_certs(ski);
