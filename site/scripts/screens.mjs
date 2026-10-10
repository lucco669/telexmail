// Screenshots for the site, made from the real app UI (../ui) with the built-in sample mailbox.
// Run after UI changes:  npm run screens
//
// The UI normally talks to Rust through Tauri's `invoke`. Here a small stand-in answers from
// crates/telex-core/src/sample.json instead, and headless Edge (or Chrome) takes the picture.
// Set BROWSER to a browser executable if yours isn't found automatically.

import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, '../..');
const uiDir = join(repo, 'ui');
const sample = readFileSync(join(repo, 'crates/telex-core/src/sample.json'), 'utf8');
const version = JSON.parse(readFileSync(join(repo, 'src-tauri/tauri.conf.json'), 'utf8')).version;
const outDir = join(here, '../public/screens');
const W = 1280, H = 800, SCALE = 2;

// Each scene runs after the app has booted. `ui` has small helpers, see SCENE_HELPERS.
const scenes = {
  inbox: '',
  reply: `ui.key('r'); await ui.wait(150);`,
  filter: `await ui.type('stron'); await ui.wait(300);`,
  images: `ui.row('View Transitions'); await ui.wait(100);`,
  spam: `ui.folder('firma/spam'); await ui.wait(50); ui.row('TOP 1 w Google'); await ui.wait(100);`,
  comfortable: `ui.click('#b-density'); ui.folder('gmail/inbox'); await ui.wait(100); ui.list.focus();`,
  account: `ui.click('[data-act="add-account"]'); await ui.wait(50);
    ui.fill('w-name', 'Anna Kowalska'); ui.fill('w-email', 'biuro@twojastudio.pl'); ui.fill('w-pass', 'password');
    document.getElementById('w-email').focus(); await ui.wait(100);`,
};

// A fixed "now" keeps the list's relative dates (06:12, Fri 18:31) the same on every run.
const MOCK = `
(() => {
  const RealDate = Date, NOW = new RealDate(2026, 9, 10, 9, 30).getTime();
  window.Date = class extends RealDate {
    constructor(...a){ a.length ? super(...a) : super(NOW); }
    static now(){ return NOW; }
  };
  const data = ${sample};
  const msgs = data.messages.map((m, i) => ({...m, id: i + 1}));
  const fold = s => s.normalize('NFD').replace(/\\p{M}/gu, '').replace(/ł/g, 'l').toLowerCase();
  const summary = m => ({id:m.id, folder:m.folder, from_name:m.from_name, from_email:m.from_email, to:m.to,
    subject:m.subject, date:m.date, size_kb:m.size_kb, unread:m.unread, flagged:m.flagged,
    has_attachments:m.attachments.length > 0});
  const byId = id => msgs.find(m => m.id === id);
  const commands = {
    snapshot: () => ({accounts: data.accounts, messages: msgs.map(summary), sample: true}),
    get_message: ({id}) => byId(id),
    set_unread: ({id, unread}) => { byId(id).unread = unread; },
    set_flagged: ({id, flagged}) => { byId(id).flagged = flagged; },
    search: ({query}) => msgs.filter(m => fold([m.subject, m.from_name, m.to, m.body].join(' ')).includes(fold(query))).map(m => m.id),
    check_mail: () => ({new: [], errors: [], busy: false}),
    push_changes: () => ({errors: [], busy: false}),
    // The version number shows in the status bar; "can't update" keeps the one-time question away.
    update_info: () => ({version: '${version}', updatable: false}),
  };
  window.__TAURI__ = {
    core: {invoke: async (cmd, args = {}) => commands[cmd] ? commands[cmd](args) ?? null : null},
    event: {listen: async () => () => {}},
    window: {getCurrentWindow: () => ({setTitle(){}})},
  };
})();`;

const SCENE_HELPERS = `
const ui = {
  list: document.getElementById('list'),
  wait: ms => new Promise(r => setTimeout(r, ms)),
  key(k, opts = {}){ (document.activeElement || document.body).dispatchEvent(new KeyboardEvent('keydown', {key:k, bubbles:true, ...opts})); },
  click(sel){ document.querySelector(sel).click(); },
  folder(key){ ui.click('[data-folder="' + key + '"]'); },
  row(subject){ [...document.querySelectorAll('#rows tr[data-id]')].find(r => r.textContent.includes(subject)).click(); },
  async type(text){ const q = document.getElementById('q'); q.focus(); q.value = text; q.dispatchEvent(new Event('input')); },
  fill(id, text){ const el = document.getElementById(id); el.value = text; el.dispatchEvent(new Event('input', {bubbles:true})); },
};`;

function page(scene, theme) {
  const html = readFileSync(join(uiDir, 'index.html'), 'utf8');
  const run = `<script>${SCENE_HELPERS}
(async () => {
  for (let i = 0; i < 200 && !document.querySelector('#rows tr[data-id]'); i++) await ui.wait(20);
  ${scenes[scene]}
  document.body.dataset.ready = '1';
})();</script>`;
  return html
    .replace('<html lang="en">', `<html lang="en" data-theme="${theme}">`)
    .replace('<head>', `<head><base href="${pathToFileURL(uiDir + '/').href}">`)
    .replace('<script src="app.js"></script>', `<script>${MOCK}</script><script src="app.js"></script>${run}`);
}

