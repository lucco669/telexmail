-- Version 15: tags, colored labels on messages (tags.rs). On a server they are IMAP keywords,
-- so Thunderbird and other programs see the same tags.

-- The tags a message carries: their keywords, lowercase, A to Z, one space apart
-- ("$label1 $label4"); empty for none. Only keywords that are tags here (a row in `tags`) are
-- kept. Moving a message keeps them; moved to a server, they go up with it.
ALTER TABLE messages ADD COLUMN keywords TEXT NOT NULL DEFAULT '';
CREATE INDEX messages_tagged ON messages(keywords) WHERE keywords <> '';

-- The tags, in the order the window shows them: the keyword, the name the user gave it (NULL
-- for Thunderbird's five until renamed: named in the interface's language) and a color of
-- `tags::COLORS`.
CREATE TABLE tags (
    keyword  TEXT PRIMARY KEY,
    name     TEXT,
    color    TEXT NOT NULL,
    position INTEGER NOT NULL
);
INSERT INTO tags (keyword, name, color, position) VALUES
    ('$label1', NULL, 'red', 0),
    ('$label2', NULL, 'orange', 1),
    ('$label3', NULL, 'green', 2),
    ('$label4', NULL, 'blue', 3),
    ('$label5', NULL, 'purple', 4);

-- An account whose server can't keep keywords (PERMANENTFLAGS without them): its tags stay on
-- this PC, and the server's empty keywords never take them away.
ALTER TABLE accounts ADD COLUMN local_tags INTEGER NOT NULL DEFAULT 0;

-- pending_ops gets kind 'keyword': `target` the keyword, `value` 1 to add it, 0 to take it off.
