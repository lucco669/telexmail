# Storage plan

How Telex keeps mail on disk, why it should stay in one place, and the work that makes it fast
at any size and safe to keep for years: paged loading, backups, local folders, POP3. Written
after v0.2.0. Phase 1 is done (after v0.2.1). Phases 2 and 3 are built for 0.6, except moving
the mail base and a zip of .eml files; phase 4 (POP3) is built for the release after 0.9.11.

## Today

- **One mail base**: `%APPDATA%\app.telexmail.desktop\telex.db`, SQLite in WAL mode (plus its
  `-wal` and `-shm` files). It holds every account: folders, message summaries, the plain-text
  body, the full raw message (attachments included), the FTS5 index, the queue of changes for
  the server (`pending_ops`), the Outbox, rules, identities and trusted senders, and the folders
  on this PC, whose mail is nowhere else. Created when the first account is added; until then
  the store is in memory.
- **Not in it**: passwords (the system's store, service "Telex": Windows Credential Manager, a
  Secret Service keyring on Linux), settings (theme,
  language and the rest: `settings.json` in the same folder, written even before the first
  account), window state such as pane sizes (WebView2 local storage under
  `%LOCALAPPDATA%\app.telexmail.desktop`), remote images (fetched on request, kept in memory for
  the session only). Settings, rules and identities also export to a file the user keeps
  (Settings › Import and export), never with passwords.
- **How much**: the first sync takes the newest 300 messages per folder
  (`imap::FIRST_SYNC_LIMIT`), then everything new. Each sync also notes how many messages of a
  folder are on the server but not here (`folders.missing`); the status bar offers them and
  `imap::download_older` takes them a thousand at a time, newest first. Mail deleted on the
  server is deleted locally; removing an account deletes its mail, but not what was moved to
  the folders on this PC. No size cap, no expiry; the file shrinks only when the user compacts
  it (Settings › Backups; SQLite otherwise reuses freed pages).
- **Schema** (version 12): `messages` has only the summary columns, with indexes on
  `(folder, date)` for the list and `(folder, unread)` for the counts. The body and the raw
  message are in `message_content` (same id, removed with the message). `messages_fts` is a
  contentless FTS5 index over subject, people and body (`contentless_delete=1`), kept in step
  by triggers (`src/migrations/007_message_content.sql`). Version 8 keeps what the Usage tab
  shows about each account besides its mail: the server's quota and the last sync
  (`src/migrations/008_usage.sql`); the sizes and counts come from `messages` when the tab
  opens. Version 9 (`src/migrations/009_conversations_and_sending.sql`) adds `messages.bcc`,
  `messages.refs` (the Message-IDs a message answers, for conversations; the upgrade reads the
  first 64 KB of every stored message once to fill it in), `folders.missing`,
  `outbox.send_after` (undo send) and `accounts.imap_security` (STARTTLS). Version 10
  (`src/migrations/010_local_folders.sql`) adds the folders on this PC (an account row
  `local` without servers, with Archive and Trash) to mail bases that have a server account,
  and `pending_ops.origin` (see phase 3). Version 11 (`011_folder_choices.sql`) keeps the
  folder the user chose for each special kind (`folders.chosen`); version 12 (`012_away.sql`)
  the away reply per account and whom it answered (`away`, `away_replies`).
- **SQLite settings**: WAL with `synchronous=NORMAL` (a power cut can lose the last commits,
  never damage the file), a 32 MiB page cache, a 256 MiB memory map, temporary tables in memory,
  and `PRAGMA optimize` when the store closes and when the window closes.
- **How the window loads it**: `folder_tree` gives the accounts and each folder's total and
  unread count, from the `(folder, unread)` index. `list_page` gives one page of a list (see
  phase 1). `ui/app.js` draws only the rows on screen and a margin. After a change (a check,
  rules, sending) it asks for the tree and the open page again, nothing else.

## Principles

- **One place.** One mail base folder holds all accounts in one SQLite database: unified search,
  rules across accounts, one file to back up, atomic changes.
- **Messages stay inside SQLite, not as loose `.eml` files.** On Windows, antivirus scans every
  file it sees and can quarantine one with a bad attachment, which silently deletes that mail
  (a known Thunderbird problem with its mbox files). Inside SQLite that can't happen, writes are
  atomic, and a backup is one consistent file.
- **Open formats through export and import** (`.eml`, mbox), not through the storage layout.
  That is the user's way out and the way in from The Bat! and Thunderbird.
- **Mail that exists only here is irreplaceable.** Local folders and POP3 make the mail base the
  only copy, so they come with backups, never before them.
- **Speed is measured, not guessed**: benchmarks on a synthetic mail base of 100k messages.

## Phase 1: fast at any size (done)

