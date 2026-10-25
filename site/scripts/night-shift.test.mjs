import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import ts from 'typescript';

// Use the site's existing compiler so these tests also run on Node 20.
async function compileSource(url) {
  const source = await readFile(url, 'utf8');
  let { outputText } = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } });
  for (const match of [...outputText.matchAll(/from ['"](\.[^'"]+)['"]/g)]) {
    const dependency = await compileSource(new URL(`${match[1]}.ts`, url));
    outputText = outputText.replace(match[0], `from '${dependency}'`);
  }
  return `data:text/javascript;base64,${Buffer.from(outputText).toString('base64')}`;
}
async function loadSource(path) { return import(await compileSource(new URL(path, import.meta.url))); }
const { encode } = await loadSource('../lib/ita2.ts');
const { initialShift, multiplier, runMetrics, suggestedLine, SHIFT_SECONDS, shiftReducer } = await loadSource('../lib/night-shift.ts');
const { clientNetwork, HUMAN, issueSession, limitBucket, sameOrigin, verifyRun } = await loadSource('../lib/night-shift-ranking.ts');
const { checkOperator } = await loadSource('../lib/night-shift-names.ts');

const start = (mode = 'shift') => shiftReducer(initialShift, { type: 'start', mode, seed: 42 });
const tick = (state, seconds) => shiftReducer(state, { type: 'tick', seconds });
const copy = (state, value = state.circuits[state.active].message) => shiftReducer(state, { type: 'type', value });
const send = (state) => shiftReducer(state, { type: 'send', seconds: encode(state.circuits[state.active].message).length * 0.15 });

test('a shift opens three distinct telegrams and keeps the initial state reusable', () => {
  const state = start();
  assert.equal(state.phase, 'playing');
  assert.equal(new Set(state.circuits.map((line) => line.message)).size, 3);
  assert.deepEqual(state.circuits.map((line) => line.deadline), [34, 39, 44]);
  assert.equal(initialShift.circuits.length, 0);
  assert.deepEqual(start().circuits, state.circuits);
});

test('incomplete telegrams cannot transmit; lowercase copies normalize', () => {
  let state = copy(start(), 'incomplete');
  assert.equal(send(state), state);
  state = copy(state, state.circuits[0].message.toLowerCase());
  assert.equal(state.circuits[0].draft, state.circuits[0].message);
  assert.notEqual(send(state).circuits[0].sendingUntil, null);
});

test('a corrected typo removes only the clean bonus, and the telegram still delivers', () => {
  let state = copy(start('practice'), '#');
  state = send(copy(state));
  assert.equal(state.streak, 0);
  state = tick(state, 10);
  assert.equal(state.delivered, 1);
  assert.equal(state.clean, 0);
  assert.equal(state.score, 100);
});

test('points arrive after the real ITA2 transmission, including mode shifts', () => {
  let state = send(copy(start()));
  const duration = encode(state.circuits[0].message).length * 0.15;
  assert.equal(state.score, 0);
  assert.equal(tick(state, duration - 0.01).delivered, 0);
  state = tick(state, duration);
  assert.equal(state.delivered, 1);
  assert.equal(state.clean, 1);
  assert.equal(state.score, 218);
  assert.equal(state.circuits[0].draft, '');
  assert.ok(state.circuits[0].ticket > 103);
});

test('all three circuits can transmit concurrently while preserving their drafts', () => {
  let state = start('practice');
  for (let i = 0; i < 3; i++) {
    state = shiftReducer(state, { type: 'select', index: i });
    state = send(copy(state));
  }
  assert.equal(state.circuits.filter((line) => line.sendingUntil !== null).length, 3);
  state = tick(state, 10);
  assert.equal(state.delivered, 3);
  assert.equal(state.clean, 3);
  assert.equal(state.bestStreak, 3);
  assert.equal(state.score, 600);
  assert.equal(multiplier(state.streak), 2);
});

