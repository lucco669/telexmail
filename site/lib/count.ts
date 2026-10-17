// Counting copies of Telex (the app's count.rs) and GitHub's downloads: what a count may say, and
// the sums the private /stats page shows. Pure functions; storage is in count-store.ts.

export const OSES = ['windows', 'appimage', 'linux'] as const;
export const LANGS = ['en', 'pl'] as const;

/** One count, as the app sends it once a day. */
export type Ping = {
  version: string;
  os: (typeof OSES)[number];
  lang: (typeof LANGS)[number];
  /** The month the copy was first counted, "2026-10". */
  since: string;
  new: boolean;
  week: boolean;
  month: boolean;
};

const KEYS = ['lang', 'month', 'new', 'os', 'since', 'version', 'week'];

/** A count the site takes, or null. Exactly the app's keys; a first count is first in every sense. */
export function parsePing(body: unknown, now = new Date()): Ping | null {
  if (!body || typeof body !== 'object' || Array.isArray(body)) return null;
  const b = body as Record<string, unknown>;
  if (Object.keys(b).sort().join() !== KEYS.join()) return null;
  const { version, os, lang, since } = b;
  if (typeof version !== 'string' || !/^\d{1,3}\.\d{1,3}\.\d{1,5}$/.test(version)) return null;
  if (!OSES.includes(os as Ping['os']) || !LANGS.includes(lang as Ping['lang'])) return null;
  if (typeof since !== 'string' || !/^20\d{2}-(0[1-9]|1[0-2])$/.test(since)) return null;
  if (since < '2026-10' || since > monthOf(now)) return null;
  if (![b.new, b.week, b.month].every((v) => typeof v === 'boolean')) return null;
  if (b.new && !(b.week && b.month)) return null;
  return { version, os: os as Ping['os'], lang: lang as Ping['lang'], since, new: b.new as boolean, week: b.week as boolean, month: b.month as boolean };
}

// ---- Dates, in UTC as the counts are ----

export const dayOf = (d: Date) => d.toISOString().slice(0, 10);
export const monthOf = (d: Date) => d.toISOString().slice(0, 7);
const addDays = (day: string, n: number) => dayOf(new Date(Date.parse(`${day}T00:00:00Z`) + n * 864e5));
/** The Monday of `day`'s ISO week. */
export function mondayOf(day: string) {
  const weekday = (new Date(`${day}T00:00:00Z`).getUTCDay() + 6) % 7;
  return addDays(day, -weekday);
}
const prevMonth = (month: string) => {
  const [y, m] = month.split('-').map(Number);
  return m === 1 ? `${y - 1}-12` : `${y}-${String(m - 1).padStart(2, '0')}`;
};

// ---- Copies counted ----

/** A row of telex_count_days. */
export type CountRow = {
  day: string; version: string; os: string; lang: string; since: string;
  copies: number; weekly: number; monthly: number; new: number;
};

export type Copies = {
  today: string;
  /** Active copies (each counts once a day). */
  day: { today: number; yesterday: number };
  week: { now: number; last: number };
  month: { now: number; last: number };
  /** New copies in the last 30 days. */
  newIn30: number;
  /** The last 60 days, oldest first. */
  daily: { day: string; copies: number; new: number }[];
  /** Yesterday's active copies by version, system and language (a full UTC day). */
  yesterday: { version: Record<string, number>; os: Record<string, number>; lang: Record<string, number> };
  /** Per month first counted: how many started then, and how many were active each month since. */
  cohorts: { since: string; started: number; active: Record<string, number> }[];
  months: string[];
};

