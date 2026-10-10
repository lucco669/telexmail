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
  src/smtp.rs        Sending (lettre + rustls): compose a draft into RFC 5322, send the Outbox.
                     Tests run it against a fake SMTP server.
  src/html.rs        HTML mail: sanitize (ammonia) into an inert document for a sandboxed frame,
                     inline cid: images, fetch remote images on request (ureq, public IPs only).
  src/filters.rs     Rules (the Sorting Office): conditions, actions, running them with notes,
                     auto-replies with RFC 3834 safeguards.
  src/mime.rs        Raw RFC 822 → Message (mail-parser): charsets, HTML-to-text, attachments.
  src/sample.rs      Built-in sample mailbox, shown until the first real account is added.
src-tauri/           Thin Tauri shell: exposes MailStore methods as commands. Keep logic out of here.
  src/secrets.rs     Passwords in Windows Credential Manager (keyring crate), service "Telex".
ui/                  The interface. index.html + styles.css + app.js, no build step, no framework.
  fonts/             IBM Plex (OFL) bundled so the app works offline.
site/                The landing page (Next.js 16, deployed on Vercel with Root Directory "site").
                     Separate npm project; see site/README.md. Its screenshots come from ui/ and
                     sample.json via `npm run screens`; retake them after visible UI changes.
