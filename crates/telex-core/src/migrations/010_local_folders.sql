-- Version 10: folders on this PC.

-- Mail kept only here, in folders that no server has: "On this PC". They form a group of their
-- own, stored like an account without servers whose id is 'local', so their keys are
-- "local/<folder>". It comes with the first server account (MailStore::upsert_account adds it
-- for new mail bases) and stays when accounts are removed: its mail exists nowhere else.
-- It starts with an Archive and a Trash; the user adds more.
INSERT INTO accounts (id, position, name, email)
SELECT 'local', 1000000, 'On this PC', ''
WHERE EXISTS (SELECT 1 FROM accounts WHERE imap_host IS NOT NULL)
  AND NOT EXISTS (SELECT 1 FROM accounts WHERE id = 'local');
INSERT OR IGNORE INTO folders (account, id, position, name, kind)
SELECT 'local', 'archive', 0, 'Archive', 'archive' FROM accounts WHERE id = 'local' AND imap_host IS NULL;
INSERT OR IGNORE INTO folders (account, id, position, name, kind)
SELECT 'local', 'trash', 1, 'Trash', 'trash' FROM accounts WHERE id = 'local' AND imap_host IS NULL;

-- Moving server mail to this PC removes it from the server: an 'expunge' that remembers the
-- local message it came from, so moving the message back before the server heard about it
-- takes the expunge back instead. Not a reference that deletes with the message: the expunge
-- still has to reach the server when the local copy is deleted later.
ALTER TABLE pending_ops ADD COLUMN origin INTEGER;
CREATE INDEX pending_ops_by_origin ON pending_ops(origin) WHERE origin IS NOT NULL;
-- Moving it from this PC to a server folder uploads it (an 'append'), whose `value` now holds
-- the flags the server copy gets: 1 read, 2 flagged. NULL, as for copies of sent mail, is read.
