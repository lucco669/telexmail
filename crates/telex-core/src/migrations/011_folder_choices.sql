-- Version 11: which server folder is Sent (Drafts, Archive, Spam, Trash) can be chosen.

-- The kind the user chose for a server folder in Settings › Accounts › Folders ('sent',
-- 'drafts', 'archive', 'spam' or 'trash'), at most one folder per kind and account. It wins
-- over the server's special-use flags and Telex's guesses (imap::plan_folders). NULL: Telex
-- decides.
ALTER TABLE folders ADD COLUMN chosen TEXT;
