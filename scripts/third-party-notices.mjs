// The third-party notices Telex ships with: every crate compiled into the app (from Cargo.lock,
// on any platform) with its license, and the fonts in ui/fonts. Each license text is printed once,
// with the components that use it. The installers carry the file next to Telex, and the site
// serves a copy (site/public/third-party-notices.txt), with one of EULA.txt for its License page;
// the installers show EULA.rtf, made from EULA.txt.
//
//   node scripts/third-party-notices.mjs          write THIRD-PARTY-NOTICES.txt and the site's copies
//   node scripts/third-party-notices.mjs --check  exit 1 when any is out of date (CI)

import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const outputs = [join(root, 'THIRD-PARTY-NOTICES.txt'), join(root, 'site/public/third-party-notices.txt')];
const eula = readFileSync(join(root, 'EULA.txt'), 'utf8').replace(/\r\n/g, '\n'), eulaCopy = join(root, 'site/public/eula.txt');
// The installers show the agreement from EULA.rtf: the MSI's tables use code page 1252, which has
// no "Ł", and RTF writes every character outside ASCII as an escape. Headings are bold.
const eulaRtf = join(root, 'EULA.rtf');
const rtfText = (t) => [...t].map((c) => {
  const n = c.codePointAt(0);
  if (c === '\\' || c === '{' || c === '}') return '\\' + c;
  if (n < 128) return c;
  return n > 0xffff ? '?' : `\\u${n > 32767 ? n - 65536 : n}?`;
}).join('');
const rtf = '{\\rtf1\\ansi\\ansicpg1252\\deff0\\uc1{\\fonttbl{\\f0\\fswiss\\fcharset0 Segoe UI;}}\\f0\\fs18\n'
  + eula.trimEnd().split('\n').map((line, i) => (i === 0 || /^\d+\. /.test(line) ? `\\b ${rtfText(line)}\\b0` : rtfText(line)) + '\\par').join('\n')
  + '\n}\n';
const ours = new Set(['telex', 'telex-core', 'telex-i18n', 'telex-security']);

const meta = JSON.parse(execFileSync('cargo', ['metadata', '--format-version', '1', '--locked'], {
  cwd: root, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024,
}));
const byId = new Map(meta.packages.map((p) => [p.id, p]));
const nodes = new Map(meta.resolve.nodes.map((n) => [n.id, n]));

// What the app is built from: the app's normal dependencies, all the way down. Build scripts,
// procedural macros' own dev tools and tests aren't in the app, so dev dependencies are left out.
const app = meta.packages.find((p) => p.name === 'telex');
const included = new Set();
const walk = (id) => {
  if (included.has(id)) return;
  included.add(id);
  for (const dep of nodes.get(id)?.deps ?? []) {
    if (dep.dep_kinds.some((k) => k.kind === null)) walk(dep.pkg);
  }
};
walk(app.id);

const LICENSE_FILE = /^(licen[cs]e|copying|notice|unlicense|copyright)([-._].*)?$/i;
const texts = new Map(); // hash → {text, users: Set}
const normalize = (t) => t.replace(/\r\n?/g, '\n').replace(/[ \t]+$/gm, '').trim();
const keyOf = (t) => createHash('sha256').update(normalize(t).replace(/\s+/g, ' ')).digest('hex').slice(0, 12);
function remember(text, user) {
  const key = keyOf(text);
  if (!texts.has(key)) texts.set(key, { text: normalize(text), users: new Set() });
  texts.get(key).users.add(user);
  return key;
}

const components = [];
for (const id of included) {
  const p = byId.get(id);
  if (!p || ours.has(p.name)) continue;
  const dir = dirname(p.manifest_path);
  const files = readdirSync(dir).filter((f) => LICENSE_FILE.test(f)).sort();
  if (p.license_file && !files.includes(p.license_file)) files.push(p.license_file);
  const label = `${p.name} ${p.version}`;
  const keys = files.filter((f) => existsSync(join(dir, f))).map((f) => remember(readFileSync(join(dir, f), 'utf8'), label));
  components.push({ label, license: p.license ?? 'see its license file', repository: p.repository ?? '', keys });
}
components.sort((a, b) => a.label.localeCompare(b.label, 'en'));

// The fonts in ui/fonts, each family with its OFL text.
const FONTS = [
  ['IBM Plex Sans, IBM Plex Mono', 'OFL.txt'],
  ['Inter', 'OFL-Inter.txt'],
  ['JetBrains Mono', 'OFL-JetBrains-Mono.txt'],
  ['Atkinson Hyperlegible Next', 'OFL-Atkinson-Hyperlegible-Next.txt'],
  ['Atkinson Hyperlegible Mono', 'OFL-Atkinson-Hyperlegible-Mono.txt'],
  ['Source Sans 3', 'OFL-Source-Sans-3.txt'],
  ['Source Code Pro', 'OFL-Source-Code-Pro.txt'],
];
const fonts = FONTS.map(([name, file]) => ({ name, key: remember(readFileSync(join(root, 'ui/fonts', file), 'utf8'), name) }));

// Texts get short labels in the order they are first used.
const order = [];
for (const c of components) for (const k of c.keys) if (!order.includes(k)) order.push(k);
for (const f of fonts) if (!order.includes(f.key)) order.push(f.key);
const label = (k) => `T${order.indexOf(k) + 1}`;

const lines = [
  'THIRD-PARTY NOTICES FOR TELEX',
  '',
  'Telex includes the open-source components and fonts below. Each keeps its own license; the',
  'license texts follow the list, each printed once with the components that use it. Telex itself',
  'is not open source: see EULA.txt. Generated from Cargo.lock by scripts/third-party-notices.mjs.',
  '',
  '== Components ==',
  '',
  ...components.map((c) => `${c.label}: ${c.license}${c.repository ? ` (${c.repository})` : ''}${c.keys.length ? ` [${c.keys.map(label).join(', ')}]` : ''}`),
  '',
  '== Fonts ==',
  '',
  ...fonts.map((f) => `${f.name}: SIL Open Font License 1.1 [${label(f.key)}]`),
  '',
  '== License texts ==',
];
for (const k of order) {
  const { text, users } = texts.get(k);
  const names = [...users];
  lines.push('', `---------- ${label(k)}: ${names.length > 6 ? `${names.slice(0, 6).join(', ')} and ${names.length - 6} more` : names.join(', ')} ----------`, '', text);
}
const notices = lines.join('\n') + '\n';

if (process.argv.includes('--check')) {
  const same = (f, text) => existsSync(f) && readFileSync(f, 'utf8').replace(/\r\n/g, '\n') === text;
  const stale = outputs.filter((f) => !same(f, notices));
  if (!same(eulaCopy, eula)) stale.push(eulaCopy);
  if (!same(eulaRtf, rtf)) stale.push(eulaRtf);
  if (stale.length) {
    console.error(`Out of date: ${stale.map((f) => f.slice(root.length + 1)).join(', ')}. Run: node scripts/third-party-notices.mjs`);
    process.exit(1);
  }
  console.log(`Third-party notices are up to date (${components.length} components, ${fonts.length} fonts).`);
} else {
  for (const f of outputs) writeFileSync(f, notices);
  writeFileSync(eulaCopy, eula);
  writeFileSync(eulaRtf, rtf);
  console.log(`Wrote ${components.length} components, ${fonts.length} fonts and ${order.length} license texts (${Math.round(notices.length / 1024)} KB).`);
}