export function summariseCopies(rows: CountRow[], now = new Date()): Copies {
  const today = dayOf(now), yesterday = addDays(today, -1);
  const monday = mondayOf(today), lastMonday = addDays(monday, -7);
  const month = monthOf(now), lastMonth = prevMonth(month);
  const sum = (pick: (r: CountRow) => number, keep: (r: CountRow) => boolean) =>
    rows.reduce((n, r) => (keep(r) ? n + pick(r) : n), 0);

  const byDay = new Map<string, { copies: number; new: number }>();
  for (const r of rows) {
    const d = byDay.get(r.day) ?? { copies: 0, new: 0 };
    d.copies += r.copies; d.new += r.new;
    byDay.set(r.day, d);
  }
  const daily = Array.from({ length: 60 }, (_, i) => {
    const day = addDays(today, i - 59);
    return { day, ...(byDay.get(day) ?? { copies: 0, new: 0 }) };
  });

  const yesterdayBy = { version: {} as Record<string, number>, os: {} as Record<string, number>, lang: {} as Record<string, number> };
  for (const r of rows) {
    if (r.day !== yesterday) continue;
    for (const key of ['version', 'os', 'lang'] as const) yesterdayBy[key][r[key]] = (yesterdayBy[key][r[key]] ?? 0) + r.copies;
  }

  const cohorts = new Map<string, { since: string; started: number; active: Record<string, number> }>();
  const months = new Set<string>();
  for (const r of rows) {
    const c = cohorts.get(r.since) ?? { since: r.since, started: 0, active: {} };
    const m = r.day.slice(0, 7);
    c.started += r.new;
    c.active[m] = (c.active[m] ?? 0) + r.monthly;
    months.add(m);
    cohorts.set(r.since, c);
  }

  return {
    today,
    day: { today: sum((r) => r.copies, (r) => r.day === today), yesterday: sum((r) => r.copies, (r) => r.day === yesterday) },
    week: {
      now: sum((r) => r.weekly, (r) => r.day >= monday && r.day <= today),
      last: sum((r) => r.weekly, (r) => r.day >= lastMonday && r.day < monday),
    },
    month: {
      now: sum((r) => r.monthly, (r) => r.day.startsWith(month)),
      last: sum((r) => r.monthly, (r) => r.day.startsWith(lastMonth)),
    },
    newIn30: sum((r) => r.new, (r) => r.day > addDays(today, -30)),
    daily,
    yesterday: yesterdayBy,
    cohorts: [...cohorts.values()].sort((a, b) => a.since.localeCompare(b.since)),
    months: [...months].sort(),
  };
}

// ---- Downloads (GitHub's totals) ----

export type GitHubRelease = {
  tag_name: string; draft: boolean; published_at: string | null;
  assets: { name: string; download_count: number }[];
};

export const KINDS = ['exe', 'msi', 'appimage', 'deb'] as const;
export type Kind = (typeof KINDS)[number];

/** What a release file is: an installer of one kind, the updater's latest.json, or neither (signatures, sums). */
export function kindOf(name: string): Kind | 'latest' | null {
  const n = name.toLowerCase();
  if (n === 'latest.json') return 'latest';
  if (n.endsWith('.exe')) return 'exe';
  if (n.endsWith('.msi')) return 'msi';
  if (n.endsWith('.appimage')) return 'appimage';
  if (n.endsWith('.deb')) return 'deb';
  return null;
}

export type ReleaseDownloads = { release: string; date: string | null; kinds: Record<Kind, number>; installers: number; latest: number };

/** Per published release, newest first: installer downloads by kind, and fetches of latest.json. */
export function releaseDownloads(releases: GitHubRelease[]): ReleaseDownloads[] {
  return releases.filter((r) => !r.draft).map((r) => {
    const kinds = { exe: 0, msi: 0, appimage: 0, deb: 0 } as Record<Kind, number>;
    let latest = 0;
    for (const a of r.assets) {
      const k = kindOf(a.name);
      if (k === 'latest') latest += a.download_count;
      else if (k) kinds[k] += a.download_count;
    }
    return { release: r.tag_name, date: r.published_at, kinds, installers: KINDS.reduce((n, k) => n + kinds[k], 0), latest };
  });
}

/** A row of telex_download_totals: GitHub's running total of one file on one day. */
export type TotalRow = { day: string; release: string; asset: string; downloads: number };

export function snapshotRows(releases: GitHubRelease[], day: string): TotalRow[] {
  return releases.filter((r) => !r.draft).flatMap((r) => r.assets
    .filter((a) => kindOf(a.name))
    .map((a) => ({ day, release: r.tag_name, asset: a.name, downloads: a.download_count })));
}

/**
 * Downloads per day from the daily snapshots: each day's growth of the totals over the day
 * before that has a snapshot (installers, and latest.json fetches). A file first seen on a day
 * counts from zero, so a release's first day includes everything before its first snapshot.
 */
export function downloadsPerDay(rows: TotalRow[]) {
  const days = [...new Set(rows.map((r) => r.day))].sort();
  const totals = new Map(days.map((d) => [d, new Map<string, number>()]));
  for (const r of rows) totals.get(r.day)!.set(r.asset, r.downloads);
  const out: { day: string; installers: number; latest: number }[] = [];
  for (let i = 1; i < days.length; i++) {
    const before = totals.get(days[i - 1])!, now = totals.get(days[i])!;
    let installers = 0, latest = 0;
    for (const [asset, n] of now) {
      const grew = Math.max(0, n - (before.get(asset) ?? 0));
      if (kindOf(asset) === 'latest') latest += grew; else installers += grew;
    }
    out.push({ day: days[i], installers, latest });
  }
  return out;
}
