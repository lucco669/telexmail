-- Version 9: Bcc, conversations, older mail left on the server, a moment to undo sending,
-- and IMAP over STARTTLS.

-- Blind copies of mail the user wrote, as typed. Only Telex's own Sent copies and drafts
-- have them: the copy that reaches the recipients never does.
ALTER TABLE messages ADD COLUMN bcc TEXT NOT NULL DEFAULT '';
-- The Message-IDs this message answers (In-Reply-To and References, without angle
-- brackets, separated by spaces), for showing a whole conversation. Filled in for older
-- mail by MailStore right after this step.
ALTER TABLE messages ADD COLUMN refs TEXT NOT NULL DEFAULT '';

-- How many messages of the folder are on the server but not here: older than the first
-- download took. NULL before the first sync.
ALTER TABLE folders ADD COLUMN missing INTEGER;

-- Mail waits in the Outbox until this time, so Send can still be undone. NULL: send now.
ALTER TABLE outbox ADD COLUMN send_after TEXT;

-- How the IMAP connection is encrypted: 'tls' from the first byte (port 993) or 'starttls'
-- (port 143). NULL for accounts added before: they used TLS.
ALTER TABLE accounts ADD COLUMN imap_security TEXT;
