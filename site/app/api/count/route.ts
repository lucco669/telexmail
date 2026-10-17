import { parsePing } from '@/lib/count';
import { addCount, countConfig, takeAllowance } from '@/lib/count-store';
import { clientNetwork } from '@/lib/night-shift-ranking';

// The app's daily count (count.rs), sent only by copies whose user said yes. Nothing about the
// request is kept but the sums: no address, no ID. The address is used only for a keyed hash that
// caps one network at COUNTS_PER_DAY, against inflated numbers; that row goes after two days.
const COUNTS_PER_DAY = 200;
const headers = { 'Cache-Control': 'no-store' };

export async function POST(request: Request) {
  // JSON only: a browser can't send that from another site without asking first, and nobody answers.
  if (!request.headers.get('content-type')?.toLowerCase().startsWith('application/json')) {
    return new Response(null, { status: 415, headers });
  }
  const text = await request.text();
  if (text.length > 1000) return new Response(null, { status: 413, headers });
  let body: unknown;
  try { body = JSON.parse(text); } catch { return new Response(null, { status: 400, headers }); }
  const ping = parsePing(body);
  if (!ping) return new Response(null, { status: 400, headers });
  if (!countConfig()) return new Response(null, { status: 503, headers });
  try {
    if (!await takeAllowance('count', clientNetwork(request), COUNTS_PER_DAY, 86400)) {
      // Said as taken: the app would only try again tomorrow anyway.
      return new Response(null, { status: 204, headers });
    }
    await addCount(ping);
  } catch {
    return new Response(null, { status: 503, headers });
  }
  return new Response(null, { status: 204, headers });
}
