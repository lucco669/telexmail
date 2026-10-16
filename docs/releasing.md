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

## Windows installer appearance

The recommended `.exe` installer and its passive updater share the Telex icon, light
palette, Segoe UI text, and artwork. `src-tauri/installer/branding.nsh` adds this through
Tauri's NSIS hooks, with short English and Polish welcome/finish text and a dedicated
update heading. Windows selects the installer language automatically. Tauri still owns
the installation, maintenance, WebView2 setup, shortcuts and restart handling.

The `.msi` uses matching banner and dialog artwork with Windows Installer's standard
controls. The app's update dialog uses the same mark and its current theme, with download
progress; after signature verification the native installer takes over.

Regenerate all icons and installer bitmaps after changing the mark:

```
npm ci --prefix site
npm run icons --prefix site
```

The generator uses `brand/telex-mark.svg` and `brand/telex-mark-small.svg`, and writes
24-bit BMPs at three times NSIS's logical dimensions (for crisp scaled displays) and
WiX's required sizes. Sidebar text stays in native controls rather than bitmap artwork.
Commit the generated files, so ordinary app
builds and release CI don't need the site's dependencies. When changing installer colors,
keep the generator and `branding.nsh` in step with the light palette in `ui/themes.css`.

Validate with `npm run build` on Windows. Preview the welcome and directory screens without
installing over an existing copy; test `/P /UPDATE /R` in an isolated installation for
the progress screen and restart. A local build without the release key cannot download
and install a signed update from GitHub.

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
4. The **Release** workflow (Actions tab) first waits for the reusable security workflow:
   dependency and secret scans, the verified GLib backport and optimized Linux regression,
   hostile-mail tests in Windows WebView2 and Linux WebKitGTK, and bounded MIME/HTML/PST fuzzing.
   Any failure stops the draft and signing jobs. It then checks that the tag matches the version in every file,
   makes a **draft** release "Telex 0.2.0", checks that the private key, its password and the
   public key belong together, then builds, signs, and uploads into that draft:
   `Telex_0.2.0_x64-setup.exe` and `.exe.sig`, `Telex_0.2.0_x64_en-US.msi` and `.msi.sig`,
   `Telex_0.2.0_amd64.AppImage` and `.AppImage.sig`, `Telex_0.2.0_amd64.deb` and `.deb.sig`,
   and `latest.json`. Windows builds first, then Linux on Ubuntu 22.04 (one at a time, so
   both get into latest.json). Last, the **Checksums** job downloads every file of the draft,
   uploads `SHA256SUMS` and attests each file (build provenance through GitHub's Sigstore: no
   key, no cost; `gh attestation verify <file> --repo lucco669/telexmail`). The site's download
   page shows "Check your download" for a release that has `SHA256SUMS`. It takes about 20–30
   minutes.
5. Wait until the whole run is green, Linux and Checksums too: the draft shows the Windows files
   about ten minutes before the Linux ones. Then check it: the files are there with
   `SHA256SUMS`, latest.json lists both
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

## SmartScreen and code signing

The minisign key above signs updates only; Windows doesn't know it. The installers carry no
Authenticode signature, so SmartScreen warns on a first install (the site says More info › Run
anyway). Updates don't warn: Telex downloads them itself, without the Mark of the Web.

- **Free, each release:** submit the new installers to Microsoft (below). It often clears the
  warning for those files within a few days; there is no promise and no fixed time. Unsigned
  reputation is per file (its hash), so every release starts over.
- **Free, once, then a line per release:** winget (below). Files winget downloads should carry
  no Mark of the Web, so no warning; test it on a clean PC before relying on that.
- **Free, larger:** the Microsoft Store (individual developers register free since 2025). An
  MSIX package is signed by the Store; Tauri doesn't build MSIX itself, and Store copies update
  through the Store, so that build would turn Telex's own updater off.
- **Paid, the real fix:** Azure Trusted Signing (about $10 a month; check whether it takes an
  individual or sole trader in Poland) through `bundle.windows.signCommand` in
  `tauri.release.conf.json` and three Azure secrets in the Windows job; or a Certum certificate
  with its SimplySign cloud key (about €100–200 a year, awkward in CI: sign locally). Since 2023
  every code-signing key lives in an HSM, so a `.pfx` in a secret is no longer possible.
  SignPath's free certificates and Certum's cheap one are for open-source projects only;
  Telex isn't one. EV no longer skips SmartScreen's reputation, so it isn't worth its price.
