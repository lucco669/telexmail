// Every text of Telex in every language it speaks.
//
// The interface: every English text in ui/app.js and ui/index.html has its Polish in ui/i18n.js,
// and its German, Czech… in ui/i18n/<code>.js. Texts are found where they are written: t('…'),
// tn(n, '…', …) and N_('…') in app.js, and the text, title, placeholder and aria-label of
// elements marked data-i18n in index.html.
// The core: every tr!("English", "Polski") and trn!(n, …) in crates/telex-core and src-tauri has
// its entry in crates/telex-core/i18n/<code>.json (keyed by the English, the first form for trn!).
//
// Missing Polish always fails (CI). The other languages are only reported, since new texts are
// written in English and Polish first; `--all` fails on them too (before a release,
// docs/releasing.md). A translation that loses or invents a placeholder ({name}, {n}) always
// fails, in any language, as does a count without the plural forms its language needs.
//
//   node scripts/check-i18n.mjs          check; exit code 1 when Polish is missing or a translation is wrong
//   node scripts/check-i18n.mjs --all    also exit code 1 when any language lacks a text
//   node scripts/check-i18n.mjs --list   also print every text found, and each language's missing texts
//   node scripts/check-i18n.mjs --json   print what each language lacks as JSON, for translating it

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';

const repo = join(dirname(fileURLToPath(import.meta.url)), '..');
const read = (p) => readFileSync(join(repo, p), 'utf8');
const args = process.argv.slice(2);
const json = args.includes('--json');
const log = json ? () => {} : console.log;

// i18n.js and the language files are plain scripts for the window: run them with just enough of one.
const sandbox = { window: {}, document: { documentElement: { lang: 'pl' } }, Intl };
for (const f of readdirSync(join(repo, 'ui/i18n')).filter((f) => f.endsWith('.js')).sort()) {
  vm.runInNewContext(read('ui/i18n/' + f), sandbox);
}
vm.runInNewContext(read('ui/i18n.js'), sandbox);
const { PL, PL_N, others } = sandbox.window.TelexI18n.dictionaries;
const LANGS = sandbox.window.TelexI18n.languages.map(([code]) => code).filter((c) => c !== 'en' && c !== 'pl');

let failed = false;
const fail = (title, lines) => { if (!lines.length) return; failed = true; console.error(`\n${title}:\n  ${lines.join('\n  ')}`); };

// ---- The interface ----

const found = new Map(); // text -> where
const add = (text, where) => { if (text && !found.has(text)) found.set(text, where); };
const unquote = (s) => s.replace(/\\(.)/g, '$1');

