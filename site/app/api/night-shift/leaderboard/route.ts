import { clientNetwork, limitBucket, sameOrigin, verifyRun } from '@/lib/night-shift-ranking';
import { databaseRequest, leaderboardConfig, listRankings, PERIODS, SORTS, takeLimit } from '@/lib/night-shift-store';

const headers = { 'Cache-Control': 'no-store' };
// The board is the same for everyone: Vercel's edge keeps it for 30 seconds, so reads don't
// reach Supabase more than twice a minute per sort and period.
const cached = { 'Cache-Control': 'public, max-age=0, s-maxage=30, stale-while-revalidate=30' };
const PUBLISH_PER_HOUR = 10;
const PUBLISH_PER_DAY = 40;

export async function GET(request: Request) {
  const search = new URL(request.url).searchParams;
  const sort = search.get('sort') ?? 'score';
  const period = search.get('period') ?? 'all';
  // Only the six real queries are answered, so made-up parameters can't bypass the cache.
  if (!SORTS.includes(sort) || !PERIODS.includes(period) || [...search.keys()].some((key) => key !== 'sort' && key !== 'period')) {
    return Response.json({ error: 'Unknown leaderboard.' }, { status: 400, headers: cached });
  }
  if (!leaderboardConfig()) return Response.json({ available: false, entries: [] }, { headers: cached });
  try { return Response.json({ available: true, entries: await listRankings(sort, period) }, { headers: cached }); }
  catch { return Response.json({ available: true, error: 'The online board is taking a break. Try again shortly.' }, { status: 503, headers }); }
}

export async function POST(request: Request) {
  if (!sameOrigin(request)) return Response.json({ error: 'Publish your score from this website.' }, { status: 403, headers });
  const config = leaderboardConfig();
  if (!config) return Response.json({ error: 'The online board is not connected yet.' }, { status: 503, headers });
  let body: unknown;
  try {
    const reader = request.body?.getReader();
    if (!reader) throw new Error('Missing run');
    const chunks: Uint8Array[] = []; let length = 0;
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      length += value.byteLength;
      if (length > 250000) { await reader.cancel(); return Response.json({ error: 'The run is too large.' }, { status: 413, headers }); }
      chunks.push(value);
    }
    body = JSON.parse(Buffer.concat(chunks).toString('utf8'));
  } catch { return Response.json({ error: 'The run could not be read.' }, { status: 400, headers }); }
  let entry;
  try { entry = verifyRun(body, config.signingSecret); }
  catch (error) { return Response.json({ error: error instanceof Error ? error.message : 'The run could not be verified.' }, { status: 400, headers }); }
  // Counted only for runs that pass the replay, so a mistyped name doesn't use one up.
  try {
    const network = clientNetwork(request);
    const allowed = await takeLimit(limitBucket('publish-hour', network, config.signingSecret), PUBLISH_PER_HOUR, 3600)
      && await takeLimit(limitBucket('publish-day', network, config.signingSecret), PUBLISH_PER_DAY, 86400);
    if (!allowed) return Response.json({ error: 'This network has published plenty of shifts for now. Try again later.' }, { status: 429, headers: { ...headers, 'Retry-After': '3600' } });
  } catch { return Response.json({ error: 'The score could not be saved. Your result is still here; try again.' }, { status: 503, headers }); }
  try {
    const response = await databaseRequest('night_shift_scores', { method: 'POST', body: JSON.stringify(entry), headers: { Prefer: 'return=minimal' } });
    if (response.status === 409) return Response.json({ error: 'This shift is already on the board.' }, { status: 409, headers });
    if (!response.ok) throw new Error('Storage failed');
    return Response.json({ published: true, id: entry.id, entry }, { status: 201, headers });
  } catch { return Response.json({ error: 'The score could not be saved. Your result is still here; try again.' }, { status: 503, headers }); }
}
