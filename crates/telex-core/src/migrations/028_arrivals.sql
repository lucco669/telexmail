-- Actual arrivals, separate from the sender's Date and from old mail downloaded/imported.
-- Only today's entries are needed; reads and moves keep the same message id.
CREATE TABLE message_arrivals (
    message INTEGER PRIMARY KEY REFERENCES messages(id) ON DELETE CASCADE,
    day TEXT NOT NULL
);
CREATE INDEX message_arrivals_by_day ON message_arrivals(day);
