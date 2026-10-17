import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import ts from 'typescript';

// The site's compiler, as in night-shift.test.mjs.
async function load(path) {
  const source = await readFile(new URL(path, import.meta.url), 'utf8');
  const { outputText } = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } });
  return import(`data:text/javascript;base64,${Buffer.from(outputText).toString('base64')}`);
}
const count = await load('../lib/count.ts');
const now = new Date('2026-10-17T12:00:00Z'); // a Saturday

const ping = { version: '0.9.11', os: 'windows', lang: 'pl', since: '2026-10', new: true, week: true, month: true };

test('a count says exactly what the app sends', () => {
  assert.deepEqual(count.parsePing(ping, now), ping);
  assert.equal(count.parsePing({ ...ping, id: 'abc' }, now), null, 'no other keys, such as an ID');
  const { lang: _, ...short } = ping;
  assert.equal(count.parsePing(short, now), null);
  assert.equal(count.parsePing({ ...ping, version: '0.9.11-beta' }, now), null);
  assert.equal(count.parsePing({ ...ping, os: 'macos' }, now), null);
  assert.equal(count.parsePing({ ...ping, since: '2026-11' }, now), null, 'not from the future');
  assert.equal(count.parsePing({ ...ping, since: '2025-12' }, now), null, 'not before counting began');
  assert.equal(count.parsePing({ ...ping, week: false }, now), null, 'a first count starts its week');
  assert.equal(count.parsePing({ ...ping, new: 'yes' }, now), null);
  assert.equal(count.parsePing([ping], now), null);
});

const row = (day, extra = {}) => ({ day, version: '0.9.11', os: 'windows', lang: 'en', since: '2026-10', copies: 1, weekly: 0, monthly: 0, new: 0, ...extra });

test('days, weeks and months add up without IDs', () => {
  const rows = [
    // Copy A, new on Monday 12 October, then every day.
    row('2026-10-12', { weekly: 1, monthly: 1, new: 1 }), row('2026-10-13'), row('2026-10-16'), row('2026-10-17'),
    // Copy B: first this week on Saturday, new in September.
    row('2026-10-17', { since: '2026-09', lang: 'pl', weekly: 1, monthly: 1 }),
    // Copy C last week only.
    row('2026-10-08', { weekly: 1, monthly: 1, version: '0.9.10' }),
    row('2026-09-20', { since: '2026-09', weekly: 1, monthly: 1, new: 1 }),
  ];
  const s = count.summariseCopies(rows, now);
  assert.deepEqual(s.day, { today: 2, yesterday: 1 });
  assert.deepEqual(s.week, { now: 2, last: 1 });
  assert.deepEqual(s.month, { now: 3, last: 1 });
  assert.equal(s.newIn30, 2);
  assert.equal(s.daily.length, 60);
  assert.deepEqual(s.daily.at(-1), { day: '2026-10-17', copies: 2, new: 0 });
  assert.deepEqual(s.yesterday.version, { '0.9.11': 1 });
  const september = s.cohorts.find((c) => c.since === '2026-09');
  assert.deepEqual(september, { since: '2026-09', started: 1, active: { '2026-09': 1, '2026-10': 1 } });
});

test('downloads per day come from the growth of GitHub totals', () => {
  const releases = [{
    tag_name: 'v0.9.11', draft: false, published_at: '2026-10-16T10:00:00Z',
    assets: [
      { name: 'Telex_0.9.11_x64-setup.exe', download_count: 10 },
      { name: 'Telex_0.9.11_x64-setup.exe.sig', download_count: 0 },
      { name: 'latest.json', download_count: 40 },
      { name: 'SHA256SUMS', download_count: 2 },
    ],
  }, { tag_name: 'v0.9.12', draft: true, published_at: null, assets: [{ name: 'x.exe', download_count: 99 }] }];
  const [r] = count.releaseDownloads(releases);
  assert.deepEqual(r.kinds, { exe: 10, msi: 0, appimage: 0, deb: 0 });
  assert.equal(r.installers, 10);
  assert.equal(r.latest, 40);
  assert.equal(count.releaseDownloads(releases).length, 1, 'drafts left out');

  const first = count.snapshotRows(releases, '2026-10-16');
  assert.deepEqual(first.map((x) => x.asset), ['Telex_0.9.11_x64-setup.exe', 'latest.json'], 'signatures and sums left out');
  const second = first.map((x) => ({ ...x, day: '2026-10-17', downloads: x.downloads + (x.asset === 'latest.json' ? 7 : 3) }));
  const newFile = { day: '2026-10-17', release: 'v0.9.11', asset: 'Telex_0.9.11_amd64.deb', downloads: 2 };
  assert.deepEqual(count.downloadsPerDay([...first, ...second, newFile]), [{ day: '2026-10-17', installers: 3 + 2, latest: 7 }]);
});

test('ISO weeks start on Monday', () => {
  assert.equal(count.mondayOf('2026-10-17'), '2026-10-12');
  assert.equal(count.mondayOf('2026-10-12'), '2026-10-12');
  assert.equal(count.mondayOf('2026-11-01'), '2026-10-26');
});
