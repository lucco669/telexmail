-- Version 25: work an upgrade leaves for after the window opens (upkeep.rs). Reading every
-- stored message at start made the window wait (schema 18's invitations: 10–15 s for 2.85 GB),
-- so such a step only queues a job here and it runs a short step at a time in the background.
-- One row a job: it works down from the newest message, and the ids below `next` are still to
-- do; `done` and `total` count messages, for the status bar. A finished job's row goes.
CREATE TABLE upkeep (
    job   TEXT PRIMARY KEY,
    next  INTEGER NOT NULL,
    done  INTEGER NOT NULL DEFAULT 0,
    total INTEGER NOT NULL
);
