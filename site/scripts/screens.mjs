// Screenshots for the site, made from the real app UI (../ui) with an English demo mailbox.
// Run after UI changes:  npm run screens
//
// The UI normally talks to Rust through Tauri's `invoke`. Here the live demo's stand-in for the
// core (demo/backend.js, shared with `npm run demo`) answers from scripts/demo-mailbox.json
// instead, with the clock stopped at 10.10.2026 09:30, and headless Edge (or Chrome) takes the
// picture. Set BROWSER to a browser executable if yours isn't found automatically.

import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, '../..');
const uiDir = join(repo, 'ui');
const demo = JSON.parse(readFileSync(join(here, 'demo-mailbox.json'), 'utf8'));
const backend = readFileSync(join(here, '../demo/backend.js'), 'utf8');
const version = JSON.parse(readFileSync(join(repo, 'src-tauri/tauri.conf.json'), 'utf8')).version;
const outDir = join(here, '../public/screens');
const W = 1280, H = 800, SCALE = 2;

// Each scene runs after the app has booted. `ui` has small helpers, see SCENE_HELPERS.
// `empty`: start with no account, as on first start. `lang`: the interface language (en by default).
// `settings`: settings to start with. Messages with `only` in the mailbox show in those scenes.
const scenes = {
  inbox: {},
  reply: { run: `ui.key('r'); await ui.wait(150);` },
  // Right-click on the shop reply: the message's menu, every action with its key.
  menu: {
    run: `const c = [...document.querySelectorAll('#rows tr[data-id]')].find(r => r.textContent.includes('Re: Online shop estimate')).querySelector('.c-subj');
    const r = c.getBoundingClientRect();
    c.dispatchEvent(new MouseEvent('contextmenu', {bubbles:true, cancelable:true, clientX:r.left + 90, clientY:r.top + r.height / 2}));
    await ui.wait(80);`,
  },
  // Search after two searches: the box, empty again, offers them and the operators.
  suggest: {
    run: `ui.list.focus(); ui.key('/'); await ui.wait(50); await ui.type('has:file'); await ui.wait(250); ui.key('Enter'); await ui.wait(200);
    ui.key('/'); await ui.wait(50); await ui.type('from:anna'); await ui.wait(250); ui.key('Enter'); await ui.wait(200);
    ui.key('/'); await ui.wait(50); await ui.type(''); await ui.wait(300);`,
  },
  // Settings › Reading: when mail counts as read.
  reading: { run: `ui.click('#b-settings'); await ui.wait(50); ui.click('#s-tab-read'); await ui.wait(150);` },
  // Shift+↓ twice in the Inbox, then M with the first letters of a folder.
  select: {
    run: `ui.key('ArrowDown', {shiftKey:true}); await ui.wait(120); ui.key('ArrowDown', {shiftKey:true}); await ui.wait(120);
    ui.key('m'); await ui.wait(80); ui.fill('pk-q', 'cl'); await ui.wait(80);`,
  },
  // Unread at the top of the folders: both accounts' new mail, each row with its account's line.
  unread: { run: `ui.folder('*/unread'); await ui.wait(150); ui.list.focus();` },
  // T on the shop reply: the enquiry in Clients, the estimate in Sent, the answer in the Outbox.
  thread: { run: `ui.row('Re: Online shop estimate'); await ui.wait(50); ui.list.focus(); ui.key('t'); await ui.wait(200);` },
  // A new message with two files, Bcc open, and the suggestions for a second recipient.
  attach: {
    run: `ui.key('n'); await ui.wait(150); ui.click('[data-act="bcc"]'); await ui.wait(200);
    const demo = window.__TELEX_DEMO__, file = (name, kb) => demo.addFile(new File([new Uint8Array(kb * 1024)], name));
    demo.emit('files-dropped', [file('homepage-v2.pdf', 1843), file('header-photo-bright.jpg', 612)]); await ui.wait(50);
    ui.fill('c-subj', 'Homepage, version 2'); ui.fill('c-bcc', 'Priya Shah <priya@lindenstudio.com>');
    const body = document.getElementById('c-body');
    body.value = 'Hi Anna,\\n\\nthe new homepage is attached, with the brighter header photo and the gallery newest first.' + body.value;
    ui.typeIn('c-to', 'Anna Kowalska <anna@kowalska-architects.pl>, m'); await ui.wait(150);`,
  },
  // Folders on this PC: last year's client mail kept off the server, with the right-click menu.
  local: {
    run: `ui.folder('local/clients-2025'); await ui.wait(100); ui.row('Our new website'); await ui.wait(150);
    const f = document.querySelector('[data-folder="local/clients-2025"]'), r = f.getBoundingClientRect();
    f.dispatchEvent(new MouseEvent('contextmenu', {bubbles:true, cancelable:true, clientX:r.left + 70, clientY:r.top + r.height / 2}));
    await ui.wait(50);`,
  },
  // Settings › Backups: the mail base, backing up and restoring.
  backups: { run: `ui.click('#b-settings'); await ui.wait(50); ui.click('#s-tab-backup'); await ui.wait(250);` },
  filter: { run: `await ui.type('krakow'); await ui.wait(300);` },
  // Operators in all mail (the scope chip): files since September, with the Folder column.
  operators: { run: `ui.click('#q-scope'); await ui.type('has:file after:2026-09'); await ui.wait(300);` },
  // The message pane dragged taller, so more of the newsletter shows.
  images: { run: `ui.pane(40); ui.row('View Transitions'); await ui.wait(300);` },
  rules: { run: `ui.click('#b-rules'); await ui.wait(100); ui.click('#r-list [data-r="edit"]'); await ui.wait(50);` },
  spam: { run: `ui.folder('studio/spam'); await ui.wait(50); ui.row('Rank #1'); await ui.wait(100);` },
  identities: {
    run: `ui.click('#b-settings'); await ui.wait(50); ui.click('#s-tab-accts'); await ui.wait(50); ui.click('[data-ids="studio"]'); await ui.wait(50);
    ui.click('#i-list [data-id="2"] [data-i="edit"]'); await ui.wait(50);`,
  },
  // Settings › Accounts: every account, its servers and how its last check went.
  accounts: { run: `ui.click('#b-settings'); await ui.wait(50); ui.click('#s-tab-accts'); await ui.wait(150);` },
  // Settings › Usage: the server's storage, this PC, 30 days of mail in and out.
  usage: { run: `ui.click('#b-settings'); await ui.wait(50); ui.click('#s-tab-use'); await ui.wait(250);` },
  // A reply written in HTML: the formatting bar, a list and a quote.
  writing: {
    settings: { compose_format: 'html' },
    run: `ui.key('n'); await ui.wait(150); ui.fill('c-to', 'Anna Kowalska <anna@kowalska-architects.pl>');
    ui.fill('c-subj', 'Studio visit on Friday');
    document.getElementById('c-html').innerHTML = '<div>Hi Anna,</div><div><br></div><div>thanks for the plans. For Friday I would like to go through:</div>'
      + '<ul><li><b>the kitchen</b>, with the new window</li><li>the <i>colours</i> for the hall</li></ul><div>See you at 10!</div><div><br></div><div>Sam</div>';
    await ui.wait(50);`,
  },
  outbox: { run: `ui.pane(30); ui.folder('studio/outbox'); await ui.wait(100);` },
  comfortable: { run: `ui.click('#b-density'); ui.folder('home/inbox'); await ui.wait(100); ui.list.focus();` },
  // Settings on its first tab: every theme drawn in its own colors.
  settings: { run: `ui.click('#b-settings'); await ui.wait(150);` },
  polish: { lang: 'pl' },
  account: {
    empty: true,
    run: `ui.fill('w-name', 'Sam Carter'); ui.fill('w-email', 'sam@lindenstudio.com'); ui.fill('w-pass', 'password');
    document.getElementById('w-start').requestSubmit(); await ui.wait(150);`,
  },
};

