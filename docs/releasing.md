# Releasing Telex

How a new version gets from `main` to the **Update** button in everyone's Telex. The short form:

```
npm run set-version -- 0.2.0
git add package.json package-lock.json src-tauri/tauri.conf.json Cargo.toml Cargo.lock
git commit -m "Release 0.2.0"
git push
git tag -a v0.2.0 -m "What's new in 0.2.0"
git push origin v0.2.0
```

Then wait for the **Release** workflow, check the draft release on GitHub and publish it.

## How updates work

- **Only release builds update.** The release workflow compiles the public half of the signing
  key into the app (`TELEX_UPDATER_PUBKEY`, read with `option_env!` in
  `src-tauri/src/updates.rs`). Builds without it (`npm run dev`, a local `npm run build`, the
  installers from the CI workflow) leave the updater out, and their version button says
  "This build can't update itself." On Linux only the AppImage updates itself; Telex installed
  from the `.deb` says the same, since that copy belongs to the package manager. Its users
  install the new `.deb` by hand, or switch to the AppImage.
- **Telex asks before it checks.** Until answered, the status bar asks "Check for updates when
  Telex starts?". Yes: a check a few seconds after each start (unless one answered in the last
  ten minutes), then at most every 24 hours while it stays open. No: never on its own.
  Settings › Updates also offers once a day only. Either way, clicking the version number in the
  status bar checks right away and opens the update dialog, which also has the checkbox. The
  answer is a setting (`updates`: start, daily, off); when the last check was is kept in the
  window's local storage.
- **A check** is one GET of
  `https://github.com/lucco669/telexmail/releases/latest/download/latest.json` with
  `User-Agent: Telex` and `Accept: application/json`, nothing else. GitHub redirects it to its
  file servers. `releases/latest` is the newest *published*, non-prerelease release: drafts and
  prereleases are invisible to the updater. The site's privacy page says the same; keep the two
  in step.
