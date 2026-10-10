# Telex

A compact, keyboard-driven email client for Windows, in the spirit of The Bat!.
Built with Rust and Tauri: small installer, fast start, no Electron, no account.

![Telex main window](docs/screenshot.png)

> **Status: early prototype.** Telex reads real mail over IMAP (SSL/TLS, password login) into a
> local, searchable mail base, but it doesn't change anything on the server yet and can't send:
> "Send" only files the message in Sent. Until you add an account it shows a sample mailbox.

## Run it on Windows

You need:

1. **Rust**: install from [rustup.rs](https://rustup.rs) (default MSVC toolchain).
2. **Visual Studio Build Tools** with the "Desktop development with C++" workload.
3. **Node.js** 20 or newer.
4. **WebView2**: already on Windows 11.

Then, in the project folder:

```
npm install
npm run dev
```

The first build compiles all Rust dependencies and takes a few minutes; later starts take seconds.

To build installers (`.exe` and `.msi` in `target\release\bundle\`):

```
npm run build
```

Every push to `main` also builds the Windows installers on GitHub Actions. Download them from the
run's **Artifacts** section (`telex-windows`).

## Keyboard

| Key | Action | Key | Action |
|---|---|---|---|
| ↑ ↓ | move | R / Shift+R | reply / reply all |
| Home / End | first / last | Shift+F | forward |
| F | flag | N | new message |
| U | toggle unread | Del / Z | delete / undo |
| / | quick filter | Ctrl+Enter | send |

## Project layout

- `crates/telex-core`: the mail engine in Rust, with tests (`cargo test -p telex-core`).
- `src-tauri`: the desktop app shell that connects the engine to the window.
- `ui`: the interface in plain HTML, CSS and JavaScript.

See [CLAUDE.md](CLAUDE.md) for design principles, architecture and the roadmap.
