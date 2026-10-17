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
- **HTML when it helps, plain text always along.** New messages and replies start in HTML (since
  0.9.8, the owner's choice); plain text in a mono face is a setting and a per-message switch, and
  HTML always goes out with a plain-text copy.
- **Mail follows the theme.** In a dark theme HTML mail goes on the theme's dark paper with its
  colors turned around (OKLab, hue kept); one click shows it on white, as sent.

Anti-goals: Electron, AI features by default, a web-app look, anything that phones home without
asking. (The one count Telex sends, once a day without an ID, is off until the user says yes;
docs/counting.md.)

## Layout of the repo

```
crates/telex-core/   All mail logic. Unit-tested. No UI, no Tauri.
  src/store.rs       MailStore: the local mail base in SQLite (+ FTS5) and the queue of changes
                     for the server (pending_ops); the folders on this PC (LOCAL_ID "local") and
                     moves to and from them. Its tests are in src/store/tests.rs; folders' own
                     templates (schema 15) in src/store/templates.rs.
  src/list.rs        The message list a page at a time: folder counts, keyset pages in every
                     sort, the quick filter (one folder or all mail, with operators such as
                     from: is:unread after:) and its SQL functions, conversations, ranges.
  src/migrations/    Database schema, one numbered file per version, listed in MIGRATIONS in
                     store.rs. Released files are never edited; a change is a new file.
  src/imap.rs        Two-way IMAP sync (async-imap + rustls, SSL/TLS or STARTTLS): download,
                     push queued changes a batch per command, folder mapping, older mail on
                     request, IDLE on the Inbox. Tests run it against a fake IMAP server.
  src/servers.rs     One mail server (by IP address) gets at most 4 checks and 8 IDLE watchers
                     at once; one that takes connections but doesn't greet rests (1 min,
                     doubling to 15) before anything connects again. And the connection log
                     (in memory, the last 1,000: each step timed, where one failed and why,
                     IMAP and SMTP) for Settings › Connections.
  src/smtp.rs        Sending (lettre + rustls): compose a draft into RFC 5322 (Bcc, files from
                     disk), send the Outbox. Tests run it against a fake SMTP server.
  src/attachments.rs Attachments of stored mail opened or saved: safe file names, files that
                     can run code, the Mark of the Web on Windows.
  src/recipients.rs  People to suggest in To, Cc and Bcc, from the mail base itself.
  src/html.rs        HTML mail: sanitize (ammonia) into an inert document for a sandboxed frame,
                     inline cid: images, fetch remote images on request (ureq, public IPs only);
                     dark paper in a theme's palette; HTML compose: what may be sent (a strict
                     allowlist: the palette's colors and sizes, pictures as data: within limits,
                     the signature's block), data: pictures into cid: parts, a stored message's
                     HTML for the editor and for a reply's quote, plain text from HTML and back.
  src/images.rs      Pictures put into a message (pasted, dropped, Insert picture…): read, turned
                     upright and made smaller as Settings › Writing says (image crate), as data:.
  src/colors.rs      CSS colors read and turned around for dark paper (OKLab), for html.rs.
  src/count.rs       The daily count, only after a yes: what it sends (no ID; first this week /
                     month flags), when it is due (UTC days), count.json; docs/counting.md.
  src/mailbox.rs     Mail as files: a folder written as mbox; mbox, .eml, .emlx, .msg files and
                     Maildir and MH folders read into a folder on this PC (mboxrd; Status,
                     X-Status and X-Mozilla-Status flags, Thunderbird's deleted ones left out).
  src/migrate/       Moving in from another mail program, found on this PC only: mod.rs (what a
                     program has, mail into folders on this PC, filters into rules, Maildir/MH
                     folders of Evolution, KMail, Claws, mutt), thunderbird.rs (profiles.ini,
                     prefs.js accounts and identities, Local Folders, msgFilterRules.dat; also
                     Betterbird, SeaMonkey, Postbox), outlook.rs (registry accounts on Windows,
                     .pst with outlook-pst, .msg with cfb), mapi.rs (Outlook's properties →
                     RFC 822). Never passwords: the user types them in the dialog.
  src/backup.rs      Backups of the mail base (VACUUM INTO on a connection of its own), the
                     backup folder's list and pruning, restoring (after_restore), compacting.
  src/usage.rs       What the Usage tab shows: quota, sizes per folder, 30 days of mail in and
                     out, top senders, largest messages, the last sync.
  src/i18n.rs        The core's language: `tr!("English", "Polski")`, set from the settings.
  src/filters.rs     Rules (the Sorting Office): conditions, actions, running them with notes,
                     auto-replies with RFC 3834 safeguards (shared with the away reply), previews.
  src/saved_searches.rs  Saved list queries (schema 13); one unreadable row is left out alone.
  src/risk.rs        Scam warning signs of an opened message, offline (docs/mail-risk.md): brand
                     lookalikes, disguised links, hidden characters. Advisory points, not a filter.
  src/spam.rs        Telex's own spam filter, offline: tokens, Robinson/Fisher scores (as SpamBayes),
                     learning from Spam and read mail and from moves into and out of Spam, new
                     Inbox mail sorted before the rules. Its tests are in src/spam/tests.rs.
  src/away.rs        The away reply, one per account: on the server as a Sieve vacation where
                     it takes one, else Telex answers new mail when it checks.
  src/sieve.rs       ManageSieve (RFC 5804, port 4190, STARTTLS before the login) and Telex's
                     marked part of the running Sieve script, put in and taken out whole.
  src/drafts.rs      Starting a draft (new, reply, reply all, forward) from an identity's
                     signature (text or HTML) and templates, or a folder's own: recipients,
                     subject, the text and the same in HTML (the original's HTML quoted).
  src/markdown.rs    Messages as Markdown for the clipboard (Copy as Markdown): a heading, who and
                     when as a list, the text (HTML-only mail through html::received_text).
  src/mime.rs        Raw RFC 822 → Message (mail-parser): charsets, HTML-to-text, attachments.
  src/autoconfig.rs  Finding an address's servers for the add-account wizard: provider list,
                     the domain's autoconfig file, SRV and MX records (Windows' resolver;
                     hickory-resolver elsewhere), probing.
  src/settings.rs    Settings (settings.json next to the mail base), Windows' languages, and the
                     settings file: export, and import as a plan (what changes) applied in parts.
  src/spell.rs       Spelling on a thread of its own: Windows' spell checker (COM, the windows
                     crate) for the languages Windows has, Hunspell dictionaries (spellbook) for
                     the rest: LibreOffice's, the distribution's, and those Telex fetches on
                     request (LibreOffice's at a pinned commit, SHA-256 checked); the user's words
                     (words.txt); which words to check (no quotes, links, addresses, ACRONYMS).
  src/sample.rs      A Polish sample mailbox for the tests. Never in the app.
  tests/scale.rs     Benchmark on a synthetic 100k-message mail base (ignored; see Commands).
src-tauri/           Thin Tauri shell: exposes MailStore methods as commands. Keep logic out of here.
  src/secrets.rs     Passwords in the system store (keyring crate), service "Telex": Credential
                     Manager on Windows, Secret Service (GNOME Keyring, KWallet) on Linux.
  src/files.rs       Attachments opened in their app or saved (dialogs), files chosen or dropped
                     to attach; only files chosen this way can be sent. Drag and drop events.
                     Pictures for the HTML editor (`editor_image`).
  src/folders.rs     Folders on this PC (make, rename, remove), a folder saved as mbox, mail
                     files read into one (dialogs, "import-progress" events).
  src/migrate.rs     Moving in: finding programs, a .pst or mail folder picked in Open, and
                     bringing accounts (passwords saved, a first login each), mail and filters.
  src/away.rs        The away reply's commands: reading, and saving (onto the server or not).
  src/spell.rs       Spelling commands: the languages (the setting's, or Windows' and the
                     interface's), checking, suggestions, the user's words, fetching dictionaries.
  src/backup.rs      Settings › Backups: backing up now or when due, the backup folder,
                     restoring (holds the busy flags, swaps the store) and compacting.
  src/count.rs       `count_copy`: today's count when `usage_count` is on, release builds only.
  src/watch.rs       One IMAP IDLE watcher per account ("inbox-changed" events), and the
                     new-mail notification (tauri-plugin-notification).
  src/updates.rs     Updates from GitHub Releases (tauri-plugin-updater), release builds only.
  src/settings.rs    Settings commands; Save and Open dialogs (tauri-plugin-dialog, from Rust only).
  src/lib.rs         The commands above and the app: one Telex at a time on a mail base
                     (tauri-plugin-single-instance, registered first: a second start shows the
                     window), and a mail base that won't open said in a dialog at start.
  tauri.release.conf.json  Merged in by the release workflow: turns on the updater's files.
  tauri.linux.conf.json    Merged on Linux by Tauri: AppImage and .deb instead of nsis/msi.
ui/                  The interface. index.html + styles.css + app.js, no build step, no framework.
  themes.css         Every color, one block per theme (data-theme on <html> and on theme cards).
  i18n.js            English and Polish: t(), tn() (Polish plurals), data-i18n in index.html.
  boot.js            Puts the saved theme, language and text size on <html> before first paint.
  links.js           A link's real destination, hidden characters made visible (no network).
  fonts/             IBM Plex, Inter, JetBrains Mono, Atkinson Hyperlegible Next and Mono, Source Sans 3
                     and Source Code Pro (OFL, from @fontsource; latin and latin-ext) bundled so the
                     app works offline; fonts.css (the Source files stand in for heavier weights).
site/                The landing page (Next.js 16, deployed on Vercel with Root Directory "site").
                     Separate npm project; see site/README.md. Its screenshots come from ui/ and
                     an English demo mailbox (site/scripts/demo-mailbox.json) via `npm run screens`;
                     its live demo (/demo) is ui/ itself with a stand-in for the core
                     (site/demo/backend.js), copied into site/public/demo-app by `npm run demo`.
                     Retake both after visible UI changes. What is built but not released is
                     marked with its fixed `since` version in site/lib/content.ts ("Coming" until it ships).
                     /api/count takes the app's counts, /stats is the owner's private page of
                     downloads and copies (STATS_PASSWORD), with Supabase sums; docs/counting.md.
brand/               The logo: telex-mark.svg (48 px and up) and telex-mark-small.svg (32 px and
                     below). `npm run icons` in site/ makes every app and site icon from them.
scripts/             set-version.mjs: sets the version in every file that has it, or checks them.
                     check-signing-key.mjs: the release key pair belongs together (release.yml).
                     check-contrast.mjs: every theme meets its WCAG level (CI; --json for the site).
                     check-i18n.mjs: every text in app.js and index.html has its Polish (CI).
docs/releasing.md    The release process, the signing keys, and what an update check sends.
docs/counting.md     Downloads and copies counted: what is sent, what is kept, GDPR, setting up /stats.
docs/storage.md      How mail is stored, and the plan: fast at any size, backups, local folders, POP3.
.github/workflows/   ci.yml: core tests and the contrast and Polish checks on Linux, Linux app clippy
                     (ubuntu-22.04), Windows clippy, core tests and installer build (can't update
                     itself). Actions are pinned to commits, the tag in a comment.
                     release.yml: on a v* tag, a draft release, then signed Windows installers,
                     then AppImage + .deb, and latest.json for both, uploaded by the draft's id
                     (so a build that ends after the draft is published still lands on it).
                     The signing key reaches only the steps that sign, as a file.
```

The mail base is `%APPDATA%\app.telexmail.desktop\telex.db` (on Linux
`~/.local/share/app.telexmail.desktop/`). It is created when the first account is added; until
then the app starts with an empty store in memory and opens the wizard.

Commands the UI calls (`src-tauri/src/lib.rs`), all backed by `telex_core::MailStore`:

| Command          | Args                         | Returns                     |
|------------------|------------------------------|-----------------------------|
| `folder_tree`    | –                            | `{accounts, counts: {"<folder key>": {total, unread, older?, stuck?}}, problems: {"<account>": reason}, away: [{account, place, starts, ends}], quota: {"<account>": {used_kb, limit_kb}}, synced: {"<account>": time}}` (empty folders left out; `older`: on the server, not downloaded; `stuck`: Outbox mail that didn't go (refused, a failed try, taken back); `problems`: accounts whose last check failed; `quota` and `synced`: the server's storage and the last good check, for the status bar and Settings › Accounts) |
| `list_page`      | `view`, `at`, `limit` (≤ 500) | `{total, start, rows}`: one page of summaries (see below) |
| `list_ids`       | `view`, `from`, `to`         | ids between two messages of the list, both included (Shift selection), or all of it when either is null (Ctrl+A) |
| `next_unread`    | `view`, `after`, `back`      | `{id, row}`: the first unread message after `after` in the list's order (before it with `back`; from the start or end when null), or null (Space) |
| `get_message`    | `id`                         | full message with body (and `bcc` on mail the user wrote), and `risk` `{score, signals, limited}` (risk.rs) |
| `message_markdown` | `ids`                      | the messages as Markdown, oldest first (Copy as Markdown, Ctrl+C); reads nothing |
| `set_unread`     | `id`, `unread`               | –                           |
| `set_flagged`    | `id`, `flagged`              | –                           |
| `set_unread_many`, `set_flagged_many` | `ids`, `unread` / `flagged` | how many changed (others are left alone) |
| `move_messages`  | `ids`, `folder`              | `[[id, from]]` for undo; all or none; within an account, or to and from the folders on this PC (server mail moved there leaves the server; moved back, it is uploaded); into or out of Spam, the spam filter learns from it |
| `mark_spam`      | `ids`, `spam`                | `[[id, from]]` for undo: to each account's Spam folder (true) or back to its Inbox (false), learned from; all or none (mail on this PC can't) |
| `spam_status`    | –                            | `{spam, ham, needed}`: messages the spam filter learned from, and how many of each it needs before it moves anything |
| `delete_messages`| `ids`                        | `{trashed: [[id, from]], removed}` (spam going to Trash is marked read first, as with `move_messages`) |
| `mark_folder_read` | `folder`                   | how many were unread        |
| `choose_folder`  | `folder`, `kind` (sent, drafts, archive, spam, trash) | – (that server folder is used for it from now on; the one that was becomes the user's own; kept over the server's flags) |
| `open_attachment`| `id`, `index`, `sure`        | `opened`, or `risky` / `spam` (not opened: ask, then again with `sure`) |
| `save_attachment`| `id`, `index`, `words`       | the path saved to, or null (cancelled); opens Save |
| `save_attachments`| `id`, `words`               | the folder saved to, or null; opens a folder picker |
| `pick_files`     | `words` `{title, pictures?}` | `[{name, size_kb, path}]` to attach (empty: cancelled; with `pictures`, the filter's name, only pictures are offered); also "files-dropped" events `{files, x, y}` (physical pixels) for files dropped on a window, and "files-over" `[x, y]` or null while they are over it |
| `recipients`     | –                            | `[{name, email, sent, received}]`, most likely first, at most 2000 |
| `save_outgoing`  | `draft`                      | new message id (a draft, kept whole: its HTML and forwarded files) |
| `discard_draft`  | `id`                         | true when that draft (on this PC, in Drafts) was removed for good: compose's Discard drops its autosave |
| `draft_html`     | `id`                         | a draft's or unsent message's HTML, cleaned for the editor with its pictures as data: (`html::editable`), or null (plain text) |
| `editor_image`   | `path` (picked or dropped this session) or `data` (base64, pasted), `signature` | `{src, width, height, bytes, smaller}`: a picture for the HTML editor as a data: URI, made the size `image_size` says (a signature's: 600 px, 100 KB), or a sentence (not PNG/JPEG/GIF/WebP, too large) |
| `html_to_text`   | `html`                       | the plain text it would go out with (compose switches to Text) |
| `text_to_html`   | `text`                       | HTML for the editor: a `<div>` per line, `>` quotes as blockquotes, links |
| `clean_html`     | `html`                       | pasted HTML cleaned to what Telex sends (palette styles, data: pictures; remote pictures left out, never fetched) |
| `mentions_attachment` | `text`, `signature`    | whether the user's own text (no quotes, forwarded part or signature) says a file is attached (`drafts::mentions_attachment`; Send asks when none is) |
| `send_message`   | `draft`                      | `{queued, wait, sent: [lines], errors}`; with `wait` > 0 it goes after that many seconds, then an "outbox-sent" event `{queued, sent, errors}` |
| `hold_message`   | `id`                         | – (takes back mail still waiting to go; it stays in the Outbox, held) |
| `get_html`       | `id`, `loadImages` (true/false/null), `dark` (palette or null) | `{document, blocked, loaded, sender, trusted}` or null for plain-text mail |
| `trusted_senders`| –                            | senders whose images load without asking, A to Z |
| `connections`    | –                            | `{servers: [{address, names, sessions: [used, max], watches: [used, max], silences, resting_until}], log: [{id, started, purpose, protocol, user, host, port, address, steps: [{step, ms}], failed, error, ended}]}` newest first (`purpose`: check, push, older, test, watch, send; `step`: lookup, wait, connect, starttls, tls, greeting, login, work; `ended` null: still open) |
| `wake_server`    | `address`                    | – (a resting server's rest ends: "Try now") |
| `usage`          | –                            | `{accounts: [{account, quota, synced_at, sync_error, folders, flow, senders, largest, queued}], base_bytes, free_bytes, since}` |
| `trust_sender`   | `email`, `trusted`           | – (that sender's images load without asking, except in Spam) |
| `open_link`      | `url`                        | – (http/https only, in the default browser) |
| `list_rules`     | –                            | rules in the order they run |
| `save_rule`      | `rule` (id 0 = new)          | the saved rule, or a sentence saying what's missing |
| `delete_rule`    | `id`                         | –                           |
| `move_rule`      | `id`, `delta` (-1 up, 1 down) | –                          |
| `move_account`   | `id`, `delta` (-1 up, 1 down) | – (server accounts; the folders on this PC stay last) |
| `preview_rule`   | `rule`, `view`               | how many of the list's first 2,000 messages the rule would match, with samples (`listed`, `read`, `limited`: the list was longer); changes and sends nothing |
| `list_saved_searches`, `save_search` (`search`, id 0 = new), `delete_search` (`id`) | | saved list queries, A to Z |
| `run_rules`      | `folder`                     | `{matched, moved, replied, problems}` (no auto-replies on demand) |
| `start_draft`    | `kind` (new/reply/reply_all/forward), `of`, `account`, `folder` (the open one, for its templates) | `{account, identity, to, cc, subject, body, html, caret_line, attachments, reply_to, forward_of}` (`html`: the same start for the HTML editor, the signature in a `telex-signature` block, the original's HTML quoted; `caret_line`: the line after a greeting) |
| `folder_templates` | –                          | `[{folder, new_template, reply_template, reply_header}]`: folders' own templates (gone folders left out) |
| `save_folder_template` | `template`             | – (all empty takes them away; a gone folder is a sentence) |
| `save_identity`  | `identity` (id 0 = new)      | the saved identity (`signature_html` cleaned, `signature` its text; a signature without formatting stays plain, `signature_html` null), or a sentence saying what's wrong |
| `delete_identity`| `id`                         | – (never the account's last one) |
| `known_provider` | `email`                      | what the provider list knows (no network), or null |
| `detect_settings`| `email`                      | `{setup, provider, source, note, unsupported, help, confirm}` or null; emits `detect-progress` (`confirm`: servers found through DNS outside the address's domain; the wizard asks before sending them the password, RFC 6186) |
| `add_account`    | `setup`, `password`          | the account (checks IMAP and SMTP first; same address = update, empty password = keep) |
| `remove_account` | `id`                         | – (local copies and saved password only; an away reply on the server is taken off first) |
| `get_away`       | `account`                    | `{account, enabled, starts, ends, subject, body, days, place, script, created}` (off when never set) |
| `save_away`      | `away`                       | `{away, why}`: onto the server (ManageSieve) where it takes it (`place: server`), else Telex answers (`why: {code, detail}`: no_server, no_tls, no_vacation, no_dates, login, refused) |
| `check_mail`     | `account` (null = all)       | `{new: [ids], new_in, sent: [lines], sorted, spam, errors: ["Account: reason"], busy}` (`new_in`: folder of the newest; with `account`, only its Inbox; `spam`: new mail the spam filter moved to Spam) |
| `push_changes`   | –                            | `{errors, busy}` (sends queued changes only) |
| `download_older` | `folder`, `limit`            | `{new, busy}`: up to `limit` of the newest messages the first download left on the server |
| `update_info`    | –                            | `{version, updatable, countable}` (false: built without the release key) |
| `count_copy`     | –                            | true when today's count went (only with `usage_count: on`, release builds; at most one a UTC day) |
| `check_update`   | –                            | `{version, notes, date}` of a newer release, or null |
| `install_update` | –                            | `{busy}`; when it works Telex closes, updates and restarts |
| `get_settings`   | –                            | `{settings, language, offer, fresh}` (`language`: the one on screen; `offer`: a language to offer once) |
| `update_settings`| `changes` (only what changes) | the same; saved at once, unknown values left as they were |
| `export_settings`| `choice` `{settings, rules, identities, servers}`, `words` | the path written, or null (cancelled); opens Save |
| `show_export`    | –                            | – (the last exported file in Explorer) |
| `open_import`    | `words` (dialog title, filter) | the plan, null (cancelled), or `{code, detail}` (not_json, not_telex, unreadable); opens Open |
| `apply_import`   | `choice` `{settings, identities, rules}` | `{report, settings}`; the file is matched again first |
| `create_local_folder` | `name`                | the new folder's key (`local/<id>`), or a sentence saying what's wrong |
| `rename_local_folder` | `folder`, `name`      | – (the user's own folders on this PC; the key stays) |
| `delete_local_folder` | `folder`              | how many rules lost their move to it; only an empty folder |
| `export_folder`  | `folder`, `words` `{title, filter, file_name}` | the mbox file written, or null; opens Save |
| `import_mail`    | `folder`, `paths` (dropped this session; null opens Open), `words` | `{added, already, problems}` or null; "import-progress" events `{done, bytes, file}`; folders on this PC only |
| `find_programs`  | –                            | `[{path, program, name, accounts: [{key, setup, identities, present, blocked?}], folders: [{key, name, bytes, messages?, tick}], filters}]`: Thunderbird and its kin, Outlook (profiles and .pst files), Maildir/MH folders; nothing over the network |
| `open_source`    | `folder` (true: a folder, false: a .pst), `words` | the same for what the user picks, or null (`move_in` takes only sources found or picked this session) |
| `spell_languages`| –                            | `{languages: [{tag, own}], chosen, auto, downloads: [{tag, bytes}], missing}` (`own`: fetched by Telex; `auto`: chosen from Windows' display languages, its language list, its region and the interface's; `missing`: downloads for those) |
| `spell_check`    | `text`                       | `[[start, length]]` in UTF-16 units: the words no chosen language knows |
| `spell_suggest`  | `word`                       | up to 7 suggestions |
| `spell_add`      | `word`                       | – (right from now on, in every language: words.txt next to the settings) |
| `spell_download`, `spell_remove` | `tag`          | the languages again; fetches LibreOffice's dictionary (GitHub, pinned, SHA-256 checked) or removes one Telex fetched |
| `move_in`        | `path`, `choice` `{accounts, passwords: {key: password}, folders, filters}` | `{accounts: [{email, error}], mail: {added, already, problems}, rules: [plan items]}`; "move-progress" events `{step, done, total, name}` |
| `open_window`    | `id`                         | – (the message in a window of its own: index.html?message=<id>) |
| `backup_state`   | –                            | `{base, base_bytes, free_bytes, dir, suggested, backups: [{path, made, bytes}], running}` |
| `backup_now`     | –                            | the backup written; the folder keeps the newest `backup_keep` |
| `auto_backup`    | –                            | a backup when `backup_every` says one is due, else null |
| `pick_backup_dir`| `words`                      | a folder or null (the window saves it as `backup_dir`) |
| `inspect_backup` | `path` (from the list; null opens Open), `words` | `{path, made, bytes, accounts, messages, on_this_pc}` or null |
| `restore_backup` | `path` (the one last inspected) | where the mail base as it was went; the window then reloads |
| `compact_base`   | –                            | `[before, after]` in bytes (VACUUM) |
| `show_backup_place` | `base`                  | – (the mail base, or the newest backup, in Explorer) |

A draft carries `reply_to`, `forward_of`, `replaces` and `autosaved` (message ids) besides the
fields the user typed (`to`, `cc`, `bcc`, `subject`): threading headers, forwarded attachments,
the draft or unsent message it was edited from, and its last autosave (a draft of its own; both
go once it is saved or sent); and `identity`, who it is from (`null` = the
account's default); and `html`, the editor's HTML when it is written in HTML (`null` = plain
text in `body`; its pictures are data: URIs, which go out as multipart/related parts with cid:). Its `attachments` are files from this PC (`path`, chosen through
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
bodies, thread, unread, keep}` (`folder: null` = all mail except Spam and Trash; `"*/drafts"`, `"*/outbox"`: that
kind of folder of every account in one list (`list::every_kind`); `sort`: date, size, flag, from,
subject; `filter`: the quick filter, `bodies` also searching the text; `thread`: a message id,
whose conversation the list shows instead of a folder; `unread`: only unread mail, besides the
ids in `keep`, which the window passes for mail read while the list is open; with `folder: null`
that is the tree's Unread, `UNREAD = "*/unread"` in app.js) and `at`: `"first"`,
`"last"`, `{after: id}`, `{before: id}` (scrolling, keyset on (sort key, id)), `{around: id}`
(the selection after a change) or `{row: n}` (a jump with the scroll bar). `start` is the
position of the first row and `total` the list's length. A filtered list is worked out once and
paged from memory until the mail base changes. After any change (a check, rules, sending, a
push) the UI asks for `folder_tree` and the open page again, nothing else.

Folder keys are `"<account>/<folder>"`, e.g. `firma/inbox`; the folders on this PC are
`local/<folder>` (an account without servers, listed last in `folder_tree`). Dates cross the bridge as local
`YYYY-MM-DDTHH:MM:SS` strings.

Settings: `theme` (one of `settings::THEMES`; "system" follows Windows' light/dark), `language`
("auto", "en", "pl"), `toolbar_style` (classic, icons, large, below, text), `threaded` (thread rows
in folder lists, off by default), `density`, `account_mark` (rail, dot, initials, tint, none: how accounts'
colors show in the tree, on the rows of every list (with two or more accounts; a dot or the
initials stand once above one account's folder), and the status bar; `data-mark` on `<html>`),
`unread_style` (quiet: read mail in `--muted`, bold: weight only, color: unread mail in
`--accent`; `data-unread` on `<html>`), `font` (plex, system, classic, inter, atkinson, source:
the window's, the list's and the mono face; `data-font` on `<html>`, sets of `--font-ui`,
`--font-list`, `--font-data` in styles.css),
`text_size` (90–200, webview zoom), `prefer_text`, `mark_read`
(moment, at_once, window, manual), `read_delay` (ms a message is on screen before it counts as
read with moment: 600, 1000, 2000, 3000, 5000, 10000), `check_every` (minutes, 0 = only on Get mail), `push_mail` (IMAP
IDLE on each Inbox), `notify` (a notification for new mail while the window isn't active),
`mail_paper` (theme, white: HTML mail in a dark theme), `risk_checks` (a line above opened mail
with scam warning signs; nothing when there are none), `spam_filter` (careful (the default),
normal, strict, off: how sure the spam filter must be to move new mail to Spam; off still
learns; `spam::set_level`, like the language), `compose_format` (html, plain; html by
default since 0.9.8, an existing settings.json keeps its value), `reply_format` (html (the
default), same, plain), `sign_replies` (the identity's signature in replies and forwards too), `image_size` (large (the default, 1600 px), medium (800 px), original: pictures put into a message), `undo_send` (0, 5, 10, 20, 30 seconds sent mail waits so Z
can take it back), `ask_subject`, `ask_attachment` (Send asks when the text mentions an attachment and none is attached), `ask_many` (Send asks before a reply to all goes to 8 people or more), `spell_check`, `spell_languages` (tags such as "pl-PL"; empty: Windows' languages, its region and the interface's, as far as there are dictionaries), `updates` (ask, start: at each start and once a day, daily, off), `usage_count` (ask, on, off: the daily count; asked in the status bar after `updates`; this PC's own like `backup_dir`), `backup_every` (off, daily,
weekly), `backup_dir` (empty: Documents\Telex backups; this PC's own, so exported settings
leave it out and imports keep it) and `backup_keep` (3, 5, 10, 20). Reading or changing them
also sets the core's language (`i18n::set`). They live in `%APPDATA%\app.telexmail.desktop\settings.json`, written whole or not at
all, even before the first account. The window keeps only its own state in localStorage
(`telex-ui`: pane sizes, the list's columns (`cols`: From and Folder widths, Size, short or full
dates), filter scope, "more" folders, folders kept in view (`pinned`), recent searches, the last
update check; `telex-look`: a
copy of theme, language, zoom, account mark, unread style and font for boot.js).

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
cargo fmt -p telex-core -p telex -p telex-security
node scripts/third-party-notices.mjs   # after changing dependencies or fonts (CI checks it)
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
  (`html::fetch_images`) and passed in as data: URIs; the webview never loads them. No
  objects, `<base>` or form submissions (`object-src`, `base-uri`, `form-action 'none'`).
- The windows' script may ask Tauri itself only for what `capabilities/default.json` lists:
  events, the title, closing (and destroy) and zoom. A new `window.__TAURI__` call needs its
  permission there; Telex's own commands and anything Rust does need none.
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
| Space / Shift+Space | page down the message; at its end, the next unread one (after the last, the next folder with unread mail: Space again goes there); Shift pages up, then the previous unread |
| Shift+↑ / ↓  | select more (or fewer) messages; Shift+Home / End to the top / bottom |
| Ctrl+click, Shift+click | add one / a range to the selection |
| Ctrl+A       | in the list: select every message |
| Ctrl+C       | with no text selected: the selected messages (or the open one) as Markdown, for notes or an AI chat (also the message menu and the copy button above a message) |
| R            | reply                          |
| Shift+R      | reply to all                   |
| Shift+F      | forward                        |
| N            | new message                    |
| E            | edit a draft or an unsent message in the Outbox |
| F            | flag / unflag (the selection: flag all, or unflag all when all are flagged) |
| U            | toggle unread (the selection: all read, or all unread when all are read) |
| click the dot / flag | at the start of a row: that message read or unread, flagged or not |
| Shift+U      | mark everything in the folder read |
| Del          | move to Trash (in Trash: press twice to delete for good, two presses of their own; holding Del counts once) |
| M            | move to a folder: type part of its name (also one on this PC) |
| drag         | messages onto a folder: it fills up, then takes them on release; Esc cancels, Z undoes |
| G            | go to a folder: type part of its name |
| T            | the selected message's conversation (T or Esc goes back) |
| → / ←        | with threads on (Settings › Reading): expand / collapse the selected thread |
| O            | only the unread mail of the list (also the dot above the list's dots, or "n unread" in the status bar); O again or Esc: all of it. In Unread (top of the tree) O goes back to the folder before |
| Z            | undo the last delete, move or Discard; within the moment after Send, take the message back (also the Undo button in the status bar) |
| /            | search (also `from:` `to:` `subject:` `is:unread` `is:flagged` `has:file` `after:` `before:` `larger:` `smaller:`) |
| ↓ ↑ Enter    | in the search box: choose a suggestion (a recent search, a person, an operator); Enter or ↓ with none chosen goes to the results |
| Tab          | in the quick filter: this folder / all mail |
| Enter, double-click | open the message in a window of its own (Esc closes it); in all-mail results and conversations Enter opens the message's folder |
| H            | switch HTML / plain text view (remembered) |
| Q            | open or fold the quoted mail of the open message |
| Esc          | clear filter, the selection, the conversation or only-unread; in compose: save draft and close |
| ↓ ↑ Enter Tab Esc | in To, Cc, Bcc: choose a suggested address |
| Ctrl+Enter   | send (compose)                 |
| Ctrl+,       | settings                       |
| Ctrl+B / I / U / K | bold / italic / underline / link (HTML compose) |
| F7           | in compose: the next spelling mistake and its suggestions (also a click on an underlined word, a right-click, or the menu key on one) |
| Ctrl+= / Ctrl+− / Ctrl+0 | larger / smaller / normal text (everywhere) |
| ? / F1       | keys and tips (also on hover over the ? in the status bar) |
| F2, Shift+F10 | in the folder tree: rename a folder on this PC; a folder's or account's menu (also right-click) |
| Shift+F10, menu key | in the list: the selected message's menu (also right-click a message; the column headings have one too) |
| Alt+↑ / ↓    | in the folder tree: move an account up or down |

## Not released yet

- Richer HTML compose (schema version 15: `identities.signature_html`, `new_template`,
  `reply_template`, `folder_templates`; one-way; setting `image_size`):
  - Pictures: pasted, dropped from Explorer where they should go, or Insert picture… (the Open
    dialog, pictures only), made smaller by the core (`images.rs`, image crate with PNG, JPEG, GIF,
    WebP only: Large 1600 px, Medium 800 px or as they are; a photo made smaller is turned upright
    and loses its EXIF, location included; GIFs stay as they are). In the editor they are data:
    URIs; `smtp::compose` and `compose_draft` make each a part of a multipart/related beside the
    HTML (`html::inline_images`, `cid:<n>.<Message-ID>`), and E and drafts turn them back
    (`html::editable`). Pasted remote pictures are left out, never fetched: fetching would tell
    their server who pasted, and sending their addresses would let it see who reads; the status
    bar says how many and to save them and Insert picture. Pictures shown in a message's text
    (cid: in its HTML) aren't listed as its files any more (`mime::files`), for received mail too.
  - `html::compose_clean` is a strict allowlist: the old tags plus tables, sub/sup, h4–h6 and
    img; `style` only `color` from `TEXT_COLORS`, `background-color` from `MARK_COLORS`,
    `font-size` 12px or 18px (the browser's small and x-large written so); img only as data:
    (PNG, JPEG, GIF, WebP in base64; 5 MB each, 15 MB and 40 in all) or cid:; the class
    `telex-signature` on a div; nothing else. Text color, highlight and size come from the bar
    (`execCommand` with styleWithCSS); the editor shows each palette color in its theme's own
    readable shade (`--pal-1..6`, `--mark-1..4` in themes.css, checked by check-contrast.mjs) by
    attribute selectors with !important, so what's sent stays as chosen.
  - HTML signatures: Settings › Writing and the identity form edit them with compose's editor
    (formatting, colors, a picture: 600 px, 100 KB each, 200 KB in all). `Identity::prepared`
    cleans them and keeps their text in `signature` for plain-text mail; one without formatting
    stays plain. In HTML compose the signature sits in `<div class="telex-signature">`, which
    another identity's replaces. Thunderbird's HTML signatures come over as HTML.
  - A reply or forward in HTML quotes the original's own HTML (`html::quotable`: sanitized, its
    own pictures within 5 MB, remote ones left out, styles down to the palette, at most 2 MB;
    else its text), in a blockquote that cfold folds as before; `start_draft` returns `html`.
  - Templates: per identity what new messages (`new_template`, a greeting) and replies
    (`reply_template`) open with, typing starting below them (`caret_line`); `{first}` (the
    first name) joins the placeholders, and a new message has only {date} and {time}. Per folder
    (`FolderTemplate`: new, reply, reply header) from Settings › Writing › Folder templates or a
    folder's menu › Templates…; each one that isn't empty wins over the identity's (the open
    folder's for a new message, the original's for a reply). Settings files carry identities'
    `signature_html` and templates, and folders' templates by account address and folder id,
    else name, like rules (`templates`; with the identities).

## Current state (v0.9.11: copy mail as Markdown, gentler on shared servers, a log of connections, and all earlier features below)

- New in 0.9.11 (no schema change; setting `usage_count`):
  - Gentler on shared servers (no schema change): 31 Purelymail accounts checked at once, each with
    its IDLE connection and retried after every failure, most likely got this PC's address ignored
    by Purelymail on 17 Oct 2026 (TLS went through, no greeting, for every account; a phone
    worked). Now connections to one server's IP wait for one of its places (`servers.rs`: 4 for checks, pushes and logins, 8 for IDLE; the
    accounts past the eighth get new mail from the regular checks), and a server that doesn't greet,
    or greets with BYE, rests before anything connects again; the error says until when
    (`SyncError::Resting`), and IDLE watchers sleep through it. Purelymail is on the provider list
    (MX `*.purelymail.com` → imap/smtp.purelymail.com); the wizard used to land on its MX host.
  - Settings › Connections (no schema change): every server reached since Telex started (its
    addresses' names, places in use, a rest with "Try now") and the last 1,000 connections, IMAP and
    SMTP, each step timed (address, waiting for a place, connecting, STARTTLS, TLS, greeting, login,
    the work), where one failed and why; filter by account and "Only problems", refreshed every 3 s
    while open, Copy details as text (`connections`, `wake_server`; `servers::LOG`, memory only).
    Opened from the status bar's Details beside "Check failed · Retry" and from a failed check's line
    in Settings › Accounts, filtered to that account.
  - Counting (setting `usage_count`): a release build asks once in the
    status bar, after the update question, whether it may be counted (Settings › Updates › Counting).
    With a yes it sends telexmail.app/api/count at most once a UTC day: version, system, language,
    first month, and first-ever / first-this-week / first-this-month flags; no ID (`count.rs`). The
    site keeps sums per day (Supabase, `site/supabase/migrations/20261017_telex_count.sql`) and a
    daily snapshot of GitHub's download totals; `/stats` shows both to the owner. The privacy page,
    the FAQ and "No telemetry" claims say so now ("No tracking"). The site needs the migration
    applied and STATS_PASSWORD and CRON_SECRET set in Vercel.
  - Copy as Markdown (`markdown.rs`, `message_markdown`; no schema change): the message menu, a copy button
    above a message and Ctrl+C (when no text is selected) put the selected messages, or the open
    one, on the clipboard as Markdown, oldest first: the subject as a heading, From, To, Cc, Bcc,
    Date and attachments as a list, then the text; mail written only in HTML gets its text from
    the HTML (`html::received_text`: links keep their addresses, table rows a line each). Up to
    200 at once; nothing is marked read. For notes or pasting into an AI chat.
  - Spam that goes to Trash (Del in Spam, or a move) is marked read on the way, in the same
    transaction, \Seen queued before the move (`spam_among`, `set_flags_in`).
  - Fixed: HTML turned into plain text (the text copy of HTML mail Telex sends) lost the spaces
    inside a run of text after one that ended in a space ("Hello <b>big world</b>").

- New in 0.9.10 (no schema change; settings `ask_attachment`, `ask_many`):
  - Replies: a Reply | Reply all switch as the reply's title (both answers come from `start_draft`;
    switching keeps the text and hand-typed addresses), a line under Cc naming who else was on the
    original with Add them, and "The original had 2 files · Attach them" (sets `forward_of`, so the
    core carries them like a forward's; threading stays on `reply_to`). Send asks, in a line above
    the buttons, when the text mentions an attachment and none is attached (`ask_attachment`) and
    when a reply to all goes to 8 people or more (`ask_many`); Send again goes ahead.
  - Quoted mail reads as blocks: plain text in the text's own color with a bar per depth and its
    introducing line on top (`quoteTree`), a long block folded to three lines with Show more; HTML
    mail's quotes (Gmail's and Thunderbird's classes and Apple's cite, now kept by the sanitizer,
    and everything from Outlook's `divRplyFwdMsg` on) fold behind one `<details>` line; Q opens
    or folds them all. A reply's quote is folded in the HTML editor until clicked (`cfold`, never
    saved).

- New in 0.9.9 (no schema change):
  - The Classic toolbar shows its words while they fit and only icons when they don't (`fitToolbar`
    measures; `.toolbar.tight`), instead of a scroll bar; other layouts still scroll, without a bar.
  - The flag above a message is a button like the envelope (`.pflag`, pressed in `--flag`), not a pill.
  - The status bar's left side clips with an ellipsis; the key hints give way below 1100 px.
  - Spelling, easier: a click on an underlined word shows its suggestions in a strip under it
    (`.spelltip`; the focus stays in the field), the lines move with the text as it is typed
    (`spellEdited`), the word being typed gets its line once the caret leaves it, a red `--spell`
    token per theme, the subject's line no longer cut off. Suggestions from every language are
    ranked by closeness without diacritics ("Wycene" → "Wycenę" before English guesses;
    `spell::distance`). Signatures and the away reply use Telex's checker; the webview's own
    (Windows' languages only) is off everywhere (`<body spellcheck="false">`).
  - New mail that lands in the open list brings it to its start (its end when sorted oldest
    first); the selection stays (`refresh({arrived})`).
  - Settings tidied: shorter notes, Appearance starts with Theme and Font, toolbar cards in one row,
    Usage as one line per account with the picked one's details below (`.uacct`), Backups' schedule,
    keep and folder side by side (`.sgrid`), account rows with their actions below, Writing's
    spelling and sending in groups of their own. The Usage and unread-card `.urow` clash is gone
    (Usage uses `.uline`).

- New in 0.9.8 (schema version 14: `spam_tokens`, `spam_trained`):
  - Telex's own spam filter (`spam.rs`; Settings › Mail › Spam filter; the message menu's Mark as
    spam / Not spam). A statistical filter learned from the user's mail on this PC: each message
    is cut into tokens (folded words of the subject and text, sender address and domain, link
    hosts, attachment types, a few header facts such as the first Authentication-Results and
    List-Unsubscribe; the user's own address is left out), each token scored with Robinson's
    estimate, the 150 strongest combined with Fisher's chi-squared method (as SpamBayes). It
    learns from mail in Spam folders (spam) and read mail in the Inbox, Sent, Archive and the
    user's folders (good), up to 400 a check and 5,000 of each by itself (`learn_pending`); any
    move into Spam or out of it to a good folder learns the message again, undoing what it was
    learned as (`move_many`, one transaction; Z learns back). In `sync_all` it runs on new Inbox
    mail before the rules and the away reply (`sort_arrivals`): above the level's cutoff (careful
    0.99, normal 0.95, strict 0.90) the message goes to the account's Spam with a note naming the
    score and the telling words, and no notification. It never moves mail from people the user
    wrote to, trusted senders or answers to the user's own mail, does nothing until it has 50 of
    each, and doesn't learn from its own moves (`spam_trained.class` -1) until the user says Not
    spam. Token counts stay when mail is removed; the row with the empty token holds the totals.
  - Links: pointing at or focusing a link shows its real destination in one line at the foot of the
    message pane (`.linktip`, like a browser's status bubble; never over the mail); right-click:
    open, copy the address. The 0.9.5 inspector card and the "n links" bar are gone.
  - Scam warnings show only when the score is above 0 (`risk_checks` turns them off).
  - HTML is the default for new messages and replies; `sign_replies`; Settings › Writing lists every
    identity's signature, edited in place and saved as typed (`save_identity`).
  - The message menu starts with Mark as read / unread.
  - One unreadable saved search no longer hides the others.
  - Speed: thread rows keep the reply graph until mail comes, goes or moves (reading and flagging
    don't rebuild it), and a rule's preview reads a list's first 2,000 messages (`PREVIEW_LIMIT`),
    saying when there were more. Numbers in docs/storage.md.
  - Recipients like `a@x.pl <a@x.pl>` or a name with @ in it send (`smtp::mailbox`: the name is
    dropped when it repeats the address, else quoted); replies don't write such names.

- New in 0.9.5 (schema 13: saved searches, richer rules and previews, optional threads, toolbar layouts, link inspection and local scam-risk checks):
  - Schema 13 stores saved list queries and duplicate protection for copy/forward rule actions.
    Rules match named headers and attachment names/types; previews count all matches without
    changing or sending mail. Copy preserves the original; automatic forwarding is arrival-only.
  - Settings › Reading offers expandable thread rows (off by default). Appearance offers five
    toolbar layouts. Link inspection and Rust scam-risk reasons stay local; scores are advisory.
  - Linux GLib 0.18.5 uses the verified upstream iterator backport in `vendor/glib`.
    `crates/telex-security` and `fuzz` exercise hostile mail in both native webviews, optimized
    GLib iterators, MIME/HTML and PST imports. The complete security workflow gates releases.

- New in 0.9.1 (no schema change):
  - Unread mail stands out (`unread_style`, Settings › Appearance › Unread mail, cards drawn as
    list rows): always bold with the dot; by default read mail goes `--muted` (quiet), or unread
    mail takes `--accent` (color), or weight only (bold). check-contrast.mjs checks accent text on
    hover and on each account's tinted row.
  - Account colors on every list: with two or more accounts, every row carries its account
    (`acc aN`), so rail and tint show in one account's folder too (tint washes the whole list
    pane, `.list.acc`); dot and initials stand once in the From heading (`#from-mark`). In lists
    that mix folders the Folder column names the account after the folder ("Inbox ·
    anna@studio.pl"; folders on this PC say "on this PC"), and takes 30% of the room by default.
  - Settings › Appearance › Message list columns: short or full dates, the Size column and Fit
    columns, the same `cols` as the headings' right-click menu.
  - A sixth font, `source`: Source Sans 3 and Source Code Pro, light. fonts.css registers the
    Light file as 400, Regular as 500 and keeps Semibold for 600 (mono: 300/400/500 as 400/500/600),
    so the window's weights draw it light without rules of their own (about 245 KB).
  - Spelling's languages on a button at the end of the compose bar ("EN · PL"); its menu ticks
    languages (`spell_languages`), goes back to automatic, turns spelling off, or opens Settings ›
    Writing. Every ticked language is checked at once, as before; Settings › Writing says so.

- New in 0.9.0 (schema version 12: `away`, `away_replies`):
  - The away reply (`away.rs`, `sieve.rs`; Settings › Accounts › Auto-reply, a status-bar chip
    while one is on): per account a text, an optional subject, first and last day, and how
    often one person may get it. Saving tries the server first: ManageSieve on the IMAP host's
    port 4190, STARTTLS before the login (never plain), PLAIN login with the account's
    password; with `vacation` (and `date`, `relational` for days) Telex writes its part
    between two marked lines into the running script, after its `require` lines, so the
    user's own filters stay; turning it off takes exactly that part out, and a script Telex
    made goes when left empty. Purelymail takes it here; OVH, home.pl and nazwa.pl don't answer
    on 4190, so for them Telex answers new mail when it checks (after the rules), with the rule
    auto-replies' safeguards (`filters::auto_reply`, `Replier::Away`). Removing an account
    takes its server reply off first.
  - Moving in from another mail program (`migrate/`, Settings › Import and export, and the
    add-account wizard when one is found): Thunderbird (Betterbird, SeaMonkey, Postbox) from
    profiles.ini and prefs.js: IMAP accounts with their servers, identities and signatures (an
    HTML signature as text), the outgoing server of the first identity; Local Folders and POP3
    accounts' mail (mbox or Maildir, ".sbd" subfolders) into folders on this PC of the same
    names ("Archives / 2019"; Archive and Trash by name); filters (msgFilterRules.dat) into rules
    through `settings::plan_rules`, the same matching a settings import does, after the
    accounts have logged in once to list their folders. Filters needing what rules can't do
    (dates, tags, priority, copying) are listed with Thunderbird's own word for it
    (`Skip::Unsupported`). Outlook: IMAP accounts from the registry (Windows), .pst files the
    profile opens or in the usual folders, read with Microsoft's outlook-pst (Unicode and ANSI;
    received mail keeps its transport headers, mail written in Outlook gets headers from its
    properties; HTML with its cid: pictures, attachments, read state and flag). .ost files are
    left out (the server has that mail). Maildir and MH folders (Evolution, KMail, Claws, mutt).
    Passwords are typed in the dialog; accounts log in six at a time; OAuth (Gmail,
    Outlook.com), POP3 and Exchange accounts are shown and why they can't come. Moving in twice
    brings nothing twice (Message-ID per folder, rules by name).
  - Spelling (`spell.rs`), Telex's own instead of the webview's: Windows' checker for the
    languages Windows has, Hunspell (spellbook) for the rest; on this PC Windows had only English,
    so Polish comes from LibreOffice's dictionary, fetched when the user clicks Download in
    Settings › Writing (pinned commit, SHA-256 checked; Polish loads in 67 ms, a check of a long
    message takes 5 ms, then under 1 ms from the cache). A word is right in any chosen language;
    the choice is automatic from Windows' languages and the interface's until the user ticks
    languages (`spell_languages`). Quotes and their introducing line, a forwarded message, links,
    addresses, words with digits, ACRONYMS and iPhone-like words aren't checked. In compose a
    layer over the subject and the plain text draws wavy lines (`--note-fg`); the HTML editor
    uses the CSS Custom Highlight API. Right-click, the menu key or F7: suggestions (inserted
    with insertText, so Ctrl+Z works), Add to dictionary (words.txt), Ignore. Once a session, the
    status bar says when the interface's language has no dictionary yet but one can be fetched.
  - Mail files: .msg (Outlook, was read as text before) and .emlx (Apple Mail) in Open and drops;
    Maildir and MH folders; messages Thunderbird deleted but hadn't compacted stay out; a file
    without a Date header gets the file's time. Reading mbox builds each message in one buffer
    and parses outside the store's lock (215 MB/s of Thunderbird mbox here; a 614 MB .pst of
    1,903 messages in 7 s).
  - Cleanup and hardening: every plain command runs on the async pool (`#[tauri::command(async)]`,
    never the window's thread); sending one Outbox at a time is a `tokio::sync::Mutex` (`sending`)
    instead of a retry loop; saving a draft composes outside the store's lock and replaces the
    old one in one transaction; a server folder that disappears keeps what only this PC had
    (moved to Archive on this PC with a note); flags from the server update only what changed;
    remote images as above; ManageSieve lines and literals have limits; deeply nested quotes and
    hostile CSS can't exhaust the stack; risky attachments include OneNote, Excel add-ins,
    search connectors and .mht; a mail base that won't open says so in a dialog instead of
    starting empty silently. The window keeps the last 30 messages and 8 HTML views, draws the
    open message again only when it changed, and folds suggestions once. The test-only sample
    mailbox is compiled only into the tests. One Telex runs at a time (a second start brings
    the window forward): two on one mail base would send an Outbox twice. The window's script
    may ask Tauri only for events, its title, closing and zoom; the CSP also forbids objects,
    `<base>` and form submissions. CI checks and tests the Windows-only code on Windows; the
    workflows' actions are pinned to commits, and the release key reaches only the steps that
    sign, as a file.

- New in 0.8.1 (no schema change):
  - Unread at the top of the tree (`UNREAD = "*/unread"` in app.js; the core gets `folder: null`
    with `unread: true`): the unread mail of every folder but Spam and Trash, with its count; G
    offers it. A list worked out once (`found_page`), like a filtered one.
  - Only unread in any folder: O, the dot above the rows' dots (`#b-unread`), or "n unread" in
    the status bar (which then says "only unread: n of total · Show all"); O again, Esc or another
    folder shows everything. In Unread, O goes back to the folder open before.
  - Mail read while an unread list is open stays in it (`state.kept`, sent as `ListView.keep`)
    until the list is left; Shift+U empties it. Space skips what is read.
  - Rows of lists that mix accounts (Unread, all mail, conversations, every account's drafts and
    Outbox; `.allmail` with `.multi`) carry their account's mark as `account_mark` says: a
    line at the row's start (rail), a 9% tint (tint), or the dot or initials by the folder's name.
    The Appearance cards draw list rows too. check-contrast.mjs checks text, the unread dot and the
    flag on each account's tinted row (MIXED pairs can be 'ui' now).
  - Fonts (`font`, Settings › Appearance, cards written in their own set): IBM Plex; System
    (Segoe UI Variable, Cascadia Mono); Classic (Tahoma, Consolas); Inter with JetBrains Mono;
    Atkinson Hyperlegible Next and Mono. System and Classic ship nothing (Linux gets stand-ins);
    the others are about 330 KB of woff2 in ui/fonts, loaded only when chosen. The list's column
    widths are measured again in the new face.

- New in 0.8.0 (schema version 11: `folders.chosen`):
  - Which folder is Sent (`imap::plan_folders` with `FolderHints` from `MailStore::folder_hints`):
    each special kind goes to the folder the user chose (`folders.chosen`, Settings › Accounts ›
    Folders or "Use for sent mail" on a folder), else of those the server flags, whose name says
    it or that had it before, one that holds mail before an empty one, then the one that had it,
    then the flag, then the name. So a server's own empty "Sent" loses to the "Wysłane" another
    program filled, and Sent doesn't hop back once both hold mail. A sync plans again after its
    download, so a first check settles it. Server folders keep the server's own name in `name`
    (the UI names special folders by kind; the tree adds "· Wysłane" when another folder has the
    kind's name, and the server name is in the tooltip). Drafts written here follow Drafts to
    another folder (`move_local_drafts`). `sync-progress` carries the folder's `key`.
  - Drafts and the Outbox aren't in the accounts' folder lists: what isn't sent yet waits at the
    end of the tree as one list each for every account (`"*/drafts"`, `"*/outbox"`, EVERY in
    app.js, with an Account column), only while it holds something; Outbox mail that didn't go
    shows as a red count there and a status-bar chip (`stuck`). G offers them the same way.
  - Sent is quieter than the Inbox in the tree. The folders on this PC show Archive and Trash only
    when they hold mail.
  - Account colors (`account_mark`, Settings › Appearance, cards drawn with the real marks): a line
    down each account's group (`.agrp`, the default), a dot, initials (from the name, else the
    address; never two alike), a tint, or nothing; the same mark in the list, status bar, pickers.
    Each `.a0`–`.a5` sets `--ac`. check-contrast.mjs checks text on the mixed backgrounds too.
  - Slim scrollbars in each theme's colors (`--field`), on the site as well.

- New in 0.7.2 (no schema change):
  - The message window no longer freezes Telex: `open_window` is async (a plain command runs
    on the main thread inside WebView2's handler, and building a webview there waited forever).
  - Checks are about eight times faster: `check_mail` (and `push_changes`) talk to every account
    at once (`join_all`; Outboxes still go one at a time, as only one sender may run), and IMAP
    sockets have Nagle off (`set_nodelay`): async-imap writes each command in four pieces, so
    every command waited for the server's delayed ACK. Four Purelymail accounts: 19 s → 2.3 s.
  - Drafts are safe: compose autosaves every 15 s when something changed, as a draft of its own
    (`autosaved`), so the draft being edited stays as it was until Esc, Save or Send, which
    remove both; New, Reply or E while writing save the message to Drafts first; Discard drops
    the autosave (`discard_draft`) and Z (or the Undo button) brings the message back. Saves go
    one at a time in the window (`inTurn`), so each knows the autosave it replaces.
  - Del in Trash takes two presses of their own: repeats are ignored, and a second press within
    0.4 s doesn't count (a double-click on Delete either).
  - An Undo button in the status bar while Z can undo something: mail waiting to go counts down
    its moment; a move, delete or discard shows it for 10 s. Accounts whose last check failed
    (`folder_tree.problems`, from `accounts.sync_error`) get a "!" in the tree and a status-bar
    chip with the reasons in its tooltip; clicking it checks again.

- New in 0.7.1 (no schema change):
  - The list's columns fit its width (`fitColumns` in app.js): the status icons, the date and
    the size are as wide as the widest text they can show in this language and font (measured
    once the faces have loaded), so a date is never cut; From takes a share of the rest (30%,
    120–280 px) and Subject what is left (at least 140 px). Dragging the edge before Subject (or
    before Folder in all mail) sets a width, kept in `telex-ui` and given way when the window
    is narrow; a double-click fits the names on screen. Size goes when the list is under 640 px.
    Right-click the headings: short or full dates, the Size column, fit columns again.
  - Read and unread by hand: the dot at the start of a row and the flag next to it are switches
    (a ring and a quiet flag under the pointer); the envelope above a message; the right-click
    menu. Settings › Reading (moved from Mail) says when mail counts as read: at once, after
    `read_delay` on screen, only in its own window (`window`), or only by hand.
  - A message's menu on right-click or Shift+F10 / the menu key: open in its window, reply,
    forward, read, flag, move, Trash, conversation, all mail from the sender, a rule from it; for
    several messages, what works on all of them. A right-click selects without reading.
  - Search: the box is called Search and widens with the focus; under it, recent searches (an
    empty box, kept in `telex-ui`), people from the mail base for the word being typed (from:,
    to: in Sent), and the operators that word could start. ↓ ↑ Enter choose; Enter or ↓ with
    nothing chosen goes to the results (and keeps the search). What matched is marked in From
    and Subject (diacritics and case ignored).
  - Account order: right-click an account › Move up / Move down, Alt+↑ ↓ on it, or ↑ ↓ in
    Settings › Accounts (`move_account`, `accounts.position`); account colors follow the
    order. Right-click a folder › Keep in view: it shows outside "more".
  - Updates: "when Telex starts, and once a day while it is open" (`updates: start`) is the Yes
    to the status bar's question; Settings › Updates has it, once a day, or only by hand.
  - Space reads on: it pages down the message, and at its end selects the next unread one in
    the list's order, wherever its page is (`MailStore::next_unread`: one query for it and one
    count for its row). After the last one it names the next folder with unread mail (not Sent,
    Drafts, the Outbox, Spam or Trash), and Space again opens it on its first unread.
    Shift+Space pages up, then goes to the previous unread. Space on a button presses it.
  - The status bar: the folder and messages on the left, the main keys in the middle, updates,
    storage, the last check and the version on the right; the two sides share what the keys
    leave. A message too long for the left takes the keys' place until it goes. U, Del, M and
    T show from 1360 px.
  - Scrolling keeps the rows drawn last while they still cover the screen with five to spare, so
    the table is rewritten far less often.

- New in 0.6.0 (schema version 10: the folders on this PC, created with
  the first server account, and `pending_ops.origin`):
  - Folders on this PC (`store.rs`, `LOCAL_ID`): a group of their own under the accounts, kept
    as an account without servers with the id "local" (never removed with an account), with
    Archive and Trash; the user's own folders are made, renamed (F2) and removed (when empty)
    in the tree, from its + or a right-click menu. Server mail moved there leaves the server: an
    expunge that remembers the local message (`origin`), taken back when the message returns
    to that account before the server heard of it. Mail moved from there to a server folder is
    uploaded (`append` with its flags in `value`). A message moved on while a move or upload
    was under way leaves the server from where it landed (`change_moved`, `change_appended`);
    an expunge without a UID finds the message by Message-ID first. Between two server accounts
    mail goes by way of this PC. Rules can file mail there from any account; replies to it come
    from the account it was sent to (`drafts::sender`); conversations include it; settings
    files carry rules' moves there (`this_pc`).
  - mbox and .eml (`mailbox.rs`): any folder saves as mbox (read on a connection of its own);
    .eml and mbox files come into a folder on this PC from Open or dropped on it in the tree,
    200 at a time, skipping Message-IDs already there.
  - Backups (`backup.rs`, Settings › Backups): the mail base by hand or every day or week into a
    folder (Documents\Telex backups until one is picked), the newest 3–20 kept, only
    `telex-backup-<time>.db` files ever removed. Restore: the file is checked, the mail base
    as it was is kept as `telex-before-restore-<time>.db`, and `after_restore` drops queued
    changes, puts unpushed moves back and moves Outboxes to Drafts. Compact (VACUUM). The
    folders on this PC say "No backup" while their mail has none (or none for a month with
    backups off).
  - Drag and drop of messages onto folders (6 px to start, a folder takes them after resting on
    it for 0.35 s, Esc cancels, Z undoes; a closed account or "more" opens after 0.65 s).
  - A message in a window of its own (double-click, Enter in a folder, the button above it):
    the same page in viewer mode (`?message=<id>`); changes there reach the main window
    ("viewer-did" events), which refreshes, pushes and keeps Z; closing a window with a message
    being written saves a draft.
  - The ? in the status bar (and ? or F1): every key and the tips that aren't obvious. The
    message header in one face. The status bar's storage bar in the account's color with the
    room left.

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
  Writing, Usage, Connections, Backups, Updates, Import and export; toolbar button and Ctrl+,): every change is saved
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
  one account, each has a color (`--acct-0..5` tokens; shown as `account_mark` says), also in the
  status bar and all-mail results. Each account shows Inbox and Sent and the open folder (Drafts
  and the Outbox wait at the end of the tree for every account at once); the rest are behind
  "N more" (with their unread count), opened per account and
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
  public IPs only, checked at connect (`PublicOnly`, ureq's resolver, so a name can't answer
  one address to the check and another to the connection), up to 40 images, four at a time,
  3 MB each, 15 MB total, 10 s each and 20 s in all, no cookies or Referer,
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
  asks in the status bar "Check for updates when Telex starts?"; the answer is a setting (`updates`:
start checks a few seconds after each start, unless a check answered in the last ten minutes, and
then daily like `daily`),
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
   HTML compose is done (see Current state), with pictures, colors, HTML signatures and the
   original's HTML in quotes (Not released yet).
4. ~~**Filters ("Sorting Office").**~~ Done; see Current state, with a rule started from a
   message in 0.5.0. Headers and attachment conditions, copy/forward actions, rule previews and
   saved searches in 0.9.5. Not yet: regular expressions and rules that run on a schedule.
5. **Gmail and Outlook.com.** OAuth2 login (both reject plain passwords over IMAP). Waiting
   for the owner to register OAuth clients with Google (Cloud project; Gmail's scope is
   restricted, so unverified apps stay in testing mode) and Microsoft (Entra app that allows
   personal accounts).
6. ~~**Templates and identities.**~~ Done; see Current state, with templates for new messages
   and replies, HTML signatures and per-folder templates (Not released yet).
7. **Shipping.** ~~Auto-update~~ (see Current state) and the website (`site/`) are done. Not
   yet: code signing (SmartScreen warns about the installer), macOS. Linux ships as an
   AppImage (self-updating) and a .deb. The repository is public and v0.9.0 is released;
   docs/releasing.md has the process.
8. **Storage** (plan in docs/storage.md). ~~Phase 1, fast at any size~~ and ~~downloading older
   messages~~: done, see Current state and the numbers in docs/storage.md. ~~Backup and
   restore~~ and ~~local folders with mbox export and .eml/mbox import~~: done in 0.6. Not
   yet: moving the mail base to another folder, and POP3.

## License

Decided 2026-10-16: freeware, all rights reserved, copyright Łukasz Szramuk. `LICENSE` (the code is
public to read, no license to reuse it), `EULA.txt` (the app is free for personal and business
use; shown by both Windows installers through `bundle.licenseFile` and installed next to the app
with `THIRD-PARTY-NOTICES.txt` through `bundle.resources`), `CONTRIBUTING.md` (no code
contributions; take one only with a CLA, so the copyright stays one person's). Not open source:
never add an open-source license or call Telex open source. `scripts/third-party-notices.mjs`
writes THIRD-PARTY-NOTICES.txt and the site's copies (`site/public/third-party-notices.txt`,
`eula.txt` for the /license page) from Cargo.lock and ui/fonts: run it after any dependency or
font change; CI fails with `--check` otherwise. The update dialog and Settings › Updates link to
the site's License page and notices.

## Open decisions

- Final product name and domain. "Telex" is the working name; the repo is `telexmail`.
- App icon: the dot-matrix T in `brand/`. `src-tauri/icons/` is generated from it with
  `npm run icons` in `site/` (not `tauri icon`, which can't use the small-size variant).

## Prior art worth reading

- Skim (github.com/nikserg/skim): MIT, Rust + Tauri 2 + WebView2 + SQLite FTS5, Windows.
  Useful reference for IMAP sync, offline queue and HTML sanitizing. Minimalist, no filters.
- The Bat! (Ritlabs): the feature model to aim for.
