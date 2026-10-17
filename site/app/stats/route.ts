import { createHash, timingSafeEqual } from 'node:crypto';
import { dayOf, downloadsPerDay, releaseDownloads, snapshotRows, summariseCopies } from '@/lib/count';
import { countConfig, countRows, githubReleases, saveTotals, takeAllowance, totalRows } from '@/lib/count-store';
import { clientNetwork } from '@/lib/night-shift-ranking';
import { statsPage, type StatsData } from '@/lib/stats-page';

// The owner's private view of downloads and copies counted. It exists only when STATS_PASSWORD
// is set (12 characters or more); the browser asks for it (any user name). Ten wrong passwords a
// network an hour, then it stops asking.
const WRONG_PER_HOUR = 10;
const headers = { 'Cache-Control': 'no-store', 'X-Robots-Tag': 'noindex, nofollow' };

function allowed(request: Request, password: string) {
  const auth = request.headers.get('authorization') ?? '';
  if (!auth.startsWith('Basic ')) return false;
  const decoded = Buffer.from(auth.slice(6), 'base64').toString('utf8');
  const given = decoded.slice(decoded.indexOf(':') + 1);
  const digest = (s: string) => createHash('sha256').update(s).digest();
  return timingSafeEqual(digest(given), digest(password));
}

export async function GET(request: Request) {
  const password = process.env.STATS_PASSWORD;
  if (!password || password.length < 12) return new Response('Not found', { status: 404, headers });
  if (!allowed(request, password)) {
    const asked = request.headers.has('authorization');
    if (asked && countConfig()) {
      try { if (!await takeAllowance('stats', clientNetwork(request), WRONG_PER_HOUR, 3600)) return new Response('Too many tries. Try again within the hour.', { status: 429, headers }); }
      catch { return new Response('Unavailable', { status: 503, headers }); }
    }
    return new Response('Telex stats: password needed.', { status: 401, headers: { ...headers, 'WWW-Authenticate': 'Basic realm="Telex stats", charset="UTF-8"' } });
  }

  const now = new Date();
  const data: StatsData = { releases: null, perDay: null, copies: null, problems: [] };
  const since = dayOf(new Date(now.getTime() - 400 * 864e5));
  const [github, counts] = await Promise.allSettled([githubReleases(), countConfig() ? countRows(since) : Promise.reject(new Error('not connected'))]);
  if (github.status === 'fulfilled') {
    data.releases = releaseDownloads(github.value);
    if (countConfig()) {
      try {
        // Opening the page takes today's snapshot too, so per-day downloads build up even without the cron.
        await saveTotals(snapshotRows(github.value, dayOf(now)));
        data.perDay = downloadsPerDay(await totalRows(since));
      } catch (e) { data.problems.push(`Download snapshots: ${(e as Error).message}`); }
    }
  } else data.problems.push(`GitHub: ${(github.reason as Error).message}`);
  if (counts.status === 'fulfilled') data.copies = summariseCopies(counts.value, now);
  else if (countConfig()) data.problems.push(`Counts: ${(counts.reason as Error).message}`);

  return new Response(statsPage(data, now), { headers: { ...headers, 'Content-Type': 'text/html; charset=utf-8' } });
}