- Linux has no such warning. The AppImage updater checks minisign, and `SHA256SUMS` with the
  attestations covers the downloads.

### Submitting a release to Microsoft (each release, about 10 minutes)

Do it while the draft is waiting to be published: the files are final once the run is green,
and a day of Microsoft's review is saved. Submit the `.exe` and the `.msi` separately, one file
per submission (Microsoft asks for single files; a bundle is analysed later or with lower
priority).

1. Download `Telex_<version>_x64-setup.exe` and `Telex_<version>_x64_en-US.msi` from the draft
   release on GitHub.
2. Open <https://www.microsoft.com/en-us/wdsi/filesubmission> and choose **Software developer**.
   Sign in with a Microsoft account (a personal one works; use the same one every time, so the
   submissions are in one history).
3. Fill in the form:
   - **Product:** Microsoft Defender SmartScreen.
   - **Company name:** Łukasz Szramuk.
   - **File:** the `.exe` (the next submission: the `.msi`).
   - **What do you believe this file is?** Incorrectly detected / should not be blocked (the
     form's wording changes; choose the option that says the file is clean).
   - **Detection name:** `Windows protected your PC: unrecognized app` (what SmartScreen says).
   - **Additional information** (paste and fill in):

     ```
     Telex is a free email client for Windows by Łukasz Szramuk, built by GitHub Actions
     from https://github.com/lucco669/telexmail (tag v<version>) and published at
     https://github.com/lucco669/telexmail/releases. SmartScreen shows "unrecognized app"
     for this new version. The file is unsigned; its build provenance can be checked with
     "gh attestation verify <file> --repo lucco669/telexmail", and its SHA-256 is in the
     release's SHA256SUMS. Earlier versions of the same app were submitted before.
     ```
4. Submit and keep the submission ID. Its state is under **Submission history** on the same
   portal. If nothing happens for a couple of weeks, Microsoft's support takes the ID and the
   file's SHA-256.

Nothing to change in Telex either way: when the review clears the file, the warning stops for
everyone downloading that exact file.

### winget (once, then each release)

Wait with the first listing until the product name is final (see CLAUDE.md, Open decisions):
the package identifier stays with the listing.

Once:

1. `winget install Microsoft.WingetCreate`
2. `wingetcreate new https://github.com/lucco669/telexmail/releases/download/v<version>/Telex_<version>_x64-setup.exe`
   It downloads the installer and asks for the rest:
   - **PackageIdentifier:** `Szramuk.Telex` (Publisher.Name; it can't be changed later).
   - **Publisher:** Łukasz Szramuk. **PackageName:** Telex. **Moniker:** telex.
   - **License:** Freeware. **LicenseUrl:** the site's `/license` page.
   - **ShortDescription:** Fast, keyboard-driven email client.
   - **InstallerType:** nullsoft (Tauri's NSIS installer; silent install is `/S`).
     **Scope:** user (Tauri installs per user by default).
   - **PackageUrl:** the site; **ReleaseNotesUrl:** the release on GitHub.
3. Check the manifest before sending it. In an administrator PowerShell, once:
   `winget settings --enable LocalManifestFiles`. Then
   `winget validate --manifest <the folder wingetcreate wrote>` and
   `winget install --manifest <that folder>` (installs Telex from it; uninstall afterwards).
4. Let wingetcreate submit (it signs in to GitHub in the browser and opens a PR on
   `microsoft/winget-pkgs` from a fork in your account). A bot checks the PR, a moderator
   merges it, usually within days. After that, `winget install Szramuk.Telex` works.

Each release, after publishing:

```
wingetcreate update Szramuk.Telex --version <version> --urls https://github.com/lucco669/telexmail/releases/download/v<version>/Telex_<version>_x64-setup.exe --submit
```

(Older Telex versions update themselves, so winget only needs the newest version for new
installs.) This step could run in release.yml later, with a GitHub token of your own as a secret.

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