Before, `snapshot` sent every summary of every account in one call (27.5 MB of JSON at 100k
messages), and the window filtered, counted and sorted them in JavaScript and rebuilt the whole
list's HTML on every arrow key. At 100k messages that took seconds per folder and minutes per
key. What changed:

1. **The large columns moved out** (migration 007): `message_content(id, body, raw)`, so
   listing reads small rows. The search index became contentless: it keeps no copy of the text
   and a message leaves it by its rowid, so the triggers never need the old text from another
   table. Same columns, tokenizer and ł folding, so searches find the same messages in the same
   order (checked on the 100k base: identical for every query in the benchmark). The upgrade
   moves the text one message at a time (a temporary trigger), so the space each frees is
   reused at once instead of the file doubling, then drops the old columns (`DROP COLUMN`
   keeps the id sequence and the foreign keys).
2. **Paging instead of `snapshot`** (`src/list.rs`): `folder_tree` for the counts, `list_page`
   for one page of a folder or of all mail, in any of the list's orders, with the quick filter.
   - A folder's pages are found by keyset on (sort key, id): "the 100 after this message". Ties
     are newest first, as before. In date order (the default) it is one row-value comparison
     that SQLite answers from the `(folder, date)` index, at the same cost on row 20 and row
     20,000. The other orders sort by Unicode-lowercased names and subjects without Re:/Fwd:,
     computed by small SQL functions written in Rust, so they match the old JavaScript exactly;
     they have no index and scan the folder for each page (6–31 ms at 40,000 messages).
   - The one exception to keyset is a jump with the scroll bar to a place no loaded page
     reaches (`{row: n}`): it counts rows from the top with OFFSET, which steps through the
     index without reading the rows (1.4 ms to row 20,000).
   - A filtered list is a search: the matching ids are worked out once, in order, and its pages
     come from that result until anything in the mail base changes (`total_changes`). Before
     that change, every page of an all-mail filter counted, paged and placed with three scans
     (126–214 ms); now it is one scan, then 0.1 ms per page.
   - Search keeps returning ids: `search` is unchanged, ranked, at most 5000.
3. **A virtual list in `ui/app.js`**: only the rows on screen and 20 above and below are in the
   DOM (about 35 instead of 40,000); spacer rows keep the table's full scroll height; up to 600
   rows near the screen are kept, and the next or previous page loads while scrolling. A held
   arrow key steps one row at a time even across page edges. Selection, Home/End, sorting, the
   quick filter, mark-as-read after 600 ms and the keyboard behave as before.
4. **SQLite settings**, as listed under Today.
5. **The benchmark**: `cargo test -p telex-core --release --test scale -- --ignored --nocapture`
   builds 100,000 synthetic messages (three accounts, twelve folders, the largest 40,000, about
   10 KB raw each, 10% with an attachment; 1.17 GB) and times what the window does.
   `TELEX_SCALE_V6=<file>` also times upgrading a version 6 base of the same mail, on a copy,
   and checks that searches find the same.

### The numbers

Measured on 10 October 2026 on the owner's PC (Windows 11, release build, SQLite 3.53.2), on the
benchmark's 100k-message base. Core: medians of 5 to 41 runs.

| Core (one call into `MailStore`)        | v0.2 (before)                                  | Phase 1                          |
|-----------------------------------------|------------------------------------------------|----------------------------------|
| Start: open, folder tree, first page    | 294 ms (`snapshot` 245 ms + JSON 48 ms, 27.5 MB) | 11 ms (2.9 KB + 28 KB of JSON) |
| Counts for the folder tree              | 165 ms                                         | 4.4 ms                           |
| Folder switch (first page)              | (all in memory)                                | 0.2 ms (max 0.9)                 |
| Next or previous page at row 20,000     |                                                | 1.8 ms                           |
| Jump to row 20,000 / to the end         |                                                | 1.4 / 0.9 ms                     |
| Other orders, first / next page (40k)   |                                                | subject 13 / 31, from 9 / 20, size 6 / 9, flag 12 / 17 ms |
| Quick filter in a 40k folder            |                                                | 15–46 ms, then 0.1 ms a page     |
| Quick filter in all mail, with the text |                                                | 35–83 ms, then 0.1 ms a page     |
| Mark read, new counts, open page again  |                                                | 6.3 ms                           |
| Open a message (text and raw)           | 0.02 ms                                        | 0.01 ms                          |
| Full-text `search`, ranked              | 0.7–93 ms                                      | 0.8–99 ms                        |
| All 40,000 summaries of a folder        | 102 ms                                         | 39 ms                            |

The window, timed in headless Edge at 1280×800 with a stand-in for the bridge that answers like
the core (its own time included):