- **latest.json** is written by tauri-action: the version, the notes (the tag's message), and for
  each kind of installer (`windows-x86_64-nsis` for the `.exe`, `windows-x86_64-msi`,
  `linux-x86_64-appimage`, `linux-x86_64-deb`) its download address and signature, plus
  `windows-x86_64` (the `.exe`) and `linux-x86_64` (the AppImage) for the rest. Telex updates
  with the same kind of installer it was installed with. The Windows job writes the file first;
  the Linux job reads it back, adds its platforms and uploads it again.
- **Installing** happens only when the user clicks **Install and restart**. Telex downloads the
  installer (from `api.github.com/repos/…/releases/assets/<id>` with
  `Accept: application/octet-stream`, again redirected to GitHub's file servers), checks its
  minisign signature against the compiled-in key, checks that the signature names the version
  latest.json announced (so an old signed installer can't pose as a new one), then runs it in
  passive mode: a progress bar, no questions. The `.exe` installs per user and needs no
  administrator; the `.msi` installs per machine and Windows asks for permission. The installer
  closes Telex and starts it again. On Linux the new AppImage is written over the old file (the
  folder it is in must be writable, e.g. `~/Applications`) and Telex restarts from it. A message
  being written is saved to Drafts first, and the install waits until no mail check or send is
  running.

## One-time setup

1. **Make the repository public.** The updater and the website download release files without
   logging in; a private repository's release files answer 404. (To keep the code private, the
   releases would have to move to a separate public repository, and `ENDPOINT` in
   `src-tauri/src/updates.rs` and `releasesRepo` in `site/lib/site.ts` with them.)
2. **Make the signing key pair** on your own PC. In PowerShell, in the project folder (the
   command comes with the project's Tauri CLI; the keys go outside the repository):

   ```
   npx tauri signer generate -w "$HOME\.tauri\telex.key"
   ```

   It asks for a password; pick a long one. This writes `telex.key` (private) and `telex.key.pub` (public).
   Keep the private key and its password in a password manager and a backup: **if either is
   lost, no installed Telex can ever update again**. Everyone would have to download a new
   installer by hand once.
3. **Give them to GitHub**, under the repository's Settings > Secrets and variables > Actions
   (repository level, not an Environment). Copy each file's contents, e.g. from Notepad
   (Ctrl+A, Ctrl+C). Line breaks that copying adds around the key are removed by the workflow
   before the key is used.
   - Secrets tab, New repository secret: `TAURI_SIGNING_PRIVATE_KEY` = the whole contents of
     `telex.key`; `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` = the password.
   - Variables tab, New repository variable: `TELEX_UPDATER_PUBKEY` = the whole contents of
     `telex.key.pub` (one line). It is public by nature; it's kept out of the code so the
     repository can't produce updatable builds without the private key. A secret with this
     name works too.
4. **Let workflows create releases.** The workflow asks for `contents: write` itself. If the run
   fails with "Resource not accessible by integration", set Settings > Actions > General >
   Workflow permissions to "Read and write permissions".

## Releasing a version

1. On an up-to-date `main` with a green CI run, set the version:

   ```
   npm run set-version -- 0.2.0
   ```

   It writes the version into `package.json`, `package-lock.json`, `src-tauri/tauri.conf.json`
   (the version the app reports and the updater compares), the workspace `Cargo.toml` and
   `Cargo.lock`. Versions are MAJOR.MINOR.PATCH only (the MSI installer allows no text
   pre-releases, at most 255.255.65535), and only upwards: the updater never installs an older
   version. Use MINOR for new features (a roadmap step), PATCH for fixes.
2. Commit those five files ("Release 0.2.0") and push.
3. Tag that commit with the release notes and push the tag:

   ```
   git tag -a v0.2.0 -m "Automatic updates" -m "Telex can now check GitHub for new versions and install them. ..."
   git push origin v0.2.0
   ```

   The first `-m` is the first line, each further `-m` a paragraph (or `-F notes.txt` for a
   file). The message is shown in Telex's update dialog and as the release text on GitHub.
   Editing the release text on GitHub later doesn't change what Telex shows: that is fixed in
   latest.json when it is built.
4. The **Release** workflow (Actions tab) checks that the tag matches the version in every file,
   makes a **draft** release "Telex 0.2.0", checks that the private key, its password and the
   public key belong together, then builds, signs, and uploads into that draft:
   `Telex_0.2.0_x64-setup.exe` and `.exe.sig`, `Telex_0.2.0_x64_en-US.msi` and `.msi.sig`,
   `Telex_0.2.0_amd64.AppImage` and `.AppImage.sig`, `Telex_0.2.0_amd64.deb` and `.deb.sig`,
   and `latest.json`. Windows builds first, then Linux on Ubuntu 22.04 (one at a time, so
   both get into latest.json). It takes about 20–30 minutes.
5. Wait until the whole run is green, Linux too: the draft shows the Windows files about ten
   minutes before the Linux ones. Then check it: the files are there, latest.json lists both
   `windows-x86_64` and `linux-x86_64`, and the installers work on a clean PC if you like
   (download them from the draft; for the AppImage see "Testing the AppImage" below).
6. **Publish** the draft. From then on, `releases/latest/download/latest.json` is this
   release's: every Telex that checks sees the update, and the site's download page lists it
   within an hour.

To test an update end to end, keep the previous version installed, publish the new one, and
click the version number in the old Telex. The very first time, release twice (0.2.0, then a
small 0.2.1) and watch 0.2.0 update itself.

## When something goes wrong

- **"The version should be … everywhere"**: the tag and the files disagree. Delete the tag
  (`git tag -d v0.2.0` and `git push origin :refs/tags/v0.2.0`), fix the version, commit, tag
  again.
- **The build failed**: fix it on `main`, delete the draft release and the tag, and tag the
  fixed commit. A failure that wasn't the code's fault: Re-run jobs on the run's page.
- **Only the Linux build failed**: the draft already has the Windows files and a latest.json
  with only Windows in it. "Re-run failed jobs" adds Linux to the same draft, or to the
  release if it was published meanwhile (both builds upload to the release by its id, made
  by the run's first job). Published without Linux, the AppImages out there say the newest
  release has no installer for them until Linux is added.
- **A published release has no Linux files** but a second draft with the same tag has them:
  0.8.0 and 0.8.1, published while their Linux build was still running, from before the
  workflow made the draft first (tauri-action then looked for the release by its tag, found
  no draft and made one). Actions › **Repair release** › Run workflow, with the tag or empty
  for every such release: it copies the draft's files onto the published release, adds their
  platforms to its latest.json and removes the draft (`scripts/repair-release.mjs`;
  "Only say what would change" for a dry run).
- **"Check the signing key pair" failed**: the secret, its password and the variable must
  come from the same `tauri signer generate`. The step (`scripts/check-signing-key.mjs`) signs a
  scratch file and verifies it the way Telex does, because the bundler itself only warns about a
  mismatched key and would build a release that every installed Telex refuses. You can run it
  locally with the three environment variables set.
- **A published release is bad**: publish a fixed, higher version. Turning the bad one back
  into a draft (or deleting it) makes `latest` point to the previous release again, which stops
  more people from getting it, but those who updated keep it until a higher version arrives.
- **The private key leaked** (with its password): whoever has it can sign an installer that Telex
  accepts, though they would still have to get it into this repository's releases. Make a new
  pair and replace the secrets and the variable. Copies installed before then trust only the
  old key, so they can't update to releases signed with the new one: their users download the
  new installer once from the site.

## A release build on your own PC

Not needed for releasing, but useful for testing the signed build. In PowerShell:

```
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content -Raw "$HOME\.tauri\telex.key"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "the password"
$env:TELEX_UPDATER_PUBKEY = (Get-Content -Raw "$HOME\.tauri\telex.key.pub").Trim()
node -e "require('fs').writeFileSync('src-tauri/tauri.updater.conf.json', JSON.stringify({plugins: {updater: {pubkey: process.env.TELEX_UPDATER_PUBKEY}}}))"
npm run build -- --config src-tauri/tauri.release.conf.json --config src-tauri/tauri.updater.conf.json
```

`src-tauri/tauri.release.conf.json` turns on the updater files (`createUpdaterArtifacts`), and
`tauri.updater.conf.json` (ignored by git) gives the bundler the public key it requires for
them. The bundler only warns if the private key doesn't match it; run
`node scripts/check-signing-key.mjs` to check that properly. The installers and their `.sig`
files land in `target\release\bundle\`.

On Linux (Ubuntu 22.04 or newer, or WSL) the same in bash, after installing what the release
workflow installs (`sudo apt install libwebkit2gtk-4.1-dev libsoup-3.0-dev
libayatana-appindicator3-dev librsvg2-dev libxdo-dev libdbus-1-dev patchelf`, plus
`build-essential`, Rust and Node.js). `src-tauri/tauri.linux.conf.json`, which Tauri adds by
itself on Linux, makes it build the AppImage and the `.deb` instead of the Windows installers:

```
export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/telex.key)"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD='the password'
export TELEX_UPDATER_PUBKEY="$(cat ~/.tauri/telex.key.pub)"
node -e "require('fs').writeFileSync('src-tauri/tauri.updater.conf.json', JSON.stringify({plugins: {updater: {pubkey: process.env.TELEX_UPDATER_PUBKEY.trim()}}}))"
npm run build -- --config src-tauri/tauri.release.conf.json --config src-tauri/tauri.updater.conf.json
```

They land in `target/release/bundle/appimage/` and `target/release/bundle/deb/`.

## Testing the AppImage

On a Linux PC with a desktop (or Windows 11's WSL, which shows Linux windows), download the
AppImage from the draft into a folder you can write to and start it:

```
chmod +x Telex_0.2.0_amd64.AppImage
./Telex_0.2.0_amd64.AppImage
```

- **Passwords** go to the desktop's keyring through the Secret Service (GNOME Keyring, KWallet,
  KeePassXC). Without one running, adding an account says so. WSL has none by default; after
  `sudo apt install gnome-keyring`, this starts Telex with a keyring unlocked by "pass":
  `dbus-run-session -- bash -c 'echo -n pass | gnome-keyring-daemon --unlock
  --components=secrets; ./Telex_0.2.0_amd64.AppImage'`.
- **Updating**: keep the previous version's AppImage, publish the new release and click the
  version number in the old Telex. It downloads the new AppImage, checks its signature, writes
  it over the old file and restarts. The `.deb` (`sudo apt install ./Telex_0.2.0_amd64.deb`)
  says it can't update itself; that is intended.
- **Where things are**: the mail base and settings in `~/.local/share/app.telexmail.desktop/`.
