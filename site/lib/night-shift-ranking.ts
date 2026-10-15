import { createHmac, randomInt, randomUUID, timingSafeEqual } from 'node:crypto';
import { encode } from './ita2';
import { initialShift, runMetrics, shiftReducer, type RunEvent } from './night-shift';
import { checkOperator } from './night-shift-names';

// Human limits for scored shifts. The game is deterministic from its seed, so a script could
// replay a perfect run; these make it type at a person's pace. Fast typists roll keys over, so
// a few very short gaps are allowed, but not a whole telegram of them.
export const HUMAN = {
  reactionSeconds: 0.1, // a new telegram is on screen this long before its first key
  fastGapSeconds: 0.03, // keystrokes closer than this count as rolled over...
  fastGapShare: 0.1, // ...and may be at most this share of them (or 4)
  burstKeys: 20, // keystrokes in any one second (240 WPM)
  maxWpm: 250, // over the whole shift
};

export type RankedSession = { id: string; seed: number; issuedAt: number };
export type LeaderboardEntry = {
  id: string; operator: string; score: number; delivered: number; clean: number;
  best_streak: number; missed: number; reaction_ms: number; writing_ms: number;
  queue_ms: number; wpm: number; accuracy: number; duration_seconds: number; created_at: string;
};

export function sameOrigin(request: Request) {
  try {
    const origin = new URL(request.headers.get('origin') ?? '');
    // Next's development adapter can normalize request.url to localhost while
    // the browser uses 127.0.0.1. Host retains the actual incoming origin.
    const host = request.headers.get('host') ?? new URL(request.url).host;
    const protocol = request.headers.get('x-forwarded-proto')?.split(',')[0].trim() ?? new URL(request.url).protocol.slice(0, -1);
    return ['http:', 'https:'].includes(origin.protocol) && origin.protocol === `${protocol}:` && origin.host === host;
  } catch { return false; }
}

export function issueSession(secret: string, now = Date.now()) {
  const session: RankedSession = { id: randomUUID(), seed: randomInt(0, 0x7fffffff), issuedAt: now };
  const payload = Buffer.from(JSON.stringify(session)).toString('base64url');
  return { ...session, token: `${payload}.${createHmac('sha256', secret).update(payload).digest('base64url')}` };
}

function readSession(token: unknown, secret: string, now: number): RankedSession {
  if (typeof token !== 'string' || token.length > 500) throw new Error('Start a new scored shift before publishing.');
  const [payload, signature, extra] = token.split('.');
  if (!payload || !signature || extra) throw new Error('This shift could not be verified.');
  const expected = createHmac('sha256', secret).update(payload).digest();
  const received = Buffer.from(signature, 'base64url');
  if (received.length !== expected.length || !timingSafeEqual(received, expected)) throw new Error('This shift could not be verified.');
  const session = JSON.parse(Buffer.from(payload, 'base64url').toString()) as RankedSession;
  if (!session || !Number.isInteger(session.seed) || !Number.isFinite(session.issuedAt) || typeof session.id !== 'string' || !/^[0-9a-f-]{36}$/.test(session.id)) throw new Error('This shift could not be verified.');
  if (now < session.issuedAt || now - session.issuedAt > 30 * 60 * 1000) throw new Error('This shift has expired. Play a new one to publish.');
  return session;
}

