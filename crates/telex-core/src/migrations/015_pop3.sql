-- Version 15: POP3 accounts (pop3.rs).

-- How an account gets its mail: 'imap', or 'pop3' from the server in imap_host, imap_port,
-- imap_user and imap_security. A POP3 account's folders are all here, like the folders on
-- this PC, and what changes in them goes to no server.
ALTER TABLE accounts ADD COLUMN incoming TEXT NOT NULL DEFAULT 'imap';
-- What stays on a POP3 server: leave it there (1) or remove it once it is here (0); while it
-- is left there, remove it after this many days (NULL: no limit), and once it is deleted here
-- for good (from Trash).
ALTER TABLE accounts ADD COLUMN pop3_leave INTEGER NOT NULL DEFAULT 1;
ALTER TABLE accounts ADD COLUMN pop3_days INTEGER;
ALTER TABLE accounts ADD COLUMN pop3_until_deleted INTEGER NOT NULL DEFAULT 1;

-- The messages of a POP3 account's server Telex knows, by their UIDL, so nothing comes twice:
-- 'here'  downloaded; `message` is it while it is here (NULL once deleted here for good)
-- 'older' left on the server by the first check, which takes only the newest; the status
--         bar offers them (folders.missing on the Inbox) and download_older takes them
-- 'known' another program (Thunderbird) downloaded it; never removed for being deleted here
-- 'big'   too big to download; it stays on the server
-- `downloaded`: when it came (for "remove after N days"); `listed`: the last day the server
-- listed it. A row the server hasn't listed for 90 days goes.
CREATE TABLE pop3_uidls (
    account    TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    uidl       TEXT NOT NULL,
    state      TEXT NOT NULL,
    message    INTEGER REFERENCES messages(id) ON DELETE SET NULL,
    downloaded TEXT,
    listed     TEXT NOT NULL,
    PRIMARY KEY (account, uidl)
) WITHOUT ROWID;
CREATE INDEX pop3_uidls_by_message ON pop3_uidls(message) WHERE message IS NOT NULL;
