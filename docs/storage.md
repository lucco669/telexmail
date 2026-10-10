# Storage plan

How Telex keeps mail on disk, why it should stay in one place, and the work that makes it fast
at any size and safe to keep for years: paged loading, backups, local folders, POP3. Written
after v0.2.0. Phase 1 is done (after v0.2.1); the later phases are the plan, not yet built.

## Today

- **One mail base**: `%APPDATA%\app.telexmail.desktop\telex.db`, SQLite in WAL mode (plus its
  `-wal` and `-shm` files). It holds every account: folders, message summaries, the plain-text
  body, the full raw message (attachments included), the FTS5 index, the queue of changes for
  the server (`pending_ops`), the Outbox, rules, identities and trusted senders. Created when
  the first account is added; until then the store is in memory.
- **Not in it**: passwords (Windows Credential Manager, service "Telex"), window preferences
  (WebView2 local storage under `%LOCALAPPDATA%\app.telexmail.desktop`), remote images (fetched
  on request, kept in memory for the session only).
- **How much**: the first sync takes the newest 300 messages per folder
  (`imap::FIRST_SYNC_LIMIT`), then everything new. Mail deleted on the server is deleted
  locally; removing an account deletes its mail. No size cap, no expiry; the file never shrinks
  (SQLite reuses freed pages, nothing runs VACUUM).
- **Schema** (version 7): `messages` has only the summary columns, with indexes on
  `(folder, date)` for the list and `(folder, unread)` for the counts. The body and the raw
  message are in `message_content` (same id, removed with the message). `messages_fts` is a
  contentless FTS5 index over subject, people and body (`contentless_delete=1`), kept in step
  by triggers (`src/migrations/007_message_content.sql`).
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

Next: "download older messages" (beyond the first 300 per folder) is now safe to add.

## Phase 2: backup, restore, location

- **Back up**: SQLite's online backup (or `VACUUM INTO`) writes one consistent copy while
  Telex runs. "Back up now", plus automatic daily or weekly backups to a folder the user picks
  (OneDrive, a NAS, a USB disk), keeping the last N, named by date. A backup holds mail,
  folders, rules, identities and trusted senders; not passwords, which stay in Credential
  Manager and are asked for again after a restore if missing.
- **Restore** puts the current mail base aside (renamed, not deleted), swaps the backup in, and
  then has to be careful with the server:
  - drop the restored `pending_ops`: replaying old read, move and delete changes would undo
    what happened on the server since;
  - resync IMAP folders with the server's state winning (flags, deletions, UIDVALIDITY);
  - move anything in a restored Outbox to Drafts: it may have been sent after the backup was
    taken, and must not go out twice.
- **Location**: a setting to move the mail base (e.g. `D:\Mail`), done by copying with the
  backup API and switching only when the copy is complete. A portable mode can come later.
- **Storage readout**: the mail base's size per account, and "Compact" (VACUUM) after removing
  an account or a large archive.

## Phase 3: local folders, export and import

- **Local folders ("On this PC")**: folders that exist only in the mail base, under any account
  or a separate "Local" group. Moving server mail there downloads it if needed and removes it
  from the server, which frees quota: the The Bat!-style archive. The schema already allows
  folders without `remote` and messages without `uid`; rules can move mail there too.
- **Export** a folder (or an account) as a zip of `.eml` files or an mbox file; **import** `.eml`
  and mbox into a local folder, keeping dates, flags where known, and attachments (the raw
  message is kept as it is).

## Phase 4: POP3

- A POP3 account downloads into a local Inbox: TLS on 995 (or STARTTLS on 110 once STARTTLS is
  supported), the same SMTP sending, the same rules on arrival.
- Downloaded messages are remembered by their UIDL, so nothing comes twice; "leave on the
  server for N days" or "delete after download".
- The autoconfig files the wizard reads also list POP3 servers; `parse_autoconfig` skips them today.

## Later, if mail bases reach tens of gigabytes

Yearly archive volumes: old years in their own SQLite files that never change, so backups copy
only the current year, and an old year can be moved to an external disk and attached again
read-only. Not needed before then.