test('pausing freezes deadlines, transmissions and edits; resuming preserves progress', () => {
  let state = send(copy(start()));
  state = tick(state, 1);
  const paused = shiftReducer(state, { type: 'pause' });
  assert.equal(tick(paused, 100), paused);
  assert.equal(copy(paused, 'NO'), paused);
  assert.equal(shiftReducer(paused, { type: 'select', index: 2 }), paused);
  const resumed = shiftReducer(paused, { type: 'resume' });
  assert.equal(resumed.now, 1);
  assert.equal(tick(resumed, 10).delivered, 1);
});

test('missed telegrams resolve chronologically; three misses end the shift immediately', () => {
  const state = tick(start(), 80);
  assert.equal(state.phase, 'finished');
  assert.equal(state.missed, 3);
  assert.equal(state.now, 44);
  assert.equal(state.score, 0);
});

test('a large frame gap completes earlier transmissions before later deadlines', () => {
  const state = tick(send(copy(start())), 45);
  assert.equal(state.delivered, 1);
  assert.equal(state.missed, 3);
  assert.equal(state.phase, 'finished');
  assert.ok(state.score > 0);
});

test('the 90-second limit excludes telegrams that would finish after the shift', () => {
  let state = start();
  state = { ...state, now: SHIFT_SECONDS - 1, circuits: state.circuits.map((line) => ({ ...line, deadline: 200 })) };
  state = tick(send(copy(state)), 20);
  assert.equal(state.now, SHIFT_SECONDS);
  assert.equal(state.phase, 'finished');
  assert.equal(state.delivered, 0);
  assert.equal(state.score, 0);
});

test('practice has no deadlines or automatic ending, and can finish or return to title', () => {
  let state = tick(start('practice'), 10000);
  assert.equal(state.phase, 'playing');
  assert.equal(state.missed, 0);
  state = shiftReducer(state, { type: 'finish' });
  assert.equal(state.phase, 'finished');
  assert.equal(tick(state, 10), state);
  assert.equal(shiftReducer(state, { type: 'reset' }), initialShift);
});

test('invalid selections and invalid timing leave a running shift intact', () => {
  const state = copy(start());
  assert.equal(shiftReducer(state, { type: 'select', index: 3 }), state);
  assert.equal(tick(state, NaN), state);
  assert.equal(tick(state, -2), state);
  assert.equal(shiftReducer(state, { type: 'send', seconds: Infinity }), state);
  assert.equal(multiplier(100), 4);
});

test('reaction starts at first viewing; writing excludes time spent in another city', () => {
  let state = start('practice');
  state = shiftReducer(state, { type: 'type', value: state.circuits[0].message[0], at: 2 });
  state = shiftReducer(state, { type: 'select', index: 1, at: 5 });
  state = shiftReducer(state, { type: 'select', index: 0, at: 12 });
  state = shiftReducer(state, { type: 'type', value: state.circuits[0].message, at: 13 });
  state = shiftReducer(state, { type: 'send', seconds: 3, at: 14 });
  state = tick(state, 3);
  const stats = runMetrics(state);
  assert.equal(stats.reactionMs, 2000);
  assert.equal(stats.writingMs, 5000);
  assert.equal(stats.queueMs, 2000);
  assert.equal(stats.accuracy, 100);
  assert.equal(stats.wpm, Math.round(state.telegrams[0].characters * 2.4 * 10) / 10);
});

test('a waiting city does not count unviewed queue time as reaction time', () => {
  let state = shiftReducer(start('practice'), { type: 'select', index: 1, at: 10 });
  state = shiftReducer(state, { type: 'type', value: state.circuits[1].message, at: 12 });
  state = shiftReducer(state, { type: 'send', seconds: 3, at: 15 });
  state = tick(state, 3);
  assert.equal(runMetrics(state).reactionMs, 2000);
  assert.equal(runMetrics(state).writingMs, 3000);
  assert.equal(runMetrics(state).queueMs, 12000);
});

test('the suggested city is the earliest waiting deadline and excludes busy lines', () => {
  let state = start();
  assert.equal(suggestedLine(state), 0);
  state = send(copy(state));
  assert.equal(suggestedLine(state), 1);
  state = shiftReducer(state, { type: 'select', index: 1 });
  state = send(copy(state));
  assert.equal(suggestedLine(state), 2);
  state = shiftReducer(state, { type: 'select', index: 2 });
  assert.equal(suggestedLine(send(copy(state))), null);
});

