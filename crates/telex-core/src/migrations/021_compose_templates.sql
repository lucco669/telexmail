-- Version 15: richer HTML compose. A signature can be HTML, and identities and folders have
-- templates for what a new message and a reply open with.

-- The signature in HTML (cleaned as sent mail is), or NULL when it is plain text. `signature`
-- keeps its text either way, for plain-text messages.
ALTER TABLE identities ADD COLUMN signature_html TEXT;
-- What a new message opens with (a greeting such as "Dzień dobry,"), and what a reply opens
-- with above the space to type. Empty: nothing.
ALTER TABLE identities ADD COLUMN new_template TEXT NOT NULL DEFAULT '';
ALTER TABLE identities ADD COLUMN reply_template TEXT NOT NULL DEFAULT '';

-- A folder's own templates: for new messages written while it is open and for replies to mail
-- in it. Each one that isn't empty takes the place of the identity's. No reference to
-- `folders`, whose rows a sync writes again: a row whose folder is gone is left out when read.
CREATE TABLE folder_templates (
    account        TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    folder         TEXT NOT NULL,
    new_template   TEXT NOT NULL DEFAULT '',
    reply_template TEXT NOT NULL DEFAULT '',
    reply_header   TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (account, folder)
);
