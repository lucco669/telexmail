-- Version 3: sending over SMTP.

-- Where the account sends from. It logs in with the IMAP user name and password.
-- NULL for accounts added before sending existed: the user edits the account to add it.
ALTER TABLE accounts ADD COLUMN smtp_host TEXT;
ALTER TABLE accounts ADD COLUMN smtp_port INTEGER;
-- 'tls' (port 465) or 'starttls' (port 587)
ALTER TABLE accounts ADD COLUMN smtp_security TEXT;

-- Every server account gets a local Outbox for mail waiting to be sent.
INSERT INTO folders (account, id, position, name, kind)
SELECT id, 'outbox', 1, 'Outbox', 'outbox' FROM accounts WHERE imap_host IS NOT NULL;

-- One row per message in an Outbox, with what SMTP needs that the message row lacks.
-- The message text is messages.raw, exactly as it will be sent and saved in Sent.
CREATE TABLE outbox (
    message    INTEGER PRIMARY KEY REFERENCES messages(id) ON DELETE CASCADE,
    account    TEXT NOT NULL,
    -- The SMTP envelope: the sender, and every recipient (To and Cc), as a JSON array.
    env_from   TEXT NOT NULL,
    env_to     TEXT NOT NULL,
    attempts   INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    -- The server refused this message for good (5xx). It isn't retried until the user
    -- edits it.
    refused    INTEGER NOT NULL DEFAULT 0
);
