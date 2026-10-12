# Security checks and evidence

The public Privacy & security page is `/security`. Keep its claims tied to implementation,
tests and completed results. Source on main can differ from published installers. Do not label
a release "audited", "virus-free", "reproducible" or "100% safe" based on these checks.

## Automated checks

`.github/workflows/security.yml` runs on pushes to main, version tags, pull requests, a weekly
schedule, and manual dispatch. It checks Windows and Linux independently. Actions are pinned
to commits; cargo-audit is pinned to 0.22.2; Gitleaks 8.30.1 archives are checked against recorded
SHA-256 hashes before extracting their executable. The workflow has read-only repository
permissions and does not use a scanner service token or upload source to a secret-scanning service.

`npm run security` runs every check even when a scan returns findings:

| Check | Scope | Failure policy |
| --- | --- | --- |
| RustSec / cargo-audit | Complete Cargo.lock, including dependencies for other platforms | Vulnerabilities and `unsound` advisories fail. Other warnings remain in the report. |
| npm audit | Root and site lockfiles, including development dependencies | Any reported vulnerability fails. npm receives the dependency inventory to perform the audit. |
| Gitleaks history | All fetched Git history (`--all`) | Findings fail. The workflow checks out full history. |
| Gitleaks current source | Git-tracked and non-ignored files, including local modifications | Findings fail. Generated/ignored build and profile files are outside this scope. |
| Security guards | Desktop CSP, mail-frame sandbox and Tauri plugin capabilities | Unexpected permissions or loosened policies fail. These are static configuration checks. |
| Core unit tests | `cargo test -p telex-core --locked --lib` | Test failures fail. This includes HTML rendering, remote images, attachments and password-export tests. Ignored live-server tests are not run. |

Missing tools, malformed reports and audit-service failures produce `error`, rather than a pass.
No advisories are suppressed by this workflow. Secret scans use the upstream default rules and
ignore inline `gitleaks:allow` comments. Pattern-based scanners can miss secrets and can report
false positives. These checks do not prove absence of telemetry, malware, or all vulnerabilities.

## Repeat locally

Use the repository's supported Rust toolchain and Node.js 22 or newer. From the repository root:

```
cargo install cargo-audit --locked --version 0.22.2 --root .bench/security-tools
npm run security:tools
npm run security
```

The runner detects those local tools without changing your global PATH. The Gitleaks installer
supports Windows and Linux x64 and needs `tar`. On another platform, install the same versions
manually on PATH. `npm run security:test` runs just the configuration and evidence-report tests.

To inspect a particular failing scan directly:

```
.bench/security-tools/bin/cargo-audit audit --deny unsound --json
.bench/security-tools/bin/gitleaks git --log-opts=--all --redact=100 --ignore-gitleaks-allow
npm audit --json
```

On Windows the local executable names end in `.exe`. Run `npm audit --json` in `site/` as well.

## What a run records

The runner writes ignored local files under `.bench/security/`. CI uploads that directory as
`security-evidence-<runner>` even if checks fail, retaining it for 90 days. Each report includes:

- The Git commit, whether the working tree was dirty, a SHA-256 of the source snapshot, and
  individual lockfile hashes. A local dirty-tree run is not represented as testing only its HEAD.
- Start and finish timestamps, platform, tool versions, and a GitHub run URL when in CI.
- Each check's command, exit code and outcome; dependency advisory IDs and warning categories.
- Unit/configuration test logs. Secret evidence contains only counts and rule names, never
  matched values, source lines, filenames or commit messages from scanner findings.

The source snapshot is hashed again after the checks. If it changes during the run, the evidence
is marked failed and must be regenerated. Source copies in `.bench/security-source-*` are kept
locally for inspection and are not part of the uploaded artifact. They contain source files;
they are not a mailbox or an app profile.

Read the GitHub run's summary for its outcome and advisories. The JSON artifact provides the
full recorded evidence; downloading it may require GitHub sign-in. The website links to runs,
rather than copying an undated result or assuming the newest one passed. These checks are
separate from the release workflow; a failed check does not automatically stop release publication.
Review the result for the release's exact commit before publishing. Branch protection and any
release gates are repository settings/workflow changes, not guarantees made by this page.

## Findings from 12 October 2026

- The initial scan found RUSTSEC-2021-0157 in ammonia 4.2.1. This change upgrades the sanitizer
  to 4.2.3, a patched version. This is a source change; it does not patch already installed releases.
- RUSTSEC-2024-0429 remains in glib 0.18.5, reached through Tauri's Linux dependency graph.
  The advisory describes unsound `VariantStrIter` methods and lists glib >=0.20.0 as patched.
  A direct version bump is incompatible with the GTK 0.18 dependency family used by Tauri.
  Reachability in Telex has not been established. The strict audit fails until this is resolved;
  do not silently ignore it or describe the dependency audit as clean.
- RUSTSEC-2024-0370 reports proc-macro-error 1.0.4 as unmaintained, also through Linux
  dependencies. This remains a visible maintenance warning.

The current mail database, backups and exported mail are not encrypted by Telex. Account
passwords are stored separately by the operating system's credential service. Windows publisher
code signing is not configured; updater signatures are a different mechanism. No independent
security audit, installer antivirus assessment or reproducible-build evidence is included here.
