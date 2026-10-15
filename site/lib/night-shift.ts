// A fictional dispatch challenge. Deadlines and scoring are game rules;
// the component uses the shared ITA2 encoder for actual transmission timing.
export const SHIFT_SECONDS = 90;
export const stations = [
  { city: 'London', identity: '51241 LONDON G', code: 'LDN' },
  { city: 'Paris', identity: '23418 PARIS F', code: 'PAR' },
  { city: 'Warsaw', identity: '82560 WARSZAWA PL', code: 'WAW' },
] as const;

const messages = [
  'NIGHT TRAIN ARRIVES 0215', 'PLEASE CONFIRM 24 BOXES', 'ALL CREW SAFELY ASHORE',
  'HOLD THE MORNING EDITION', 'CARGO READY AT PIER 7', 'THE WEATHER IS CLEAR',
  'RESERVE 2 SEATS MONDAY', 'LIGHTHOUSE BACK ONLINE', 'SEND REPORT BY 0600',
  'SHIP DEPARTS AT DAWN', 'THE BRIDGE IS OPEN', 'MEET AT PLATFORM 3',
  'URGENT PARTS RECEIVED', 'NEW ARRIVAL AT GATE 4', 'THANK YOU AND GOOD NIGHT',
  'FERRY DELAYED 30 MINUTES', 'BRING THE BLUE FOLDER', 'RADIO CHECK COMPLETE',
  'COFFEE FOR THE NIGHT CREW', 'PLEASE CALL AGAIN AT 0900', 'ALL SIGNALS RECEIVED',
  'THE LAST TRAIN IS ON TIME', 'TOMORROW LOOKS BRIGHT', 'KEEP THE LINE OPEN',
];

export type Mode = 'shift' | 'practice';
export type Circuit = {
  ticket: number; message: string; draft: string; deadline: number; allowance: number;
  spoiled: boolean; sendingUntil: number | null; points: number;
  arrivedAt: number; viewedAt: number | null; firstKeyAt: number | null;
  writingSince: number | null; writingSeconds: number; inserted: number; errors: number;
};
export type RunEvent = { at: number } & (
  | { type: 'select'; index: number }
  | { type: 'type'; value: string }
  | { type: 'send' }
);
export type TelegramResult = {
  city: number; characters: number; inserted: number; errors: number;
  reactionSeconds: number; writingSeconds: number; queueSeconds: number;
};
export type ShiftState = {
  phase: 'ready' | 'playing' | 'paused' | 'finished'; mode: Mode; now: number;
  score: number; delivered: number; clean: number; streak: number; bestStreak: number;
  missed: number; active: number; deck: string[]; cursor: number; circuits: Circuit[];
  bulletin: string; receipts: string[];
  telegrams: TelegramResult[]; events: RunEvent[];
  finishReason: 'complete' | 'missed' | 'retired' | null;
};
export type ShiftAction =
  | { type: 'start'; mode: Mode; seed: number }
  | { type: 'tick'; seconds: number }
  | { type: 'advance'; at: number }
  | { type: 'select'; index: number; at?: number }
  | { type: 'type'; value: string; at?: number }
  | { type: 'send'; seconds: number; at?: number }
  | { type: 'pause' | 'resume' | 'finish' | 'reset'; at?: number };

export const initialShift: ShiftState = {
  phase: 'ready', mode: 'shift', now: 0, score: 0, delivered: 0, clean: 0,
  streak: 0, bestStreak: 0, missed: 0, active: 0, deck: [], cursor: 0,
  circuits: [], bulletin: 'Three cities. One night. Keep the words moving.', receipts: [],
  telegrams: [], events: [], finishReason: null,
};

function shuffled(seed: number) {
  const deck = [...messages];
  let random = seed >>> 0;
  for (let i = deck.length - 1; i > 0; i--) {
    random = (Math.imul(random, 1664525) + 1013904223) >>> 0;
    const j = random % (i + 1);
    [deck[i], deck[j]] = [deck[j], deck[i]];
  }
  return deck;
}

