// The live demo: the real app interface (../ui) with a stand-in for its Rust core
// (demo/backend.js) and the English demo mailbox, all in the visitor's browser.
// Run after UI changes, or after changing the mailbox or the backend:  npm run demo
//
// Writes public/demo-app/, which the site serves at /demo-app/index.html (the /demo page shows
// it in a frame). Nothing in it talks to a network: the mailbox is in demo-data.js.

import { copyFileSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { removeGeneratedDirectory } from './generated-files.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const site = resolve(here, '..');
const repo = resolve(site, '..');
const uiDir = join(repo, 'ui');
const out = join(site, 'public', 'demo-app');

const mailbox = JSON.parse(readFileSync(join(here, 'demo-mailbox.json'), 'utf8'));
const version = JSON.parse(readFileSync(join(repo, 'src-tauri', 'tauri.conf.json'), 'utf8')).version;

// Start clean, so files the UI no longer has don't linger.
removeGeneratedDirectory(out, join(site, 'public'));
mkdirSync(join(out, 'fonts'), { recursive: true });

for (const file of ['app.js', 'links.js', 'accent.js', 'i18n.js', 'boot.js', 'styles.css', 'themes.css', 'fonts.css', 'telex-mark.svg']) copyFileSync(join(uiDir, file), join(out, file));
mkdirSync(join(out, 'i18n'));
for (const file of readdirSync(join(uiDir, 'i18n'))) if (file.endsWith('.js')) copyFileSync(join(uiDir, 'i18n', file), join(out, 'i18n', file));
for (const file of readdirSync(join(uiDir, 'fonts'))) {
  if (file.endsWith('.woff2') || /^OFL.*\.txt$/.test(file)) copyFileSync(join(uiDir, 'fonts', file), join(out, 'fonts', file));
}
copyFileSync(join(site, 'demo', 'backend.js'), join(out, 'backend.js'));
writeFileSync(join(out, 'demo-data.js'),
  `// The demo mailbox (site/scripts/demo-mailbox.json) and the app version, for backend.js.\n`
  + `window.TELEX_DEMO_DATA = ${JSON.stringify({ version, mailbox })};\n`);

// The page: its title, kept out of search results, and the stand-in loaded before boot.js
// (which reads the look the stand-in saves).
const html = readFileSync(join(uiDir, 'index.html'), 'utf8');
const page = html
  .replace(/<title>[^<]*<\/title>/, '<title>Telex – live demo</title>\n  <meta name="robots" content="noindex">')
  .replace('<script src="boot.js"></script>', '<script src="demo-data.js"></script><script src="backend.js"></script><script src="boot.js"></script>');
for (const [what, ok] of [['title', page.includes('Telex – live demo')], ['backend', page.includes('src="backend.js"')]]) {
  if (!ok) throw new Error(`ui/index.html changed: couldn't put the ${what} in. Update scripts/demo.mjs.`);
}
writeFileSync(join(out, 'index.html'), page);

// What was written.
const walk = (dir) => readdirSync(dir).flatMap((f) => (statSync(join(dir, f)).isDirectory() ? walk(join(dir, f)) : [join(dir, f)]));
let total = 0;
for (const file of walk(out).sort()) {
  const size = statSync(file).size;
  total += size;
  console.log(`${relative(site, file).replaceAll('\\', '/').padEnd(68)} ${(size / 1024).toFixed(1).padStart(7)} KB`);
}
console.log(`${walk(out).length} files, ${(total / 1024).toFixed(1)} KB in public/demo-app (Telex ${version}).`);
