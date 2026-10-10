-- Version 6: identities, each with its own signature and reply/forward templates.

-- Who a message is from. An account has one or more (aliases, a personal and a company
-- name); one is the default. Templates use {name}, {email}, {from}, {date}, {time} and
-- {subject} of the original message.
CREATE TABLE identities (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    account        TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    position       INTEGER NOT NULL,
    name           TEXT NOT NULL,
    email          TEXT NOT NULL,
    signature      TEXT NOT NULL DEFAULT '',
    reply_header   TEXT NOT NULL,
    forward_header TEXT NOT NULL,
    is_default     INTEGER NOT NULL DEFAULT 0
);

-- Every account so far gets one, sending as it did before: the account's name and
-- address, "Pozdrawiam" and the Polish quote header.
INSERT INTO identities (account, position, name, email, signature, reply_header, forward_header, is_default)
SELECT id, 0, name, email, 'Pozdrawiam',
       'Dnia {date} o {time}, {name} napisał(a):',
       '-------- Przekazana wiadomość --------' || char(10) || 'Od: {from}' || char(10)
           || 'Data: {date} {time}' || char(10) || 'Temat: {subject}',
       1
FROM accounts;
