# Telex

A compact, fast, keyboard-driven email client for Windows (and Linux, as an AppImage), in the
spirit of The Bat!. Rust does the mail work; the window is a Tauri app (WebView2 on Windows,
WebKitGTK on Linux) with a plain HTML/CSS/JS interface.

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
  Links in spam are not clickable. Update checks happen only after the user says yes.
- **Polish-friendly.** Old charsets (Windows-1250, ISO-8859-2) must decode correctly. Reply quote
  header is Polish by default: `Dnia 10.10.2026 o 06:12, Anna Kowalska napisał(a):`. The interface
  speaks English and Polish, following the Windows display language unless the user picks one;
  so do the core's errors and notes (`tr!`).
- **Readable.** Every theme meets WCAG 2.2 AA contrast (high contrast: AAA), checked by a script.
- **Plain text first.** Compose is plain text in a mono face by default. HTML compose is a
  setting and a per-message switch, and HTML always goes out with a plain-text copy.
- **Mail follows the theme.** In a dark theme HTML mail goes on the theme's dark paper with its
  colors turned around (OKLab, hue kept); one click shows it on white, as sent.

Anti-goals: Electron, AI features by default, a web-app look, anything that phones home.

## Layout of the repo

```
crates/telex-core/   All mail logic. Unit-tested. No UI, no Tauri.
  src/store.rs       MailStore: the local mail base in SQLite (+ FTS5) and the queue of changes
                     for the server (pending_ops).
  src/list.rs        The message list a page at a time: folder counts, keyset pages in every
                     sort, the quick filter (one folder or all mail, with operators such as
                     from: is:unread after:) and its SQL functions, conversations, ranges.
  src/migrations/    Database schema, one numbered file per version, listed in MIGRATIONS in
                     store.rs. Released files are never edited; a change is a new file.
  src/imap.rs        Two-way IMAP sync (async-imap + rustls, SSL/TLS or STARTTLS): download,
                     push queued changes a batch per command, folder mapping, older mail on
                     request, IDLE on the Inbox. Tests run it against a fake IMAP server.
  src/smtp.rs        Sending (lettre + rustls): compose a draft into RFC 5322 (Bcc, files from
                     disk), send the Outbox. Tests run it against a fake SMTP server.
  src/attachments.rs Attachments of stored mail opened or saved: safe file names, files that
                     can run code, the Mark of the Web on Windows.
  src/recipients.rs  People to suggest in To, Cc and Bcc, from the mail base itself.
  src/html.rs        HTML mail: sanitize (ammonia) into an inert document for a sandboxed frame,
                     inline cid: images, fetch remote images on request (ureq, public IPs only);
                     dark paper in a theme's palette; HTML compose: what may be sent, plain text
                     from HTML and HTML from plain text.
  src/colors.rs      CSS colors read and turned around for dark paper (OKLab), for html.rs.
  src/usage.rs       What the Usage tab shows: quota, sizes per folder, 30 days of mail in and
                     out, top senders, largest messages, the last sync.
  src/i18n.rs        The core's language: `tr!("English", "Polski")`, set from the settings.
  src/filters.rs     Rules (the Sorting Office): conditions, actions, running them with notes,
                     auto-replies with RFC 3834 safeguards.
  src/drafts.rs      Starting a draft (new, reply, reply all, forward) from an identity's
                     signature and templates: recipients, subject, quoted text.
  src/mime.rs        Raw RFC 822 → Message (mail-parser): charsets, HTML-to-text, attachments.
  src/autoconfig.rs  Finding an address's servers for the add-account wizard: provider list,
                     the domain's autoconfig file, SRV and MX records (Windows' resolver;
                     hickory-resolver elsewhere), probing.
  src/settings.rs    Settings (settings.json next to the mail base), Windows' languages, and the
                     settings file: export, and import as a plan (what changes) applied in parts.
  src/sample.rs      A Polish sample mailbox for the tests. Never in the app.
  tests/scale.rs     Benchmark on a synthetic 100k-message mail base (ignored; see Commands).
src-tauri/           Thin Tauri shell: exposes MailStore methods as commands. Keep logic out of here.
  src/secrets.rs     Passwords in the system store (keyring crate), service "Telex": Credential
                     Manager on Windows, Secret Service (GNOME Keyring, KWallet) on Linux.
  src/files.rs       Attachments opened in their app or saved (dialogs), files chosen or dropped
                     to attach; only files chosen this way can be sent.
  src/watch.rs       One IMAP IDLE watcher per account ("inbox-changed" events), and the
                     new-mail notification (tauri-plugin-notification).
  src/updates.rs     Updates from GitHub Releases (tauri-plugin-updater), release builds only.
  src/settings.rs    Settings commands; Save and Open dialogs (tauri-plugin-dialog, from Rust only).
  tauri.release.conf.json  Merged in by the release workflow: turns on the updater's files.
  tauri.linux.conf.json    Merged on Linux by Tauri: AppImage and .deb instead of nsis/msi.
ui/                  The interface. index.html + styles.css + app.js, no build step, no framework.
  themes.css         Every color, one block per theme (data-theme on <html> and on theme cards).
  i18n.js            English and Polish: t(), tn() (Polish plurals), data-i18n in index.html.
  boot.js            Puts the saved theme, language and text size on <html> before first paint.
  fonts/             IBM Plex (OFL) bundled so the app works offline.
site/                The landing page (Next.js 16, deployed on Vercel with Root Directory "site").
                     Separate npm project; see site/README.md. Its screenshots come from ui/ and
                     an English demo mailbox (site/scripts/demo-mailbox.json) via `npm run screens`;
                     its live demo (/demo) is ui/ itself with a stand-in for the core
                     (site/demo/backend.js), copied into site/public/demo-app by `npm run demo`.
                     Retake both after visible UI changes. What is built but not released is
                     marked `since: NEXT` in site/lib/content.ts ("Coming in 0.5" until it ships).
brand/               The logo: telex-mark.svg (48 px and up) and telex-mark-small.svg (32 px and
                     below). `npm run icons` in site/ makes every app and site icon from them.
scripts/             set-version.mjs: sets the version in every file that has it, or checks them.
                     check-signing-key.mjs: the release key pair belongs together (release.yml).
                     check-contrast.mjs: every theme meets its WCAG level (CI; --json for the site).
                     check-i18n.mjs: every text in app.js and index.html has its Polish (CI).
docs/releasing.md    The release process, the signing keys, and what an update check sends.
docs/storage.md      How mail is stored, and the plan: fast at any size, backups, local folders, POP3.
.github/workflows/   ci.yml: core tests and the contrast and Polish checks on Linux, Linux app clippy
                     (ubuntu-22.04), Windows installer build (can't update itself).
                     release.yml: on a v* tag, signed Windows installers, then AppImage + .deb;
                     latest.json for both in a draft release.
```