brand/               The logo: telex-mark.svg (48 px and up) and telex-mark-small.svg (32 px and
                     below). `npm run icons` in site/ makes every app and site icon from them.
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
| `save_outgoing`  | `draft`, `send`              | new message id (a draft; `send` only for the sample mailbox) |
| `send_message`   | `draft`                      | `{queued, sent: [lines], errors, sample}` |
| `search`         | `query`                      | ids matching subject, people or body (FTS5) |
| `get_html`       | `id`, `loadImages` (true/false/null) | `{document, blocked, loaded, sender, trusted}` or null for plain-text mail |
| `trust_sender`   | `email`, `trusted`           | – (that sender's images load without asking, except in Spam) |
| `open_link`      | `url`                        | – (http/https only, in the default browser) |
| `list_rules`     | –                            | rules in the order they run |
| `save_rule`      | `rule` (id 0 = new)          | the saved rule, or a sentence saying what's missing |
| `delete_rule`    | `id`                         | –                           |
| `move_rule`      | `id`, `delta` (-1 up, 1 down) | –                          |
| `run_rules`      | `folder`                     | `{matched, moved, replied, problems}` (no auto-replies on demand) |
| `add_account`    | `setup`, `password`          | the account (checks IMAP and SMTP first; same address = update, empty password = keep) |
| `remove_account` | `id`                         | – (local copies and saved password only) |
| `check_mail`     | –                            | `{new: [ids], sent: [lines], sorted, errors: ["Account: reason"], busy}` |
| `push_changes`   | –                            | `{errors, busy}` (sends queued changes only) |

A draft carries `reply_to`, `forward_of` and `replaces` (message ids) besides the fields the
user typed: threading headers, forwarded attachments, and the draft or unsent message it was
edited from. The changing commands (and `send_message`) apply locally at once; for server
mail the store also queues the change, and the UI calls `push_changes` 2.5 s after the last
one. `busy: true` means another check or push was running and nothing started; the UI
retries. `check_mail` sends each Outbox first, then syncs, and emits `sync-progress` events
(`{account, folder, done, total}`) while it downloads.

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
  The one loosening is `style-src 'unsafe-inline'` (with Tauri told not to add hashes to
  style-src, which would cancel it): the HTML mail frame inherits the app's CSP, and mail
  layout lives in inline styles. Remote mail images are fetched by the core
  (`html::fetch_images`) and passed in as data: URIs; the webview never loads them.
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
| E            | edit a draft or an unsent message in the Outbox |
| F            | flag / unflag                  |
| U            | toggle unread                  |
| Del          | move to Trash (in Trash: press twice to delete for good) |
| Z            | undo last delete               |
| /            | quick filter                   |
| H            | switch HTML / plain text view (remembered) |
| Esc          | clear filter; in compose: save draft and close |
| Ctrl+Enter   | send (compose)                 |

## Current state (v0.1.0 + roadmap steps 1, 1b, 2, 3 and 4)

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
- Sending: each server account has SMTP settings (server defaults to the IMAP one; SSL/TLS
  on 465 or STARTTLS on 587; same login as IMAP), checked together with IMAP when the account
  is saved. Accounts saved before step 2 have none until edited (Edit in the account dialog;
  an empty password keeps the saved one). Send builds the message (`smtp::compose`: UTF-8
  plain text, In-Reply-To/References for replies, forwarded attachments from the original's
  raw), puts it in the account's local Outbox and sends the Outbox at once; `check_mail`
  sends it too. Accepted mail moves to Sent and its exact bytes go to the server's Sent
  folder through the change queue (APPEND, kind `append`); the next download adopts the copy
  by Message-ID. A server refusal (5xx, except login errors) stays in the Outbox with the
  reason and isn't retried; anything else is retried at every check. E reopens a draft or an
  unsent message; saving or sending replaces the old one. The status bar says "Sent" only
  after the SMTP server accepts the message.
- Quick filter matches headers as you type and bodies through FTS5 (diacritics-insensitive).
- HTML mail: `get_html` renders the stored raw message (`html::render`): ammonia keeps
  formatting, tables and inline styles and drops scripts, forms, iframes, event handlers,
  `<style>` blocks, javascript: links and relative URLs; cid: images become data: URIs.
  Remote images and CSS backgrounds are left out and counted, unless the user loads them
  (per message, or always for a trusted sender outside Spam: `trusted_senders`, schema
  version 4). Then the core downloads them itself (`html::fetch_images`: http(s) only,
  public IPs only, up to 40 images, 3 MB each, 15 MB total, 10 s, no cookies or Referer,
  generic User-Agent) and renders again with data: URIs. The document carries its own CSP
  (`default-src 'none'; img-src data:; style-src 'unsafe-inline'`). The UI shows it in an
  iframe with `sandbox="allow-same-origin"` (no scripts can run; same-origin only lets the
  page size the frame, catch link clicks and pass keys on). Links open in the default
  browser via `open_link`; mailto: opens compose; links in Spam do nothing. H switches to
  the plain-text body. The canvas is white in both themes.
- Rules (`filters.rs`, schema version 5: `rules`, `auto_replies`): name, account (or all),
  all/any conditions on From, To/Cc, Subject, Body or anywhere (contains, doesn't contain,
  is, starts with, ends with; case and diacritics ignored; addresses match by address and
  by name), actions (move to a folder of the rule's account, mark read, flag, move to Trash,
  auto-reply) and "stop". They run top to bottom on mail that arrives in an Inbox
  (`imap::Downloaded::arrived`: never a folder's first download) inside `sync_all`, so
  their moves and flags go out in the same sync; and on demand on a folder (`run_rules`,
  no auto-replies). Each message gets a note naming the rule and only what it changed, so
  running again adds nothing. Auto-replies (`Auto-Submitted: auto-replied`, threaded) skip
  mailing lists, bulk mail, robots, no-reply and own addresses and Spam, and answer a
  sender at most once per rule every 4 days (`filters::AUTO_REPLY_EVERY`); `check_mail`
  sends them in the same check. Notes look like information; in Spam and the Outbox like
  warnings.
- Mail is stored in full (`messages.raw`), so attachments can be opened later without
  downloading again.
- Before an account is added: the built-in sample mailbox. "Get mail" delivers one sample
  message there; "Send" only files the message in Sent.

## Roadmap

1. ~~**Real mail, read-only.**~~ Done; see Current state. Not yet: STARTTLS, a way to
   download messages older than the first 300, IMAP IDLE (push).
1b. ~~**Write back.**~~ Done; see Current state. Not yet: CONDSTORE/QRESYNC (flag changes are
   found by asking for the flags of every stored message, fine for thousands, not millions).
2. ~~**Sending.**~~ Done; see Current state. Not yet: attaching new files (needs a file
   picker), Bcc, drafts on the server's Drafts folder, and skipping the Sent copy for servers
   that save one themselves (Gmail and Outlook.com do; matters with step 5).
3. ~~**HTML mail.**~~ Done; see Current state. Not yet: `<style>` blocks (dropped, since CSS
   can load from the network; keeping them needs a CSS sanitizer), opening attachments, a
   way to manage trusted senders other than per message.
4. ~~**Filters ("Sorting Office").**~~ Done; see Current state. Not yet: regular expressions,
   conditions on headers and attachments, actions that copy or forward, rules that run on
   a schedule.
5. **Gmail and Outlook.com.** OAuth2 login (both reject plain passwords over IMAP).
6. **Templates and identities.** Quote templates, signatures, multiple identities per account.
7. **Shipping.** Auto-update, code signing, a website.

## Open decisions

- License (none chosen yet).
- Final product name and domain. "Telex" is the working name; the repo is `telexmail`.
- App icon: the dot-matrix T in `brand/`. `src-tauri/icons/` is generated from it with
  `npm run icons` in `site/` (not `tauri icon`, which can't use the small-size variant).

## Prior art worth reading

- Skim (github.com/nikserg/skim): MIT, Rust + Tauri 2 + WebView2 + SQLite FTS5, Windows.
  Useful reference for IMAP sync, offline queue and HTML sanitizing. Minimalist, no filters.
- The Bat! (Ritlabs): the feature model to aim for.
