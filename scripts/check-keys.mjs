// The help's keys (KEYS in ui/app.js, between "// keys:start" and "// keys:end") and the keyboard
// table in CLAUDE.md say the same:
//   - every row of the table has at least one key in KEYS that names it (`doc`),
//   - every key's `doc` is a row of the table, and the keys it shows are written in that row,
//   - every key's group is one of KEY_GROUPS, and a key the help can press (`run`) is one key,
//   - the keys in the status bar's hints (ui/index.html) are keys of the help.
//
//   node scripts/check-keys.mjs          check; exit code 1 when they differ

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';

const repo = join(dirname(fileURLToPath(import.meta.url)), '..');
const read = (p) => readFileSync(join(repo, p), 'utf8');
const problems = [];

// The list itself, run on its own with N_ as in app.js.
const app = read('ui/app.js');
const from = app.indexOf('// keys:start'), to = app.indexOf('// keys:end');
if (from < 0 || to < from) {
  console.error('ui/app.js: no "// keys:start" … "// keys:end" around KEY_GROUPS and KEYS.');
  process.exit(1);
}
const { KEY_GROUPS, KEYS } = vm.runInNewContext(`${app.slice(from, to)};({KEY_GROUPS, KEYS})`, { N_: (s) => s });
const line = (r) => `app.js: {g:'${r.g}', k:'${r.k}'}`;

// The table: every row under "## Keyboard" up to the next heading, its key and what it does.
const md = read('CLAUDE.md').replace(/\r\n/g, '\n');
const section = md.slice(md.indexOf('\n## Keyboard\n'), md.indexOf('\n## ', md.indexOf('\n## Keyboard\n') + 5));
const rows = new Map();
for (const l of section.split('\n')) {
  const m = /^\|\s*(.+?)\s*\|\s*(.*?)\s*\|\s*$/.exec(l);
  if (!m || /^-+$/.test(m[1]) || m[1] === 'Key') continue;
  rows.set(m[1], `${m[1]} ${m[2]}`);
}
if (!rows.size) {
  console.error('CLAUDE.md: no keyboard table under "## Keyboard".');
  process.exit(1);
}

// A key is in a row when the row says it: one letter or sign exactly, a longer name (Esc, Menu,
// click) in any case.
const says = (text, key) => (key.length === 1 ? text.includes(key) : text.toLowerCase().includes(key.toLowerCase()));
const groups = new Set(KEY_GROUPS.map(([g]) => g));
for (const r of KEYS) {
  if (!groups.has(r.g)) problems.push(`${line(r)}: no group "${r.g}" in KEY_GROUPS.`);
  if (!r.d) problems.push(`${line(r)}: says nothing (d).`);
  if (r.run && /\s/.test(r.k)) problems.push(`${line(r)}: run, but more than one key.`);
  const text = rows.get(r.doc);
  if (!text) { problems.push(`${line(r)}: doc '${r.doc}' is no row of CLAUDE.md's keyboard table.`); continue; }
  for (const key of r.k.split(/ |\+(?=.)/)) {
    if (!says(text, key)) problems.push(`${line(r)}: "${key}" isn't in the table's row '${r.doc}'.`);
  }
}
for (const doc of rows.keys()) {
  if (!KEYS.some((r) => r.doc === doc)) problems.push(`CLAUDE.md: the row '${doc}' has no key in KEYS (app.js), so the help leaves it out.`);
}

// The status bar's hints show keys the help has.
const html = read('ui/index.html');
const hint = /<span class="hint">([\s\S]*?)<\/span>\s*<span class="st-side st-right">/.exec(html)?.[1] ?? '';
const known = new Set(KEYS.flatMap((r) => r.k.split(/ |\+(?=.)/)));
for (const [, key] of hint.matchAll(/<kbd>([^<]+)<\/kbd>/g)) {
  if (!known.has(key)) problems.push(`index.html: the status bar's hint <kbd>${key}</kbd> is no key of the help.`);
}

console.log(`${KEYS.length} keys in ${KEY_GROUPS.length} groups; ${rows.size} rows in CLAUDE.md's keyboard table.`);
if (problems.length) {
  console.error(`\n${problems.length} out of step:\n  ${problems.join('\n  ')}`);
  process.exit(1);
}
console.log('The help and the keyboard table agree.');
