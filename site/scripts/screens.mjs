// Screenshots for the site, made from the real app UI (../ui) with an English demo mailbox.
// Run after UI changes:  npm run screens
//
// The UI normally talks to Rust through Tauri's `invoke`. Here a small stand-in answers from
// scripts/demo-mailbox.json instead, and headless Edge (or Chrome) takes the picture.
// Set BROWSER to a browser executable if yours isn't found automatically.

import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, '../..');
const uiDir = join(repo, 'ui');
const demo = JSON.parse(readFileSync(join(here, 'demo-mailbox.json'), 'utf8'));
const version = JSON.parse(readFileSync(join(repo, 'src-tauri/tauri.conf.json'), 'utf8')).version;
const outDir = join(here, '../public/screens');
const W = 1280, H = 800, SCALE = 2;

// Each scene runs after the app has booted. `ui` has small helpers, see SCENE_HELPERS.
// `empty`: start with no account, as on first start. `lang`: the interface language (en by default).
const scenes = {
  inbox: {},
  reply: { run: `ui.key('r'); await ui.wait(150);` },
  filter: { run: `await ui.type('krakow'); await ui.wait(300);` },
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
    run: `ui.key('n'); await ui.wait(150); ui.fill('c-to', 'Anna Kowalska <anna@kowalska-architekci.pl>');
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

// The HTML mail frame's document, as html::wrap in the core makes it. Keep the two in step.
const FRAME =
  '<!doctype html><html><head><meta charset="utf-8">' +
  `<meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src data:; style-src 'unsafe-inline'; font-src data:">` +
  '<meta name="referrer" content="no-referrer"><style>html{background:#fff;color-scheme:light}' +
  "body{margin:0;padding:12px 14px 28px;font:14px/1.5 'Segoe UI',system-ui,sans-serif;color:#1a1e23;overflow-wrap:anywhere}" +
  'img{max-width:100%;height:auto}table{max-width:100%}a{color:#2453a6}pre{white-space:pre-wrap}' +
  'blockquote{margin:0 0 0 2px;padding-left:10px;border-left:2px solid #d2d7de;color:#59636e}</style></head><body>{body}</body></html>';

// Runs in the page before app.js and stands in for Tauri, answering the way the core does.
// It is turned into source with toString(), so it can use nothing from this file.
function installMock({ data, scene, empty, version, frame, theme, lang }) {
  // A fixed "now" keeps the list's relative dates (06:12, Fri 18:31) the same on every run.
  const RealDate = Date, NOW = new RealDate(2026, 9, 10, 9, 30).getTime();
  window.Date = class extends RealDate {
    constructor(...a) { a.length ? super(...a) : super(NOW); }
    static now() { return NOW; }
  };
  // `only`: a message that shows up in one scene (the unsent one in the Outbox).
  const msgs = data.messages.filter((m) => !m.only || m.only === scene).map((m, i) => ({ ...m, id: i + 1 }));
  const accounts = empty ? [] : data.accounts;
  const fold = (s) => s.normalize('NFD').replace(/\p{M}/gu, '').replace(/ł/g, 'l').toLowerCase();
  const summary = (m) => ({
    id: m.id, folder: m.folder, from_name: m.from_name, from_email: m.from_email, to: m.to, subject: m.subject,
    date: m.date, size_kb: m.size_kb, unread: m.unread, flagged: m.flagged, has_attachments: m.attachments.length > 0,
  });
  const byId = (id) => msgs.find((m) => m.id === id);
  const settings = { theme, language: lang, density: 'compact', text_size: 100, prefer_text: false, mark_read: 'moment', check_every: 10,
    push_mail: true, notify: true, mail_paper: 'theme', compose_format: scene === 'writing' ? 'html' : 'plain', reply_format: 'same',
    undo_send: 5, ask_subject: true, spell_check: true, updates: 'off' };
  const settingsView = () => ({ settings: { ...settings }, language: settings.language, offer: null, fresh: false });

  // The message list as crates/telex-core/src/list.rs answers it: one folder (or all mail
  // but Spam and Trash), sorted, filtered on headers as typed and, with `bodies`, on the text.
  const kindOf = (key) => { const [a, f] = key.split('/'); return accounts.find((x) => x.id === a)?.folders.find((x) => x.id === f)?.kind; };
  const nameOf = (addr) => { const s = String(addr || '').trim(); if (!s) return '(no recipient)'; const m = s.match(/^\s*"?([^"<]+?)"?\s*</); return m ? m[1] : s; };
  function listed({ folder, sort, ascending, filter, bodies }) {
    const q = filter.trim().toLowerCase();
    const out = (m) => ['sent', 'drafts', 'outbox'].includes(kindOf(m.folder));
    const key = {
      date: (m) => m.date, size: (m) => m.size_kb, flag: (m) => (m.flagged ? 2 : 0) + (m.unread ? 1 : 0),
      from: (m) => (out(m) ? nameOf(m.to) : m.from_name).toLowerCase(),
      subject: (m) => m.subject.toLowerCase().replace(/^((re|fwd?):\s*)+/i, ''),
    }[sort];
    const dir = ascending ? 1 : -1;
    return (empty ? [] : msgs)
      .filter((m) => (folder ? m.folder === folder : !['spam', 'trash'].includes(kindOf(m.folder))))
      .filter((m) => !q || [m.from_name, m.from_email, m.subject, m.to].join(' ').toLowerCase().includes(q)
        || (bodies && fold([m.subject, m.from_name, m.to, m.body].join(' ')).includes(fold(q))))
      .sort((a, b) => {
        const x = key(a), y = key(b), c = (x < y ? -1 : x > y ? 1 : 0) * dir;
        if (c) return c;
        return sort === 'date' ? (a.id - b.id) * dir : a.date < b.date ? 1 : a.date > b.date ? -1 : b.id - a.id;
      });
  }
  function listPage({ view, at, limit }) {
    const all = listed(view), place = (id) => all.findIndex((m) => m.id === id);
    let start = 0, end;
    if (at === 'last') start = Math.max(0, all.length - limit);
    else if (at.after != null) start = place(at.after) + 1;
    else if (at.before != null) { end = Math.max(0, place(at.before)); start = Math.max(0, end - limit); }
    else if (at.around != null) start = Math.max(0, place(at.around) - (limit >> 1));
    else if (at.row != null) start = Math.min(at.row, Math.max(0, all.length - limit));
    end ??= Math.min(all.length, start + limit);
    return { total: all.length, start, rows: all.slice(start, end).map(summary) };
  }
  function folderTree() {
    const counts = {};
    for (const m of empty ? [] : msgs) {
      const c = (counts[m.folder] ??= { total: 0, unread: 0 });
      c.total++;
      if (m.unread) c.unread++;
    }
    return { accounts, counts };
  }

  // drafts::start: the identity the message was sent to (else the default), its signature,
  // then the quote under its reply header.
  function startDraft({ kind, of, account }) {
    const m = of ? byId(of) : null;
    const acct = accounts.find((a) => a.id === (m ? m.folder.split('/')[0] : account));
    const addressed = m ? [m.to, m.cc, m.from_email].join(', ').toLowerCase() : '';
    const me = acct.identities.find((i) => !i.is_default && addressed.includes(i.email.toLowerCase()))
      ?? acct.identities.find((i) => i.is_default);
    const sig = me.signature.trimEnd();
    const layout = (tail) => (sig ? `\n\n${sig}\n` + (tail ? `\n${tail}` : '') : tail ? `\n\n${tail}` : '\n');
    const draft = { account: acct.id, identity: me.id, to: '', cc: '', subject: '', body: layout(null), attachments: [], reply_to: null, forward_of: null };
    if (!m) return draft;
    const fill = (t) => t.replaceAll('{name}', m.from_name || m.from_email).replaceAll('{email}', m.from_email)
      .replaceAll('{from}', `${m.from_name} <${m.from_email}>`).replaceAll('{subject}', m.subject)
      .replaceAll('{date}', `${m.date.slice(8, 10)}.${m.date.slice(5, 7)}.${m.date.slice(0, 4)}`).replaceAll('{time}', m.date.slice(11, 16));
    const lines = m.body.replace(/\n+$/, '').split('\n');
    const strip = (s) => s.replace(/^((re|fwd?|odp|pd):\s*)+/i, '');
    if (kind === 'forward') {
      return { ...draft, subject: `Fwd: ${strip(m.subject)}`, attachments: m.attachments, forward_of: m.id,
        body: layout(`${fill(me.forward_header)}\n\n${lines.join('\n')}`) };
    }
    const own = acct.identities.map((i) => i.email.toLowerCase());
    const cc = kind === 'reply_all'
      ? [m.to, m.cc].join(',').split(',').map((a) => a.trim()).filter((a) => a && !own.some((o) => a.toLowerCase().includes(o)))
      : [];
    return { ...draft, to: `${m.from_name} <${m.from_email}>`, cc: cc.join(', '), subject: `Re: ${strip(m.subject)}`, reply_to: m.id,
      body: layout(`${fill(me.reply_header)}\n${lines.map((l) => (l.startsWith('>') ? '>' + l : '> ' + l)).join('\n')}`) };
  }

  // colors.rs, short: on a dark theme's paper, light backgrounds turn dark and dark text
  // light (OKLab lightness, hue kept), white becoming the theme's background exactly.
  const lin = (c) => ((c /= 255) <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
  const unlin = (c) => Math.round(255 * Math.min(1, Math.max(0, c <= 0.0031308 ? c * 12.92 : 1.055 * c ** (1 / 2.4) - 0.055)));
  const toLab = ([r, g, b]) => {
    [r, g, b] = [lin(r), lin(g), lin(b)];
    const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
    const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
    const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
    return [0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s, 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s, 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s];
  };
  const fromLab = ([L, A, B]) => {
    const l = (L + 0.3963377774 * A + 0.2158037573 * B) ** 3, m = (L - 0.1055613458 * A - 0.0638541728 * B) ** 3, s = (L - 0.0894841775 * A - 1.291485548 * B) ** 3;
    return [4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s, -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s, -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s].map(unlin);
  };
  const hexRgb = (h) => { h = h.replace('#', ''); if (h.length === 3) h = [...h].map((c) => c + c).join(''); return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16)); };
  const rgbHex = (c) => '#' + c.map((v) => v.toString(16).padStart(2, '0')).join('');
  function recolor(hex, toPaper, paper, ink) {
    const [L, A, B] = toLab(hexRgb(hex));
    const bg = toPaper || (L >= 0.5 && toPaper !== false);
    const end = bg ? paper : ink;
    const nl = bg ? (L <= 0.5 ? L : 0.5 - (Math.min(L, 1) - 0.5) * (0.5 - paper[0]) / 0.5) : (L >= 0.62 ? L : ink[0] - (ink[0] - 0.62) * (L / 0.62) ** 2);
    const gray = Math.abs(nl - L) > 1e-9 ? Math.max(0, 1 - Math.hypot(A, B) / 0.04) : 0;
    return rgbHex(fromLab([nl, A * (1 - gray) + end[1] * gray, B * (1 - gray) + end[2] * gray]));
  }
  function darkHtml(html, p) {
    const paper = toLab(hexRgb(p.paper)), ink = toLab(hexRgb(p.ink));
    const hexes = /#[0-9a-f]{6}\b|#[0-9a-f]{3}\b/gi;
    return html.replace(/style="([^"]*)"/g, (_, css) => 'style="' + css.split(';').map((d) => {
      const [prop, ...rest] = d.split(':'), v = rest.join(':'), k = (prop || '').trim();
      if (!v) return d;
      const role = k === 'color' ? false : k.startsWith('background') ? true : k.startsWith('border') ? null : undefined;
      return role === undefined ? d : `${prop}:${v.replace(hexes, (h) => recolor(h, role, paper, ink))}`;
    }).join(';') + '"');
  }
  const darkFrame = (p) => frame.replace('html{background:#fff;color-scheme:light}', `html{background:${p.paper};color-scheme:dark}`)
    .replace('color:#1a1e23;', `color:${p.ink};`).replace('a{color:#2453a6}', `a{color:${p.link}}`)
    .replace('border-left:2px solid #d2d7de;color:#59636e', `border-left:2px solid ${p.line};color:${p.muted}`);

  // usage.rs: sizes by folder, mail in and out per day for 30 days, the busiest senders.
  function usage() {
    const day = (d) => d.slice(0, 10), since = new RealDate(NOW - 29 * 864e5), sinceDay = since.toISOString().slice(0, 10);
    const quotas = { studio: { used_kb: 1_660_000, limit_kb: 2_621_440 } };
    return {
      base_bytes: 61_800_000, free_bytes: 0, since: sinceDay,
      accounts: accounts.map((a) => {
        const mine = msgs.filter((m) => m.folder.startsWith(a.id + '/'));
        const folders = a.folders.map((f) => {
          const key = `${a.id}/${f.id}`, ms = mine.filter((m) => m.folder === key);
          return { folder: key, kind: f.kind, count: ms.length, unread: ms.filter((m) => m.unread).length, size_kb: ms.reduce((n, m) => n + m.size_kb, 0) * 40 };
        }).filter((f) => f.count).sort((x, y) => y.size_kb - x.size_kb);
        const flow = Array.from({ length: 30 }, (_, i) => {
          const d = new RealDate(since.getTime() + i * 864e5).toISOString().slice(0, 10);
          const on = mine.filter((m) => day(m.date) === d), k = (m) => kindOf(m.folder);
          // The demo mailbox is small: a steady trickle keeps the chart honest-looking.
          const base = ((i * 7) % 5) + (i % 7 === 5 || i % 7 === 6 ? 0 : 3);
          return { received: on.filter((m) => !['sent', 'drafts', 'outbox', 'spam'].includes(k(m))).length + base,
            sent: on.filter((m) => k(m) === 'sent').length + (i % 3 === 0 ? 1 : 0), spam: on.filter((m) => k(m) === 'spam').length + (i % 4 === 1 ? 1 : 0) };
        });
        const counts = new Map();
        for (const m of mine.filter((m) => !['sent', 'drafts', 'outbox', 'spam'].includes(kindOf(m.folder)))) {
          const c = counts.get(m.from_email) ?? { name: m.from_name, email: m.from_email, count: 0 };
          c.count += 6; counts.set(m.from_email, c);
        }
        return {
          account: a.id, quota: quotas[a.id] ?? null, synced_at: '2026-10-10T09:24:00', sync_error: null, folders, flow,
          senders: [...counts.values()].sort((x, y) => y.count - x.count).slice(0, 5),
          largest: [...mine].sort((x, y) => y.size_kb - x.size_kb).slice(0, 5).map(summary), queued: 0,
        };
      }),
    };
  }
  const textToHtml = (text) => text.split('\n').map((l) => {
    const e = l.replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' }[c]));
    return l.startsWith('>') ? `<blockquote><div>${e.replace(/^&gt; ?/, '') || '<br>'}</div></blockquote>` : `<div>${e || '<br>'}</div>`;
  }).join('').replace(/<\/blockquote><blockquote>/g, '');

  const commands = {
    folder_tree: folderTree,
    list_page: listPage,
    get_message: ({ id }) => byId(id),
    set_unread: ({ id, unread }) => { byId(id).unread = unread; },
    set_flagged: ({ id, flagged }) => { byId(id).flagged = flagged; },
    // html::render: remote images are left out and counted until the user loads them.
    get_html: ({ id, dark }) => {
      const m = byId(id);
      if (!m.html) return null;
      const remote = new Set();
      const html = m.html.replace(/\s(?:src|background)="(https?:[^"]*)"/g, (_, url) => { remote.add(url); return ''; });
      const doc = dark ? darkFrame(dark).replace('{body}', darkHtml(html, dark)) : frame.replace('{body}', html);
      return { document: doc, blocked: remote.size, loaded: false, sender: m.from_email, trusted: false };
    },
    usage,
    trusted_senders: () => ['news@frontendweekly.news'],
    draft_html: () => null,
    text_to_html: ({ text }) => textToHtml(text),
    html_to_text: ({ html }) => html.replace(/<br>/g, '').replace(/<\/(div|p|li)>/g, '\n').replace(/<[^>]+>/g, ''),
    clean_html: ({ html }) => html,
    start_draft: startDraft,
    list_rules: () => data.rules,
    known_provider: () => null,
    detect_settings: ({ email }) => {
      const domain = email.split('@')[1];
      return {
        setup: { email, host: `mail.${domain}`, port: 993, username: email, smtp_host: `mail.${domain}`, smtp_port: 465, smtp_security: 'tls' },
        provider: null, source: 'srv', note: null, unsupported: null, help: null,
      };
    },
    // The login never finishes, so the picture shows it running.
    add_account: () => new Promise(() => {}),
    check_mail: () => ({ new: [], sent: [], sorted: 0, errors: [], busy: false }),
    push_changes: () => ({ errors: [], busy: false }),
    // The version number shows in the status bar; "can't update" keeps the one-time question away.
    update_info: () => ({ version, updatable: false }),
    // settings.json, as crates/telex-core/src/settings.rs keeps it: the scene's theme and language.
    get_settings: () => settingsView(),
    update_settings: ({ changes }) => { Object.assign(settings, changes); return settingsView(); },
  };
  window.__TAURI__ = {
    core: { invoke: async (cmd, args = {}) => (commands[cmd] ? commands[cmd](args) ?? null : null) },
    event: { listen: async () => () => {} },
    window: { getCurrentWindow: () => ({ setTitle() {} }) },
  };
}

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
};`;

// JSON that is safe inside a <script> element.
const scriptJson = (v) => JSON.stringify(v).replace(/</g, '\\u003c');

function page(scene, theme) {
  const html = readFileSync(join(uiDir, 'index.html'), 'utf8');
  const { run = '', empty = false, lang = 'en' } = scenes[scene];
  const mock = `<script>(${installMock})(${scriptJson({ data: demo, scene, empty, version, frame: FRAME, theme, lang })});</script>`;
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
    // A function as the replacement, so "$" in the inserted code is taken literally.
    .replace('<script src="app.js"></script>', () => `${mock}<script src="app.js"></script>${play}`);
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
