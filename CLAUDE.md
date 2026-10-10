# Telex

A compact, fast, keyboard-driven email client for Windows, in the spirit of The Bat!.
Rust does the mail work; the window is a Tauri WebView2 app with a plain HTML/CSS/JS interface.

The owner is an experienced web developer (Astro, Sanity) who is new to Rust. When you make a
Rust-specific choice (ownership, lifetimes, async runtime, error types), explain it in one or two
plain sentences in your summary.

## What Telex should feel like

- **Dense.** 20 px rows by default, a condensed face in the message list, mono for dates and sizes.
  No spacious, Mac-style padding. A "Comfortable" density toggle exists for people who want air.
- **Keyboard-first.** Every common action has a single key (see the table below). Mouse is optional.
- **Fast and light.** Starts instantly, small installer, no Electron, no account or cloud service.
- **Local mail base.** Mail is stored on the user's disk and searchable offline.
- **Powerful filters.** The goal is something like The Bat!'s "Sorting Office": rules that sort,
  flag, move and auto-reply, with the reason shown on each affected message (the `note` field).
- **Private by default.** Remote images in HTML mail are blocked until the user allows them.
  Links in spam are not clickable.
- **Polish-friendly.** Old charsets (Windows-1250, ISO-8859-2) must decode correctly. Reply quote
  header is Polish by default: `Dnia 10.10.2026 o 06:12, Anna Kowalska napisał(a):`. The UI itself
  is English for now.
- **Plain text first.** Compose is plain text in a mono face; HTML compose may come later.

Anti-goals: Electron, AI features by default, a web-app look, anything that phones home.

## Layout of the repo

```
crates/telex-core/   All mail logic. Unit-tested. No UI, no Tauri.
  src/store.rs       MailStore: the local mail base in SQLite (+ FTS5) and the queue of changes
                     for the server (pending_ops). Same API for sample and real mail.
  src/migrations/    Database schema, one numbered file per version, listed in MIGRATIONS in
                     store.rs. Released files are never edited; a change is a new file.
  src/imap.rs        Two-way IMAP sync (async-imap + rustls): download, push queued changes,
                     folder mapping. Tests run it against a fake IMAP server.
  src/mime.rs        Raw RFC 822 → Message (mail-parser): charsets, HTML-to-text, attachments.
  src/sample.rs      Built-in sample mailbox, shown until the first real account is added.
src-tauri/           Thin Tauri shell: exposes MailStore methods as commands. Keep logic out of here.
  src/secrets.rs     Passwords in Windows Credential Manager (keyring crate), service "Telex".
ui/                  The interface. index.html + styles.css + app.js, no build step, no framework.
  fonts/             IBM Plex (OFL) bundled so the app works offline.
site/                The landing page (Next.js 16, deployed on Vercel with Root Directory "site").
                     Separate npm project; see site/README.md. Its screenshots come from ui/ and
                     sample.json via `npm run screens`; retake them after visible UI changes.
.github/workflows/   CI: core tests on Linux, Windows installer build.
```

The mail base is `%APPDATA%\app.telexmail.desktop\telex.db`. It is created when the first account
is added; until then the app shows the sample mailbox (in memory, never saved).

Commands the UI calls (`src-tauri/src/lib.rs`), all backed by `telex_core::MailStore`:

| Command          | Args                         | Returns                     |
|------------------|------------------------------|-----------------------------|
| `snapshot`       | –                            | accounts + all summaries + `sample` flag |
| `get_message`    | `id`                         | full message with body      |
| `set_unread`     | `id`, `unread`               | –                           |
| `set_flagged`    | `id`, `flagged`              | –                           |
| `move_message`   | `id`, `folder`               | – (same account only)       |
| `delete_message` | `id`                         | `moved_to_trash` / `removed`|
| `save_outgoing`  | `draft`, `send`              | new message id              |
| `search`         | `query`                      | ids matching subject, people or body (FTS5) |
| `add_account`    | `setup`, `password`          | the account (logs in first; same address = update) |
| `remove_account` | `id`                         | – (local copies and saved password only) |
| `check_mail`     | –                            | `{new: [ids], errors: ["Account: reason"], busy}` |
| `push_changes`   | –                            | `{errors, busy}` (sends queued changes only) |

The four changing commands apply locally at once; for server mail the store also queues the
change, and the UI calls `push_changes` 2.5 s after the last one. `busy: true` means another
check or push was running and nothing started; the UI retries. `check_mail` emits
`sync-progress` events (`{account, folder, done, total}`) while it downloads.

Folder keys are `"<account>/<folder>"`, e.g. `firma/inbox`. Dates cross the bridge as local
`YYYY-MM-DDTHH:MM:SS` strings.

## Commands

```
npm install                  # once: installs the Tauri CLI
npm run dev                  # run the app with live reload of ui/
npm run build                # release build + installers in target/release/bundle/
cargo test -p telex-core     # core tests (run these after every core change)
cargo test -p telex-core -- --ignored   # also checks TLS against a real server (needs internet)
cargo clippy -p telex-core --all-targets -- -D warnings
cargo fmt --all
```