| Window                       | v0.2 (before)                       | Phase 1                        |
|------------------------------|-------------------------------------|--------------------------------|
| Start to first rows          | 2.9–3.0 s                           | 57–62 ms                       |
| Folder switch                | 0.3–14 s (3,500 to 40,000 messages) | 5–9 ms                         |
| Arrow key                    | over 120 s (didn't finish)          | 4.5–4.8 ms median, max 9 ms    |
| Home / End                   |                                     | 4 ms                           |
| Typing in the quick filter   |                                     | 12–15 ms                       |
| Rows in the DOM              | 40,000                              | 35                             |

Against the targets: first list under 300 ms (about 70 ms all told), folder switch under 50 ms,
an arrow key under one frame (16 ms), and the quick filter under 100 ms. The ranked `search`
command, which the window no longer uses for the filter, takes up to 99 ms for the most common
word: on the same data the contentless index is 5–10% slower there than the old one (93 ms),
the price of deleting by rowid.

Upgrading the 1.17 GB version 6 base took 12.6 s and left a 1.43 GB file: the old columns'
space is free inside it and is reused by new mail (Compact comes with phase 2). Copying the text
first and dropping the columns after, the first try, took 21 s and left 2.24 GB.

"Download older messages" (beyond the first 300 per folder) came next and is done (see Today).

### What came after phase 1, measured the same way

Measured on 10 October 2026 on the same PC and benchmark (in the benchmark, a reply answers
the message before it in its folder, so conversations are a few messages long):

| Core (one call into `MailStore`)                      | Time            |
|-------------------------------------------------------|-----------------|
| Counts for the folder tree, with the older-mail counts | 5.2 ms         |
| Conversation of a message (T), 78,000-message account | 42 ms           |
| Everyone to suggest in To (2,000 people)              | 95 ms, once per message written |
| Operators: `is:unread`, `from:kowal is:flagged` (40k) | 10 / 15 ms      |
| Operators in all mail: `has:file larger:50k`          | 12 ms           |
| Ids of a Shift+click range / of all 40,000 (Ctrl+A)   | 1.9 / 2.0 ms    |
| Mark 100 messages read at once                        | 0.8 ms          |
| Move 100 messages and back                            | 2.2 ms          |

The rest of the table above stayed the same.

### Thread rows and rule previews (0.9.8)

0.9.5's thread rows (Settings › Reading) worked out the account's whole reply graph again after
any change to the mail base, so with mail marked read after a moment nearly every arrow key paid
for it, and each page built the full row list and searched it. Now the graph is kept until mail
comes, goes, moves or gets new headers (`list_shape`, a counter that temporary triggers on this
connection raise; reading and flagging don't), a list's groups know each message's group and
row, and the unread counts are read for the page's groups only. Lists that depend on read and
flag states (only unread, the Flag order, the quick filter) group their messages again after any
change, still without working out the graph.

A rule's preview read every message of the list in full while it held the mail base, so a check
or a send waited for it: now it reads the list's first 2,000 (`filters::PREVIEW_LIMIT`) after one
pass for their order, and says when there were more.

Measured on 15 October 2026 on the owner's PC, the same benchmark, both versions one after the
other on the same base (the threaded list is the 40,000-message Inbox by date: 17,229 rows):

| Core (one call into `MailStore`)                       | 0.9.5          | 0.9.8           |
|--------------------------------------------------------|----------------|-----------------|
| Threaded: first page after mail came or went           | 126 ms         | 65–76 ms        |
| Threaded: next page at row 15,000 / jump there         | 1.2 / 1.1 ms   | 0.13 / 0.14 ms  |
| Threaded: mark read, new counts, open page again       | 141 ms         | 7.7–8.4 ms (plain list: 9.6) |
| Threaded: mark read, next unread (Space)               | 135 ms         | 8.9 ms          |
| Threaded: Shift+click across 100 rows                  | 1.0 ms         | 0.01 ms         |
| Rule preview, body contains, all mail (94,000)         | 67.5 s         | 66–75 ms (first 2,000) |

## Phase 2: backup, restore, location (built for 0.6, except location)

- **Back up** (`src/backup.rs`, Settings › Backups): `VACUUM INTO` on a read-only connection of
  its own writes one consistent, compacted copy while Telex runs; the store's connection goes on
  writing meanwhile (WAL). "Back up now", and backups by themselves every day or week (the
  window asks `auto_backup` a minute after start and then hourly; due when the newest backup is
  older than the period, ten minutes' slack). They go to a folder the user picks (OneDrive, a
  NAS, a USB disk; Documents\Telex backups until then) as `telex-backup-YYYY-MM-DD-HHMMSS.db`,
  written as `.part` and renamed once it opens as a mail base; the newest 3 to 20 are kept and
  nothing else in the folder is ever touched. A backup holds mail, folders, rules, identities
  and trusted senders; not passwords (the system's store) nor settings.json (Import and export).
- **Restore** checks the file (a Telex mail base, not from a newer Telex), copies it next to the
  mail base, opens and upgrades it there, and makes it safe to meet the servers
  (`MailStore::after_restore`):
  - the restored `pending_ops` are dropped: replaying old read, move and delete changes would
    undo what happened on the server since, so at the next check the server's state wins
    (flags, deletions, UIDVALIDITY); newer mail has UIDs above `last_uid` and comes down again;
  - mail whose move hadn't reached the server goes back where the server has it;
  - anything in a restored Outbox moves to Drafts with a note: it may have been sent after the
    backup was taken, and must not go out twice.
  Then the current mail base is renamed `telex-before-restore-<time>.db` (never deleted) and the
  copy takes its place. The app holds the busy flags meanwhile, lets go of the file (an empty
  store in memory), reopens whatever is there afterwards, and the window starts again.
- **Location** (not built yet): a setting to move the mail base (e.g. `D:\Mail`), done by
  copying with `VACUUM INTO` and switching only when the copy is complete. A portable mode can
  come later.
- **Storage readout**: the Usage tab has sizes per account; Settings › Backups shows the file,
  its size and its free space, and "Compact" (VACUUM, with the busy flags held) gives that space
  back to the disk.

## Phase 3: local folders, export and import (built for 0.6)

- **Folders on this PC**: a separate group, stored as an account row with the id `local` and no
  servers, so its keys are `local/<folder>` and every query that works on folder keys works on
  it. It comes with the first server account and stays when accounts are removed. Moving server
  mail there removes it from the server, which frees quota: the The Bat!-style archive. The row
  is detached at once (`srv_folder` and `uid` cleared, its queued changes dropped) and an
  `expunge` with the old place goes out, remembering the message in `pending_ops.origin` (not
  a foreign key: the expunge must still go out when the local copy is deleted). Moving it back
  to that account before the expunge went out takes the expunge back and moves it there on the
  server, with its read state and flag; otherwise it is uploaded (`append`, flags in `value`)
  and the next download adopts the server's copy by Message-ID. Races with a push in flight are
  settled when its answer comes: a message moved or uploaded while it was already on its way
  to this PC (or elsewhere) leaves the server again from where it landed. An expunge without a
  known UID finds the message by Message-ID, never taking one still stored here. Rules can move
  mail from any account there; conversations reach into it.
- **Export** (`src/mailbox.rs`): a folder as an mbox file (mboxrd, LF line ends, "Status:" and
  "X-Status:" for read and flagged), read on a connection of its own and renamed into place
  when complete. **Import**: `.eml`, `.emlx`, `.msg` and mbox files (Thunderbird's, mutt's, The
  Bat!'s exports), Maildir and MH folders, and Outlook's `.pst` files (`migrate/`) into a folder
  on this PC, streamed and stored 200 at a time with CRLF line ends as IMAP wants
  them, flags from Status, X-Status and X-Mozilla-Status (read when none say), the mbox "From "
  line's date for messages without a Date header, and Message-IDs already in the folder
  skipped. A zip of `.eml` files is not built.

## Phase 4: POP3 (built)

- A POP3 account (`src/pop3.rs`, schema version 15, `src/migrations/015_pop3.sql`) downloads
  into its own Inbox: SSL/TLS on 995 or STLS on 110, never plain; the same SMTP sending; the
  same spam filter, rules and away reply on arrival. All its folders are in the mail base, like
  the folders on this PC but under the account, so its mail, like theirs, is only here once it
  leaves the server: the same reminder to back up applies.
- Downloaded messages are remembered by their UIDL (`pop3_uidls`), so nothing comes twice, also
  mail deleted here while it is still on the server. What stays there is the account's choice:
  everything until it is deleted here for good (the default, as in Thunderbird), at most N days,
  or nothing once it is here. The first check takes the newest 300 and remembers the rest as
  older mail, offered in the status bar; a UIDL the server hasn't listed for 90 days is
  forgotten.
- The autoconfig files and SRV records (`_pop3s`) the wizard reads give POP3 when there is no
  IMAP. Thunderbird's popstate.dat says which mail it already downloaded, so moving in fetches
  only what is new.
- After a restore the next check is like a first one: mail from since the backup that is still
  on the server comes down again, as old mail (no rules or replies twice). Mail removed from the
  server after the backup was made is gone with the backup's age.

## Later, if mail bases reach tens of gigabytes

Yearly archive volumes: old years in their own SQLite files that never change, so backups copy
only the current year, and an old year can be moved to an external disk and attached again
read-only. Not needed before then.
