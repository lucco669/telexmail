# Storage plan

How Telex keeps mail on disk, why it should stay in one place, and the work that makes it fast
at any size and safe to keep for years: paged loading, backups, local folders, POP3. Written
after v0.2.0; the phases below are the plan, not yet built.

## Today (v0.2)

- **One mail base**: `%APPDATA%\app.telexmail.desktop\telex.db`, SQLite in WAL mode (plus its
  `-wal` and `-shm` files). It holds every account: folders, message summaries, the plain-text
  body, the full raw message (`messages.raw`, attachments included), the FTS5 index, the queue
  of changes for the server (`pending_ops`), the Outbox, rules, identities and trusted senders.
  Created when the first account is added; until then the store is in memory.
- **Not in it**: passwords (Windows Credential Manager, service "Telex"), window preferences
  (WebView2 local storage under `%LOCALAPPDATA%\app.telexmail.desktop`), remote images (fetched
  on request, kept in memory for the session only).
- **How much**: the first sync takes the newest 300 messages per folder
  (`imap::FIRST_SYNC_LIMIT`), then everything new. Mail deleted on the server is deleted
  locally; removing an account deletes its mail. No size cap, no expiry; the file never shrinks
  (SQLite reuses freed pages, nothing runs VACUUM).
- **Schema**: `messages` has the summary columns, `body` and `raw` in one row; index
  `(folder, date)`; FTS5 external-content table `messages_fts` over subject, people and body,
  kept in step by triggers (see `src/migrations/001_base.sql`).
- **How the window loads it**: `snapshot` sends every summary of every account in one call;
  `ui/app.js` keeps them all in `MSGS`, filters the folder and counts unread in JavaScript, and
  rebuilds the whole list's HTML on every selection change (each arrow key).

## What is fast and what won't scale

SQLite is not the bottleneck. Reading a page of 50 summaries from the `(folder, date)` index
takes well under a millisecond; FTS5 should answer within tens of milliseconds at 100k messages
(estimate; phase 1 measures it).

The window is. Fine at today's sizes, but with tens of thousands of messages:
- `snapshot` means megabytes of JSON over the bridge and a slow start, and all of it in memory.
- Rebuilding a 10k-row list on every arrow key lags visibly.
- Large `body` and `raw` values share rows with the summaries, so listing touches more pages
  than it needs.

This has to be fixed before anything makes the mail base grow ("download older mail", POP3,
local archives).

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

## Phase 1: fast at any size

Targets on the 100k-message benchmark (goals to verify): start to first list under 300 ms,
switching folder under 50 ms, an arrow key under one frame (16 ms), full-text search under
100 ms.

1. **Split the large columns out** (migration 007): `message_content(id → messages.id, body,
   raw)`, so `messages` holds only summary columns and listing reads small rows. FTS5 still
   needs the body. Options: an external-content FTS over a view joining the two tables, or a
   contentless table with `contentless_delete=1` (the bundled SQLite is 3.53, which has it),
   filled by the store on insert. Pick the one that keeps search results identical and the
   existing search tests passing.
2. **Page per folder instead of `snapshot`**: commands for the folder tree with total and unread
   counts from SQL, and for one page of a folder's summaries in the chosen sort (keyset paging
   on (sort key, id), not OFFSET). Search keeps returning ids, resolved page by page. Mail
   changes that arrive (sync, rules, sending) update counts and the open page without a full
   reload.
3. **Virtual list in `ui/app.js`**: draw only the visible rows (about 40) plus a margin; the
   table keeps its full scroll height. Selection, Home/End, sorting, quick filter, the
   unread-after-600-ms rule and the keyboard table behave as now.
4. **SQLite settings**: `synchronous=NORMAL` (safe in WAL mode), `mmap_size`, a larger
   `cache_size`, `temp_store=memory`; `PRAGMA optimize` on close.
5. **Benchmarks**: an ignored core test (or a small bench) that builds 100k synthetic messages
   across folders and times listing, paging, counting and searching, so the numbers above are
   checked, and rechecked after later changes.

Then "download older messages" (beyond the first 300 per folder) becomes safe to add.

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
