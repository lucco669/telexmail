# Telex

A compact, keyboard-driven email client for Windows, in the spirit of The Bat!.
Built with Rust and Tauri: small installer, fast start, no Electron, no account.

![Telex main window](docs/screenshot.png)

> **Status: early prototype.** Telex syncs real mail over IMAP (SSL/TLS, password login) both
> ways: a local, searchable mail base, with reading, flagging, moving and deleting mirrored to
> the server. It sends over SMTP (SSL/TLS or STARTTLS) through an Outbox that retries, and
> keeps a copy in Sent on the server. HTML mail shows in a sandbox where no script can run,
> with remote images blocked until you allow them. Rules sort, flag and answer new mail and
> leave a note saying why. Each account can have several identities, each with its own
> signature and reply templates. Release builds update themselves from GitHub Releases, but
> only check after you say yes (or click the version number). Until you add an account it
> shows a sample mailbox.

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
run's **Artifacts** section (`telex-windows`). These builds, like your own, can't update themselves.

## Releases and updates

Pushing a tag such as `v0.2.0` runs the **Release** workflow: it builds installers signed for the
updater and a `latest.json`, and puts them in a draft GitHub release. Publishing the draft makes
the update visible to every Telex that checks. Set the version first with
`npm run set-version -- 0.2.0`. The whole process, the one-time key setup and exactly what an
update check sends are in [docs/releasing.md](docs/releasing.md).

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