function nextCircuit(state: ShiftState, offset = 0): Circuit {
  const allowance = Math.max(18, 34 - state.delivered * 0.7) + offset;
  return {
    ticket: 101 + state.cursor, message: state.deck[state.cursor % state.deck.length],
    draft: '', deadline: state.now + allowance, allowance, spoiled: false,
    sendingUntil: null, points: 0,
    arrivedAt: state.now, viewedAt: null, firstKeyAt: null, writingSince: null,
    writingSeconds: 0, inserted: 0, errors: 0,
  };
}

export function multiplier(streak: number) { return Math.min(4, 1 + Math.floor(streak / 3)); }

export function runMetrics(state: ShiftState) {
  const count = state.telegrams.length;
  const sums = state.telegrams.reduce((total, telegram) => ({
    characters: total.characters + telegram.characters, inserted: total.inserted + telegram.inserted,
    errors: total.errors + telegram.errors, reaction: total.reaction + telegram.reactionSeconds,
    writing: total.writing + telegram.writingSeconds, queue: total.queue + telegram.queueSeconds,
  }), { characters: 0, inserted: 0, errors: 0, reaction: 0, writing: 0, queue: 0 });
  return {
    reactionMs: count ? Math.round(sums.reaction / count * 1000) : null,
    writingMs: count ? Math.round(sums.writing / count * 1000) : null,
    queueMs: count ? Math.round(sums.queue / count * 1000) : null,
    wpm: sums.writing > 0 ? Math.round(sums.characters / 5 / (sums.writing / 60) * 10) / 10 : 0,
    accuracy: sums.inserted ? Math.round((sums.inserted - sums.errors) / sums.inserted * 1000) / 10 : 100,
    characters: sums.characters,
  };
}

export function suggestedLine(state: ShiftState): number | null {
  const waiting = state.circuits.map((line, index) => ({ line, index })).filter(({ line }) => line.sendingUntil === null);
  waiting.sort((a, b) => a.line.deadline - b.line.deadline);
  return waiting[0]?.index ?? null;
}

