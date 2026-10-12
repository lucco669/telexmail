// Prepare an isolated build of the real desktop app. Generated files never enter a release.
import { cpSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const work = join(repo, '.bench');
const ui = join(work, 'ui');
mkdirSync(work, { recursive: true });
cpSync(join(repo, 'ui'), ui, { recursive: true });
let source = readFileSync(join(ui, 'app.js'), 'utf8').replaceAll('\r\n', '\n');
const replace = (before, after) => {
  if (!source.includes(before)) throw new Error(`Benchmark hook no longer matches: ${before}`);
  source = source.replace(before, after);
};
// Both comparison clients are offline. No network checks, sending, or real accounts.
replace('if (ACCOUNTS.length) getMail({quiet:true});\n  else openAccountDialog();',
  'if (!ACCOUNTS.length) throw new Error("Benchmark mailbox is empty");');
replace("if (!state.compose) renderPreview();\n}\n// A message's",
  "if (!state.compose) return renderPreview();\n}\n// A message's");
replace('select(row.id);\n  if (reveal(to))', 'await select(row.id);\n  if (reveal(to))');
replace('renderStatus();\n  // The message on screen stays as it is while typing',
  'renderStatus();\n  window.dispatchEvent(new CustomEvent("telex-benchmark-view", {detail:{query:state.q, bodies:state.inText, total:list.total}}));\n  // The message on screen stays as it is while typing');
replace('boot();\n})();', `${readFileSync(join(repo, 'scripts/benchmark/desktop.js'), 'utf8')}\n})();`);
writeFileSync(join(ui, 'app.js'), source);
const identifier = 'app.telexmail.benchmark';
const profile = join(process.env.APPDATA, identifier);
mkdirSync(profile, { recursive: true });
if (existsSync(join(work, 'telex.db'))) cpSync(join(work, 'telex.db'), join(profile, 'telex.db'));
writeFileSync(join(profile, 'settings.json'), JSON.stringify({
  language: 'en', theme: 'light', prefer_text: true, mark_read: 'manual',
  check_every: 0, push_mail: false, notify: false, updates: 'never', backup_every: 'off',
}));
writeFileSync(join(work, 'tauri.conf.json'), JSON.stringify({
  identifier, productName: 'Telex Benchmark',
  // A Windows drive path can deserialize as a URL; Tauri resolves this relative to src-tauri.
  build: { frontendDist: '../.bench/ui' },
  app: { windows: [{ label: 'main', title: 'Telex Benchmark', width: 1280, height: 800, minWidth: 760, minHeight: 480 }] },
}, null, 2));
console.log('Prepared .bench/ui and a separate app.telexmail.benchmark profile.');
