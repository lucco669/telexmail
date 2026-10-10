-- Version 4: senders whose remote images load without asking (except in Spam).

CREATE TABLE trusted_senders (
    email TEXT PRIMARY KEY COLLATE NOCASE,
    added TEXT NOT NULL
);
