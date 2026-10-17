-- Version 15: the address book (contacts.rs, vcard.rs).

-- A person the user keeps: a name and any of addresses, a phone, an organization, a note.
-- `uid` is the vCard UID: kept from an imported card (so importing it again updates the same
-- contact), made for one written here.
CREATE TABLE contacts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uid TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL DEFAULT '',
    phone TEXT NOT NULL DEFAULT '',
    organization TEXT NOT NULL DEFAULT '',
    note TEXT NOT NULL DEFAULT '',
    changed TEXT NOT NULL
);

-- A contact's addresses in order; position 0 is the primary one. An address belongs to one
-- contact at most.
CREATE TABLE contact_emails (
    contact INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    email TEXT NOT NULL,
    PRIMARY KEY (contact, position)
) WITHOUT ROWID;
CREATE UNIQUE INDEX contact_emails_by_email ON contact_emails (lower(email));

-- A named list of people, offered in To, Cc and Bcc and put in as its members' addresses.
CREATE TABLE contact_groups (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE
);

-- A group's members in order: a contact (its primary address), or an address that isn't one.
CREATE TABLE contact_group_members (
    grp INTEGER NOT NULL REFERENCES contact_groups(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    contact INTEGER REFERENCES contacts(id) ON DELETE CASCADE,
    name TEXT NOT NULL DEFAULT '',
    email TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (grp, position)
) WITHOUT ROWID;
CREATE INDEX contact_group_members_by_contact ON contact_group_members (contact);
