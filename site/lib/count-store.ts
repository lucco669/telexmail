import 'server-only';
import { createHmac } from 'node:crypto';
import type { CountRow, GitHubRelease, Ping, TotalRow } from './count';
import { site } from './site';

// The counts live in the same Supabase project as Night Shift's board (supabase/migrations/
// 20261017_telex_count.sql), written only by this server with the secret key.
export function countConfig() {
  const url = process.env.SUPABASE_URL;
  const key = process.env.SUPABASE_SECRET_KEY || process.env.SUPABASE_SERVICE_ROLE_KEY;
  if (!url || !key) return null;
  try { if (new URL(url).protocol !== 'https:') return null; } catch { return null; }
  return { url: url.replace(/\/$/, ''), key };
}

async function request(path: string, init: RequestInit = {}) {
  const config = countConfig();
  if (!config) throw new Error('Count storage is not connected.');
  const response = await fetch(`${config.url}/rest/v1/${path}`, {
    ...init, cache: 'no-store', signal: AbortSignal.timeout(8000),
    headers: { apikey: config.key, ...(config.key.startsWith('eyJ') ? { Authorization: `Bearer ${config.key}` } : {}), 'Content-Type': 'application/json', ...init.headers },
  });
  if (!response.ok) throw new Error(`Supabase answered ${response.status}.`);
  return response;
}

export async function addCount(ping: Ping) {
  await request('rpc/telex_count_add', {
    method: 'POST',
    body: JSON.stringify({
      p_version: ping.version, p_os: ping.os, p_lang: ping.lang, p_since: ping.since,
      p_new: ping.new, p_week: ping.week, p_month: ping.month,
    }),
  });
}

/**
 * One use of a per-network allowance (Night Shift's limiter, `night_shift_take`), under a keyed
 * hash of the network, never the address. Throws when storage can't be reached.
 */
export async function takeAllowance(action: string, network: string, limit: number, windowSeconds: number) {
  const config = countConfig();
  if (!config) throw new Error('Count storage is not connected.');
  const bucket = `${action}:${createHmac('sha256', config.key).update(`telex-count:${network}`).digest('base64url').slice(0, 22)}`;
  const response = await request('rpc/night_shift_take', {
    method: 'POST', body: JSON.stringify({ p_bucket: bucket, p_limit: limit, p_window_seconds: windowSeconds }),
  });
  return await response.json() === true;
}

// PostgREST answers at most 1,000 rows at a time.
async function every<T>(path: string): Promise<T[]> {
  const rows: T[] = [];
  for (let offset = 0; ; offset += 1000) {
    const page = await (await request(`${path}&limit=1000&offset=${offset}`)).json() as T[];
    rows.push(...page);
    if (page.length < 1000) return rows;
  }
}

/** Counts from `from` (a UTC day) on. */
export const countRows = (from: string) =>
  every<CountRow>(`telex_count_days?select=*&day=gte.${from}&order=day.asc,version.asc,os.asc,lang.asc,since.asc`);

export const totalRows = (from: string) =>
  every<TotalRow>(`telex_download_totals?select=*&day=gte.${from}&order=day.asc,asset.asc`);

/** Today's totals, replacing an earlier snapshot of the same day. */
export async function saveTotals(rows: TotalRow[]) {
  if (rows.length === 0) return;
  await request('telex_download_totals?on_conflict=day,asset', {
    method: 'POST', body: JSON.stringify(rows), headers: { Prefer: 'resolution=merge-duplicates,return=minimal' },
  });
}

/** Every release with its files' download counts, from GitHub's API (not cached: totals move). */
export async function githubReleases(): Promise<GitHubRelease[]> {
  const headers: Record<string, string> = { Accept: 'application/vnd.github+json', 'X-GitHub-Api-Version': '2022-11-28' };
  if (process.env.GITHUB_TOKEN) headers.Authorization = `Bearer ${process.env.GITHUB_TOKEN}`;
  const response = await fetch(`https://api.github.com/repos/${site.releasesRepo}/releases?per_page=100`, {
    headers, cache: 'no-store', signal: AbortSignal.timeout(10000),
  });
  if (!response.ok) throw new Error(`GitHub answered ${response.status} ${response.statusText}`);
  return await response.json() as GitHubRelease[];
}
