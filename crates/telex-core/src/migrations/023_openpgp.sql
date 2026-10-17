-- Version 15: OpenPGP (openpgp.rs): the keys Telex knows, Autocrypt peers, and what compose
-- remembers per recipient. Decrypted mail is never stored: it lives in memory while open.

-- Every key: the user's own (with their secret part) and other people's certificates.
CREATE TABLE pgp_keys (
    fingerprint TEXT PRIMARY KEY,              -- the primary key's, upper-case hex
    public      BLOB NOT NULL,                 -- the certificate (transferable public key), binary
    secret      BLOB,                          -- own keys: the transferable secret key, binary,
                                               -- protected by its passphrase (S2K) as it came
    user_ids    TEXT NOT NULL DEFAULT '',      -- one per line, as the key says them
    emails      TEXT NOT NULL DEFAULT '',      -- lower-case addresses of its user ids, space-separated
    created     TEXT NOT NULL,
    expires     TEXT,
    source      TEXT NOT NULL,                 -- own (made here), file, autocrypt, message
    verified    INTEGER NOT NULL DEFAULT 0,    -- the user said the key is the person's
    added       TEXT NOT NULL
) WITHOUT ROWID;

-- The key ids and fingerprints of every part of a key (primary and subkeys): a signature
-- names its key by one of these, an encrypted message its recipients.
CREATE TABLE pgp_key_ids (
    id          TEXT NOT NULL,                 -- 16 hex digits, or a whole fingerprint
    fingerprint TEXT NOT NULL REFERENCES pgp_keys(fingerprint) ON DELETE CASCADE,
    PRIMARY KEY (id, fingerprint)
) WITHOUT ROWID;
CREATE INDEX pgp_key_ids_key ON pgp_key_ids(fingerprint);

-- Autocrypt (Level 1): what each address that wrote says about itself.
CREATE TABLE autocrypt_peers (
    email          TEXT PRIMARY KEY,           -- lower-case
    fingerprint    TEXT REFERENCES pgp_keys(fingerprint) ON DELETE SET NULL,
    prefer_encrypt INTEGER NOT NULL DEFAULT 0, -- 1: prefer-encrypt=mutual
    last_seen      TEXT NOT NULL,              -- the newest message from it
    autocrypt_seen TEXT                        -- the newest message from it with a key
) WITHOUT ROWID;

-- Compose's Sign and Encrypt, remembered per recipient (lower-case address).
CREATE TABLE pgp_prefs (
    email   TEXT PRIMARY KEY,
    encrypt INTEGER NOT NULL,
    sign    INTEGER NOT NULL
) WITHOUT ROWID;
