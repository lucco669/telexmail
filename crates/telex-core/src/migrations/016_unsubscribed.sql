-- Mailing lists the user left with Unsubscribe (one click, a mail, or the list's page), by
-- the list's List-Id or else the sender's address, lowercase. Every message of the list says
-- so; `message` is the one it was done from.
CREATE TABLE unsubscribed (
    list    TEXT PRIMARY KEY,
    at      TEXT NOT NULL,
    -- one_click, mail or link
    method  TEXT NOT NULL,
    message INTEGER
);
