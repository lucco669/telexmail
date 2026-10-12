# Measuring Telex

The public performance page uses fresh measurements of the desktop app and a shared,
synthetic mailbox. It never treats the Rust process alone as the whole app, a database
query as a desktop interaction, or an unmeasured competitor as a comparison result.

## What the test measures

- Windows installer bytes, from the GitHub release shown by the site. This excludes
  provisioning the shared WebView2 runtime and the user's mail. Linux packages have
  their own sizes; the Windows size is not a claim about every platform.
- Launch to an inbox and selected message ready. The first launch is separate from
  four repeat launches. Neither is called a cold boot.
- Folder switching, reading an uncached message, and jumping to the end of a
  40,000-message inbox. Measurements include the core, IPC, the real desktop UI, and
  two animation frames after the work. Physical display scanout is not measured.
- Full-text search across 100,000 messages. Three fixed queries cover different
  result counts. The UI's 150 ms body-search delay is included, and its result cache
  is invalidated between repetitions without changing searchable text.
- Idle private RAM and CPU. We sample approximately once per second for 30 seconds,
  after five seconds to settle, and include the complete process tree. CPU comes
  from process CPU-time deltas, divided by measured wall time and logical processors.
  The first CPU sample is omitted. Very short lived processes between samples can be
  missed; this is an idle-resource test, not a CPU trace.

Private working set is resident private memory. It can change as Windows pages memory
in and out. Summing total working sets would count shared pages more than once; we sum
private working sets instead. Private committed memory is also recorded, but is not
labelled RAM. Dedicated/shared GPU memory is not part of this metric.

## Shared mailbox

`crates/telex-core/tests/scale.rs` generates 100,000 deterministic messages: three
groups, twelve populated folders, five years of dates, plain text and HTML bodies,
and roughly 10% with attachments. The largest inbox has 40,000 messages. Each message
has a synthetic Message-ID. The raw messages export as mbox for competitors.

Telex's fixture uses three synthetic accounts, with network checks and IDLE disabled.
Thunderbird's fixture uses Local Folders with three branches. This compares local
mail workloads, not mail-server connections or identical account configurations.
Sync, sending, battery life and features such as calendars are outside this test.

Do not compare equal account counts when different amounts of mail are downloaded.
Telex normally starts with the latest 300 messages per folder; this fixture contains
the entire 100,000-message corpus in every client.

## Why mail is searchable as it is saved

Telex uses a contentless SQLite FTS5 index over subject, sender, recipients and body.
The insert/update/delete triggers in `007_message_content.sql` keep it in step with
stored mail. `MailStore::add_synced` saves each batch and its index entries in the
same transaction. Normal sync/import therefore has no separate search-index catch-up
phase after storage completes. Schema upgrades can rebuild the index once.

Message summaries are separate from bodies and raw MIME mail. Folder lists read a
page of summaries, not every full message. The FTS index keeps search tokens without
another full copy of the body. These are specific design properties, not claims
that indexing is free or every feature is faster than another mail client.

The core fixture construction time includes synthetic message generation, full-message
storage and index maintenance, in batches of 1,000. It excludes network transfer and
MIME parsing: the generator already supplies parsed fields. If the existing fixture
database is reused, no new construction time is published. Thunderbird's separate
Gloda index parses exported messages and intentionally throttles background work.
The preparation wait is not a controlled import or indexing speed comparison.