function findBrowser() {
  const candidates = [
    process.env.BROWSER,
    'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
    'C:/Program Files/Microsoft/Edge/Application/msedge.exe',
    'C:/Program Files/Google/Chrome/Application/chrome.exe',
    '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
    '/usr/bin/google-chrome',
    '/usr/bin/chromium',
  ];
  const found = candidates.find(p => p && existsSync(p));
  if (!found) throw new Error('No Edge or Chrome found. Set BROWSER to its executable.');
  return found;
}

function shoot(browser, file, out, size = [W, H], scale = SCALE) {
  const profile = mkdtempSync(join(tmpdir(), 'telex-profile-'));
  try {
    execFileSync(browser, [
      '--headless=new', '--disable-gpu', '--hide-scrollbars', '--no-first-run', '--no-default-browser-check',
      '--allow-file-access-from-files', `--user-data-dir=${profile}`,
      `--force-device-scale-factor=${scale}`, `--window-size=${size[0]},${size[1]}`,
      '--virtual-time-budget=5000', `--screenshot=${out}`, pathToFileURL(file).href,
    ], {stdio: 'ignore', timeout: 60_000});
  } finally {
    rmSync(profile, {recursive: true, force: true});
  }
}

// Link previews (Open Graph images, 1200×630), one per page in lib/pages.ts: the page's line next
// to a screenshot. Saved as app/<page>/opengraph-image.png, which Next.js picks up by itself.
function ogPage({ eyebrow, title, screen, windowTitle }) {
  const shot = pathToFileURL(join(outDir, `${screen}-light.png`)).href;
  const mark = (size) => readFileSync(join(repo, size < 40 ? 'brand/telex-mark-small.svg' : 'brand/telex-mark.svg'), 'utf8')
    .replace('<title>Telex</title>', '').replace('<svg ', `<svg width="${size}" height="${size}" `);
  return `<!doctype html><html><head><meta charset="utf-8"><base href="${pathToFileURL(uiDir + '/').href}">
<link rel="stylesheet" href="fonts.css"><style>
*{box-sizing:border-box;margin:0}
body{width:1200px;height:630px;overflow:hidden;position:relative;background:#fff;color:#1a1e23;font-family:"IBM Plex Sans"}
.band{position:absolute;left:0;right:0;bottom:0;height:200px;background:#f3f5f8;border-top:1px solid #e9ecf0}
.text{position:absolute;left:64px;top:60px;width:520px}
.logo{display:flex;align-items:center;gap:14px;font:600 36px/1 "IBM Plex Sans Condensed"}
.eyebrow{margin-top:52px;font:500 16px/1 "IBM Plex Mono";letter-spacing:.08em;text-transform:uppercase;color:#2453a6}
h1{margin-top:18px;font:600 66px/0.98 "IBM Plex Sans Condensed";letter-spacing:-0.025em}
p{margin-top:22px;font-size:24px;line-height:1.4;color:#59636e}
.facts{position:absolute;left:64px;bottom:52px;font:500 15px/1 "IBM Plex Mono";letter-spacing:.06em;text-transform:uppercase;color:#59636e}
.win{position:absolute;left:620px;top:84px;width:860px;border:1px solid #d2d7de;border-radius:12px;overflow:hidden;background:#f7f8fa;
  box-shadow:0 1px 2px rgb(15 20 28/.06),0 30px 70px -20px rgb(15 20 28/.35)}
.tb{height:34px;display:flex;align-items:center;gap:8px;padding-left:12px;font-size:13px;border-bottom:1px solid #d2d7de}
.win img{display:block;width:100%}
</style></head><body><div class="band"></div>
<div class="text"><div class="logo">${mark(44)}Telex</div><div class="eyebrow">${eyebrow}</div><h1>${title}</h1>
<p>A compact, fast email client for Windows.</p></div>
<div class="facts">Rust · WebView2 · Offline search · No telemetry</div>
<div class="win"><div class="tb">${mark(15)}${windowTitle}</div><img src="${shot}"></div>
</body></html>`;
}

const browser = findBrowser();
const work = mkdtempSync(join(tmpdir(), 'telex-screens-'));
mkdirSync(outDir, {recursive: true});
const only = process.argv.slice(2);
for (const scene of Object.keys(scenes)) {
  if (only.length && !only.includes(scene)) continue;
  for (const theme of ['light', 'dark']) {
    const file = join(work, `${scene}-${theme}.html`);
    writeFileSync(file, page(scene, theme));
    shoot(browser, file, join(outDir, `${scene}-${theme}.png`));
    console.log(`screens/${scene}-${theme}.png`);
  }
}
if (!only.length || only.includes('og')) {
  // Node loads these TypeScript files directly (they have no imports of their own).
  const { pages } = await import('../lib/pages.ts');
  const { hero, screens } = await import('../lib/content.ts');
  const windowTitles = Object.fromEntries([hero, ...screens].map((s) => [s.id, s.title]));
  for (const p of Object.values(pages)) {
    const dir = join(here, '../app', p.path === '/' ? '' : p.path.slice(1));
    const file = join(work, `og-${p.label}.html`);
    const eyebrow = p.path === '/' ? 'Email client for Windows' : p.label;
    writeFileSync(file, ogPage({ eyebrow, title: p.ogTitle, screen: p.ogScreen, windowTitle: windowTitles[p.ogScreen] }));
    shoot(browser, file, join(dir, 'opengraph-image.png'), [1200, 630], 1);
    writeFileSync(join(dir, 'opengraph-image.alt.txt'), `Telex – ${p.ogTitle} A screenshot of the app next to the headline.`);
    console.log(`app${p.path === '/' ? '' : p.path}/opengraph-image.png`);
  }
}
rmSync(work, {recursive: true, force: true});
