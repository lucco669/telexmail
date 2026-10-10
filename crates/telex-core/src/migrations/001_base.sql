-- Telex mail database, version 1: the base schema.
-- Released: never edit this file. Every change is a new numbered file next to it,
-- listed in MIGRATIONS in store.rs. A new database runs them all in order.

CREATE TABLE accounts (
    id        TEXT PRIMARY KEY,
    position  INTEGER NOT NULL,
    name      TEXT NOT NULL,
    email     TEXT NOT NULL,
    -- NULL for local accounts (the sample mailbox).
    imap_host TEXT,
    imap_port INTEGER,
    imap_user TEXT
);

CREATE TABLE folders (
    account     TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    id          TEXT NOT NULL,
    position    INTEGER NOT NULL,
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL,
    -- Name on the IMAP server, NULL for local folders.
    remote      TEXT,
    -- IMAP UIDVALIDITY. When the server changes it, every UID in the folder is void.
    uidvalidity INTEGER,
    -- Highest UID downloaded so far. The next sync asks only for newer ones.
    last_uid    INTEGER,
    PRIMARY KEY (account, id)
);

CREATE TABLE messages (
    -- AUTOINCREMENT: an id is never reused, so the UI can cache bodies by id.
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    -- "<account>/<folder>"
    folder        TEXT NOT NULL,
    -- IMAP UID inside the folder. NULL for mail that exists only here (drafts, sample).
    uid           INTEGER,
    from_name     TEXT NOT NULL,
    from_email    TEXT NOT NULL,
    to_addr       TEXT NOT NULL,
    cc            TEXT NOT NULL,
    subject       TEXT NOT NULL,
    date          TEXT NOT NULL,
    size_kb       INTEGER NOT NULL,
    unread        INTEGER NOT NULL,
    flagged       INTEGER NOT NULL,
    remote_images INTEGER NOT NULL,
    note          TEXT,
    -- JSON array of {name, size_kb}.
    attachments   TEXT NOT NULL,
    body          TEXT NOT NULL,
    -- The message exactly as the server sent it, kept for HTML view and attachments later.
    raw           BLOB
);

CREATE INDEX messages_by_folder ON messages(folder, date);
CREATE UNIQUE INDEX messages_by_uid ON messages(folder, uid) WHERE uid IS NOT NULL;

-- Full-text search. "remove_diacritics 2" lets "zazolc" find "zażółć", except for ł,
-- which Unicode doesn't treat as l plus an accent: the triggers fold it to l themselves
-- and MailStore::search folds the query the same way. A 'delete' must repeat exactly
-- what was indexed, so every trigger uses the same expressions.
CREATE VIRTUAL TABLE messages_fts USING fts5(
    subject, from_name, from_email, to_addr, body,
    content = 'messages', content_rowid = 'id',
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER messages_fts_insert AFTER INSERT ON messages BEGIN
    INSERT INTO messages_fts (rowid, subject, from_name, from_email, to_addr, body)
    VALUES (new.id,
        replace(replace(new.subject, 'ł', 'l'), 'Ł', 'L'),
        replace(replace(new.from_name, 'ł', 'l'), 'Ł', 'L'),
        new.from_email,
        replace(replace(new.to_addr, 'ł', 'l'), 'Ł', 'L'),
        replace(replace(new.body, 'ł', 'l'), 'Ł', 'L'));
END;

CREATE TRIGGER messages_fts_delete AFTER DELETE ON messages BEGIN
    INSERT INTO messages_fts (messages_fts, rowid, subject, from_name, from_email, to_addr, body)
    VALUES ('delete', old.id,
        replace(replace(old.subject, 'ł', 'l'), 'Ł', 'L'),
        replace(replace(old.from_name, 'ł', 'l'), 'Ł', 'L'),
        old.from_email,
        replace(replace(old.to_addr, 'ł', 'l'), 'Ł', 'L'),
        replace(replace(old.body, 'ł', 'l'), 'Ł', 'L'));
END;

CREATE TRIGGER messages_fts_update AFTER UPDATE OF subject, from_name, from_email, to_addr, body
ON messages BEGIN
    INSERT INTO messages_fts (messages_fts, rowid, subject, from_name, from_email, to_addr, body)
    VALUES ('delete', old.id,
        replace(replace(old.subject, 'ł', 'l'), 'Ł', 'L'),
        replace(replace(old.from_name, 'ł', 'l'), 'Ł', 'L'),
        old.from_email,
        replace(replace(old.to_addr, 'ł', 'l'), 'Ł', 'L'),
        replace(replace(old.body, 'ł', 'l'), 'Ł', 'L'));
    INSERT INTO messages_fts (rowid, subject, from_name, from_email, to_addr, body)
    VALUES (new.id,
        replace(replace(new.subject, 'ł', 'l'), 'Ł', 'L'),
        replace(replace(new.from_name, 'ł', 'l'), 'Ł', 'L'),
        new.from_email,
        replace(replace(new.to_addr, 'ł', 'l'), 'Ł', 'L'),
        replace(replace(new.body, 'ł', 'l'), 'Ł', 'L'));
END;