const SCENE_HELPERS = `
const ui = {
  list: document.getElementById('list'),
  wait: ms => new Promise(r => setTimeout(r, ms)),
  key(k, opts = {}){ (document.activeElement || document.body).dispatchEvent(new KeyboardEvent('keydown', {key:k, bubbles:true, ...opts})); },
  click(sel){ document.querySelector(sel).click(); },
  // A folder behind "more" is opened from the folder menu, so the tree stays as it starts.
  folder(key){ const b = document.querySelector('[data-folder="' + key + '"]'); if (b) b.click(); else { const s = document.getElementById('folder-select'); s.value = key; s.dispatchEvent(new Event('change')); } },
  pane(listPercent){ document.getElementById('win').style.setProperty('--list-h', listPercent + '%'); },
  row(subject){ [...document.querySelectorAll('#rows tr[data-id]')].find(r => r.textContent.includes(subject)).click(); },
  async type(text){ const q = document.getElementById('q'); q.focus(); q.value = text; q.dispatchEvent(new Event('input')); },
  fill(id, text){ const el = document.getElementById(id); el.value = text; el.dispatchEvent(new Event('input', {bubbles:true})); },
  // Typed with the focus in the field, caret at the end, so suggestions show.
  typeIn(id, text){ const el = document.getElementById(id); el.focus(); el.value = text; el.setSelectionRange(text.length, text.length); el.dispatchEvent(new Event('input', {bubbles:true})); },
};`;

