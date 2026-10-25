import { timingSafeEqual } from 'node:crypto';
import { dayOf, snapshotRows } from '@/lib/count';
import { countConfig, githubReleases, saveTotals } from '@/lib/count-store';

// A Coolify scheduled task (00:05 UTC) calls this once a day with `Authorization: Bearer <CRON_SECRET>`:
// GitHub's download totals for today, so /stats can show downloads per day.
export async function GET(request: Request) {
  const secret = process.env.CRON_SECRET;
  const given = Buffer.from(request.headers.get('authorization') ?? '');
  const wanted = Buffer.from(`Bearer ${secret}`);
  if (!secret || given.length !== wanted.length || !timingSafeEqual(given, wanted)) {
    return new Response(null, { status: 401 });
  }
  if (!countConfig()) return Response.json({ saved: 0, error: 'storage not connected' }, { status: 503 });
  const rows = snapshotRows(await githubReleases(), dayOf(new Date()));
  await saveTotals(rows);
  return Response.json({ saved: rows.length });
}
