// Every English text in ui/app.js and ui/index.html has its Polish in ui/i18n.js.
// Texts are found where they are written: t('…'), tn(n, '…', …) and N_('…') in app.js, and the
// text, title, placeholder and aria-label of elements marked data-i18n in index.html.
//
//   node scripts/check-i18n.mjs          check; exit code 1 when a translation is missing
//   node scripts/check-i18n.mjs --list   also print every text found

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';

const repo = join(dirname(fileURLToPath(import.meta.url)), '..');
const read = (p) => readFileSync(join(repo, p), 'utf8');

// i18n.js is a plain script for the window: run it with just enough of one.
const sandbox = { window: {}, document: { documentElement: { lang: 'pl' } }, Intl };
vm.runInNewContext(read('ui/i18n.js'), sandbox);
const { PL, PL_N, FROM_CORE } = sandbox.window.TelexI18n.dictionaries;

const found = new Map(); // text -> where
const add = (text, where) => { if (text && !found.has(text)) found.set(text, where); };
const unquote = (s) => s.replace(/\\(.)/g, '$1');

const app = read('ui/app.js');
for (const m of app.matchAll(/(?<![\w$.])(?:t|tn|N_)\(\s*(?:[^,()'"`]+,\s*)?'((?:[^'\\]|\\.)*)'/g)) {
  add(unquote(m[1]), `app.js:${app.slice(0, m.index).split('\n').length}`);
}

const html = read('ui/index.html');
for (const m of html.matchAll(/<(\w+)((?:[^>"]|"[^"]*")*?\bdata-i18n\b(?:[^>"]|"[^"]*")*)>([^<]*)/g)) {
  const [, , attrs, text] = m;
  const line = `index.html:${html.slice(0, m.index).split('\n').length}`;
  add(text.trim(), line);
  for (const a of attrs.matchAll(/\b(?:title|placeholder|aria-label)="([^"]*)"/g)) add(a[1], line);
}

const missing = [...found].filter(([text]) => !(text in PL) && !(text in PL_N));
const used = new Set(found.keys());
const unused = [...Object.keys(PL), ...Object.keys(PL_N)].filter((k) => !used.has(k));

if (process.argv.includes('--list')) for (const [text, where] of found) console.log(`${where.padEnd(16)} ${text}`);
console.log(`${found.size} texts in app.js and index.html; ${Object.keys(PL).length + Object.keys(PL_N).length} Polish entries, ${Object.keys(FROM_CORE).length} more for the core's sentences.`);
if (unused.length) console.log(`\nPolish for texts no longer used (remove them):\n  ${unused.join('\n  ')}`);
if (missing.length) {
  console.error(`\n${missing.length} without Polish:\n  ` + missing.map(([text, where]) => `${where}  ${text}`).join('\n  '));
  process.exit(1);
}
console.log('Every text has its Polish.');
