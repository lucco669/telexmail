-- Version 7: message text and raw mail in their own table, so listing a folder reads only
-- small rows (docs/storage.md, phase 1).

-- The search index read the body from `messages`. It becomes a contentless table: it keeps
-- only the index, not a copy of the text, and a row is removed by its rowid alone
-- (contentless_delete, SQLite 3.43+). Same columns, tokenizer and ł folding as version 1,
-- so searches find the same messages in the same order. It is built again below.
DROP TRIGGER messages_fts_insert;
DROP TRIGGER messages_fts_delete;
DROP TRIGGER messages_fts_update;
DROP TABLE messages_fts;

-- The plain-text body and the message exactly as it was received or sent, one row per
-- message with the message's id. MailStore writes `messages` first, then this row.
CREATE TABLE message_content (
    id   INTEGER PRIMARY KEY REFERENCES messages(id) ON DELETE CASCADE,
    body TEXT NOT NULL,
    raw  BLOB
);
-- Moved one message at a time: the space each one frees is reused for the next copy, so
-- the file doesn't grow by the size of all the mail during the upgrade.
CREATE TEMP TRIGGER move_content BEFORE UPDATE OF raw ON messages BEGIN
    INSERT INTO message_content (id, body, raw) VALUES (old.id, old.body, old.raw);
END;
UPDATE messages SET body = '', raw = NULL;
DROP TRIGGER move_content;

ALTER TABLE messages DROP COLUMN body;
ALTER TABLE messages DROP COLUMN raw;

CREATE VIRTUAL TABLE messages_fts USING fts5(
    subject, from_name, from_email, to_addr, body,
    content = '', contentless_delete = 1,
    tokenize = 'unicode61 remove_diacritics 2'
);
INSERT INTO messages_fts (rowid, subject, from_name, from_email, to_addr, body)
SELECT m.id,
    replace(replace(m.subject, 'ł', 'l'), 'Ł', 'L'),
    replace(replace(m.from_name, 'ł', 'l'), 'Ł', 'L'),
    m.from_email,
    replace(replace(m.to_addr, 'ł', 'l'), 'Ł', 'L'),
    replace(replace(c.body, 'ł', 'l'), 'Ł', 'L')
FROM messages m JOIN message_content c ON c.id = m.id;

-- A message is indexed when its text arrives, and leaves the index with its row.
CREATE TRIGGER message_content_fts_insert AFTER INSERT ON message_content BEGIN
    INSERT INTO messages_fts (rowid, subject, from_name, from_email, to_addr, body)
    SELECT m.id,
        replace(replace(m.subject, 'ł', 'l'), 'Ł', 'L'),
        replace(replace(m.from_name, 'ł', 'l'), 'Ł', 'L'),
        m.from_email,
        replace(replace(m.to_addr, 'ł', 'l'), 'Ł', 'L'),
        replace(replace(new.body, 'ł', 'l'), 'Ł', 'L')
    FROM messages m WHERE m.id = new.id;
END;

CREATE TRIGGER messages_fts_delete AFTER DELETE ON messages BEGIN
    DELETE FROM messages_fts WHERE rowid = old.id;
END;

CREATE TRIGGER messages_fts_update AFTER UPDATE OF subject, from_name, from_email, to_addr
ON messages BEGIN
    DELETE FROM messages_fts WHERE rowid = old.id;
    INSERT INTO messages_fts (rowid, subject, from_name, from_email, to_addr, body)
    SELECT new.id,
        replace(replace(new.subject, 'ł', 'l'), 'Ł', 'L'),
        replace(replace(new.from_name, 'ł', 'l'), 'Ł', 'L'),
        new.from_email,
        replace(replace(new.to_addr, 'ł', 'l'), 'Ł', 'L'),
        replace(replace(c.body, 'ł', 'l'), 'Ł', 'L')
    FROM message_content c WHERE c.id = new.id;
END;

CREATE TRIGGER message_content_fts_update AFTER UPDATE OF body ON message_content BEGIN
    DELETE FROM messages_fts WHERE rowid = old.id;
    INSERT INTO messages_fts (rowid, subject, from_name, from_email, to_addr, body)
    SELECT m.id,
        replace(replace(m.subject, 'ł', 'l'), 'Ł', 'L'),
        replace(replace(m.from_name, 'ł', 'l'), 'Ł', 'L'),
        m.from_email,
        replace(replace(m.to_addr, 'ł', 'l'), 'Ł', 'L'),
        replace(replace(new.body, 'ł', 'l'), 'Ł', 'L')
    FROM messages m WHERE m.id = new.id;
END;

-- The folder tree's counts: total and unread per folder, from this index alone.
CREATE INDEX messages_by_unread ON messages(folder, unread);
