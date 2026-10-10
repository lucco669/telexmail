-- Version 8: what the usage view shows about each account besides the mail itself.

-- The server's storage quota (IMAP QUOTA, RFC 9208), in KiB as the server counts them,
-- when the server reports one. NULL when it doesn't, or before the first sync.
ALTER TABLE accounts ADD COLUMN quota_used_kb INTEGER;
ALTER TABLE accounts ADD COLUMN quota_limit_kb INTEGER;
-- The last sync: when one last worked, and why the last one failed (NULL when it worked).
ALTER TABLE accounts ADD COLUMN synced_at TEXT;
ALTER TABLE accounts ADD COLUMN sync_error TEXT;