// JSON that is safe inside a <script> element.
const scriptJson = (v) => JSON.stringify(v).replace(/</g, '\\u003c');

function page(scene, theme) {
  const html = readFileSync(join(uiDir, 'index.html'), 'utf8');
  const { run = '', empty = false, lang = 'en', settings = {} } = scenes[scene];
  // The stand-in, as in the live demo but started by hand: a fixed clock, this scene's look.
  const options = { data: demo, scene, empty, version, theme, lang, settings, frozenNow: '2026-10-10T09:30:00' };
  const mock = `<script>window.TELEX_DEMO_MANUAL = true;</script><script>${backend}</script>`
    + `<script>installTelexDemo(${scriptJson(options)});</script>`;
  // The folder tree's "Add account" button is drawn with the first render, with or without
  // accounts; with accounts, the list's first page comes right after.
  const play = `<script>${SCENE_HELPERS}
(async () => {
  for (let i = 0; i < 200 && !document.querySelector('#tree .add-acct'); i++) await ui.wait(20);
  for (let i = 0; i < 200 && ${!empty} && !document.querySelector('#rows tr.sel'); i++) await ui.wait(20);
  ${run}
  document.body.dataset.ready = '1';
})();</script>`;
  return html
    .replace('<html lang="en">', `<html lang="${lang}" data-theme="${theme}">`)
    .replace('<head>', `<head><base href="${pathToFileURL(uiDir + '/').href}">`)
    // A function as the replacement, so "$" in the inserted code is taken literally. The
    // stand-in goes before boot.js, as in the live demo.
    .replace('<script src="boot.js"></script>', () => `${mock}<script src="boot.js"></script>`)
    .replace('<script src="app.js"></script>', () => `<script src="app.js"></script>${play}`);
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
// The inbox in every theme, for the accessibility page: public/screens/theme-<id>.png, at 1×
// (they are shown small). The list is ui/themes.css's.
if (!only.length || only.includes('themes')) {
  const ids = [...readFileSync(join(uiDir, 'themes.css'), 'utf8').matchAll(/\[data-theme="([\w-]+)"\]\s*\{/g)].map((m) => m[1]);
  for (const theme of ids) {
    const file = join(work, `theme-${theme}.html`);
    writeFileSync(file, page('inbox', theme));
    shoot(browser, file, join(outDir, `theme-${theme}.png`), [W, H], 1);
    console.log(`screens/theme-${theme}.png`);
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
