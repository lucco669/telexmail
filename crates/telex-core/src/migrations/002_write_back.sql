-- Version 2: changes the user makes reach the server.

-- Where each message is on the server. It differs from `folder` while a move the user made
-- hasn't reached the server yet. NULL for mail that exists only here (drafts, sample).
ALTER TABLE messages ADD COLUMN srv_folder TEXT;
-- The Message-ID header, to find a moved message when the server doesn't say its new UID.
ALTER TABLE messages ADD COLUMN message_id TEXT;
UPDATE messages SET srv_folder = folder WHERE uid IS NOT NULL;

DROP INDEX messages_by_uid;
CREATE UNIQUE INDEX messages_by_uid ON messages(srv_folder, uid) WHERE uid IS NOT NULL;
CREATE INDEX messages_by_message_id ON messages(srv_folder, message_id) WHERE uid IS NULL;

-- Changes the server hasn't heard about yet, oldest first. MailStore keeps at most one
-- per message and kind, so a flag toggled five times is sent once.
CREATE TABLE pending_ops (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    account    TEXT NOT NULL,
    -- The local message. NULL for 'expunge', whose message is already gone here.
    message    INTEGER,
    -- 'seen', 'flagged', 'move' or 'expunge'
    kind       TEXT NOT NULL,
    -- seen, flagged: the new value, 1 or 0
    value      INTEGER,
    -- move: destination folder key
    target     TEXT,
    -- expunge: where the message is on the server, and what it was
    srv_folder TEXT,
    uid        INTEGER,
    message_id TEXT,
    subject    TEXT,
    attempts   INTEGER NOT NULL DEFAULT 0,
    last_error TEXT
);
CREATE INDEX pending_ops_by_account ON pending_ops(account, id);
CREATE INDEX pending_ops_by_message ON pending_ops(message);

-- A message that disappears locally takes its queued changes with it.
CREATE TRIGGER pending_ops_follow_messages AFTER DELETE ON messages BEGIN
    DELETE FROM pending_ops WHERE message = old.id;
END;