test('only delivered copies contribute to weighted accuracy and timing averages', () => {
  let state = copy(start('practice'), '#');
  state = tick(state, 1);
  state = send(copy(state));
  assert.equal(runMetrics(state).characters, 0);
  state = tick(state, 10);
  const { characters, accuracy } = runMetrics(state);
  assert.equal(accuracy, Math.round(characters / (characters + 1) * 1000) / 10);
  assert.equal(state.telegrams.length, 1);
});

const signingSecret = 'unit-test-only-signature-key';
function rankedRun({ firstKey = 2, gap = () => .08, minLength = 0 } = {}) {
  let session, state;
  do {
    session = issueSession(signingSecret, 1000000);
    state = shiftReducer(initialShift, { type: 'start', mode: 'shift', seed: session.seed });
  } while (state.circuits[0].message.length < minLength);
  state = shiftReducer(state, { type: 'advance', at: firstKey });
  const message = state.circuits[0].message;
  for (let i = 1; i <= message.length; i++) state = shiftReducer(state, { type: 'type', value: message.slice(0, i), at: i === 1 ? state.now : state.now + gap(i) });
  state = shiftReducer(state, { type: 'send', seconds: encode(message).length * .15, at: state.now + .08 });
  state = shiftReducer(state, { type: 'advance', at: 90 });
  return { session, state, body: { token: session.token, operator: 'Test Operator', elapsed: state.now, events: state.events }, now: session.issuedAt + state.now * 1000 + 1000 };
}

test('ranked results are replayed from a signed seed and derive their own score and stats', () => {
  const { body, state, now } = rankedRun();
  const row = verifyRun({ ...body, score: 99999999, wpm: 9999 }, signingSecret, now);
  assert.equal(row.score, state.score);
  assert.equal(row.delivered, state.delivered);
  assert.equal(row.wpm, runMetrics(state).wpm);
  assert.equal(row.reaction_ms, runMetrics(state).reactionMs);
  assert.ok(!('events' in row));
});

test('changed signatures, wrong keys and expired sessions are rejected', () => {
  const { body, now } = rankedRun();
  assert.throws(() => verifyRun({ ...body, token: body.token + 'x' }, signingSecret, now), /verified/);
  assert.throws(() => verifyRun(body, 'wrong-test-key', now), /verified/);
  assert.throws(() => verifyRun(body, signingSecret, now + 31 * 60000), /expired/);
});

test('ranked submissions reject pasted text, impossible wall times and early finishes', () => {
  const { body, session, now } = rankedRun();
  const state = shiftReducer(initialShift, { type: 'start', mode: 'shift', seed: session.seed });
  assert.throws(() => verifyRun({ ...body, events: [{ type: 'type', at: 3, value: state.circuits[0].message }] }, signingSecret, now), /each character/);
  assert.throws(() => verifyRun(body, signingSecret, session.issuedAt + 1000), /before its timer/);
  assert.throws(() => verifyRun({ ...body, elapsed: 10 }, signingSecret, now), /Finish a full/);
});

test('ranked submissions reject invalid names, action ordering and unfinished sends', () => {
  const { body, now } = rankedRun();
  assert.throws(() => verifyRun({ ...body, operator: '<script>' }, signingSecret, now), /operator name/);
  assert.throws(() => verifyRun({ ...body, events: [{ type: 'select', at: 3, index: 1 }, { type: 'select', at: 2, index: 0 }] }, signingSecret, now), /out of order/);
  assert.throws(() => verifyRun({ ...body, events: [{ type: 'send', at: 3 }] }, signingSecret, now), /before its copy/);
  assert.throws(() => verifyRun({ ...body, events: [{ type: 'select', at: 3, index: 0.5 }] }, signingSecret, now), /invalid action/);
});

test('origin checks preserve localhost aliases and reject foreign or missing origins', () => {
  assert.equal(sameOrigin(new Request('http://localhost:3000/api/night-shift/session', { headers: { Host: '127.0.0.1:3000', Origin: 'http://127.0.0.1:3000' } })), true);
  assert.equal(sameOrigin(new Request('https://telexmail.app/api/night-shift/session', { headers: { Host: 'telexmail.app', Origin: 'https://unrelated.example' } })), false);
  assert.equal(sameOrigin(new Request('https://telexmail.app/api/night-shift/session')), false);
  assert.equal(sameOrigin(new Request('https://telexmail.app/api/night-shift/session', { headers: { Host: 'telexmail.app', Origin: 'http://telexmail.app' } })), false);
});

