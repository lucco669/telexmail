-- Version 25: answered and forwarded mail. On a server they are IMAP's \Answered flag and the
-- $Forwarded keyword (as Thunderbird, Apple Mail and Dovecot write it), synced both ways like
-- \Seen and \Flagged; on this PC they are kept here.
ALTER TABLE messages ADD COLUMN answered INTEGER NOT NULL DEFAULT 0;
ALTER TABLE messages ADD COLUMN forwarded INTEGER NOT NULL DEFAULT 0;

-- The message a message in the Outbox answers or forwards (a message id; ids are never reused).
-- It is marked answered or forwarded once the SMTP server takes the message, not before.
ALTER TABLE outbox ADD COLUMN answers INTEGER;
ALTER TABLE outbox ADD COLUMN forwards INTEGER;

-- pending_ops gets kinds 'answered' and 'forwarded': `value` 1 to set it, 0 to take it off.

-- Mail kept only here (POP3 accounts, the folders on this PC) that a message in Sent answers
-- (its In-Reply-To, the first of `refs`) was answered. Server mail hears it from its server at
-- the next check.
UPDATE messages SET answered = 1
WHERE srv_folder IS NULL AND message_id IS NOT NULL AND message_id IN (
    SELECT CASE WHEN instr(m.refs, ' ') > 0 THEN substr(m.refs, 1, instr(m.refs, ' ') - 1)
                ELSE m.refs END
    FROM messages m JOIN folders f ON m.folder = f.account || '/' || f.id
    WHERE f.kind = 'sent' AND m.refs <> ''
);