The mail base is `%APPDATA%\app.telexmail.desktop\telex.db` (on Linux
`~/.local/share/app.telexmail.desktop/`). It is created when the first account is added; until
then the app starts with an empty store in memory and opens the wizard.

Commands the UI calls (`src-tauri/src/lib.rs`), all backed by `telex_core::MailStore`:

| Command          | Args                         | Returns                     |
|------------------|------------------------------|-----------------------------|
| `folder_tree`    | –                            | `{accounts, counts: {"<folder key>": {total, unread, older?}}}` (empty folders left out; `older`: on the server, not downloaded) |
| `list_page`      | `view`, `at`, `limit` (≤ 500) | `{total, start, rows}`: one page of summaries (see below) |
| `list_ids`       | `view`, `from`, `to`         | ids between two messages of the list, both included (Shift selection), or all of it when either is null (Ctrl+A) |
| `get_message`    | `id`                         | full message with body (and `bcc` on mail the user wrote) |
| `set_unread`     | `id`, `unread`               | –                           |
| `set_flagged`    | `id`, `flagged`              | –                           |
| `move_message`   | `id`, `folder`               | – (same account only)       |
| `delete_message` | `id`                         | `moved_to_trash` / `removed`|
| `set_unread_many`, `set_flagged_many` | `ids`, `unread` / `flagged` | how many changed (others are left alone) |
| `move_messages`  | `ids`, `folder`              | `[[id, from]]` for undo; all or none, same account |
| `delete_messages`| `ids`                        | `{trashed: [[id, from]], removed}` |
| `mark_folder_read` | `folder`                   | how many were unread        |
| `open_attachment`| `id`, `index`, `sure`        | `opened`, or `risky` / `spam` (not opened: ask, then again with `sure`) |
| `save_attachment`| `id`, `index`, `words`       | the path saved to, or null (cancelled); opens Save |
| `save_attachments`| `id`, `words`               | the folder saved to, or null; opens a folder picker |
| `show_saved`     | –                            | – (the last saved attachment in Explorer) |
| `pick_files`     | `words`                      | `[{name, size_kb, path}]` to attach (empty: cancelled); also "files-dropped" events for files dropped on the window |
| `recipients`     | –                            | `[{name, email, sent, received}]`, most likely first, at most 2000 |
| `save_outgoing`  | `draft`                      | new message id (a draft, kept whole: its HTML and forwarded files) |
| `draft_html`     | `id`                         | a draft's or unsent message's HTML, cleaned for the editor, or null (plain text) |
| `html_to_text`   | `html`                       | the plain text it would go out with (compose switches to Text) |
| `text_to_html`   | `text`                       | HTML for the editor: a `<div>` per line, `>` quotes as blockquotes, links |
| `clean_html`     | `html`                       | pasted HTML cleaned to what Telex sends |
| `send_message`   | `draft`                      | `{queued, wait, sent: [lines], errors}`; with `wait` > 0 it goes after that many seconds, then an "outbox-sent" event `{queued, sent, errors}` |
| `hold_message`   | `id`                         | – (takes back mail still waiting to go; it stays in the Outbox, held) |
| `search`         | `query`                      | ids matching subject, people or body (FTS5), best first, at most 5000 |
| `get_html`       | `id`, `loadImages` (true/false/null), `dark` (palette or null) | `{document, blocked, loaded, sender, trusted}` or null for plain-text mail |
| `trusted_senders`| –                            | senders whose images load without asking, A to Z |
| `usage`          | –                            | `{accounts: [{account, quota, synced_at, sync_error, folders, flow, senders, largest, queued}], base_bytes, free_bytes, since}` |
| `trust_sender`   | `email`, `trusted`           | – (that sender's images load without asking, except in Spam) |
| `open_link`      | `url`                        | – (http/https only, in the default browser) |
| `list_rules`     | –                            | rules in the order they run |
| `save_rule`      | `rule` (id 0 = new)          | the saved rule, or a sentence saying what's missing |
| `delete_rule`    | `id`                         | –                           |
| `move_rule`      | `id`, `delta` (-1 up, 1 down) | –                          |
| `run_rules`      | `folder`                     | `{matched, moved, replied, problems}` (no auto-replies on demand) |
| `start_draft`    | `kind` (new/reply/reply_all/forward), `of`, `account` | `{account, identity, to, cc, subject, body, attachments, reply_to, forward_of}` |
| `save_identity`  | `identity` (id 0 = new)      | the saved identity, or a sentence saying what's wrong |
| `delete_identity`| `id`                         | – (never the account's last one) |
| `known_provider` | `email`                      | what the provider list knows (no network), or null |
| `detect_settings`| `email`                      | `{setup, provider, source, note, unsupported, help}` or null; emits `detect-progress` |
| `add_account`    | `setup`, `password`          | the account (checks IMAP and SMTP first; same address = update, empty password = keep) |
| `remove_account` | `id`                         | – (local copies and saved password only) |
| `check_mail`     | `account` (null = all)       | `{new: [ids], new_in, sent: [lines], sorted, errors: ["Account: reason"], busy}` (`new_in`: folder of the newest; with `account`, only its Inbox) |
| `push_changes`   | –                            | `{errors, busy}` (sends queued changes only) |
| `download_older` | `folder`, `limit`            | `{new, busy}`: up to `limit` of the newest messages the first download left on the server |
| `update_info`    | –                            | `{version, updatable}` (false: built without the release key) |
| `check_update`   | –                            | `{version, notes, date}` of a newer release, or null |
| `install_update` | –                            | `{busy}`; when it works Telex closes, updates and restarts |
| `get_settings`   | –                            | `{settings, language, offer, fresh}` (`language`: the one on screen; `offer`: a language to offer once) |
| `update_settings`| `changes` (only what changes) | the same; saved at once, unknown values left as they were |
| `export_settings`| `choice` `{settings, rules, identities, servers}`, `words` | the path written, or null (cancelled); opens Save |
| `show_export`    | –                            | – (the last exported file in Explorer) |
| `open_import`    | `words` (dialog title, filter) | the plan, null (cancelled), or `{code, detail}` (not_json, not_telex, unreadable); opens Open |
| `apply_import`   | `choice` `{settings, identities, rules}` | `{report, settings}`; the file is matched again first |

A draft carries `reply_to`, `forward_of` and `replaces` (message ids) besides the fields the
user typed (`to`, `cc`, `bcc`, `subject`): threading headers, forwarded attachments, and the
draft or unsent message it was edited from; and `identity`, who it is from (`null` = the
account's default); and `html`, the editor's HTML when it is written in HTML (`null` = plain
text in `body`). Its `attachments` are files from this PC (`path`, chosen through
`pick_files` or dropped this session; others are refused) and ones carried over from
`forward_of` (no `path`, found by name). Accounts in
`folder_tree` carry their `identities`. The changing commands (and `send_message`) apply locally at once; for server
mail the store also queues the change, and the UI calls `push_changes` 2.5 s after the last
one. `busy: true` means another check or push was running and nothing started; the UI
retries. `check_mail` sends each Outbox first, then syncs, and emits `sync-progress` events
(`{account, folder, done, total}`) while it downloads. While new mail shows at once
(`push_mail`), an "inbox-changed" event (the account id) says IDLE saw a change; the UI then
calls `check_mail` with that account.

The window never loads all mail. `list_page` takes a `view` `{folder, sort, ascending, filter,
bodies, thread}` (`folder: null` = all mail except Spam and Trash; `sort`: date, size, flag, from,
subject; `filter`: the quick filter, `bodies` also searching the text; `thread`: a message id,
whose conversation the list shows instead of a folder) and `at`: `"first"`,
`"last"`, `{after: id}`, `{before: id}` (scrolling, keyset on (sort key, id)), `{around: id}`
(the selection after a change) or `{row: n}` (a jump with the scroll bar). `start` is the
position of the first row and `total` the list's length. A filtered list is worked out once and
paged from memory until the mail base changes. After any change (a check, rules, sending, a
push) the UI asks for `folder_tree` and the open page again, nothing else.

Folder keys are `"<account>/<folder>"`, e.g. `firma/inbox`. Dates cross the bridge as local
`YYYY-MM-DDTHH:MM:SS` strings.

Settings: `theme` (one of `settings::THEMES`; "system" follows Windows' light/dark), `language`
("auto", "en", "pl"), `density`, `text_size` (90–200, webview zoom), `prefer_text`, `mark_read`
(moment, at_once, manual), `check_every` (minutes, 0 = only on Get mail), `push_mail` (IMAP
IDLE on each Inbox), `notify` (a notification for new mail while the window isn't active),
`mail_paper` (theme, white: HTML mail in a dark theme), `compose_format` (plain, html),
`reply_format` (same, plain, html), `undo_send` (0, 5, 10, 20, 30 seconds sent mail waits so Z
can take it back), `ask_subject`, `spell_check` and `updates` (ask, daily, off). Reading or changing them
also sets the core's language (`i18n::set`). They live in `%APPDATA%\app.telexmail.desktop\settings.json`, written whole or not at
all, even before the first account. The window keeps only its own state in localStorage
(`telex-ui`: pane sizes, filter scope, "more" folders, the last update check; `telex-look`: a
copy of theme, language and zoom for boot.js).

## Commands

```
npm install                  # once: installs the Tauri CLI
npm run dev                  # run the app with live reload of ui/
npm run build                # release build + installers in target/release/bundle/
npm run set-version -- 0.2.0 # before a release; then commit, tag v0.2.0 (docs/releasing.md)
cargo test -p telex-core     # core tests (run these after every core change)
cargo test -p telex-core -- --ignored   # also checks TLS against a real server (needs internet)
cargo test -p telex-core --release --test scale -- --ignored --nocapture   # 100k-message benchmark
cargo clippy -p telex-core --all-targets -- -D warnings
cargo clippy -p telex --all-targets -- -D warnings
cargo fmt --all
node scripts/check-contrast.mjs   # every theme meets its WCAG level (after changing a color)
node scripts/check-i18n.mjs       # every text has its Polish (after changing a text)
```

Windows prerequisites: Rust via rustup (MSVC toolchain), Visual Studio Build Tools with
"Desktop development with C++", Node.js 20+, WebView2 (preinstalled on Windows 11).
Linux: `libwebkit2gtk-4.1-dev libsoup-3.0-dev libayatana-appindicator3-dev librsvg2-dev
libxdo-dev libdbus-1-dev` (+ `patchelf` to bundle).

## Rules for changes

- Put behavior in `telex-core` with a test. The Tauri layer only converts errors to strings.
- The UI talks to Rust only through `invoke`. No `fetch` to the network from the UI.
- The CSP in `tauri.conf.json` allows only local files. Do not add CDNs or remote fonts.
  The one loosening is `style-src 'unsafe-inline'` (with Tauri told not to add hashes to
  style-src, which would cancel it): the HTML mail frame inherits the app's CSP, and mail
  layout lives in inline styles. Remote mail images are fetched by the core
  (`html::fetch_images`) and passed in as data: URIs; the webview never loads them.
- Colors are tokens in `ui/themes.css`, set by every theme; `styles.css` only uses them. Do not
  hard-code colors. A new token or a new place where text sits on a background goes into the
  pairs of `scripts/check-contrast.mjs`. Text needs 4.5:1 (AAA themes 7:1), and the borders of
  fields and buttons use `--field` (3:1), not `--line`. A new theme is a block in themes.css,
  an id in `settings::THEMES` and in `THEMES` in app.js, and a name in i18n.js.
- Every word on screen goes through `t()` / `tn()` (or `N_()` where it is defined), with its
  Polish in `ui/i18n.js`; static text in index.html is marked `data-i18n`. The English is the
  key; `|context` tells apart one English word that needs two Polish ones. The core writes
  every sentence with `tr!("English", "Polski")` (crates/telex-core/src/i18n.rs) in the
  language the settings say; test the Polish with `i18n::with`. Notes already written on a
  message stay in the language they were written in. Reasons the UI words itself still cross
  the bridge as codes (autoconfig's notes, the import's `skip`).
- Keep the keyboard table below and the status-bar hints in `ui/index.html` in sync.
- Run `cargo fmt`, clippy and the core tests before committing.
- Status text in the UI is short and specific ("Moved to Trash. Press Z to undo.").
  Never claim mail was sent until SMTP actually sends it.

## Keyboard

| Key          | Action                         |
|--------------|--------------------------------|
| ↑ / ↓        | previous / next message        |
| Home / End   | first / last message           |
| Shift+↑ / ↓  | select more (or fewer) messages; Shift+Home / End to the top / bottom |
| Ctrl+click, Shift+click | add one / a range to the selection |
| Ctrl+A       | in the list: select every message |
| R            | reply                          |
| Shift+R      | reply to all                   |
| Shift+F      | forward                        |
| N            | new message                    |
| E            | edit a draft or an unsent message in the Outbox |
| F            | flag / unflag (the selection: flag all, or unflag all when all are flagged) |
| U            | toggle unread (the selection: all read, or all unread when all are read) |
| Shift+U      | mark everything in the folder read |
| Del          | move to Trash (in Trash: press twice to delete for good) |
| M            | move to a folder: type part of its name |
| G            | go to a folder: type part of its name |
| T            | the selected message's conversation (T or Esc goes back) |
| Z            | undo the last delete or move; within the moment after Send, take the message back |
| /            | quick filter (also `from:` `to:` `subject:` `is:unread` `is:flagged` `has:file` `after:` `before:` `larger:` `smaller:`) |
| Tab          | in the quick filter: this folder / all mail |
| Enter, double-click | open the message in a window of its own (Esc closes it); in all-mail results and conversations Enter opens the message's folder |
| H            | switch HTML / plain text view (remembered) |
| Esc          | clear filter, the selection or the conversation; in compose: save draft and close |
| ↓ ↑ Enter Tab Esc | in To, Cc, Bcc: choose a suggested address |
| Ctrl+Enter   | send (compose)                 |
| Ctrl+,       | settings                       |
| Ctrl+B / I / U / K | bold / italic / underline / link (HTML compose) |
| Ctrl+= / Ctrl+− / Ctrl+0 | larger / smaller / normal text (everywhere) |
| ? / F1       | keys and tips (also on hover over the ? in the status bar) |

## Current state (v0.5.0: roadmap steps 1, 1b, 2, 3, 4, 6, 7 without code signing, storage phase 1, settings, HTML compose, usage, Linux, attachments, Bcc, suggestions, several messages at once, M/G/T, IDLE, older mail, undo send, filter operators, STARTTLS)

- New in 0.5.0 (schema version 9: `messages.bcc`, `messages.refs`, `folders.missing`,
  `outbox.send_after`, `accounts.imap_security`; the upgrade reads the start of every stored
  message once to fill in `refs`):
  - Attachments: click opens one in its app (`attachments.rs`: written under
    `%TEMP%\Telex\<id>\` with a safe name and the Mark of the Web, emptied at start; programs,
    scripts, shortcuts, disk images and macro documents, and anything in Spam, open on a second
    click); the arrow saves one, "Save all…" saves every one into a folder. Compose attaches
    files (Attach…, or dropped on the window; dropped with no message open, they start one),
    × takes one off, and over 20 MB in all it warns. Files are read when the message is built;
    only files chosen through Telex's dialog or a drop this session can go out (`files.rs`).
  - Bcc: a field behind "Bcc" next to Cc; blind copies go in the envelope and Telex's own
    copy (Outbox, Sent, drafts) keeps the header; what goes out leaves it out
    (`smtp::without_bcc`).
  - Suggestions in To, Cc and Bcc (`recipients.rs`): people written to first (sent counts
    five times), then those who write most, own addresses and robots (no-reply, bounces)
    left out, each with the name used last; matched on the start of any word of the name or
    address, diacritics ignored.
  - Several messages at once: Shift+arrows, Shift+Home/End, Shift+click, Ctrl+click, Ctrl+A;
    F, U, Del, M and Z work on them all, in one transaction each (`set_*_many`, `move_many`,
    `delete_many`). The server gets the same change to many messages as one command: one
    `UID STORE` per flag and value, one `UID MOVE` (or COPY + EXPUNGE) per destination, one
    expunge, at most 500 UIDs each; new UIDs from COPYUID's two sets; a refused batch goes
    again one message at a time, so only the message the server minds is held back. Shift+U
    marks a folder read.
  - M moves the selection to a folder and G goes to one, both through a small picker: type
    part of the name (diacritics ignored), folders used last first. Z undoes a move as well
    as a delete.
  - T shows the selected message's conversation: everything it answers and everything that
    answers it, by Message-ID, In-Reply-To and References, across the account's folders
    except Spam and Trash (`MailStore::conversation`: one pass over the account's links, then
    followed in memory, however deep).
  - Quick-filter operators (`list::Terms`): `from:` `to:` `subject:` `is:unread|read|flagged|
    unflagged` `has:file` `after:` `before:` (2026, 2026-09, 2026-09-15 or 15.09.2026)
    `larger:` `smaller:` (500k, 5m); the rest are words as before.
  - New mail at once (`push_mail`, on by default): one IMAP IDLE connection per account on its
    Inbox (`imap::watch_inbox`, renewed every 25 minutes, reconnected after a pause that grows
    to ten minutes); a change there makes the UI check that Inbox only (`Scope::Inbox`). A
    server without IDLE is left to the regular checks. A notification (`notify`) names the
    sender and subject of new Inbox mail while the window isn't active.
  - Older mail: each sync remembers how many messages of a folder are on the server but not
    here (`folders.missing`); the status bar says so and downloads them a thousand at a time,
    newest first, without running rules (`imap::download_older`).
  - Undo send (`undo_send`, 5 seconds by default): sent mail waits in the Outbox
    (`outbox.send_after`), then goes in the background; Z within the moment holds it there and
    opens it again in compose. Enter in a header field no longer sends; Ctrl+Enter does.
  - A rule from the selected message: Rules › "New rule for mail from …" (in Sent, "to …"),
    filed where the message is when that is one of the user's folders, else flagged.
  - IMAP over STARTTLS (port 143) besides SSL/TLS (993), in the account form and from setup
    files that offer only STARTTLS; never unencrypted.

- Working window with folder tree, sortable message list, preview, compose, quick filter,
  resizable panes, density toggle, a settings window, eight themes, English and Polish.
- Settings (`settings.rs`, a dialog with tabs Appearance, Language, Accounts, Mail, Reading,
  Writing, Usage, Updates, Import and export; toolbar button and Ctrl+,): every change is saved
  at once and shows at once. Accounts holds the list (servers, last check), the add-account
  wizard, the server form and each account's identities; "Add account" anywhere opens it.
  Reading: HTML or plain text, HTML mail in a dark theme (theme paper or white), the trusted
  senders. Writing: new messages and replies in plain text or HTML, ask before sending without
  a subject, spell check. Themes:
  Match Windows, Light, Dark, Paper, Heather, Fjord, Phosphor, High contrast light and dark
  (AAA); each card in the dialog is drawn in its own colors (data-theme on the card) and shows
  its level, read from the theme's `--wcag`. Rows, text size 90–200% (Ctrl+= / Ctrl+− /
  Ctrl+0), how often to check mail, when a message counts as read, HTML or plain text, daily
  update checks. Windows contrast themes (forced colors) get the system highlight for the
  selection and pressed choices; reduced motion turns off spinners and transitions.
- Language: "auto" takes the first of Windows' display languages Telex speaks
  (`GetUserPreferredUILanguages`), else English. On the first start the account wizard has an
  English | Polski switch, and when Windows is English but the region Polish
  (`GetUserDefaultLocaleName`), a one-line nudge in Polish. Switching redraws everything live
  (`relabel` in app.js). Polish plurals use `Intl.PluralRules`; dates and decimals follow.
- Import and export: one JSON file (`format: telex-settings`, `version` 1) with the settings,
  rules (account by address, move targets by folder id and name), identities (per account
  address) and server settings, never passwords. Import reads it into a plan (`Import::read`):
  settings that would change, each rule and identity as new, updated (same name / address),
  unchanged or skipped with a reason code (no_account, no_folder, unreadable, invalid), and
  accounts not set up here, whose "Set up…" opens the form filled in with a warning to check
  the servers. The user ticks parts; `apply_import` matches again and applies. A file from a
  newer Telex is read as far as it goes. Older versions' window preferences (density, HTML
  view, update answer in localStorage) move into settings.json on the first start.
- Real IMAP accounts: a wizard (Settings › Accounts; "Add account" under the folder tree
  opens it, and it opens by itself while there is none) asks for name, email and password, finds the servers (see below) and logs
  in to both before saving; the full settings form is behind "Set up manually", opens
  filled in when finding or logging in fails, and is where accounts are edited. SSL/TLS (993)
  or STARTTLS (143), password login. Folders come from the server; special folders
  are recognized by RFC 6154 flags, then by common English/Polish names.
- Two-way sync on start, on "Get mail" and as often as Settings › Mail says (10 minutes by default): push queued changes, download,
  push what changed meanwhile. The first sync takes the newest 300 messages per folder
  (`imap::FIRST_SYNC_LIMIT`); later syncs take everything new, and older ones come on request
  (the status bar's "Download"). Downloading uses EXAMINE and
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
  is saved. Accounts saved before step 2 have none until edited (Settings › Accounts › Edit;
  an empty password keeps the saved one). Send builds the message (`smtp::compose`: UTF-8
  plain text, or for HTML multipart/alternative with the cleaned HTML and its plain text,
  In-Reply-To/References for replies, forwarded attachments from the original's raw), puts it in the account's local Outbox and sends the Outbox at once; `check_mail`
  sends it too. Accepted mail moves to Sent and its exact bytes go to the server's Sent
  folder through the change queue (APPEND, kind `append`); the next download adopts the copy
  by Message-ID. A server refusal (5xx, except login errors) stays in the Outbox with the
  reason and isn't retried; anything else is retried at every check. E reopens a draft or an
  unsent message; saving or sending replaces the old one. Drafts are saved as whole messages
  (`smtp::compose_draft`, half-typed recipients allowed), so their HTML and forwarded files
  come back with E. The status bar says "Sent" only after the SMTP server accepts the message.
- HTML compose: the Text | HTML switch in compose (Settings › Writing picks the start: new
  messages plain or HTML; replies in the message's format, plain or HTML). The editor is a
  contenteditable with the browser's own commands (bold, italic, underline, strikethrough,
  lists, quote, link with Ctrl+K, clear); pasted HTML goes through `clean_html`. The core
  cleans again before sending (`html::compose_clean`: those tags, links to http(s) and
  mailto only, no styles or images) and makes the plain text (`html::to_text`: lines, "- "
  and "1. " lists, ">" quotes, a link's address after its text). Switching to Text keeps the
  words and drops the formatting; a reply in HTML quotes the original's text as a
  blockquote. Signatures are plain text, put in line by line.
- Quick filter: sender, recipients and subject match as you type (any part, case ignored),
  message text a moment later through FTS5 (whole words as prefixes, diacritics ignored). It
  looks in the open folder or, toggled with Tab or the chip in the box, in all mail (every
  account, not Spam or Trash), where a Folder column with the account's dot replaces Size.
- The window loads mail a page at a time (`list.rs`; storage phase 1): `folder_tree` for the
  tree and counts, `list_page` for the list. The list draws only the rows on screen plus 20
  above and below, in a table whose spacer rows keep the full scroll height; it keeps up to 600
  rows near the screen and asks for the next or previous page while scrolling. Sorting and
  ties are as before (newest first within equal keys). On the 100k benchmark: first list in
  about 60 ms, folder switch under 10 ms, an arrow key about 5 ms (docs/storage.md).
- Folder tree: one line per account, its address (the name is in the tooltip); with more than
  one account, each has a color dot (`--acct-0..5` tokens), also in the status bar and all-mail
  results. Each account shows Inbox and Sent, Drafts and the Outbox when they hold mail, and the
  open folder; the rest are behind "N more" (with their unread count), opened per account and
  remembered (localStorage `telex-ui`: `more`, and `scope` for the filter).
- Usage (`usage.rs`, schema version 8: `accounts.quota_used_kb`, `quota_limit_kb`, `synced_at`,
  `sync_error`): each sync asks servers with QUOTA (RFC 9208) for the storage of the Inbox's
  quota root and keeps it; `check_mail` records each account's last good sync and last error.
  The status bar shows the open account's quota as a small bar (warning colors from 80%; the
  tree shows a chip from 90%), clicking it opens Settings › Usage: per account the server's
  storage, this PC by kind of folder (a stacked bar), mail received, sent and in Spam per day
  over 30 days, the busiest senders (click: all their mail), the largest messages (click:
  open), what still waits, and the mail base's size. The counts cover downloaded mail.
- Mail base (schema version 9; version 7 added `message_content`): `messages` holds the summaries; body and raw message live in
  `message_content` (same id, removed with the message); `messages_fts` is a contentless FTS5
  index (`contentless_delete=1`) filled by triggers. The upgrade from version 6 moves the text
  row by row, so the file grows little. SQLite runs with WAL, `synchronous=NORMAL`, a 32 MiB
  cache, 256 MiB memory map and temp tables in memory, and `PRAGMA optimize` on close.
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
  the plain-text body. In the light themes the canvas is white. In the dark ones (Dark, Fjord,
  Phosphor, High contrast dark) the UI passes the theme's tokens (`--surface`, `--fg`,
  `--accent`, `--line`, `--muted`, checked to be plain hex colors) and the core recolors
  inline styles, `bgcolor` and `color` (`colors.rs`): light backgrounds go dark (white to the
  theme's surface, grays taking its tint), dark text goes light (black to its text color),
  dark backgrounds and light text stay, hue is kept, images are untouched. Settings › Reading
  can keep mail on white; the sun button above a message shows that one on white.
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
- Identities (schema version 6): each account has one or more (name, address, signature,
  reply header, forward header), one of them the default; the last one can't be deleted.
  Existing and new accounts start with one: the account's name and address, "Pozdrawiam",
  the Polish quote header. Templates take {name} {email} {from} {date} {time} {subject} of
  the original. `drafts::start` builds every draft except edits: a reply goes to Reply-To
  when there is one, reply all leaves out your own addresses and repeats nobody, the
  identity is the one the message was sent to (an alias) or the default, and Re:/Fwd:/Odp:/
  PD: prefixes don't pile up. Compose has a From picker (all identities, by account);
  switching swaps the signature in the text. Sent mail and drafts are from the chosen
  identity (From header and SMTP envelope). Managed per account: Settings › Accounts ›
  Identities.
- Mail is stored in full (`messages.raw`), so attachments can be opened later without
  downloading again.
- No account yet: an empty store in memory (`MailStore::is_in_memory`), a welcome panel and
  the wizard; the first account creates `telex.db`. Removing the last account keeps the file.
- Finding servers (`autoconfig.rs`): Telex's provider list (checked against Mozilla's ISPDB
  and live TLS; WP, o2 and WEB.DE log in with the part before the @), then the domain's
  autoconfig file (`autoconfig.<domain>`, `/.well-known/`), its SRV records (RFC 6186/8314:
  `_imaps`, `_submissions`, `_submission`), its MX records pointing to a listed provider,
  and last probing `imap.`/`mail.`/`smtp.`/the MX host/the domain (TLS with a valid
  certificate and a greeting, 6 s each, all at once). Only DNS and the domain's own servers
  are asked. Tips (`note`: app password, turn on IMAP) and reasons (`unsupported`: Microsoft
  sign-in, Proton Bridge) cross the bridge as codes; the UI words them, ready for translation.
- Updates (`src-tauri/src/updates.rs`, tauri-plugin-updater): only builds compiled with the
  release key's public half in `TELEX_UPDATER_PUBKEY` (`option_env!`) register the plugin;
  its `plugins.updater` config (key, `requireSignedVersion`, passive Windows install) is put
  into the context at start, so no key is in the repository. Dev and CI builds say "This build
  can't update itself". The endpoint is set in code: `releases/latest/download/latest.json`
  of lucco669/telexmail (User-Agent "Telex", nothing else sent). The status bar shows the
  version; clicking it opens the update dialog and checks. Until answered, an updatable build
  asks in the status bar "Check for updates once a day?"; the answer is a setting (`updates`),
  the last check is in the window's localStorage (`updateChecked`). A found update shows as
  a status-bar chip; installing saves an open compose to Drafts, waits until no check or send
  is running (it holds both busy flags), downloads with "update-progress" events, verifies,
  runs the installer and exits; the installer restarts Telex. On Linux only the AppImage
  updates (in place, then Telex restarts into it); a .deb says it can't. Releases: `release.yml` on a
  `v*` tag (tag must match the version everywhere; the tag message becomes the notes),
  tauri-action v1 with `uploadUpdaterJson`, draft release; see docs/releasing.md.

## Roadmap

1. ~~**Real mail, read-only.**~~ Done; see Current state, with STARTTLS, older messages on
   request and IMAP IDLE in 0.5.0.
1b. ~~**Write back.**~~ Done; see Current state. Not yet: CONDSTORE/QRESYNC (flag changes are
   found by asking for the flags of every stored message, fine for thousands, not millions).
2. ~~**Sending.**~~ Done; see Current state, with attaching files, Bcc and undo send in
   0.5.0. Not yet: drafts on the server's Drafts folder, and skipping the Sent copy for servers
   that save one themselves (Gmail and Outlook.com do; matters with step 5).
3. ~~**HTML mail.**~~ Done; see Current state, with opening and saving attachments in
   0.5.0. Not yet: `<style>` blocks (dropped, since CSS can load from the network; keeping
   them needs a CSS sanitizer).
   HTML compose is done (see Current state); not yet: images and colors in it, HTML
   signatures, quoting a reply's original HTML instead of its text.
4. ~~**Filters ("Sorting Office").**~~ Done; see Current state, with a rule started from a
   message in 0.5.0. Not yet: regular expressions, conditions on headers and attachments,
   actions that copy or forward, rules that run on a schedule.
5. **Gmail and Outlook.com.** OAuth2 login (both reject plain passwords over IMAP). Waiting
   for the owner to register OAuth clients with Google (Cloud project; Gmail's scope is
   restricted, so unverified apps stay in testing mode) and Microsoft (Entra app that allows
   personal accounts).
6. ~~**Templates and identities.**~~ Done; see Current state. Not yet: templates for new
   messages (a greeting), HTML signatures, per-folder templates.
7. **Shipping.** ~~Auto-update~~ (see Current state) and the website (`site/`) are done. Not
   yet: code signing (SmartScreen warns about the installer), macOS. Linux ships as an
   AppImage (self-updating) and a .deb. The repository is public and v0.5.0 is released;
   docs/releasing.md has the process.
8. **Storage** (plan in docs/storage.md). ~~Phase 1, fast at any size~~ and ~~downloading older
   messages~~: done, see Current state and the numbers in docs/storage.md. Next: backup and
   restore with a movable mail base, local folders with .eml/mbox export and import, and POP3.

## Open decisions

- License (none chosen yet).
- Final product name and domain. "Telex" is the working name; the repo is `telexmail`.
- App icon: the dot-matrix T in `brand/`. `src-tauri/icons/` is generated from it with
  `npm run icons` in `site/` (not `tauri icon`, which can't use the small-size variant).

## Prior art worth reading

- Skim (github.com/nikserg/skim): MIT, Rust + Tauri 2 + WebView2 + SQLite FTS5, Windows.
  Useful reference for IMAP sync, offline queue and HTML sanitizing. Minimalist, no filters.
- The Bat! (Ritlabs): the feature model to aim for.
