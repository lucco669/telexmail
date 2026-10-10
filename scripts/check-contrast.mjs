// Checks every theme in ui/themes.css against WCAG 2.2 contrast:
//   text            4.5:1 (level AA) or 7:1 (level AAA) on every background it is drawn on,
//   UI and icons    3:1 for focus rings, field borders, the unread dot and the flag (1.4.11).
// Each theme says its level in its block (`--wcag: AA;` or `--wcag: AAA;`). The script also
// checks that the themes are the ones `THEMES` lists in crates/telex-core/src/settings.rs and
// the settings window in ui/app.js offers.
//
//   node scripts/check-contrast.mjs                 check; exit code 1 when something fails
//   node scripts/check-contrast.mjs --json <file>   also write every measurement as JSON
//                                                   (the website's accessibility page reads it)

import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = join(dirname(fileURLToPath(import.meta.url)), '..');
const read = (p) => readFileSync(join(repo, p), 'utf8');

// [foreground, background, what it is]. Text pairs need the theme's text level, UI pairs 3:1.
const TEXT = [
  ['fg', 'surface', 'Message text, list rows'],
  ['fg', 'panel', 'Folder names, dialogs'],
  ['fg', 'chrome', 'Toolbar and status bar'],
  ['fg', 'hover', 'A row under the pointer'],
  ['fg', 'sel', 'The selected row and folder'],
  ['muted', 'surface', 'Dates and sizes'],
  ['muted', 'panel', 'Labels and notes in dialogs'],
  ['muted', 'chrome', 'Column headings, status bar'],
  ['muted', 'hover', 'Dates under the pointer'],
  ['muted', 'sel', 'Dates in the selected row'],
  ['faint', 'surface', 'Placeholders, image stand-ins'],
  ['faint', 'panel', 'Account addresses, small notes'],
  ['accent', 'surface', 'Links in messages'],
  ['accent', 'panel', 'Unread counts, links in dialogs'],
  ['accent', 'chrome', 'Sort arrows'],
  ['accent', 'sel', 'Unread count of the open folder'],
  ['accent-fg', 'accent', 'Selected row in the focused list, main buttons'],
  ['flag', 'panel', 'The Flagged label'],
  ['warn-fg', 'warn-bg', 'Blocked images bar'],
  ['note-fg', 'note-bg', 'Errors, rule notes in Spam'],
  ['note-fg', 'surface', 'Delete buttons'],
  ['chip-fg', 'chip-bg', 'Rule notes, tags, the update chip'],
  ['chip-fg', 'img-a', 'Loaded image stand-ins'],
  ['chip-fg', 'img-b', 'Loaded image stand-ins'],
];
const UI = [
  ['field', 'surface', 'Field and button borders'],
  ['field', 'panel', 'Field borders in dialogs'],
  ['accent', 'surface', 'Focus ring, unread dot'],
  ['accent', 'panel', 'Focus ring in dialogs'],
  ['accent', 'chrome', 'Focus ring on the toolbar'],
  ['accent', 'hover', 'Unread dot under the pointer'],
  ['accent', 'sel', 'Unread dot in the selected row'],
  ['flag', 'surface', 'Flag icon'],
  ['flag', 'hover', 'Flag icon under the pointer'],
  ['flag', 'sel', 'Flag icon in the selected row'],
  ...[0, 1, 2, 3, 4, 5].flatMap((i) => [
    [`acct-${i}`, 'panel', `Account ${i + 1}'s dot in the folder tree`],
    [`acct-${i}`, 'surface', `Account ${i + 1}'s dot in All mail`],
  ]),
];
const NEED = { AA: 4.5, AAA: 7 };

