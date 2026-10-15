-- Version 14: Telex's own spam filter (spam.rs), learned from the user's mail.

-- How many spam and good ("ham") messages each token was seen in. The row with the empty
-- token holds how many messages of each were learned from: a token is never empty.
CREATE TABLE spam_tokens (
    token TEXT PRIMARY KEY,
    spam  INTEGER NOT NULL DEFAULT 0,
    ham   INTEGER NOT NULL DEFAULT 0
) WITHOUT ROWID;
INSERT INTO spam_tokens (token, spam, ham) VALUES ('', 0, 0);

-- What the filter knows about a message: 1 learned as spam, 0 learned as good mail,
-- -1 moved to Spam by the filter itself (not learned from, so it doesn't teach itself).
-- Moving a message into Spam or out of it learns it again; a message learned once isn't
-- learned twice. A removed message takes its row along; what was learned stays.
CREATE TABLE spam_trained (
    message INTEGER PRIMARY KEY REFERENCES messages(id) ON DELETE CASCADE,
    class   INTEGER NOT NULL
);