export function shiftReducer(state: ShiftState, action: ShiftAction): ShiftState {
  if (action.type === 'advance') return action.at > state.now ? shiftReducer(state, { type: 'tick', seconds: action.at - state.now }) : state;
  if ('at' in action && action.at !== undefined && Number.isFinite(action.at) && action.at > state.now) {
    state = shiftReducer(state, { type: 'tick', seconds: action.at - state.now });
  }
  if (action.type === 'reset') return initialShift;
  if (action.type === 'start') {
    const started: ShiftState = {
      ...initialShift, phase: 'playing', mode: action.mode, deck: shuffled(action.seed),
      circuits: [], receipts: [], telegrams: [], events: [], bulletin: 'Copy a telegram, press Enter, then switch to the next waiting city.',
    };
    for (let i = 0; i < stations.length; i++) {
      started.circuits.push(nextCircuit(started, i * 5));
      started.cursor++;
    }
    started.circuits[0].viewedAt = 0;
    return started;
  }
  if (action.type === 'pause') return state.phase === 'playing' ? { ...state, phase: 'paused' } : state;
  if (action.type === 'resume') return state.phase === 'paused' ? { ...state, phase: 'playing' } : state;
  if (action.type === 'finish') return state.phase === 'playing' || state.phase === 'paused' ? { ...state, phase: 'finished', finishReason: 'retired' } : state;
  if (state.phase !== 'playing') return state;
  if (action.type === 'select') {
    if (!Number.isInteger(action.index) || action.index < 0 || action.index >= stations.length || action.index === state.active) return state;
    return {
      ...state, active: action.index, events: [...state.events, { type: 'select', index: action.index, at: state.now }],
      circuits: state.circuits.map((line, index) => {
        if (index === state.active && line.writingSince !== null) return { ...line, writingSeconds: line.writingSeconds + state.now - line.writingSince, writingSince: null };
        if (index === action.index) return { ...line, viewedAt: line.viewedAt ?? state.now, writingSince: line.firstKeyAt !== null && line.sendingUntil === null ? state.now : null };
        return line;
      }),
    };
  }
  const circuit = state.circuits[state.active];
  if (action.type === 'type') {
    if (circuit.sendingUntil !== null) return state;
    const draft = action.value.toUpperCase().slice(0, circuit.message.length + 5);
    if (draft === circuit.draft) return state;
    let prefix = 0;
    while (prefix < draft.length && prefix < circuit.draft.length && draft[prefix] === circuit.draft[prefix]) prefix++;
    let suffix = 0;
    while (suffix < draft.length - prefix && suffix < circuit.draft.length - prefix && draft[draft.length - 1 - suffix] === circuit.draft[circuit.draft.length - 1 - suffix]) suffix++;
    const inserted = draft.length - prefix - suffix;
    const errors = Array.from(draft.slice(prefix, draft.length - suffix)).filter((character, i) => character !== circuit.message[prefix + i]).length;
    return {
      ...state,
      events: [...state.events, { type: 'type', value: draft, at: state.now }],
      circuits: state.circuits.map((line, i) => i === state.active ? {
        ...line, draft, spoiled: line.spoiled || !line.message.startsWith(draft),
        firstKeyAt: inserted ? line.firstKeyAt ?? state.now : line.firstKeyAt,
        writingSince: inserted ? line.writingSince ?? state.now : line.writingSince,
        inserted: line.inserted + inserted, errors: line.errors + errors,
      } : line),
    };
  }
  if (action.type === 'send') {
    if (circuit.sendingUntil !== null || circuit.draft !== circuit.message || !Number.isFinite(action.seconds) || action.seconds <= 0) return state;
    const streak = circuit.spoiled ? 0 : state.streak + 1;
    const bonus = state.mode === 'shift' ? Math.max(0, Math.ceil(circuit.deadline - state.now)) * 2 : 0;
    const points = (100 + bonus + (circuit.spoiled ? 0 : 50)) * multiplier(streak);
    return {
      ...state, streak, bestStreak: Math.max(state.bestStreak, streak),
      events: [...state.events, { type: 'send', at: state.now }],
      bulletin: `${stations[state.active].city} is transmitting. Switch cities now; the other deadlines are still running.`,
      circuits: state.circuits.map((line, i) => i === state.active ? {
        ...line, sendingUntil: state.now + action.seconds, points,
        writingSeconds: line.writingSeconds + (line.writingSince !== null ? state.now - line.writingSince : 0), writingSince: null,
      } : line),
    };
  }
  if (action.type !== 'tick' || !Number.isFinite(action.seconds) || action.seconds <= 0) return state;

  // Resolve events in time order, including simultaneous lines and long frame gaps.
  const until = state.mode === 'shift' ? Math.min(SHIFT_SECONDS, state.now + action.seconds) : state.now + action.seconds;
  let next: ShiftState = { ...state, circuits: [...state.circuits], receipts: [...state.receipts], telegrams: [...state.telegrams] };
  for (;;) {
    const events = next.circuits.map((line) => line.sendingUntil ?? (next.mode === 'shift' ? line.deadline : Infinity));
    const time = Math.min(...events);
    if (time > until || !Number.isFinite(time)) break;
    next.now = time;
    for (let i = 0; i < events.length; i++) {
      if (events[i] !== time) continue;
      const line = next.circuits[i];
      if (line.sendingUntil !== null) {
        next.score += line.points;
        next.delivered++;
        if (!line.spoiled) next.clean++;
        next.telegrams.push({
          city: i, characters: line.message.length, inserted: line.inserted, errors: line.errors,
          reactionSeconds: Math.max(0, (line.firstKeyAt ?? line.arrivedAt) - (line.viewedAt ?? line.arrivedAt)),
          writingSeconds: line.writingSeconds, queueSeconds: (line.firstKeyAt ?? line.arrivedAt) - line.arrivedAt,
        });
        const receipt = `${stations[i].code} / ${line.spoiled ? 'DELIVERED' : 'CLEAN SEND'} / +${line.points}`;
        next.receipts = [receipt, ...next.receipts].slice(0, 3);
        next.bulletin = `${stations[i].city}: delivered. A new telegram is waiting.`;
      } else {
        next.missed++;
        next.streak = 0;
        next.bulletin = `${stations[i].city}: deadline missed. ${3 - next.missed} chances left.`;
      }
      next.circuits[i] = nextCircuit(next);
      if (i === next.active) next.circuits[i].viewedAt = next.now;
      next.cursor++;
      if (next.missed >= 3) return { ...next, phase: 'finished', finishReason: 'missed' };
    }
  }
  next.now = until;
  if (next.mode === 'shift' && until >= SHIFT_SECONDS) { next.phase = 'finished'; next.finishReason = 'complete'; }
  return next;
}
