# Counting downloads and copies

What Telex counts, how, and why it is built this way. The privacy page
(`site/app/privacy/page.tsx`, `#counting` and `#downloads`) says the same to users; keep the two
in step.

## Downloads

GitHub counts every download of every release file. The site reads those totals
(`api.github.com/repos/lucco669/telexmail/releases`) and keeps one snapshot a day in Supabase
(`telex_download_totals`, the Vercel cron in `site/vercel.json`), so `/stats` can show downloads
per day as well as in total. No personal data reaches us: GitHub only gives totals per file.

What the numbers mean:

- The `.exe`, `.msi`, `.AppImage` and `.deb` counts are installer downloads, from the site, from
  GitHub's page, and **from the updater**: an update downloads the newest release's `.exe` (Windows)
  or `.AppImage` through GitHub's API (latest.json names `api.github.com/.../releases/assets/<id>`),
  which as far as we know GitHub counts like any download. So the newest release's numbers likely
  include updates from older versions; the copies' `new` flags tell new installs apart.
- `latest.json` is fetched by every update check (`src-tauri/src/updates.rs`), so it counts checks
  by copies that said yes to updates, several a day per copy with "when Telex starts". A rough
  sign of life, not a count of people.
- Robots, mirrors and repeated downloads count too.

## Copies in use (only with the user's yes)

From 0.9.11 a release build asks once in the status bar, after the update question is answered:
"Count this copy of Telex once a day, anonymously?" The answer is the setting `usage_count`
(ask, on, off; Settings › Updates › Counting). It belongs to the PC like `backup_dir`: exported
settings leave it out and an import keeps this PC's. Dev and CI builds (no `TELEX_UPDATER_PUBKEY`)
never count and never ask.

With a yes, `count_copy` (`src-tauri/src/count.rs`, called 15 s after start and every hour) sends
at most one count per UTC day (`telex_core::count`), a POST to `https://telexmail.app/api/count`:

```json
{"version": "0.9.11", "os": "windows", "lang": "pl", "since": "2026-10",
 "new": false, "week": true, "month": false}
```

`os` is windows, appimage or linux (the .deb); `lang` the interface's; `since` the month of the
copy's first count; `new`, `week`, `month` whether this is its first count ever, this ISO week,
this month. `count.json` next to settings.json keeps the last day and the first month. A failed
count is quiet and tried again an hour later.

There is no identifier, on purpose. The site adds counts up per UTC day, version, system, language
and first month (`telex_count_days`), and:

- a day's sum of counts is that day's active copies (each sends at most one a day);
- a week's sum of `week` flags is the copies active that ISO week, a month's sum of `month` flags
  those active that month (the trick Brave's usage ping uses);
- `new` gives new copies per day, and `since` gives cohorts: of the copies first counted in
  October, how many were still used in November, December and so on.

What it can't do: tell a reinstall, a second PC or a cleared `count.json` from a new person, or
count anyone who said no. The numbers are a floor of real use, and only from the first release
that asks.

## GDPR and ePrivacy

- Sending anything from a user's device that the app doesn't need to work requires consent
  (ePrivacy Directive art. 5(3); in Poland the Prawo komunikacji elektronicznej). Hence the
  question, off until a yes, and a setting to withdraw it.
- The request carries the user's IP address to Vercel (the site's host), which is personal data in
  transit. The site never stores it: only a keyed hash of the network (IPv4 address or IPv6 /64)
  for the 200-a-day cap, deleted after two days (Night Shift's `night_shift_take`). Legal basis:
  consent, GDPR art. 6(1)(a).
- What is stored is sums with no identifier, which can't be tied to a person. So there are no
  per-person rights requests to serve, and nothing to delete when someone says no later.
- Vercel and Supabase are processors under their own terms (DPAs in their dashboards). Note
  them in the record of processing if you keep one.
- Keep it minimal: anything new in the count (a feature used, the number of accounts) is a new
  purpose. It needs its line on the privacy page, and a payload change means a new question,
  not the old yes.

This is how it is built, not legal advice; a short check by a lawyer before relying on it is cheap.

## Setting it up

1. In Supabase (the Night Shift project): run `site/supabase/migrations/20261017_telex_count.sql`
   (after `20261013_night_shift.sql`, whose limiter it uses).
2. In Vercel › the site's project › Environment Variables: `SUPABASE_URL`, `SUPABASE_SECRET_KEY`
   (there for Night Shift already), `STATS_PASSWORD` (12+ characters), `CRON_SECRET` (any long
   random value; Vercel sends it to the cron), optionally `GITHUB_TOKEN` (no scopes; a higher
   GitHub API limit). Redeploy.
3. Open `https://telexmail.app/stats` (any user name, the password). Downloads show at once; per
   day after two snapshots; copies once a release with the question is out and people say yes.