const hex = (s) => {
  const m = /^#([0-9a-f]{6})$/i.exec(s.trim());
  return m ? [0, 2, 4].map((i) => parseInt(m[1].slice(i, i + 2), 16)) : null;
};
const lum = ([r, g, b]) => {
  const c = (v) => ((v /= 255) <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
  return 0.2126 * c(r) + 0.7152 * c(g) + 0.0722 * c(b);
};
export const ratio = (a, b) => {
  const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
};

// Theme blocks: a selector list with [data-theme="id"], then its custom properties.
function parseThemes(css) {
  const themes = new Map();
  const clean = css.replace(/\/\*[\s\S]*?\*\//g, '');
  for (const [, selector, body] of clean.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const ids = [...selector.matchAll(/\[data-theme="([\w-]+)"\]/g)].map((m) => m[1]);
    if (!ids.length) continue;
    const tokens = {};
    for (const [, name, value] of body.matchAll(/--([\w-]+)\s*:\s*([^;]+);/g)) tokens[name] = value.trim();
    const scheme = /color-scheme\s*:\s*(light|dark)/.exec(body)?.[1];
    for (const id of ids) themes.set(id, { id, scheme, level: tokens.wcag, tokens });
  }
  return themes;
}

const themes = parseThemes(read('ui/themes.css'));
const problems = [];
const results = [];

for (const t of themes.values()) {
  if (!NEED[t.level]) problems.push(`${t.id}: say its level with --wcag: AA; or --wcag: AAA;`);
  if (!t.scheme) problems.push(`${t.id}: set color-scheme to light or dark`);
  const pairs = [];
  const check = (fg, bg, what, need, kind) => {
    const a = hex(t.tokens[fg] ?? ''), b = hex(t.tokens[bg] ?? '');
    if (!a || !b) { problems.push(`${t.id}: --${!a ? fg : bg} is missing or not #rrggbb`); return; }
    const r = ratio(a, b);
    pairs.push({ fg, bg, what, kind, need, ratio: Math.round(r * 100) / 100 });
    if (r < need) problems.push(`${t.id}: --${fg} on --${bg} is ${r.toFixed(2)}:1, needs ${need}:1 (${what})`);
  };
  for (const [fg, bg, what] of TEXT) check(fg, bg, what, NEED[t.level] ?? 4.5, 'text');
  for (const [fg, bg, what] of UI) check(fg, bg, what, 3, 'ui');
  const min = (kind) => Math.min(...pairs.filter((p) => p.kind === kind).map((p) => p.ratio));
  results.push({ ...t, minText: min('text'), minUi: min('ui'), pairs });
}

// The same themes everywhere.
const ids = [...themes.keys()];
const rust = [...(/pub const THEMES: &\[&str\] = &\[([\s\S]*?)\];/.exec(read('crates/telex-core/src/settings.rs'))?.[1] ?? '')
  .matchAll(/"([\w-]+)"/g)].map((m) => m[1]).filter((id) => id !== 'system');
const ui = [...(/const THEMES = \[([\s\S]*?)\];/.exec(read('ui/app.js'))?.[1] ?? '')
  .matchAll(/'([\w-]+)'/g)].map((m) => m[1]).filter((id) => id !== 'system');
for (const [where, list] of [['settings.rs THEMES', rust], ['app.js THEMES', ui]]) {
  if (list.join() !== ids.join()) problems.push(`${where} lists ${list.join(', ')}; themes.css has ${ids.join(', ')}`);
}

for (const r of results) {
  console.log(`${r.id.padEnd(15)} ${r.scheme?.padEnd(6)} ${String(r.level).padEnd(4)} text ≥ ${r.minText.toFixed(2)}:1   ui ≥ ${r.minUi.toFixed(2)}:1`);
}
const out = process.argv.indexOf('--json');
if (out > 0) {
  const file = process.argv[out + 1];
  const data = results.map(({ id, scheme, level, tokens, minText, minUi, pairs }) => ({ id, scheme, level, tokens, minText, minUi, pairs }));
  writeFileSync(file, JSON.stringify({ checked: TEXT.length + UI.length, themes: data }, null, 2) + '\n');
  console.log(`Wrote ${file}`);
}
if (problems.length) {
  console.error(`\n${problems.length} problem${problems.length === 1 ? '' : 's'}:\n  ` + problems.join('\n  '));
  process.exit(1);
}
console.log(`\nAll ${results.length} themes pass (${TEXT.length} text and ${UI.length} UI pairs each).`);
