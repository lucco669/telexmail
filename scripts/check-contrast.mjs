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

// The tag colors, as `tags::COLORS` in crates/telex-core/src/tags.rs.
const TAG_COLORS = ['red', 'orange', 'yellow', 'green', 'teal', 'blue', 'purple', 'gray'];

// [foreground, background, what it is]. Text pairs need the theme's text level, UI pairs 3:1.
const TEXT = [
  ['fg', 'surface', 'Message text, list rows'],
  ['fg', 'panel', 'Folder names, dialogs'],
  ['fg', 'panel', 'The link address strip under a message'],
  ['fg', 'chrome', 'Toolbar and status bar'],
  ['fg', 'hover', 'A row under the pointer'],
  ['fg', 'sel', 'The selected row and folder'],
  ['muted', 'surface', 'Dates and sizes'],
  ['muted', 'panel', 'Labels and notes in dialogs'],
  ['muted', 'panel', 'The full link address in that strip'],
  ['muted', 'chrome', 'Column headings, status bar'],
  ['muted', 'hover', 'Dates under the pointer'],
  ['muted', 'sel', 'Dates in the selected row'],
  ['faint', 'surface', 'Placeholders, image stand-ins'],
  ['faint', 'panel', 'Account addresses, small notes'],
  ['accent', 'surface', 'Links in messages, unread mail in color'],
  ['accent', 'panel', 'Unread counts, links in dialogs'],
  ['accent', 'chrome', 'Sort arrows'],
  ['accent', 'sel', 'Unread count of the open folder, unread mail in color when picked'],
  ['accent', 'hover', 'Unread mail in color under the pointer'],
  ['accent-fg', 'accent', 'Selected row in the focused list, main buttons'],
  ['flag', 'panel', 'The Flagged label'],
  ['warn-fg', 'warn-bg', 'Blocked images bar'],
  ['note-fg', 'note-bg', 'Errors, rule notes in Spam'],
  ['note-fg', 'surface', 'Delete buttons, a failed check in Usage'],
  ['note-fg', 'panel', 'A failed check in Settings › Accounts; warnings in the link strip'],
  ['note-fg', 'panel', 'Elevated and high mail risk scores'],
  ['chip-fg', 'chip-bg', 'Rule notes, the update chip'],
  ...TAG_COLORS.map((c) => [`tag-${c}-fg`, `tag-${c}`, `A ${c} tag`]),
  ['chip-fg', 'img-a', 'Loaded image stand-ins'],
  ['chip-fg', 'img-b', 'Loaded image stand-ins'],
  ['muted', 'panel', 'Invitation card: labels, people who declined or haven’t answered'],
  ['chip-fg', 'chip-bg', 'Invitation card: your answer'],
  ['warn-fg', 'warn-bg', 'Invitation card: a newer version came'],
  ['note-fg', 'note-bg', 'Invitation card: cancelled, or in Spam'],
  ['accent-fg', 'accent', 'Invitation card: the answer chosen'],
  ['faint', 'surface', 'Invitation card: the comment field’s placeholder'],
];
const UI = [
  ['accent', 'line-soft', 'Update download progress'],
  ['field', 'surface', 'Field and button borders'],
  ['field', 'panel', 'Field borders in dialogs'],
  ['accent', 'surface', 'Focus ring, unread dot'],
  ['accent', 'panel', 'Focus ring in dialogs'],
  ['accent', 'chrome', 'Focus ring on the toolbar'],
  ['accent', 'panel', 'The calendar icon on an invitation card'],
  ['accent', 'hover', 'Unread dot under the pointer'],
  ['accent', 'sel', 'Unread dot in the selected row'],
  ['flag', 'surface', 'Flag icon'],
  ['flag', 'hover', 'Flag icon under the pointer'],
  ['flag', 'sel', 'Flag icon in the selected row'],
  ['flag', 'chrome', 'Usage meter, nearly full'],
  ['note-fg', 'chrome', 'Usage meter, full'],
  ['spell', 'surface', 'The wavy line under a spelling mistake'],
  ...TAG_COLORS.flatMap((c) => [
    [`tag-${c}-fg`, 'panel', `A ${c} tag's dot in menus and Settings`],
    [`tag-${c}-fg`, 'surface', `A ${c} tag's dot on a message`],
  ]),
  ...[0, 1, 2, 3, 4, 5].flatMap((i) => [
    [`acct-${i}`, 'panel', `Account ${i + 1}'s dot in the folder tree`],
    [`acct-${i}`, 'surface', `Account ${i + 1}'s dot in All mail`],
  ]),
];
// Text (and with 'ui', marks) on backgrounds the stylesheet mixes from two tokens: [foreground,
// [a, share of a, b, how], what, kind]. "oklab" is color-mix(in oklab, a p%, b); "over" is
// color-mix(…, a p%, transparent) drawn over b, as the browser blends it.
const MIXED = [0, 1, 2, 3, 4, 5].flatMap((i) => {
  const row = [`acct-${i}`, 0.09, 'surface', 'oklab']; // a row of a list that mixes accounts, tinted
  return [
    ['fg', [`acct-${i}`, 0.26, 'surface', 'oklab'], `Account ${i + 1}'s initials`],
    ['fg', [`acct-${i}`, 0.24, 'panel', 'over'], `Account ${i + 1}'s tinted line under the pointer`],
    ['fg', row, `A tinted row of account ${i + 1}`],
    ['muted', row, `Dates and folders in a tinted row of account ${i + 1}`],
    ['accent', row, `Unread mail in color, and its dot, in a tinted row of account ${i + 1}`],
    ['flag', row, `The flag in a tinted row of account ${i + 1}`, 'ui'],
  ];
});
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
// sRGB ⇄ OKLab (Björn Ottosson's matrices), for color-mix(in oklab, …).
const lin = (v) => ((v /= 255) <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
const gam = (v) => Math.round(255 * Math.min(1, Math.max(0, v <= 0.0031308 ? 12.92 * v : 1.055 * v ** (1 / 2.4) - 0.055)));
const toLab = (rgb) => {
  const [r, g, b] = rgb.map(lin);
  const [l, m, s] = [0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b, 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b,
    0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b].map(Math.cbrt);
  return [0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s, 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s];
};
const fromLab = ([L, A, B]) => {
  const [l, m, s] = [L + 0.3963377774 * A + 0.2158037573 * B, L - 0.1055613458 * A - 0.0638541728 * B, L - 0.0894841775 * A - 1.291485548 * B].map((v) => v ** 3);
  return [4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s, -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s].map(gam);
};
const mix = (a, p, b, how) => (how === 'oklab'
  ? fromLab(toLab(a).map((v, i) => v * p + toLab(b)[i] * (1 - p)))
  : a.map((v, i) => Math.round(v * p + b[i] * (1 - p))));

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
  for (const [fg, [a, p, b, how], what, kind = 'text'] of MIXED) {
    const [x, y, z] = [fg, a, b].map((k) => hex(t.tokens[k] ?? ''));
    if (!x || !y || !z) continue; // a missing token is reported above
    const back = mix(y, p, z, how), r = ratio(x, back), need = kind === 'ui' ? 3 : NEED[t.level] ?? 4.5;
    const bg = `${a} ${Math.round(p * 100)}% ${how === 'over' ? 'over' : 'with'} ${b}`;
    const color = '#' + back.map((v) => v.toString(16).padStart(2, '0')).join('');
    pairs.push({ fg, bg, color, what, kind, need, ratio: Math.round(r * 100) / 100 });
    if (r < need) problems.push(`${t.id}: --${fg} on ${bg} is ${r.toFixed(2)}:1, needs ${need}:1 (${what})`);
  }
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
  writeFileSync(file, JSON.stringify({ checked: TEXT.length + MIXED.length + UI.length, themes: data }, null, 2) + '\n');
  console.log(`Wrote ${file}`);
}
if (problems.length) {
  console.error(`\n${problems.length} problem${problems.length === 1 ? '' : 's'}:\n  ` + problems.join('\n  '));
  process.exit(1);
}
const mixedUi = MIXED.filter((m) => m[3] === 'ui').length;
console.log(`\nAll ${results.length} themes pass (${TEXT.length + MIXED.length - mixedUi} text and ${UI.length + mixedUi} UI pairs each).`);
