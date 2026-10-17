-- Version 15: drafts on the server, and servers that keep their own copy of sent mail.

-- How sent mail reaches the server's Sent folder: 'auto' (the provider list, else what Telex
-- found out after a send), 'server' (the server files it itself, as Gmail and Outlook.com
-- do: Telex uploads nothing) or 'telex' (Telex uploads a copy).
ALTER TABLE accounts ADD COLUMN sent_copy TEXT NOT NULL DEFAULT 'auto';
-- What Telex found out after a send in 'auto': 1 the server had filed the message in Sent
-- itself, 0 it hadn't. NULL until then.
ALTER TABLE accounts ADD COLUMN server_saves_sent INTEGER;

-- pending_ops.value of an 'append' is a set of bits, as before: 1 \Seen, 2 \Flagged, and now
-- 4 \Draft (a draft saved here, for the server's Drafts). NULL is still a copy of sent mail.