Windows prerequisites: Rust via rustup (MSVC toolchain), Visual Studio Build Tools with
"Desktop development with C++", Node.js 20+, WebView2 (preinstalled on Windows 11).

## Rules for changes

- Put behavior in `telex-core` with a test. The Tauri layer only converts errors to strings.
- The UI talks to Rust only through `invoke`. No `fetch` to the network from the UI.
- The CSP in `tauri.conf.json` allows only local files. Do not add CDNs or remote fonts.
- Colors are CSS tokens in `ui/styles.css` with light and dark sets. Do not hard-code colors.
- Keep the keyboard table below and the status-bar hints in `ui/index.html` in sync.
- Run `cargo fmt`, clippy and the core tests before committing.
- Status text in the UI is short and specific ("Moved to Trash. Press Z to undo.").
  Never claim mail was sent until SMTP actually sends it.

## Keyboard

| Key          | Action                         |
|--------------|--------------------------------|
| ↑ / ↓        | previous / next message        |
| Home / End   | first / last message           |
| R            | reply                          |
| Shift+R      | reply to all                   |
| Shift+F      | forward                        |
| N            | new message                    |
| F            | flag / unflag                  |
| U            | toggle unread                  |
| Del          | move to Trash (in Trash: press twice to delete for good) |
| Z            | undo last delete               |
| /            | quick filter                   |
| Esc          | clear filter; in compose: save draft and close |
| Ctrl+Enter   | send (compose)                 |

## Current state (v0.1.0 + roadmap steps 1 and 1b)

- Working window with folder tree, sortable message list, preview, compose, quick filter,
  resizable panes, density toggle, light/dark theme.
- Real IMAP accounts: account dialog ("Add account" under the folder tree), SSL/TLS on port
  993 only (no STARTTLS yet), password login. Folders come from the server; special folders
  are recognized by RFC 6154 flags, then by common English/Polish names.
- Two-way sync on start, on "Get mail" and every 10 minutes: push queued changes, download,
  push what changed meanwhile. The first sync takes the newest 300 messages per folder
  (`imap::FIRST_SYNC_LIMIT`); later syncs take everything new. Downloading uses EXAMINE and
  BODY.PEEK only. Mail deleted on the server disappears locally; flags changed elsewhere come
  down unless a local change to the same flag is still queued; a UIDVALIDITY change
  re-downloads the folder.
- Changes to server mail (read, flag, move, delete) apply locally at once and wait in
  `pending_ops`, which survives restarts and lost connections. Each message keeps its
  server location (`srv_folder`, `uid`) apart from the folder the UI shows, until the move
  reaches the server. Moves use UID MOVE, or COPY + \Deleted + EXPUNGE without MOVE; the new
  UID comes from COPYUID, else from a Message-ID search, else the next download adopts the
  message instead of duplicating it. A change refused 3 times (`imap::MAX_ATTEMPTS`) is
  dropped, reported, and a refused move puts the message back.
- Quick filter matches headers as you type and bodies through FTS5 (diacritics-insensitive).
- Mail is stored in full (`messages.raw`), so HTML view and attachments can come later
  without downloading again. The UI still shows the plain-text body.
- Before an account is added: the built-in sample mailbox. "Get mail" delivers one sample
  message there. "Send" only files the message in Sent, for every account.

## Roadmap

1. ~~**Real mail, read-only.**~~ Done; see Current state. Not yet: STARTTLS, a way to
   download messages older than the first 300, IMAP IDLE (push).
1b. ~~**Write back.**~~ Done; see Current state. Not yet: CONDSTORE/QRESYNC (flag changes are
   found by asking for the flags of every stored message, fine for thousands, not millions).
2. **Sending.** SMTP via `lettre`, outbox with retry, Sent copy via IMAP APPEND.
3. **HTML mail.** Render in a sandboxed iframe (`srcdoc`, no scripts), remote content blocked
   until allowed per message or per sender.
4. **Filters ("Sorting Office").** Rule editor, run on arrival and on demand, `note` explains why.
5. **Gmail and Outlook.com.** OAuth2 login (both reject plain passwords over IMAP).
6. **Templates and identities.** Quote templates, signatures, multiple identities per account.
7. **Shipping.** Auto-update, code signing, a website.

## Open decisions

- License (none chosen yet).
- Final product name and domain. "Telex" is the working name; the repo is `telexmail`.
- App icon is a placeholder (`src-tauri/icons/`, generated). Regenerate all sizes with
  `npm run tauri icon path/to/icon.png` once there is a real one.

## Prior art worth reading

- Skim (github.com/nikserg/skim): MIT, Rust + Tauri 2 + WebView2 + SQLite FTS5, Windows.
  Useful reference for IMAP sync, offline queue and HTML sanitizing. Minimalist, no filters.
- The Bat! (Ritlabs): the feature model to aim for.