const app = read('ui/app.js');
const englishOther = new Map(); // tn's first English form -> its second
for (const m of app.matchAll(/(?<![\w$.])(?:t|tn|N_)\(\s*(?:[^,()'"`]+,\s*)?'((?:[^'\\]|\\.)*)'(?:\s*,\s*'((?:[^'\\]|\\.)*)')?/g)) {
  add(unquote(m[1]), `app.js:${app.slice(0, m.index).split('\n').length}`);
  if (m[2] !== undefined && m[0].startsWith('tn')) englishOther.set(unquote(m[1]), unquote(m[2]));
}

const html = read('ui/index.html');
for (const m of html.matchAll(/<(\w+)((?:[^>"]|"[^"]*")*?\bdata-i18n\b(?:[^>"]|"[^"]*")*)>([^<]*)/g)) {
  const [, , attrs, text] = m;
  const line = `index.html:${html.slice(0, m.index).split('\n').length}`;
  add(text.trim(), line);
  for (const a of attrs.matchAll(/\b(?:title|placeholder|aria-label)="([^"]*)"/g)) add(a[1], line);
}

const english = (key) => key.split('|')[0];
const holes = (s) => [...new Set([...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]))].sort().join(',');
// A count's form for 1 may say "a minute" instead of "{n} minute", as the Polish "Co minutę" does.
const holesOk = (key, text, counted) => holes(english(key)) === holes(text)
  || (counted && holes(english(key).replaceAll('{n}', '')) === holes(text.replaceAll('{n}', '')));
// The categories a language's whole numbers take (Intl.PluralRules), which a count must have.
const categories = (lang) => { const r = new Intl.PluralRules(lang); return [...new Set(Array.from({ length: 200 }, (_, n) => r.select(n)))]; };

const uiMissing = {};
const wrong = [];
for (const [lang, table, counts] of [['pl', PL, PL_N], ...LANGS.map((l) => [l, others[l]?.T ?? {}, others[l]?.N ?? {}])]) {
  uiMissing[lang] = [...found].filter(([text]) => !(text in table) && !(text in counts));
  for (const [key, text] of Object.entries(table)) {
    if (typeof text !== 'string') wrong.push(`${lang}: ${JSON.stringify(key)} is not a text`);
    else if (!holesOk(key, text, false)) wrong.push(`${lang}: ${JSON.stringify(key)} → ${JSON.stringify(text)}`);
  }
  const needed = lang === 'pl' ? null : categories(lang);
  for (const [key, forms] of Object.entries(counts)) {
    const list = lang === 'pl' ? forms : Object.values(forms);
    if (lang === 'pl' && (!Array.isArray(forms) || forms.length !== 3)) wrong.push(`pl: ${JSON.stringify(key)} needs three forms`);
    if (needed) for (const c of needed) if (typeof forms[c] !== 'string' && typeof forms.other !== 'string') wrong.push(`${lang}: ${JSON.stringify(key)} lacks the "${c}" form`);
    for (const text of list) if (typeof text === 'string' && !holesOk(key, text, true)) wrong.push(`${lang}: ${JSON.stringify(key)} → ${JSON.stringify(text)}`);
  }
}
const used = new Set(found.keys());
const unusedPl = [...Object.keys(PL), ...Object.keys(PL_N)].filter((k) => !used.has(k));

if (args.includes('--list')) for (const [text, where] of found) log(`${where.padEnd(16)} ${text}`);
log(`${found.size} texts in app.js and index.html; ${Object.keys(PL).length + Object.keys(PL_N).length} Polish entries.`);
if (unusedPl.length) log(`\nPolish for texts no longer used (remove them):\n  ${unusedPl.join('\n  ')}`);
for (const lang of LANGS) {
  const unused = [...Object.keys(others[lang]?.T ?? {}), ...Object.keys(others[lang]?.N ?? {})].filter((k) => !used.has(k));
  if (unused.length) log(`\n${lang}: texts no longer used (remove them from ui/i18n/${lang}.js):\n  ${unused.join('\n  ')}`);
}

// ---- The core ----

// The text a Rust string literal stands for.
const unescapeRust = (s) => s.replace(/\\\r?\n\s*/g, '').replace(/\\(u\{[0-9a-fA-F]+\}|x[0-9a-fA-F]{2}|.)/g, (m, c) => {
  if (c.startsWith('u{')) return String.fromCodePoint(parseInt(c.slice(2, -1), 16));
  if (c[0] === 'x' && c.length === 3) return String.fromCharCode(parseInt(c.slice(1), 16));
  return { n: '\n', t: '\t', r: '\r', 0: '\0' }[c] ?? c;
});
const STR = String.raw`"((?:[^"\\]|\\[\s\S])*)"`;
const sentences = new Map(); // English -> {pl: [Polish…], where, counted}
const rust = [];
const walk = (d) => {
  for (const f of readdirSync(d)) {
    const p = join(d, f);
    if (statSync(p).isDirectory()) walk(p);
    else if (p.endsWith('.rs')) rust.push(p);
  }
};
walk(join(repo, 'crates/telex-core/src'));
walk(join(repo, 'src-tauri/src'));
// Tests aren't translated: test files, the sample mailbox, a file's test module (at its end) and comments.
for (const file of rust.filter((f) => !/[\\/](tests|sample)\.rs$/.test(f))) {
  const whole = readFileSync(file, 'utf8');
  const testAt = whole.search(/^#\[cfg\(test\)\]\s*\r?\nmod /m);
  const src = (testAt < 0 ? whole : whole.slice(0, testAt)).replace(/^[ \t]*\/\/.*$/gm,(c) => ' '.repeat(c.length));
  const where = (i) => `${relative(repo, file).replaceAll('\\', '/')}:${src.slice(0, i).split('\n').length}`;
  for (const m of src.matchAll(new RegExp(String.raw`\btr!\(\s*${STR}\s*,\s*${STR}`, 'g'))) {
    const en = unescapeRust(m[1]);
    if (!sentences.has(en)) sentences.set(en, { pl: [unescapeRust(m[2])], where: where(m.index), counted: false });
  }
  // trn!(count, "one", "other", "pl one", "pl few", "pl many"): the count may hold commas only in brackets.
  for (const m of src.matchAll(new RegExp(String.raw`\btrn!\(\s*(?:[^,"()]|\([^()]*\))+,\s*${STR}\s*,\s*${STR}\s*,\s*${STR}\s*,\s*${STR}\s*,\s*${STR}`, 'g'))) {
    const en = unescapeRust(m[1]);
    if (!sentences.has(en)) sentences.set(en, { other: unescapeRust(m[2]), pl: [3, 4, 5].map((i) => unescapeRust(m[i])), where: where(m.index), counted: true });
  }
}

// What a Rust format string takes: names ({host}, {n:>3}), how many positional arguments ({}, {0}).
function rustHoles(s) {
  const names = new Set(); let positional = 0;
  for (const m of s.replace(/\{\{|\}\}/g, '').matchAll(/\{([^{}]*)\}/g)) {
    const arg = m[1].split(':')[0];
    if (arg === '' || /^\d+$/.test(arg)) positional++; else names.add(arg);
  }
  return { names, positional };
}
const catalogs = {};
const coreMissing = {};
for (const lang of LANGS) {
  let catalog = {};
  try { catalog = JSON.parse(read(`crates/telex-core/i18n/${lang}.json`)); } catch (e) { wrong.push(`${lang}.json: ${e.message}`); }
  catalogs[lang] = catalog;
  coreMissing[lang] = [...sentences].filter(([en]) => !(en in catalog)).map(([en, s]) => [en, s.where]);
  for (const [en, value] of Object.entries(catalog)) {
    const s = sentences.get(en);
    if (!s) continue;
    const texts = typeof value === 'string' ? [value] : Object.values(value ?? {});
    if (s.counted && typeof value === 'object') for (const c of categories(lang)) if (typeof value[c] !== 'string' && typeof value.other !== 'string') wrong.push(`${lang}.json: ${JSON.stringify(en)} lacks the "${c}" form`);
    // A translation may use what the English or the Polish uses (both are variables in scope),
    // must keep what both use, and takes as many positional arguments as the English.
    const a = rustHoles(en), p = s.pl.map(rustHoles);
    const allowed = new Set([...a.names, ...p.flatMap((h) => [...h.names]), ...(s.other ? rustHoles(s.other).names : [])]);
    const kept = [...a.names].filter((n) => p.every((h) => h.names.has(n)));
    for (const text of texts) {
      const h = rustHoles(text);
      const extra = [...h.names].filter((n) => !allowed.has(n));
      const lost = kept.filter((n) => !h.names.has(n) && !(s.counted && n === 'n'));
      if (typeof text !== 'string' || extra.length || lost.length || h.positional !== a.positional) {
        wrong.push(`${lang}.json: ${JSON.stringify(en)} → ${JSON.stringify(text)}`);
      }
    }
  }
  const unused = Object.keys(catalog).filter((k) => !sentences.has(k));
  if (unused.length) log(`\n${lang}.json: sentences no longer in the code (remove them):\n  ${unused.join('\n  ')}`);
}
log(`${sentences.size} sentences of the core in tr! and trn!.`);

// ---- Verdict ----

if (json) {
  const out = {};
  for (const lang of LANGS) {
    out[lang] = {
      ui: uiMissing[lang].filter(([text]) => !(text in PL_N) && !englishOther.has(text)).map(([en, where]) => ({ en, pl: PL[en] ?? null, where })),
      plural: uiMissing[lang].filter(([text]) => text in PL_N || englishOther.has(text)).map(([en, where]) => ({ en_one: en, en_other: englishOther.get(en) ?? en, pl: PL_N[en] ?? null, where })),
      core: coreMissing[lang].map(([en, where]) => ({ en, ...(sentences.get(en).counted ? { en_other: sentences.get(en).other } : {}), pl: sentences.get(en).pl, where })),
    };
  }
  console.log(JSON.stringify(out, null, 1));
}
fail('Translations whose placeholders or forms are wrong', wrong);
fail(`${uiMissing.pl.length} without Polish`, uiMissing.pl.map(([text, where]) => `${where}  ${text}`));
const all = args.includes('--all');
const report = [];
for (const lang of LANGS) {
  const n = uiMissing[lang].length + coreMissing[lang].length;
  if (!n) continue;
  report.push(`${lang}: ${uiMissing[lang].length} of the interface, ${coreMissing[lang].length} of the core`);
  if (args.includes('--list') || all) {
    for (const [text, where] of [...uiMissing[lang], ...coreMissing[lang]]) report.push(`    ${where}  ${text}`);
  }
}
if (report.length) {
  if (all) fail('Missing in other languages', report);
  else log(`\nNot translated yet (English shows instead; --list names them, --all fails on them):\n  ${report.join('\n  ')}`);
}
if (failed) process.exit(1);
log(report.length ? '\nEvery text has its Polish.' : '\nEvery text has its Polish, and every language is complete.');