export function verifyRun(body: unknown, secret: string, now = Date.now()): LeaderboardEntry {
  if (!body || typeof body !== 'object') throw new Error('The run is missing.');
  const input = body as { token?: unknown; operator?: unknown; elapsed?: unknown; events?: unknown };
  const session = readSession(input.token, secret, now);
  const checked = checkOperator(input.operator);
  if ('error' in checked) throw new Error(checked.error);
  const operator = checked.name;
  if (typeof input.elapsed !== 'number' || !Number.isFinite(input.elapsed) || input.elapsed <= 0 || input.elapsed > 90) throw new Error('This shift has an invalid duration.');
  if (input.elapsed * 1000 > now - session.issuedAt + 750) throw new Error('The shift finished before its timer could run.');
  if (!Array.isArray(input.events) || input.events.length > 2000) throw new Error('The typing record is invalid.');
  let state = shiftReducer(initialShift, { type: 'start', mode: 'shift', seed: session.seed });
  let last = 0;
  const keys: number[] = [];
  for (const raw of input.events) {
    if (!raw || typeof raw !== 'object') throw new Error('The typing record is invalid.');
    const event = raw as RunEvent;
    if (typeof event.at !== 'number' || !Number.isFinite(event.at) || event.at < last || event.at > input.elapsed) throw new Error('The typing record is out of order.');
    state = shiftReducer(state, { type: 'advance', at: event.at });
    if (state.phase !== 'playing') throw new Error('The typing record continues after the shift ended.');
    if (event.type === 'type') {
      if (typeof event.value !== 'string' || event.value.length > 40 || !/^[A-Z0-9 ]*$/.test(event.value)) throw new Error('The typing record contains unsupported characters.');
      const line = state.circuits[state.active];
      // A scored shift uses individual keystrokes, never an entire pasted telegram.
      let prefix = 0;
      while (prefix < event.value.length && prefix < line.draft.length && event.value[prefix] === line.draft[prefix]) prefix++;
      let suffix = 0;
      while (suffix < event.value.length - prefix && suffix < line.draft.length - prefix && event.value[event.value.length - 1 - suffix] === line.draft[line.draft.length - 1 - suffix]) suffix++;
      if (event.value.length - prefix - suffix > 1) throw new Error('Scored shifts require typing each character. Try practice for pasted text.');
      state = shiftReducer(state, { type: 'type', value: event.value });
      keys.push(event.at);
    } else if (event.type === 'select' && Number.isInteger(event.index) && event.index >= 0 && event.index <= 2) {
      state = shiftReducer(state, { type: 'select', index: event.index });
    } else if (event.type === 'send') {
      const line = state.circuits[state.active];
      if (line.sendingUntil !== null || line.draft !== line.message) throw new Error('A telegram was sent before its copy was complete.');
      state = shiftReducer(state, { type: 'send', seconds: encode(line.message).length * .15 });
    } else throw new Error('The typing record contains an invalid action.');
    last = event.at;
  }
  state = shiftReducer(state, { type: 'advance', at: input.elapsed });
  if (state.phase !== 'finished' || state.finishReason === 'retired' || state.delivered < 1 || Math.abs(state.now - input.elapsed) > .001) throw new Error('Finish a full scored shift, or reach three missed deadlines, before publishing.');
  humanPace(keys, state.telegrams.map((telegram) => telegram.reactionSeconds));
  const metrics = runMetrics(state);
  if (metrics.wpm <= 0 || metrics.wpm > HUMAN.maxWpm || metrics.reactionMs === null || metrics.writingMs === null) throw new Error('The typing timings could not be verified.');
  return {
    id: session.id, operator, score: state.score, delivered: state.delivered, clean: state.clean,
    best_streak: state.bestStreak, missed: state.missed, reaction_ms: metrics.reactionMs,
    writing_ms: metrics.writingMs, queue_ms: metrics.queueMs!, wpm: metrics.wpm,
    accuracy: metrics.accuracy, duration_seconds: Math.round(state.now * 1000) / 1000, created_at: new Date(now).toISOString(),
  };
}

function humanPace(keys: number[], reactions: number[]) {
  const tooFast = 'This shift was typed faster than a person can. Scored shifts need live typing.';
  if (reactions.some((seconds) => seconds < HUMAN.reactionSeconds)) throw new Error(tooFast);
  let fast = 0;
  for (let i = 1; i < keys.length; i++) if (keys[i] - keys[i - 1] < HUMAN.fastGapSeconds) fast++;
  if (fast > Math.max(4, keys.length * HUMAN.fastGapShare)) throw new Error(tooFast);
  for (let i = HUMAN.burstKeys; i < keys.length; i++) if (keys[i] - keys[i - HUMAN.burstKeys] < 1) throw new Error(tooFast);
}

// The caller's network, for rate limits: an IPv4 address, or an IPv6 /64 (one home or phone
// usually has a whole /64). Vercel sets these headers itself; elsewhere they fall back to "local".
export function clientNetwork(request: Request) {
  const raw = (request.headers.get('x-real-ip') ?? request.headers.get('x-forwarded-for')?.split(',')[0] ?? '').trim().toLowerCase();
  if (/^\d{1,3}(\.\d{1,3}){3}$/.test(raw)) return raw;
  if (/^[0-9a-f:.]+$/.test(raw) && raw.includes(':')) {
    const [head, tail = ''] = raw.split('::');
    const left = head ? head.split(':') : [];
    const right = tail ? tail.split(':') : [];
    const groups = raw.includes('::') ? [...left, ...Array(Math.max(0, 8 - left.length - right.length)).fill('0'), ...right] : left;
    return `${groups.slice(0, 4).map((group) => group.replace(/^0+(?=.)/, '')).join(':')}::/64`;
  }
  return 'local';
}

// A rate-limit bucket that doesn't store the address: a keyed hash of the network and action.
export function limitBucket(action: string, network: string, secret: string) {
  return `${action}:${createHmac('sha256', secret).update(`night-shift-limit:${network}`).digest('base64url').slice(0, 22)}`;
}
