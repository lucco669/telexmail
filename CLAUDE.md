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
crates/telex-core/   All mail logic. Models, MailStore, sample mailbox. Unit-tested. No UI, no Tauri.
src-tauri/           Thin Tauri shell: exposes MailStore methods as commands. Keep logic out of here.
ui/                  The interface. index.html + styles.css + app.js, no build step, no framework.
  fonts/             IBM Plex (OFL) bundled so the app works offline.
.github/workflows/   CI: core tests on Linux, Windows installer build.
```

Commands the UI calls (`src-tauri/src/lib.rs`), all backed by `telex_core::MailStore`:

| Command          | Args                         | Returns                     |
|------------------|------------------------------|-----------------------------|
| `snapshot`       | –                            | accounts + all summaries    |
| `get_message`    | `id`                         | full message with body      |
| `set_unread`     | `id`, `unread`               | –                           |
| `set_flagged`    | `id`, `flagged`              | –                           |
| `move_message`   | `id`, `folder`               | – (same account only)       |
| `delete_message` | `id`                         | `moved_to_trash` / `removed`|
| `save_outgoing`  | `draft`, `send`              | new message id              |
| `check_mail`     | –                            | ids of new messages         |

Folder keys are `"<account>/<folder>"`, e.g. `firma/inbox`. Dates cross the bridge as local
`YYYY-MM-DDTHH:MM:SS` strings.

## Commands

```
npm install                  # once: installs the Tauri CLI
npm run dev                  # run the app with live reload of ui/
npm run build                # release build + installers in target/release/bundle/
cargo test -p telex-core     # core tests (run these after every core change)
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
| Del          | move to Trash (from Trash: remove) |
| Z            | undo last delete               |
| /            | quick filter                   |
| Esc          | clear filter; in compose: save draft and close |
| Ctrl+Enter   | send (compose)                 |

## Current state (v0.1.0)

- Working window with folder tree, sortable message list, preview, compose, quick filter,
  resizable panes, density toggle, light/dark theme.
- Data is a built-in sample mailbox (`crates/telex-core/src/sample.json`), two accounts.
- "Get mail" delivers one sample message. "Send" only files the message in Sent.
- Nothing is persisted between runs except UI layout (localStorage).

## Roadmap

1. **Real mail, read-only.** One IMAP account with password login (own domain first).
   `async-imap` + `rustls`, `mail-parser` for MIME and charsets, credentials in Windows
   Credential Manager (`keyring` crate). Store in SQLite (`rusqlite`) with FTS5 search.
   Keep the `MailStore` API so the UI barely changes. Add an account setup dialog.
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
