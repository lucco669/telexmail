-- Saved list queries, and once-only copy/forward actions on each original message.
CREATE TABLE saved_searches (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    view TEXT NOT NULL
);
CREATE TABLE rule_actions_done (
    rule INTEGER NOT NULL REFERENCES rules(id) ON DELETE CASCADE,
    message INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    action TEXT NOT NULL,
    PRIMARY KEY (rule, message, action)
);