test('ranked replays ask for a human pace: reaction, rolled-over keys, bursts', () => {
  const tooFast = /faster than a person/;
  const human = rankedRun({ firstKey: .4, gap: (i) => i % 7 === 0 ? .015 : .11 });
  assert.equal(verifyRun(human.body, signingSecret, human.now).delivered, 1);
  const instant = rankedRun({ firstKey: HUMAN.reactionSeconds / 2 });
  assert.throws(() => verifyRun(instant.body, signingSecret, instant.now), tooFast);
  const rolled = rankedRun({ gap: (i) => i % 2 ? .02 : .2 });
  assert.throws(() => verifyRun(rolled.body, signingSecret, rolled.now), tooFast);
  const burst = rankedRun({ gap: () => .045, minLength: HUMAN.burstKeys + 2 });
  assert.throws(() => verifyRun(burst.body, signingSecret, burst.now), tooFast);
});

test('operator names: charset, length, one alphabet, words in English and Polish', () => {
  for (const name of ['Test Operator', 'Anna Kowalska', 'Łukasz_99', 'Żółw', "O'Brien", 'Ναυτίλος', 'Радист', 'Dickens', 'Hancock', 'Principal', 'Nazir', 'Therapist', 'Grapefruit', 'Fukuda', 'Telex Fan', 'Night Tester', 'Nightingale', 'Anna Kassowa']) {
    assert.ok('name' in checkOperator(name), name);
  }
  assert.deepEqual(checkOperator('  Anna   Nowak '), { name: 'Anna Nowak' });
  for (const name of ['A', 'x'.repeat(21), '<script>', '-Anna', 'Anna--Nowak', '12345', 'Аnna', `Anna${String.fromCharCode(0x200b)}Nowak`, `Anna${String.fromCharCode(0x202e)}`, '👀 Eyes']) {
    assert.ok('error' in checkOperator(name), name);
  }
  for (const name of ['fuck', 'F U C K', 'fuuuck', 'sh1t', 'kurwa', 'KURWA MAĆ', 'k.u.r.w.a', 'chuj', 'pierdolony', 'jebać', 'Admin', 'Telex', 'dick', 'n1gg3r']) {
    assert.ok('error' in checkOperator(name), name);
  }
  assert.throws(() => verifyRun({ ...rankedRun().body, operator: 'k u r w a' }, signingSecret, rankedRun().now), /different operator name/);
});

test('rate limits key a hashed network: IPv4 alone, IPv6 by its /64', () => {
  const at = (headers) => clientNetwork(new Request('https://telexmail.app/api/night-shift/session', { headers }));
  assert.equal(at({ 'x-real-ip': '203.0.113.7' }), '203.0.113.7');
  assert.equal(at({ 'cf-connecting-ip': '198.51.100.4', 'x-forwarded-for': '10.0.0.1' }), '198.51.100.4');
  assert.equal(at({ 'x-forwarded-for': '203.0.113.7, 10.0.0.1' }), '203.0.113.7');
  assert.equal(at({ 'x-real-ip': '2001:db8:0:12::1' }), '2001:db8:0:12::/64');
  assert.equal(at({ 'x-real-ip': '2001:0db8:0000:0012:abcd:1:2:3' }), '2001:db8:0:12::/64');
  assert.equal(at({}), 'local');
  assert.equal(at({ 'x-real-ip': 'not an address' }), 'local');
  const bucket = limitBucket('publish-hour', '203.0.113.7', signingSecret);
  assert.match(bucket, /^publish-hour:[\w-]{22}$/);
  assert.ok(!bucket.includes('203'));
  assert.notEqual(bucket, limitBucket('publish-hour', '203.0.113.8', signingSecret));
  assert.notEqual(bucket, limitBucket('publish-hour', '203.0.113.7', 'another-secret'));
});
