import { clientNetwork, issueSession, limitBucket, sameOrigin } from '@/lib/night-shift-ranking';
import { leaderboardConfig, takeLimit } from '@/lib/night-shift-store';

const headers = { 'Cache-Control': 'no-store' };
// A shift lasts 90 seconds; 40 an hour leaves room for restarts, not for a script.
const SESSIONS_PER_HOUR = 40;

export async function POST(request: Request) {
  if (!sameOrigin(request)) return Response.json({ error: 'Start your shift from this website.' }, { status: 403, headers });
  const config = leaderboardConfig();
  if (!config) return Response.json({ available: false }, { status: 503, headers });
  try {
    if (!await takeLimit(limitBucket('session', clientNetwork(request), config.signingSecret), SESSIONS_PER_HOUR, 3600)) {
      return Response.json({ error: 'Too many shifts from this network. Scored play is back within the hour.' }, { status: 429, headers: { ...headers, 'Retry-After': '3600' } });
    }
  } catch { return Response.json({ available: false }, { status: 503, headers }); }
  return Response.json(issueSession(config.signingSecret), { headers });
}
