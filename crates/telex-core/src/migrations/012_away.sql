-- Version 12: an automatic reply per account while the user is away.

CREATE TABLE away (
    account  TEXT PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    -- 1 while it should answer (between `starts` and `ends`, when they are set).
    enabled  INTEGER NOT NULL DEFAULT 0,
    -- First and last day, "YYYY-MM-DD"; NULL: from now on, until it is turned off.
    starts   TEXT,
    ends     TEXT,
    -- Empty: the server's (or Telex's) usual "Re:" of the original subject.
    subject  TEXT NOT NULL DEFAULT '',
    body     TEXT NOT NULL DEFAULT '',
    -- The same sender gets it at most once in this many days.
    days     INTEGER NOT NULL DEFAULT 4,
    -- 'server': a Sieve script on the server answers, even while Telex is closed;
    -- 'telex': Telex answers new mail when it checks.
    place    TEXT NOT NULL DEFAULT 'telex',
    -- The server's script Telex wrote its part into, and whether Telex made that script.
    script   TEXT,
    created  INTEGER NOT NULL DEFAULT 0
);

-- When Telex last answered a sender for an account's away reply.
CREATE TABLE away_replies (
    account TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    sender  TEXT NOT NULL COLLATE NOCASE,
    sent_at TEXT NOT NULL,
    PRIMARY KEY (account, sender)
);
