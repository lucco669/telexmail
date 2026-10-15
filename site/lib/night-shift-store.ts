import 'server-only';
import type { LeaderboardEntry } from './night-shift-ranking';

export function leaderboardConfig() {
  const url = process.env.SUPABASE_URL;
  const key = process.env.SUPABASE_SECRET_KEY || process.env.SUPABASE_SERVICE_ROLE_KEY;
  // Sessions are signed with a secret of their own, never the database key: leaking or rotating
  // one must not touch the other.
  const signingSecret = process.env.NIGHT_SHIFT_SIGNING_SECRET;
  if (!url || !key || !signingSecret || signingSecret.length < 32 || signingSecret === key) return null;
  try { if (new URL(url).protocol !== 'https:') return null; } catch { return null; }
  return { url: url.replace(/\/$/, ''), key, signingSecret };
}

export async function databaseRequest(path: string, init: RequestInit = {}) {
  const config = leaderboardConfig();
  if (!config) throw new Error('Leaderboard storage is not connected.');
  return fetch(`${config.url}/rest/v1/${path}`, {
    ...init, cache: 'no-store', signal: AbortSignal.timeout(8000),
    headers: { apikey: config.key, ...(config.key.startsWith('eyJ') ? { Authorization: `Bearer ${config.key}` } : {}), 'Content-Type': 'application/json', ...init.headers },
  });
}

export const SORTS = ['score', 'speed', 'reaction'];
export const PERIODS = ['all', 'week', 'today'];

export async function listRankings(sort: string, period: string) {
  const query = new URLSearchParams({ p_sort: SORTS.includes(sort) ? sort : 'score', p_period: PERIODS.includes(period) ? period : 'all' });
  const response = await databaseRequest(`rpc/night_shift_leaderboard?${query}`);
  if (!response.ok) throw new Error('The leaderboard could not be loaded.');
  return await response.json() as LeaderboardEntry[];
}

// Counts one use of a bucket in the current window (a Postgres counter); false once it is over
// the limit. Throws when storage can't be reached, so callers fail closed.
export async function takeLimit(bucket: string, limit: number, windowSeconds: number) {
  const response = await databaseRequest('rpc/night_shift_take', {
    method: 'POST', body: JSON.stringify({ p_bucket: bucket, p_limit: limit, p_window_seconds: windowSeconds }),
  });
  if (!response.ok) throw new Error('The rate limit could not be checked.');
  return await response.json() === true;
}