Sources: [Telex's index triggers](../crates/telex-core/src/migrations/007_message_content.sql),
[storage code](../crates/telex-core/src/store.rs),
[Thunderbird global search](https://support.mozilla.org/en-US/kb/global-search),
[Gloda's scheduling design](https://searchfox.org/comm-central/source/mailnews/db/gloda/modules/GlodaIndexer.sys.mjs).

## Reproduce on Windows

Use the repository's normal Rust/Node prerequisites and Node 24 or newer for the
Thunderbird index verifier (`node:sqlite`). Commands below run in PowerShell at the
repository root. Generated mailboxes, profiles, binaries and raw reports stay in
ignored `.bench/`, with Telex's isolated profile in `%APPDATA%/app.telexmail.benchmark`.
These commands do not use the production Telex database or Thunderbird profile.

`npm run benchmark` runs the full Telex sequence and restores the ordinary release
binary afterward. For individual steps, use the commands below.

```powershell
New-Item -ItemType Directory -Path .bench -Force | Out-Null
$env:TELEX_SCALE_DB = Join-Path (Get-Location) '.bench/telex.db'
$env:TELEX_SCALE_EXPORT = Join-Path (Get-Location) '.bench/mailbox'
cargo test -p telex-core --release --test scale -- --ignored --nocapture |
    Tee-Object -FilePath .bench/core.txt
node scripts/benchmark/prepare.mjs
npm run tauri -- build --config .bench/tauri.conf.json --no-bundle
Copy-Item -LiteralPath target/release/telex.exe -Destination .bench/telex-benchmark.exe
pwsh -NoProfile -File scripts/benchmark/run-telex.ps1 -Runs 5
```

The generated UI is a copy of `ui/`, with timing hooks installed by exact, checked
replacements. The mail engine and desktop shell are the real release build. The app
identifier and profile are different, and no debug port is opened. Measurement data
crosses a local window-title channel, never a server. The normal app sources are
not instrumented. The runner refuses an executable without the benchmark product name.

Keep the benchmark window visible and unminimized. Avoid games, builds, initial
indexing and other heavy work during measurements. Allow each run to finish. If a
window is minimized, a query times out, or a process cannot be read, fix the cause
and rerun rather than keeping a flattering partial result.

The custom build occupies `target/release/telex.exe`. Before building normal
installers, restore the ordinary configuration:

```powershell
npm run tauri -- build --no-bundle
```

## Thunderbird comparison

```powershell
node scripts/benchmark/prepare-thunderbird.mjs
```

Start the installed Thunderbird with `-no-remote -profile <absolute .bench/thunderbird
path>`. Open the generated folders and allow global search indexing to finish.
Thunderbird suppresses Gloda indexing while offline; prepare online with this
local-only profile, verify the index, then close it and restart with `-offline` for resource measurements.
Open `twojastudio/inbox`, select its newest message, and use the same window size as
Telex (1280×800 client area). Use Thunderbird's table list, unthreaded and sorted
newest first. Both apps display the plain-text body of the same newest inbox message.
Record the isolated
instance's PID in `.bench/thunderbird.pid` after each restart, then verify every
folder's searchable live-message count for that process:

```powershell
Set-Content -LiteralPath .bench/thunderbird.pid -Value <benchmark PID>
node --no-warnings scripts/benchmark/verify-thunderbird.mjs
```

Record the PID of the instance started with the benchmark profile. Do not select a
real-mail instance just because its executable has the same name. With the window
visible, unminimized and in the background, matching the public Telex selection:

```powershell
pwsh -NoProfile -File scripts/benchmark/record-client.ps1 `
  -RootId <benchmark PID> -Client thunderbird -Version <installed version> `
  -Notes 'Local Folders; 100,000 messages; index verified; offline inbox with a selected message' -Runs 5
node scripts/benchmark/summarize.mjs
```

The summary refuses mismatched fixture IDs, message counts, machines, scenarios,
sampling durations and incomplete process reads. It excludes minimized and foreground
samples and requires matching background conditions and stable window dimensions
(allowing two pixels for frame rounding). Match the actual recorded Windows window
bounds; Tauri's requested client size can differ from Thunderbird's outer-window size.
It writes the aggregate data to
`site/public/benchmarks/windows-100k.json`. Raw reports remain local; public data
contains no mailbox content, profile paths, usernames or process command lines.

Telex's resource samples come from five fresh launches. Thunderbird's five sampling
windows come from one fresh offline launch after its index is complete. These
are idle measurements, not a comparison of how memory grows over a day of use.
Wait at least five seconds after selecting the inbox message before recording.

Public resource figures select only visible, unminimized background samples, using
the same rule for every client and requiring at least 20 samples. CPU intervals that
cross a focus change are omitted, so a gap in eligible sampling cannot dilute CPU usage.
The JSON records included and total sample counts; raw reports retain every state.

## Outlook and future metrics

Classic Outlook and new Outlook are separate products. Add each only after importing
the same full corpus into an isolated profile and verifying indexing. Do not reuse
a Thunderbird number, an empty Outlook profile, or a screenshot of a different
mailbox. `record-client.ps1` supports both names; it requires a verification record
with `fixtureId`, `messages: 100000`, and `indexingComplete: true` before measurement.
Until such a run exists, the site explicitly marks their comparison as unmeasured.

Battery claims require a separate, controlled battery/power experiment. Idle CPU is
not a battery-life estimate. Network sync comparisons need a controlled mail server,
identical missing messages, matched connection conditions and completed downloads.
Do not turn the synthetic offline benchmark into a sync or energy claim.

## Validate and refresh

```powershell
node --test scripts/benchmark/summarize.test.mjs
cd site
npm run typecheck
npm run build
```

Record the app version, benchmark build, machine, date, corpus and sample counts.
Keep the full distributions, including slower samples. The page reports medians and
ranges; the JSON also contains nearest-rank p95, where enough samples exist to be
useful. Retest after changes that affect performance. The homepage reads measured
data directly and gets installer bytes from the existing release lookup.

References: [Windows memory counters](https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-process_memory_counters_ex2),
[WebView2's process model](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/process-model).
