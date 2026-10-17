-- Version 15: calendar invitations in mail (calendar/invites.rs). One row for each message
-- that holds a calendar with an event: which event (UID, and RECURRENCE-ID for one date of a
-- repeating one, '' for the series), its version (SEQUENCE, DTSTAMP in UTC), what it asks
-- (METHOD: REQUEST, CANCEL, REPLY, PUBLISH…), and for a REPLY the answer it gives. A newer
-- SEQUENCE of the same UID makes older invitations outdated; the user's REPLY in the Outbox
-- or Sent is their answer. The upgrade reads every stored message once to fill it in.
CREATE TABLE invites (
    message    INTEGER PRIMARY KEY REFERENCES messages(id) ON DELETE CASCADE,
    uid        TEXT NOT NULL,
    recurrence TEXT NOT NULL DEFAULT '',
    sequence   INTEGER NOT NULL DEFAULT 0,
    method     TEXT NOT NULL,
    stamp      TEXT,
    answer     TEXT
);
CREATE INDEX invites_uid ON invites (uid);
