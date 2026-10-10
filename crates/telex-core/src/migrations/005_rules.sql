-- Version 5: rules (the Sorting Office).

CREATE TABLE rules (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    -- Rules run in this order.
    position    INTEGER NOT NULL,
    name        TEXT NOT NULL,
    enabled     INTEGER NOT NULL DEFAULT 1,
    -- An account id, or NULL for every account.
    account     TEXT,
    -- 1: every condition must match; 0: any one is enough.
    match_all   INTEGER NOT NULL DEFAULT 1,
    -- JSON: [{"field": "from", "op": "contains", "value": "..."}]
    conditions  TEXT NOT NULL,
    -- JSON: [{"type": "move", "folder": "acct/folder"}, {"type": "flag"}, ...]
    actions     TEXT NOT NULL,
    -- Later rules don't run on a message this one matched.
    stop        INTEGER NOT NULL DEFAULT 0
);

-- When a rule last auto-replied to a sender, so each sender gets at most one reply per
-- rule every few days (RFC 3834).
CREATE TABLE auto_replies (
    rule    INTEGER NOT NULL REFERENCES rules(id) ON DELETE CASCADE,
    sender  TEXT NOT NULL COLLATE NOCASE,
    sent_at TEXT NOT NULL,
    PRIMARY KEY (rule, sender)
);
