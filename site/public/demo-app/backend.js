/*
 * Telex live demo: a stand-in for Tauri and the Rust core (crates/telex-core), in the browser.
 *
 * The demo runs the unmodified interface from ../ui (index.html, app.js, i18n.js, …). That
 * interface talks to Rust only through window.__TAURI__.core.invoke; this file defines that
 * object and answers every command from a demo mailbox held in memory, the way the core does
 * (list paging and the quick filter from list.rs, drafts.rs, filters.rs, recipients.rs,
 * usage.rs, the HTML frame of html.rs and the dark paper of colors.rs). Nothing leaves the
 * browser: no fetch, no storage beyond the look boot.js reads. A reload starts fresh.
 *
 * One function, installTelexDemo(options), does it all. It uses nothing from outside itself,
 * so site/scripts/screens.mjs can inline this file (or the function's source) into a page.
 *
 *   options.data       the mailbox (site/scripts/demo-mailbox.json)
 *   options.live       true on the website: dates follow the visitor's calendar, Get mail
 *                      brings the mailbox's `incoming` messages, IMAP IDLE "pushes" one after
 *                      `idle.after_s` seconds, ?theme= and ?lang= are read
 *   options.scene      a screenshot's scene name (messages with `only` show in their scenes;
 *                      "live" is the live demo's name)
 *   options.empty      no account yet, as on the first start
 *   options.frozenNow  "2026-10-10T09:30": screenshots replace Date with this moment
 *   options.version    shown in the status bar
 *   options.theme, options.lang   the settings' theme and language ("auto" follows the browser)
 *   options.settings   other settings to start with, e.g. {compose_format: 'html'}
 *   options.frame      the HTML mail frame template (defaults to html::wrap's, below)
 *
 * Loaded as a plain file, it starts itself in live mode when window.TELEX_DEMO_DATA is set
 * ({version, mailbox}, written by site/scripts/demo.mjs as demo-data.js) and
 * window.TELEX_DEMO_MANUAL isn't. It must load before boot.js, which reads the look it saves.
 * window.__TELEX_DEMO__ exposes emit() and addFile() for screenshot scenes and tests.
 */
function installTelexDemo(options) {
  'use strict';
  const {
    data, live = false, scene = null, empty = false, frozenNow = null, version = '0.0.0',
    theme: askedTheme = 'system', lang: askedLang = 'auto', settings: startSettings = {}, frame: frameOption = null,
  } = options;

  const THEMES = ['system', 'light', 'dark', 'paper', 'heather', 'fjord', 'phosphor', 'contrast-light', 'contrast-dark'];
  const clone = (v) => (v === undefined ? null : JSON.parse(JSON.stringify(v)));
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  // Screenshots answer at once, so a scene's short waits see the result; the live demo takes
  // the moment a real server would.
  const later = (ms) => (live ? sleep(ms) : Promise.resolve());

  /* ---------- the clock ---------- */
  const RealDate = Date;
  const pad = (n) => String(n).padStart(2, '0');
  const parse = (s) => {
    const [d, t = '00:00:00'] = String(s).split('T');
    const [y, mo, da] = d.split('-').map(Number);
    const [h = 0, mi = 0, se = 0] = t.split(':').map(Number);
    return new RealDate(y, mo - 1, da, h, mi, Math.floor(se));
  };
  const fmt = (d) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
  const addDays = (d, n) => new RealDate(d.getFullYear(), d.getMonth(), d.getDate() + n, d.getHours(), d.getMinutes(), d.getSeconds());
  let nowMs = () => RealDate.now();
  if (frozenNow) {
    // A fixed "now" keeps the list's relative dates (06:12, Fri 18:31) the same on every run.
    const NOW = parse(frozenNow).getTime();
    window.Date = class extends RealDate {
      constructor(...a) { if (a.length) super(...a); else super(NOW); }
      static now() { return NOW; }
    };
    nowMs = () => NOW;
  }
  const now = () => new RealDate(nowMs());
  const stamp = () => fmt(now());

  /* ---------- settings and language ---------- */
  const autoLang = (() => {
    const all = (navigator.languages && navigator.languages.length ? navigator.languages : [navigator.language || 'en'])
      .map((l) => String(l).slice(0, 2).toLowerCase());
    return all.find((l) => l === 'en' || l === 'pl') === 'pl' ? 'pl' : 'en';
  })();
  const settings = {
    theme: THEMES.includes(askedTheme) ? askedTheme : 'system',
    language: ['auto', 'en', 'pl'].includes(askedLang) ? askedLang : 'auto',
    density: 'compact', text_size: 100, prefer_text: false, mark_read: 'moment', check_every: 10, push_mail: true, notify: true,
    mail_paper: 'theme', compose_format: 'plain', reply_format: 'same', undo_send: 5, ask_subject: true, spell_check: true,
    updates: 'off', ...startSettings,
  };
  const lang = () => (settings.language === 'auto' ? autoLang : settings.language === 'pl' ? 'pl' : 'en');
  const fill = (s, vars) => (vars ? s.replace(/\{(\w+)\}/g, (m, k) => (k in vars ? String(vars[k]) : m)) : s);
  // The core's tr!: every sentence in English and Polish, in the interface's language.
  const tr = (en, pl, vars) => fill(lang() === 'pl' ? pl : en, vars);
  const fail = (en, pl, vars) => { throw tr(en, pl, vars); };
  const DEMO = {
    servers: ['This is a demo: it can’t log in to real mail servers. Download Telex to use your own mail.',
      'To tylko demo: nie łączy się z prawdziwymi serwerami poczty. Pobierz Telex, aby używać własnej poczty.'],
    open: ['This is a demo: its attachments are only names, with nothing inside to open. Download Telex to open your own.',
      'To tylko demo: załączniki mają tu tylko nazwy, nie ma czego otworzyć. Pobierz Telex, aby otwierać własne.'],
    save: ['This is a demo: it doesn’t save files. Download Telex to save attachments from your own mail.',
      'To tylko demo: nie zapisuje plików. Pobierz Telex, aby zapisywać załączniki z własnej poczty.'],
    export: ['This is a demo: it doesn’t write files. Download Telex to export your settings.',
      'To tylko demo: nie zapisuje plików. Pobierz Telex, aby eksportować ustawienia.'],
    import: ['This is a demo: it doesn’t read settings files. Download Telex to import yours.',
      'To tylko demo: nie wczytuje plików ustawień. Pobierz Telex, aby zaimportować swoje.'],
    remove: ['This is a demo: its accounts can’t be removed. Download Telex to set up your own.',
      'To tylko demo: jego kont nie można usunąć. Pobierz Telex, aby dodać własne.'],
    update: ['This is a demo: it doesn’t update itself. Download the newest Telex instead.',
      'To tylko demo: nie aktualizuje się. Pobierz najnowszego Telexa.'],
  };
  const refuse = (key) => fail(...DEMO[key]);

  /* ---------- the mailbox ---------- */
  const DEMO_DAY = data.today || '2026-10-10';
  // `only`: a message that shows in some scenes (the unsent one in the Outbox); "live" is the live demo.
  const inScene = (m) => !m.only || [].concat(m.only).includes(live ? 'live' : scene);
  const picked = empty ? [] : data.messages.filter(inScene);
  // The live demo moves every date by whole days so the mailbox's "today" is the visitor's,
  // times of day kept; one day less when the newest message would still be in the future.
  let shift = 0;
  if (live) {
    const t = now(), today = new RealDate(t.getFullYear(), t.getMonth(), t.getDate());
    shift = Math.round((today - parse(DEMO_DAY)) / 864e5);
    const newest = picked.reduce((a, m) => (m.date > a ? m.date : a), '');
    if (newest && addDays(parse(newest), shift) > t) shift -= 1;
  }
  const shiftDate = (s) => (shift ? fmt(addDays(parse(s), shift)) : s);
  // Quote headers in bodies ("On 09.10.2026 at 18:31, …") move with their messages.
  const shiftText = (s) => (shift ? s.replace(/\b(\d{2})\.(\d{2})\.(\d{4})\b/g, (all, d, m, y) => {
    const x = addDays(new RealDate(+y, +m - 1, +d), shift);
    return `${pad(x.getDate())}.${pad(x.getMonth() + 1)}.${x.getFullYear()}`;
  }) : s);

  const accounts = empty ? [] : clone(data.accounts);
  for (const a of accounts) if (a.imap && !a.imap.security) a.imap.security = 'tls';
  let rules = clone(data.rules || []);
  const trusted = new Set((data.trusted || []).map((e) => e.toLowerCase()));
  const older = empty ? {} : { ...(data.older || {}) };
  const incoming = live && !empty ? clone(data.incoming || []) : [];
  const olderMail = data.older_mail || {};
  const files = new Map(); // "demo:<n>" -> File chosen or dropped this session
  const autoReplies = new Map(); // "rule|sender" -> when the rule last answered
  const lastSync = {}; // account -> local date string
  let nextId = 1, nextFile = 1, checkedOnce = false, busy = false;

  const startLang = lang();
  const message = (m, extra = {}) => ({
    id: nextId++, folder: m.folder, from_name: m.from_name || '', from_email: m.from_email || '', to: m.to || '', cc: m.cc || '',
    bcc: m.bcc || '', subject: m.subject || '', date: m.date, size_kb: Math.max(1, m.size_kb || 1), unread: !!m.unread,
    flagged: !!m.flagged, remote_images: !!m.remote_images, note: (startLang === 'pl' && m.note_pl) || m.note || null,
    attachments: (m.attachments || []).map((a) => ({ name: a.name, size_kb: a.size_kb })), body: m.body || '', html: m.html || null,
    thread: m.thread || null, reply_to_line: m.reply_to || null, out: null, ...extra,
  });
  const msgs = picked.map((m) => message({ ...m, date: shiftDate(m.date), body: shiftText(m.body || '') }));
  const byId = (id) => msgs.find((m) => m.id === id);
  const need = (id) => byId(id) ?? fail('message {id} does not exist', 'wiadomość {id} nie istnieje', { id });

  /* ---------- folders and accounts ---------- */
  const acctId = (key) => String(key).split('/')[0];
  const account = (id) => accounts.find((a) => a.id === id);
  const needAccount = (id) => account(id) ?? fail('account "{a}" does not exist', 'konto „{a}” nie istnieje', { a: id });
  const folderOf = (key) => { const [a, f] = String(key).split('/'); return account(a)?.folders.find((x) => x.id === f); };
  const kindOf = (key) => folderOf(key)?.kind;
  const needFolder = (key) => folderOf(key) ?? fail('folder "{k}" does not exist', 'folder „{k}” nie istnieje', { k: key });
  const keyOfKind = (acct, kind) => { const f = account(acct)?.folders.find((x) => x.kind === kind); return f ? `${acct}/${f.id}` : null; };
  const PL_KIND = { inbox: 'Odebrane', outbox: 'Skrzynka nadawcza', drafts: 'Szkice', sent: 'Wysłane', archive: 'Archiwum', spam: 'Spam', trash: 'Kosz' };
  const isOutgoing = (kind) => kind === 'sent' || kind === 'drafts' || kind === 'outbox';
  for (const m of msgs) if (kindOf(m.folder) === 'outbox') m.out = { send_after: null, held: false };
  const defaultIdentity = (a) => a.identities.find((i) => i.is_default) ?? a.identities[0];

  /* ---------- text helpers ---------- */
  // Lowercase without diacritics (filters::fold, and the search index's remove_diacritics).
  const fold = (s) => String(s).normalize('NFD').replace(/[\u0300-\u036f]/g, '').replace(/[łŁ]/g, 'l').replace(/ß/g, 's')
    .replace(/[øØ]/g, 'o').toLowerCase();
  const escHtml = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  // smtp::split_addresses: commas and semicolons, except inside quotes and angle brackets.
  function splitAddresses(input) {
    const parts = []; let cur = '', quoted = false, angled = false;
    for (const c of String(input || '')) {
      if (c === '"') quoted = !quoted;
      else if (c === '<' && !quoted) angled = true;
      else if (c === '>' && !quoted) angled = false;
      else if ((c === ',' || c === ';') && !quoted && !angled) { parts.push(cur); cur = ''; continue; }
      cur += c;
    }
    parts.push(cur);
    return parts.map((p) => p.trim()).filter(Boolean);
  }
  // smtp::mailbox_parts: "Anna <a@x.pl>" -> [name, email].
  const mailboxParts = (entry) => {
    const i = entry.lastIndexOf('<');
    if (i < 0) return ['', entry.trim()];
    return [entry.slice(0, i).trim().replace(/^"+|"+$/g, '').trim(), entry.slice(i + 1).trim().replace(/>+$/, '').trim()];
  };
  const addressKey = (a) => {
    const s = a.trim(), o = s.lastIndexOf('<'), c = s.lastIndexOf('>');
    return (o >= 0 && c > o ? s.slice(o + 1, c) : s).trim().toLowerCase();
  };
  // list::display_name: the name in "Anna Kowalska <anna@x.pl>", else the whole text.
  function displayName(addr) {
    const s = String(addr || '').trim();
    if (!s) return '(no recipient)';
    const rest = s.startsWith('"') ? s.slice(1) : s;
    let end = rest.search(/["<]/); if (end < 0) end = rest.length;
    const name = rest.slice(0, end).trimEnd();
    let tail = rest.slice(end); if (tail.startsWith('"')) tail = tail.slice(1); tail = tail.trimStart();
    return name && tail.startsWith('<') ? name : s;
  }
  const subjectKey = (subject) => {
    let s = String(subject).toLowerCase();
    for (;;) { const p = ['re:', 'fwd:', 'fw:'].find((x) => s.startsWith(x)); if (!p) return s; s = s.slice(p.length).trimStart(); }
  };
  const stripPrefixes = (subject) => {
    let s = String(subject).trim();
    for (;;) {
      const p = ['re:', 'fwd:', 'fw:', 'odp:', 'pd:', 'aw:', 'wg:'].find((x) => s.toLowerCase().startsWith(x));
      if (!p) return s;
      s = s.slice(p.length).trimStart();
    }
  };
  const EMAIL = /^[^\s@<>(),;:"[\]\\]+@[^\s@<>(),;:"[\]\\]+\.[^\s@<>(),;:"[\]\\.]{2,}$/;
  const validMailbox = (part) => {
    const m = part.match(/^([^<]*)<([^<>]*)>\s*$/);
    return EMAIL.test((m ? m[2] : part).trim());
  };

  /* ---------- the message list (list.rs) ---------- */
  const summary = (m) => ({
    id: m.id, folder: m.folder, from_name: m.from_name, from_email: m.from_email, to: m.to, subject: m.subject,
    date: m.date, size_kb: m.size_kb, unread: m.unread, flagged: m.flagged, has_attachments: m.attachments.length > 0,
  });
  // The quick filter's operators: from: to: subject: is: has:file after: before: larger: smaller:.
  function tokens(filter) {
    const out = []; let cur = '', quoted = false;
    for (const c of filter) {
      if (c === '"') { quoted = !quoted; if (cur.includes(':')) cur += c; }
      else if (/\s/.test(c) && !quoted) { if (cur) { out.push(cur); cur = ''; } }
      else cur += c;
    }
    if (cur) out.push(cur);
    return out;
  }
  function parseDay(value) {
    const parts = value.split(/[-/.]/), n = (s) => (/^\d+$/.test(s) ? Number(s) : NaN);
    let y, m = 1, d = 1;
    if (parts.length === 1 && parts[0].length === 4) y = n(parts[0]);
    else if (parts.length === 2 && parts[0].length === 4) [y, m] = [n(parts[0]), n(parts[1])];
    else if (parts.length === 2 && parts[1].length === 4) [y, m] = [n(parts[1]), n(parts[0])];
    else if (parts.length === 3 && parts[0].length === 4) [y, m, d] = [n(parts[0]), n(parts[1]), n(parts[2])];
    else if (parts.length === 3 && parts[2].length === 4) [y, m, d] = [n(parts[2]), n(parts[1]), n(parts[0])];
    else return null;
    const x = new RealDate(y, m - 1, d);
    if (Number.isNaN(x.getTime()) || x.getFullYear() !== y || x.getMonth() !== m - 1 || x.getDate() !== d) return null;
    return `${y}-${pad(m)}-${pad(d)}T00:00:00`;
  }
  function parseKb(value) {
    const v = value.trim().toLowerCase().replace(/b+$/, '').trim();
    const i = v.search(/[a-z]/);
    const [number, unit] = i < 0 ? [v, 'k'] : [v.slice(0, i).trim(), v.slice(i)];
    const n = Number(number.replace(',', '.'));
    const factor = { k: 1, m: 1024, g: 1024 * 1024 }[unit];
    return number && !Number.isNaN(n) && factor && n >= 0 ? Math.round(n * factor) : null;
  }
  function parseTerms(filter) {
    const t = { text: '', from: [], to: [], subject: [], unread: null, flagged: null, has_file: false, after: null, before: null, larger: null, smaller: null };
    const words = [];
    for (const token of tokens(filter)) {
      const at = token.indexOf(':');
      if (at < 0) { words.push(token); continue; }
      const key = token.slice(0, at).toLowerCase(), value = token.slice(at + 1).replace(/^"+|"+$/g, '').trim();
      const known = ['from', 'to', 'subject', 'is', 'has', 'after', 'before', 'larger', 'smaller'].includes(key);
      if (known && !value) continue; // still being typed
      let used = true;
      const v = value.toLowerCase();
      if (key === 'from') t.from.push(v);
      else if (key === 'to') t.to.push(v);
      else if (key === 'subject') t.subject.push(v);
      else if (key === 'is') {
        if (v === 'unread' || v === 'new') t.unread = true;
        else if (v === 'read' || v === 'seen') t.unread = false;
        else if (v === 'flagged' || v === 'starred') t.flagged = true;
        else if (v === 'unflagged') t.flagged = false;
        else used = false;
      } else if (key === 'has') used = t.has_file = t.has_file || ['file', 'files', 'attachment', 'attachments'].includes(v);
      else if (key === 'after' || key === 'before') { const d = parseDay(value); if (d) t[key] = d; else used = false; }
      else if (key === 'larger' || key === 'smaller') { const k = parseKb(value); if (k != null) t[key] = k; else used = false; }
      else used = false;
      if (!used) words.push(token);
    }
    t.text = words.join(' ');
    return t;
  }
  // list.rs's telex_has: fields contain the needle, case ignored (a needle with spaces may span fields).
  const has = (needle, fields) => (/\s/.test(needle) ? fields.join(' ').toLowerCase().includes(needle)
    : fields.some((f) => String(f).toLowerCase().includes(needle)));
  // The full-text index: every word typed is the start of a word in the subject, the people or the text.
  const WORD = /[^\p{L}\p{N}]+/u;
  const ftsWords = (text) => text.split(/\s+/).map((w) => fold(w.replace(/"/g, ''))).flatMap((w) => w.split(WORD)).filter(Boolean);
  const ftsMatch = (m, words) => {
    const doc = fold([m.subject, m.from_name, m.from_email, m.to, m.body].join(' ')).split(WORD);
    return words.every((w) => doc.some((d) => d.startsWith(w)));
  };
  function conversation(id) {
    const m = need(id);
    if (!m.thread) return [id];
    const acct = acctId(m.folder);
    return msgs.filter((x) => x.id === id || (x.thread === m.thread && acctId(x.folder) === acct
      && !['spam', 'trash'].includes(kindOf(x.folder)))).map((x) => x.id);
  }
  function listed(view = {}) {
    const t = parseTerms(view.filter || '');
    let base;
    if (view.thread != null) { const ids = new Set(conversation(view.thread)); base = (m) => ids.has(m.id); }
    else if (view.folder) { needFolder(view.folder); base = (m) => m.folder === view.folder; }
    else base = (m) => !['spam', 'trash'].includes(kindOf(m.folder));
    const needle = t.text.trim().toLowerCase(), words = view.bodies ? ftsWords(t.text) : [];
    const match = (m) => base(m)
      && (!needle || has(needle, [m.from_name, m.from_email, m.subject, m.to]) || (words.length > 0 && ftsMatch(m, words)))
      && t.from.every((v) => has(v, [m.from_name, m.from_email]))
      && t.to.every((v) => has(v, [m.to, m.cc, m.bcc]))
      && t.subject.every((v) => has(v, [m.subject]))
      && (t.unread == null || m.unread === t.unread) && (t.flagged == null || m.flagged === t.flagged)
      && (!t.has_file || m.attachments.length > 0)
      && (!t.after || m.date >= t.after) && (!t.before || m.date < t.before)
      && (t.larger == null || m.size_kb >= t.larger) && (t.smaller == null || m.size_kb < t.smaller);
    const desc = !view.ascending, sort = view.sort || 'date';
    const fixed = view.thread == null && view.folder ? kindOf(view.folder) : null;
    const out = (m) => isOutgoing(fixed ?? kindOf(m.folder));
    const key = {
      date: (m) => m.date, size: (m) => m.size_kb, flag: (m) => (m.flagged ? 2 : 0) + (m.unread ? 1 : 0),
      from: (m) => (out(m) ? displayName(m.to) : m.from_name).toLowerCase(), subject: (m) => subjectKey(m.subject),
    }[sort] ?? ((m) => m.date);
    const cmp = (x, y) => (x < y ? -1 : x > y ? 1 : 0);
    // Ties: newest first, as the list always did; for dates the id breaks them.
    return msgs.filter(match).sort((a, b) => cmp(key(a), key(b)) * (desc ? -1 : 1)
      || (sort === 'date' ? (a.id - b.id) * (desc ? -1 : 1) : cmp(b.date, a.date) || b.id - a.id));
  }
  function listPage({ view, at, limit }) {
    const n = Math.min(500, Math.max(1, Number(limit) || 100));
    const all = listed(view), total = all.length;
    const place = (id) => { const i = all.findIndex((m) => m.id === id); return i < 0 ? fail('message {id} does not exist', 'wiadomość {id} nie istnieje', { id }) : i; };
    let start = 0, end = n;
    if (at === 'last') { start = Math.max(0, total - n); end = total; }
    else if (at && at.after != null) { start = place(at.after) + 1; end = start + n; }
    else if (at && at.before != null) { end = place(at.before); start = Math.max(0, end - n); }
    else if (at && at.around != null) { const i = all.findIndex((m) => m.id === at.around); start = i < 0 ? 0 : Math.max(0, i - (n >> 1)); end = start + n; }
    else if (at && at.row != null) { start = Math.min(Number(at.row) || 0, Math.max(0, total - n)); end = start + n; }
    end = Math.min(end, total); start = Math.min(start, end);
    return { total, start, rows: all.slice(start, end).map(summary) };
  }
  function listIds({ view, from, to }) {
    const ids = listed(view).map((m) => m.id), a = ids.indexOf(from), b = ids.indexOf(to);
    return from != null && to != null && a >= 0 && b >= 0 ? ids.slice(Math.min(a, b), Math.max(a, b) + 1) : ids;
  }
  function folderTree() {
    const counts = {};
    for (const m of msgs) {
      const c = (counts[m.folder] ??= { total: 0, unread: 0 });
      c.total++;
      if (m.unread) c.unread++;
    }
    for (const [key, n] of Object.entries(older)) if (n > 0 && folderOf(key)) (counts[key] ??= { total: 0, unread: 0 }).older = n;
    return { accounts, counts };
  }

  /* ---------- changes to mail (store.rs) ---------- */
  const setMany = (ids, field, value) => ids.reduce((n, id) => { const m = byId(id); if (m && m[field] !== value) { m[field] = value; return n + 1; } return n; }, 0);
  function moveMany(ids, folder) {
    needFolder(folder);
    const to = acctId(folder);
    const places = ids.map((id) => {
      const m = need(id);
      if (acctId(m.folder) !== to) {
        fail("can't move a message from {from} to {to}: they belong to different accounts",
          'nie można przenieść wiadomości z {from} do {to}: należą do różnych kont', { from: m.folder, to: folder });
      }
      return [id, m.folder];
    });
    for (const [id] of places) byId(id).folder = folder;
    return places;
  }
  function deleteMany(ids) {
    const plan = ids.map((id) => {
      const m = need(id), inTrash = kindOf(m.folder) === 'trash';
      const trash = inTrash ? null : keyOfKind(acctId(m.folder), 'trash')
        ?? fail('account "{account}" has no Trash folder', 'konto „{account}” nie ma folderu Kosz', { account: acctId(m.folder) });
      return [m, trash];
    });
    const report = { trashed: [], removed: 0 };
    for (const [m, trash] of plan) {
      if (trash) { report.trashed.push([m.id, m.folder]); m.folder = trash; }
      else { msgs.splice(msgs.indexOf(m), 1); report.removed++; }
    }
    return report;
  }
  // A draft or an unsent message, replaced by the one saved or sent from it.
  const removeLocal = (id) => { const m = byId(id); if (m && ['drafts', 'outbox'].includes(kindOf(m.folder))) msgs.splice(msgs.indexOf(m), 1); return m; };
  const addNote = (m, text) => { if (!m.note) m.note = text; else if (!m.note.includes(text)) m.note += ' ' + text; };

  /* ---------- drafts (drafts.rs) ---------- */
  const addressLine = (name, email) => {
    const n = String(name || '').trim();
    if (!n || n === email) return email;
    return /[,;"]/.test(n) ? `"${n.replace(/"/g, "'")}" <${email}>` : `${n} <${email}>`;
  };
  function identityFor(a, m) {
    const addressed = splitAddresses(`${m.to}, ${m.cc}, ${m.from_email}`).map(addressKey);
    return a.identities.find((i) => !i.is_default && addressed.includes(i.email.toLowerCase())) ?? defaultIdentity(a);
  }
  const layout = (sig, tail) => (sig ? (tail != null ? `\n\n${sig}\n\n${tail}` : `\n\n${sig}\n`) : tail != null ? `\n\n${tail}` : '\n');
  const textLines = (body) => body.replace(/\n+$/, '').split('\n').filter((l) => !l.startsWith('[img:'));
  function startDraft({ kind, of, account: acct }) {
    const m = kind === 'new' ? null : of != null ? need(of) : fail('Select a message first.', 'Najpierw wybierz wiadomość.');
    const a = needAccount(m ? acctId(m.folder) : acct);
    const me = m ? identityFor(a, m) : defaultIdentity(a);
    const sig = (me.signature || '').trimEnd();
    const draft = { account: a.id, identity: me.id, to: '', cc: '', subject: '', body: layout(sig, null), attachments: [], reply_to: null, forward_of: null };
    if (!m) return draft;
    const d = parse(m.date);
    const vars = {
      name: m.from_name.trim() ? m.from_name : m.from_email, email: m.from_email, from: addressLine(m.from_name, m.from_email),
      date: `${pad(d.getDate())}.${pad(d.getMonth() + 1)}.${d.getFullYear()}`, time: `${pad(d.getHours())}:${pad(d.getMinutes())}`, subject: m.subject,
    };
    const expand = (tpl) => ['name', 'email', 'from', 'date', 'time', 'subject'].reduce((s, k) => s.split(`{${k}}`).join(vars[k]), tpl || '');
    if (kind === 'forward') {
      return { ...draft, subject: `Fwd: ${stripPrefixes(m.subject)}`, attachments: m.attachments, forward_of: m.id,
        body: layout(sig, `${expand(me.forward_header)}\n\n${textLines(m.body).join('\n')}`) };
    }
    // Replying to your own message goes to its recipients again.
    const outgoing = isOutgoing(kindOf(m.folder));
    const to = splitAddresses(outgoing ? m.to : m.reply_to_line || addressLine(m.from_name, m.from_email)), cc = [];
    if (kind === 'reply_all') {
      const own = [...a.identities.map((i) => i.email.toLowerCase()), a.email.toLowerCase()];
      const seen = to.map(addressKey);
      const add = (list, line) => {
        for (const x of splitAddresses(line)) {
          const k = addressKey(x);
          if (!own.includes(k) && !seen.includes(k)) { seen.push(k); list.push(x); }
        }
      };
      if (!outgoing) add(to, m.to);
      add(cc, m.cc);
    }
    const quote = textLines(m.body).map((l) => (l.startsWith('>') ? '>' + l : '> ' + l)).join('\n');
    return { ...draft, to: to.join(', '), cc: cc.join(', '), subject: `Re: ${stripPrefixes(m.subject)}`, reply_to: m.id,
      body: layout(sig, `${expand(me.reply_header)}\n${quote}`) };
  }

  /* ---------- HTML compose (html.rs) ---------- */
  const linkify = (escaped) => escaped.replace(/https?:\/\/[^\s<]+/g, (u) => {
    const url = u.replace(/[.,);:!?]+$/, '');
    return `<a href="${url}">${url}</a>${u.slice(url.length)}`;
  });
  // html::from_text: a <div> per line, blank lines kept, ">" quotes as blockquotes, links.
  function fromText(text) {
    const lines = String(text).split(/\r?\n/);
    if (lines.length && lines[lines.length - 1] === '') lines.pop();
    let out = '';
    for (let i = 0; i < lines.length;) {
      if (lines[i].startsWith('>')) {
        const inner = [];
        while (i < lines.length && lines[i].startsWith('>')) { const l = lines[i].slice(1); inner.push(l.startsWith(' ') ? l.slice(1) : l); i++; }
        out += `<blockquote>${fromText(inner.join('\n'))}</blockquote>`;
      } else {
        const line = lines[i++];
        out += line.trim() ? `<div>${linkify(escHtml(line))}</div>` : '<div><br></div>';
      }
    }
    return out;
  }
  const COMPOSE_TAGS = new Set(['a', 'b', 'strong', 'i', 'em', 'u', 's', 'strike', 'br', 'p', 'div', 'span', 'ul', 'ol', 'li', 'blockquote',
    'pre', 'code', 'h1', 'h2', 'h3', 'hr']);
  const DROPPED = new Set(['script', 'style', 'head', 'title', 'template', 'iframe', 'object', 'noscript', 'svg', 'math']);
  // html::compose_clean: those tags, links to http(s) and mailto only, nothing else.
  function cleanHtml(html) {
    const doc = new DOMParser().parseFromString(`<body>${String(html)}</body>`, 'text/html');
    const walk = (node) => {
      let out = '';
      for (const n of node.childNodes) {
        if (n.nodeType === 3) out += n.nodeValue.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/\u00a0/g, '&nbsp;');
        else if (n.nodeType === 1) {
          const tag = n.tagName.toLowerCase();
          if (DROPPED.has(tag)) continue;
          if (!COMPOSE_TAGS.has(tag)) { out += walk(n); continue; }
          if (tag === 'br' || tag === 'hr') { out += `<${tag}>`; continue; }
          let attrs = '';
          if (tag === 'a') {
            const href = (n.getAttribute('href') || '').trim();
            if (/^(https?:|mailto:)/i.test(href)) attrs = ` href="${escHtml(href)}"`;
          }
          out += `<${tag}${attrs}>${walk(n)}</${tag}>`;
        }
      }
      return out;
    };
    return walk(doc.body).trim();
  }
  const decodeEntities = (s) => s.replace(/&(#x[0-9a-f]+|#\d+|amp|lt|gt|quot|apos|nbsp);/gi, (all, e) => {
    const k = e.toLowerCase();
    if (k[0] === '#') { const c = k[1] === 'x' ? parseInt(k.slice(2), 16) : parseInt(k.slice(1), 10); try { return String.fromCodePoint(c); } catch { return all; } }
    return { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'", nbsp: ' ' }[k];
  });
  // html::to_text: lines for blocks, "- " and "1. " for lists, ">" for quotes, a link's address after its text.
  function toText(html) {
    const lines = []; let line = '', hasText = false, quote = 0, pre = 0, blank = false;
    const lists = [], links = [];
    const emit = (l) => lines.push(((quote ? '>'.repeat(quote) + ' ' : '') + l.trimEnd()).trimEnd());
    const newline = () => { emit(line); line = ''; hasText = false; };
    const block = () => { if (hasText) newline(); };
    const push = (s) => {
      if (blank && s.trim()) { blank = false; if (!hasText && lines.length && lines[lines.length - 1] !== '') emit(''); }
      if (s) { line += s; hasText = hasText || !!s.trim(); }
    };
    const text = (s) => {
      if (!s) return;
      if (pre) { s.split('\n').forEach((p, i) => { if (i) newline(); push(p); }); return; }
      let c = '';
      for (const ch of s) {
        if (/\s/.test(ch)) { if (!(!hasText && !c) && !c.endsWith(' ') && !line.endsWith(' ')) c += ' '; }
        else c += ch;
      }
      if (c.trim() || hasText) push(c);
    };
    const tag = (src) => {
      const closing = src.startsWith('/');
      const name = (src.replace(/^\//, '').match(/^[a-z0-9]+/i)?.[0] || '').toLowerCase();
      if (name === 'br') newline();
      else if (name === 'p') { block(); blank = blank || closing || lines.length > 0; }
      else if (['div', 'h1', 'h2', 'h3'].includes(name)) block();
      else if (name === 'pre') { block(); pre = closing ? Math.max(0, pre - 1) : pre + 1; }
      else if (name === 'hr') { block(); line += '----------'; hasText = true; newline(); }
      else if (name === 'blockquote' && !closing) { block(); if (blank && lines.length && lines[lines.length - 1] !== '') emit(''); blank = false; quote++; }
      else if (name === 'blockquote') { block(); quote = Math.max(0, quote - 1); }
      else if ((name === 'ul' || name === 'ol') && !closing) { block(); lists.push(name === 'ol' ? 1 : null); }
      else if (name === 'ul' || name === 'ol') { block(); lists.pop(); }
      else if (name === 'li' && !closing) {
        block();
        const depth = Math.max(1, lists.length), last = lists.length - 1;
        let bullet = '- ';
        if (last >= 0 && lists[last] != null) { bullet = `${lists[last]}. `; lists[last]++; }
        line += '   '.repeat(depth - 1) + bullet;
      } else if (name === 'li') block();
      else if (name === 'a' && !closing) {
        const m = src.match(/href\s*=\s*("([^"]*)"|'([^']*)'|([^\s>]+))/i);
        links.push([decodeEntities(m ? m[2] ?? m[3] ?? m[4] ?? '' : ''), line.length]);
      } else if (name === 'a' && links.length) {
        const [href, start] = links.pop(), t = line.slice(start).trim(), bare = href.replace(/^mailto:/i, '');
        if (href && t !== href && t !== bare) text(` <${bare}>`);
      }
    };
    let rest = String(html);
    while (rest) {
      if (rest[0] === '<') {
        let i = 1, q = null;
        for (; i < rest.length; i++) { const c = rest[i]; if (q) { if (c === q) q = null; } else if (c === '"' || c === "'") q = c; else if (c === '>') break; }
        tag(rest.slice(1, i));
        rest = rest.slice(i + 1);
      } else {
        const end = rest.indexOf('<') < 0 ? rest.length : rest.indexOf('<');
        text(decodeEntities(rest.slice(0, end)));
        rest = rest.slice(end);
      }
    }
    block();
    while (lines.length && lines[lines.length - 1] === '') lines.pop();
    return lines.join('\n');
  }
  // html::compose_document's body: what an HTML message written in Telex shows.
  const composedHtml = (clean) => `<div style="font-family:Segoe UI,Arial,sans-serif;font-size:14px">${clean.replace(/<blockquote>/g,
    '<blockquote type="cite" style="margin:0 0 0 .8ex;border-left:2px solid #ccc;padding-left:1ex">')}</div>`;

  /* ---------- HTML mail (html.rs, colors.rs) ---------- */
  const FRAME_CSP = "default-src 'none'; img-src data:; style-src 'unsafe-inline'; font-src data:";
  const frameStyle = (p) => `html{background:${p ? p.paper : '#fff'};color-scheme:${p ? 'dark' : 'light'}}`
    + `body{margin:0;padding:12px 14px 28px;font:14px/1.5 'Segoe UI',system-ui,sans-serif;color:${p ? p.ink : '#1a1e23'};overflow-wrap:anywhere}`
    + `img{max-width:100%;height:auto}table{max-width:100%}a{color:${p ? p.link : '#2453a6'}}pre{white-space:pre-wrap}`
    + `blockquote{margin:0 0 0 2px;padding-left:10px;border-left:2px solid ${p ? p.line : '#d2d7de'};color:${p ? p.muted : '#59636e'}}`;
  // html::wrap: an inert document for the sandboxed frame, with its own CSP.
  const frameDoc = (body, p) => (frameOption && !p ? frameOption.replace('{body}', () => body)
    : '<!doctype html><html><head><meta charset="utf-8">'
    + `<meta http-equiv="Content-Security-Policy" content="${FRAME_CSP}">`
    + `<meta name="referrer" content="no-referrer"><style>${frameStyle(p)}</style></head><body>${body}</body></html>`);
  // colors.rs, short: on a dark theme's paper, light backgrounds turn dark and dark text light
  // (OKLab lightness, hue kept), white becoming the theme's background exactly.
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
      const [prop, ...rest] = d.split(':'), v = rest.join(':'), k = (prop || '').trim().toLowerCase();
      if (!v) return d;
      const role = k === 'color' ? false : k.startsWith('background') ? true : k.startsWith('border') ? null : undefined;
      return role === undefined ? d : `${prop}:${v.replace(hexes, (h) => recolor(h, role, paper, ink))}`;
    }).join(';') + '"').replace(/\b(bgcolor|color)="(#[0-9a-f]{6}|#[0-9a-f]{3})"/gi,
      (all, attr, hex) => `${attr}="${recolor(hex, attr.toLowerCase() === 'bgcolor', paper, ink)}"`);
  }
  // "Load images": the frame takes data: images only, and the demo has no network, so each
  // remote image becomes a drawing of its size, tinted after its address, with its alt text.
  function imageSvg(url, tagSrc) {
    const attr = (name) => tagSrc.match(new RegExp(`\\s${name}\\s*=\\s*"([^"]*)"`, 'i'))?.[1];
    const w = Math.min(1200, Math.max(40, Number(attr('width')) || 600)), h = Math.min(900, Math.max(24, Number(attr('height')) || Math.round(w * 0.42)));
    let hash = 0;
    for (const c of url) hash = (hash * 31 + c.charCodeAt(0)) >>> 0;
    const hue = 195 + (hash % 80), label = decodeEntities(attr('alt') || decodeURIComponent(url.split(/[?#]/)[0].split('/').pop() || 'image'));
    const size = Math.max(11, Math.min(h * 0.26, (w * 0.9) / Math.max(8, label.length * 0.55), 30));
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">`
      + `<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="hsl(${hue},38%,28%)"/>`
      + `<stop offset="1" stop-color="hsl(${hue + 35},34%,46%)"/></linearGradient>`
      + '<pattern id="p" width="16" height="16" patternUnits="userSpaceOnUse" patternTransform="rotate(35)">'
      + '<rect width="7" height="16" fill="#fff" fill-opacity=".06"/></pattern></defs>'
      + '<rect width="100%" height="100%" fill="url(#g)"/><rect width="100%" height="100%" fill="url(#p)"/>'
      + `<circle cx="${w - h * 0.5}" cy="${h * 0.5}" r="${h * 0.9}" fill="#fff" fill-opacity=".07"/>`
      + `<text x="50%" y="50%" dy=".35em" text-anchor="middle" font-family="Segoe UI,Arial,sans-serif" font-size="${size.toFixed(1)}" `
      + `font-weight="600" fill="#fff" fill-opacity=".93">${escHtml(label)}</text></svg>`;
    return 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(svg);
  }
  function getHtml({ id, loadImages, dark }) {
    const m = need(id);
    if (!m.html) return null;
    const spam = kindOf(m.folder) === 'spam', isTrusted = trusted.has(m.from_email.toLowerCase());
    const load = loadImages ?? (isTrusted && !spam);
    const remote = new Set();
    let html = m.html.replace(/<img\b[^>]*>/gi, (tag) => tag.replace(/\ssrc\s*=\s*"(https?:[^"]*)"/i, (_, url) => {
      remote.add(url);
      return load ? ` src="${imageSvg(url, tag)}"` : '';
    })).replace(/\sbackground\s*=\s*"(https?:[^"]*)"/gi, (_, url) => { remote.add(url); return ''; })
      .replace(/url\(\s*['"]?(https?:[^)'"]*)['"]?\s*\)/gi, (_, url) => { remote.add(url); return 'none'; });
    const p = dark && ['paper', 'ink', 'link', 'line', 'muted'].every((k) => /^#[0-9a-f]{3}([0-9a-f]{3})?$/i.test(String(dark[k] || ''))) ? dark : null;
    if (p) html = darkHtml(html, p);
    const n = remote.size;
    return { document: frameDoc(html, p), blocked: load ? 0 : n, loaded: load ? n : 0, sender: m.from_email, trusted: isTrusted };
  }

  /* ---------- sending (smtp.rs) ---------- */
  function checkFiles(attachments) {
    for (const a of attachments || []) {
      if (a.path && !files.has(a.path)) {
        fail('Attach "{name}" again: Telex attaches only files chosen in it.',
          'Dołącz plik „{name}” jeszcze raz: Telex dołącza tylko pliki wybrane w nim.', { name: a.name });
      }
    }
  }
  function checkRecipients(d) {
    const fields = [[d.to, tr('To', 'Do')], [d.cc, tr('Cc', 'DW')], [d.bcc, tr('Bcc', 'UDW')]];
    let count = 0;
    for (const [line, field] of fields) {
      for (const part of splitAddresses(line)) {
        if (!validMailbox(part)) fail('"{part}" in {field} isn\'t a valid address.', '„{part}” w polu {field} nie jest poprawnym adresem.', { part, field });
        count++;
      }
    }
    if (!count) fail('Add at least one recipient in To.', 'Dodaj co najmniej jednego odbiorcę w polu Do.');
  }
  // A message the user wrote, as the store keeps it: in Drafts or in the Outbox.
  function written(d, kind) {
    const a = needAccount(d.account);
    const me = a.identities.find((i) => i.id === d.identity) ?? defaultIdentity(a);
    const clean = d.html != null ? cleanHtml(d.html) : null;
    const body = clean != null ? toText(clean) : String(d.body || '');
    const attachments = (d.attachments || []).map((x) => ({ name: x.name, size_kb: Math.max(1, x.size_kb | 0) }));
    const orig = d.reply_to != null ? byId(d.reply_to) : null, replaced = d.replaces != null ? removeLocal(d.replaces) : null;
    let thread = replaced?.thread ?? null;
    if (orig) thread = orig.thread ??= `t${orig.id}`;
    return message({
      folder: keyOfKind(a.id, kind), from_name: me.name, from_email: me.email, to: d.to, cc: d.cc, bcc: d.bcc,
      subject: d.subject || '', date: stamp(), size_kb: Math.ceil((body.length + (clean ? clean.length * 2 : 0)) / 1024) + 2
        + attachments.reduce((n, x) => n + x.size_kb, 0), attachments, body, thread,
    }, { html: clean != null ? composedHtml(clean) : null, clean });
  }
  // `"Re: Estimate" to Anna Kowalska`
  function sentLine(m) {
    const first = splitAddresses(m.to)[0] || '';
    const i = first.indexOf('<');
    const who = i > 0 && first.slice(0, i).trim() ? first.slice(0, i).trim().replace(/^"+|"+$/g, '') : first.replace(/^<|>$/g, '');
    return tr('"{subject}" to {who}', '„{subject}” do {who}', { subject: m.subject, who });
  }
  // Send what is due in the account's Outbox: into Sent, oldest first.
  function sendOutbox(acct) {
    const box = keyOfKind(acct, 'outbox'), sent = keyOfKind(acct, 'sent'), t = nowMs(), lines = [];
    const due = msgs.filter((m) => m.folder === box && m.out && !m.out.held && (!m.out.send_after || m.out.send_after <= t)).sort((a, b) => a.id - b.id);
    for (const m of due) {
      if (!sent) { m.out = null; m.note = 'Sent. This account has no Sent folder, so it stays here.'; }
      else Object.assign(m, { folder: sent, date: stamp(), note: null, out: null });
      lines.push(sentLine(m));
    }
    return lines;
  }
  function sendMessage({ draft }) {
    checkFiles(draft.attachments);
    checkRecipients(draft);
    needAccount(draft.account);
    const wait = settings.undo_send | 0;
    const m = written(draft, 'outbox');
    m.out = { send_after: wait ? nowMs() + wait * 1000 : null, held: false };
    msgs.push(m);
    if (!wait) return { queued: m.id, wait: 0, sent: sendOutbox(m.folder.split('/')[0]), errors: [] };
    const acct = acctId(m.folder);
    setTimeout(() => emit('outbox-sent', { queued: m.id, sent: sendOutbox(acct), errors: [] }), wait * 1000 + 300);
    return { queued: m.id, wait, sent: [], errors: [] };
  }
  function holdMessage({ id }) {
    const m = byId(id);
    if (!m || !m.out || m.out.held || !m.out.send_after || m.out.send_after <= nowMs()) {
      fail('Too late to undo: the message is on its way.', 'Za późno, żeby cofnąć: wiadomość już wychodzi.');
    }
    m.out = { send_after: null, held: true };
    m.note = tr('Not sent: you took it back. Press E to change it and send it, or Del to drop it.',
      'Nie wysłano: cofnięto wysyłanie. Naciśnij E, żeby ją zmienić i wysłać, albo Del, żeby ją usunąć.');
  }
  function saveOutgoing({ draft }) {
    checkFiles(draft.attachments);
    const m = written(draft, 'drafts');
    msgs.push(m);
    return m.id;
  }

  /* ---------- rules (filters.rs) ---------- */
  function checkRule(r) {
    if (!String(r.name || '').trim()) fail('Give the rule a name.', 'Nadaj regule nazwę.');
    if (!r.conditions?.length) fail('Add at least one condition.', 'Dodaj co najmniej jeden warunek.');
    if (r.conditions.some((c) => !String(c.value || '').trim())) fail('Every condition needs some text to look for.', 'Każdy warunek potrzebuje szukanego tekstu.');
    if (!r.actions?.length) fail('Add at least one action.', 'Dodaj co najmniej jedno działanie.');
    const a = r.account ? account(r.account) ?? fail('That account no longer exists.', 'Tego konta już nie ma.') : null;
    if (r.actions.filter((x) => x.type === 'move' || x.type === 'delete').length > 1) {
      fail('A rule can move a message to one place only.', 'Reguła może przenieść wiadomość tylko w jedno miejsce.');
    }
    for (const x of r.actions) {
      if (x.type === 'move') {
        if (!a) fail("To move mail, pick the rule's account.", 'Żeby przenosić pocztę, wybierz konto reguły.');
        if (!a.folders.some((f) => `${a.id}/${f.id}` === x.folder)) fail('Pick the folder to move to.', 'Wybierz folder, do którego przenieść.');
      } else if (x.type === 'auto_reply') {
        if (!a) fail('To answer mail, pick the account that answers.', 'Żeby odpowiadać na pocztę, wybierz konto, które odpowiada.');
        if (!String(x.body || '').trim()) fail('Write the text of the automatic reply.', 'Napisz treść automatycznej odpowiedzi.');
      }
    }
  }
  function candidates(field, m) {
    const from = [m.from_email, m.from_name];
    const to = splitAddresses(`${m.to}, ${m.cc}`).flatMap((x) => {
      const i = x.indexOf('<');
      return i < 0 ? [x] : [x.slice(i + 1).replace(/>+$/, '').trim(), x.slice(0, i).trim().replace(/^"+|"+$/g, '')];
    });
    return { from, to, subject: [m.subject], body: [m.body], anywhere: [...from, ...to, m.subject, m.body] }[field] ?? [];
  }
  function conditionMatches(c, m) {
    const needle = fold(String(c.value).trim()), all = candidates(c.field, m).map(fold);
    const hit = (s) => (c.op === 'is' ? s === needle : c.op === 'starts_with' ? s.startsWith(needle) : c.op === 'ends_with' ? s.endsWith(needle) : s.includes(needle));
    return c.op === 'not_contains' ? !all.some(hit) : all.some(hit);
  }
  const ruleMatches = (r, m) => (r.match_all ? r.conditions.every((c) => conditionMatches(c, m)) : r.conditions.some((c) => conditionMatches(c, m)));
  const joinAnd = (parts) => (parts.length < 2 ? parts.join('')
    : tr('{rest} and {last}', '{rest} i {last}', { rest: parts.slice(0, -1).join(', '), last: parts[parts.length - 1] }));
  const looksAutomated = (address) => {
    const local = address.split('@')[0].toLowerCase().split('+')[0];
    return ['mailer-daemon', 'postmaster', 'noreply', 'no-reply', 'no_reply', 'donotreply', 'do-not-reply', 'do_not_reply', 'bounce', 'bounces',
      'listserv', 'majordomo'].includes(local) || local.startsWith('owner-') || local.endsWith('-request') || local.startsWith('bounce');
  };
  // RFC 3834: never robots, lists, yourself or Spam, and a sender at most once per rule every 4 days.
  function autoReply(rule, m, body) {
    const a = account(acctId(m.folder)), sender = m.from_email.trim(), key = `${rule.id}|${sender.toLowerCase()}`;
    if (!sender || sender.toLowerCase() === a.email.toLowerCase() || looksAutomated(sender) || kindOf(m.folder) === 'spam') return false;
    if (nowMs() - (autoReplies.get(key) ?? -Infinity) < 4 * 864e5) return false;
    const me = identityFor(a, m), subject = m.subject.trim();
    m.thread ??= `t${m.id}`;
    msgs.push(message({
      folder: keyOfKind(a.id, 'outbox'), from_name: me.name, from_email: me.email,
      to: !m.from_name.trim() || m.from_name === m.from_email ? sender : `"${m.from_name.replace(/"/g, "'")}" <${sender}>`,
      subject: subject.toLowerCase().startsWith('re:') ? subject : `Re: ${subject}`, date: stamp(), size_kb: 2, body, thread: m.thread,
    }, { out: { send_after: null, held: false } }));
    autoReplies.set(key, nowMs());
    return true;
  }
  // Run the enabled rules, top to bottom, on these messages; each one they change gets a note.
  function runRules(ids, arrival) {
    const report = { matched: 0, moved: 0, replied: 0, problems: [] };
    const enabled = rules.filter((r) => r.enabled);
    for (const id of ids) {
      const m = byId(id);
      if (!m || !enabled.length) continue;
      const acct = acctId(m.folder), was = { ...m }, notes = [];
      let dest = null;
      for (const rule of enabled.filter((r) => !r.account || r.account === acct)) {
        if (!ruleMatches(rule, was)) continue;
        const did = [], name = rule.name;
        for (const x of rule.actions) {
          if (x.type === 'mark_read' && was.unread) { m.unread = false; did.push(tr('marked it read', 'oznaczyła ją jako przeczytaną')); }
          else if (x.type === 'flag' && !was.flagged) { m.flagged = true; did.push(tr('flagged it', 'oznaczyła ją flagą')); }
          else if (x.type === 'move' && x.folder !== was.folder) {
            const f = folderOf(x.folder);
            if (f && acctId(x.folder) === acct) {
              did.push(tr('moved it to {to}', 'przeniosła ją do folderu {pl_to}', { to: f.name, pl_to: PL_KIND[f.kind] ?? f.name }));
              dest = { folder: x.folder };
            } else {
              report.problems.push(tr('rule "{name}" moves mail to a folder that no longer exists',
                'reguła „{name}” przenosi pocztę do folderu, którego już nie ma', { name }));
            }
          } else if (x.type === 'delete' && kindOf(was.folder) !== 'trash') { did.push(tr('moved it to Trash', 'przeniosła ją do Kosza')); dest = { trash: true }; }
          else if (x.type === 'auto_reply' && arrival && autoReply(rule, m, x.body)) { did.push(tr('answered it', 'odpowiedziała na nią')); report.replied++; }
        }
        if (did.length) notes.push(tr('Rule "{name}" {did}.', 'Reguła „{name}” {did}.', { name, did: joinAnd(did) }));
        if (rule.stop) break;
      }
      if (!notes.length) continue;
      report.matched++;
      addNote(m, notes.join(' '));
      if (dest?.folder) { m.folder = dest.folder; report.moved++; }
      else if (dest?.trash) { deleteMany([m.id]); report.moved++; }
    }
    return report;
  }
  function saveRule({ rule: r }) {
    checkRule(r);
    const clean = { id: r.id | 0, name: String(r.name).trim(), enabled: r.enabled !== false, account: r.account || null, match_all: r.match_all !== false,
      conditions: r.conditions.map((c) => ({ field: c.field, op: c.op, value: c.value })), actions: r.actions.map((x) => ({ ...x })), stop: !!r.stop };
    if (!clean.id) { clean.id = rules.reduce((n, x) => Math.max(n, x.id), 0) + 1; rules.push(clean); }
    else {
      const i = rules.findIndex((x) => x.id === clean.id);
      if (i < 0) fail('That rule no longer exists.', 'Tej reguły już nie ma.');
      rules[i] = clean;
    }
    return clean;
  }
  function moveRule({ id, delta }) {
    const from = rules.findIndex((r) => r.id === id);
    if (from < 0) return;
    const to = Math.min(rules.length - 1, Math.max(0, from + Math.sign(delta)));
    rules.splice(to, 0, rules.splice(from, 1)[0]);
  }

  /* ---------- identities ---------- */
  function saveIdentity({ identity: i }) {
    if (!String(i.name || '').trim()) fail('Enter the name people see.', 'Wpisz nazwę, którą widzą inni.');
    const email = String(i.email || '').trim(), at = email.indexOf('@');
    const ok = at > 0 && email.indexOf('@', at + 1) < 0 && email.slice(at + 1).includes('.') && !/\s/.test(email);
    if (!ok) fail('"{email}" doesn\'t look like an email address.', '„{email}” nie wygląda na adres e-mail.', { email });
    const a = needAccount(i.account);
    const saved = { id: i.id | 0, account: a.id, name: i.name.trim(), email, signature: i.signature ?? '', reply_header: i.reply_header ?? '',
      forward_header: i.forward_header ?? '', is_default: !!i.is_default };
    if (!saved.id) {
      saved.id = accounts.flatMap((x) => x.identities).reduce((n, x) => Math.max(n, x.id), 0) + 1;
      a.identities.push(saved);
    } else {
      const k = a.identities.findIndex((x) => x.id === saved.id);
      if (k < 0) fail('That identity no longer exists.', 'Tej tożsamości już nie ma.');
      a.identities[k] = saved;
    }
    if (saved.is_default) for (const x of a.identities) if (x.id !== saved.id) x.is_default = false;
    if (!a.identities.some((x) => x.is_default)) a.identities[0].is_default = true;
    return a.identities.find((x) => x.id === saved.id);
  }
  function deleteIdentity({ id }) {
    const a = accounts.find((x) => x.identities.some((i) => i.id === id));
    if (!a) return;
    if (a.identities.length <= 1) {
      fail('An account needs at least one identity. Change this one instead.', 'Konto potrzebuje co najmniej jednej tożsamości. Zmień tę zamiast ją usuwać.');
    }
    a.identities = a.identities.filter((i) => i.id !== id);
    if (!a.identities.some((x) => x.is_default)) a.identities[0].is_default = true;
  }

  /* ---------- people to suggest (recipients.rs) ---------- */
  function recipients() {
    const own = accounts.flatMap((a) => [a.email, ...a.identities.map((i) => i.email)]).map((e) => e.toLowerCase());
    const robot = (email) => {
      const s = email.split('@')[0].replace(/[^\p{L}\p{N}]/gu, '');
      return ['noreply', 'donotreply', 'mailerdaemon', 'postmaster', 'bounce', 'notreply'].some((r) => s.startsWith(r) || s.endsWith(r));
    };
    const people = new Map();
    const note = (name, email, at, count, sent) => {
      email = email.trim();
      const key = email.toLowerCase();
      if (!email.includes('@') || own.includes(key) || robot(key)) return;
      const p = people.get(key) ?? { name: '', email, sent: 0, received: 0, last: '' };
      people.set(key, p);
      p[sent ? 'sent' : 'received'] += count;
      if (at >= p.last) { p.last = at; if (name) p.name = name; } else if (!p.name) p.name = name;
    };
    for (const m of msgs) {
      const kind = kindOf(m.folder);
      if (isOutgoing(kind)) for (const e of splitAddresses(`${m.to}, ${m.cc}, ${m.bcc}`)) { const [n, a] = mailboxParts(e); note(n, a, m.date, 1, true); }
    }
    const received = new Map();
    for (const m of msgs) {
      const kind = kindOf(m.folder);
      if (isOutgoing(kind) || kind === 'spam' || kind === 'trash') continue;
      const k = m.from_email.toLowerCase(), r = received.get(k) ?? { name: '', email: m.from_email, count: 0, at: '' };
      r.count++;
      if (m.date >= r.at) { r.at = m.date; r.name = m.from_name.toLowerCase() === m.from_email.toLowerCase() ? '' : m.from_name; }
      received.set(k, r);
    }
    for (const r of received.values()) note(r.name, r.email, r.at, r.count, false);
    return [...people.values()].sort((a, b) => (b.sent * 5 + b.received) - (a.sent * 5 + a.received) || (b.last > a.last ? 1 : b.last < a.last ? -1 : 0)
      || (a.email < b.email ? -1 : 1)).slice(0, 2000).map(({ name, email, sent, received: got }) => ({ name, email, sent, received: got }));
  }

  /* ---------- usage (usage.rs) ---------- */
  function usage() {
    const t = now(), today = new RealDate(t.getFullYear(), t.getMonth(), t.getDate()), since = addDays(today, -29), sinceDay = fmt(since).slice(0, 10);
    const quotas = data.quota || {};
    return {
      base_bytes: 61_800_000 + msgs.length * 24_000, free_bytes: 0, since: sinceDay,
      accounts: accounts.map((a) => {
        const mine = msgs.filter((m) => acctId(m.folder) === a.id);
        const folders = a.folders.map((f) => {
          const key = `${a.id}/${f.id}`, ms = mine.filter((m) => m.folder === key);
          // The demo mailbox is small: sizes are scaled up to look like a real one.
          return { folder: key, kind: f.kind, count: ms.length, unread: ms.filter((m) => m.unread).length, size_kb: ms.reduce((n, m) => n + m.size_kb, 0) * 40 };
        }).filter((f) => f.count).sort((x, y) => y.size_kb - x.size_kb);
        const flow = Array.from({ length: 30 }, (_, i) => {
          const d = fmt(addDays(since, i)).slice(0, 10), on = mine.filter((m) => m.date.slice(0, 10) === d), k = (m) => kindOf(m.folder);
          // A steady trickle keeps the chart honest-looking.
          const base = ((i * 7) % 5) + (i % 7 === 5 || i % 7 === 6 ? 0 : 3);
          return { received: on.filter((m) => !isOutgoing(k(m)) && k(m) !== 'spam').length + base,
            sent: on.filter((m) => k(m) === 'sent').length + (i % 3 === 0 ? 1 : 0), spam: on.filter((m) => k(m) === 'spam').length + (i % 4 === 1 ? 1 : 0) };
        });
        const counts = new Map();
        for (const m of mine.filter((x) => !isOutgoing(kindOf(x.folder)) && kindOf(x.folder) !== 'spam')) {
          const c = counts.get(m.from_email) ?? { name: m.from_name, email: m.from_email, count: 0 };
          c.count += 6; counts.set(m.from_email, c);
        }
        return {
          account: a.id, quota: quotas[a.id] ?? null, synced_at: lastSync[a.id] ?? null, sync_error: null, folders, flow,
          senders: [...counts.values()].sort((x, y) => y.count - x.count).slice(0, 5),
          largest: [...mine].sort((x, y) => y.size_kb - x.size_kb).slice(0, 5).map(summary), queued: 0,
        };
      }),
    };
  }

  /* ---------- checking mail ---------- */
  function emitProgress(folder, done, total) {
    const a = account(acctId(folder)), f = folderOf(folder);
    emit('sync-progress', { account: a?.name ?? acctId(folder), folder: f?.name ?? folder, done, total });
  }
  async function checkMail({ account: only = null } = {}) {
    if (busy) return { new: [], new_in: null, sent: [], sorted: 0, errors: [], busy: true };
    busy = true;
    try {
      const out = { new: [], new_in: null, sent: [], sorted: 0, errors: [], busy: false };
      const list = accounts.filter((a) => a.imap && (!only || a.id === only));
      // The check at start finds the mailbox as the demo left it.
      if (!checkedOnce) {
        checkedOnce = true;
        await later(300);
        for (const a of list) lastSync[a.id] = stamp();
        return out;
      }
      for (const a of list) out.sent.push(...sendOutbox(a.id));
      // One new message per check, from the mailbox's queue; with an account (IDLE), its Inbox only.
      const i = incoming.findIndex((m) => list.some((a) => a.id === acctId(m.folder)) && (!only || kindOf(m.folder) === 'inbox'));
      if (i >= 0) {
        const item = incoming.splice(i, 1)[0];
        await later(250);
        emitProgress(item.folder, 0, 1);
        await later(400);
        const m = message({ ...item, date: stamp(), unread: true });
        msgs.push(m);
        out.new.push(m.id);
        emitProgress(item.folder, 1, 1);
        if (kindOf(item.folder) === 'inbox') {
          const r = runRules([m.id], true);
          out.sorted += r.matched;
          if (r.replied) out.sent.push(...sendOutbox(acctId(item.folder)));
          out.errors.push(...r.problems.map((p) => `${account(acctId(item.folder)).name}: ${p}`));
        }
        out.new_in = byId(m.id)?.folder ?? null;
        await later(100);
      } else await later(650);
      for (const a of list) lastSync[a.id] = stamp();
      return out;
    } finally { busy = false; }
  }
  // Older mail: plausible messages from the mailbox's templates, dated before the folder's oldest.
  async function downloadOlder({ folder, limit }) {
    needFolder(folder);
    if (busy) return { new: 0, busy: true };
    const n = Math.min(Math.max(1, limit | 0), older[folder] || 0);
    if (!n) return { new: 0, busy: false };
    busy = true;
    try {
      const acct = acctId(folder), templates = olderMail[acct] || [];
      if (!templates.length) return { new: 0, busy: false };
      let seed = n * 7919 + folder.length;
      const rnd = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);
      const oldest = msgs.filter((m) => m.folder === folder).reduce((a, m) => (!a || m.date < a ? m.date : a), '') || stamp();
      let at = parse(oldest);
      const counters = templates.map((tp) => tp.start ?? 0), uses = templates.map(() => 0), made = [], monthly = new Set();
      const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];
      emitProgress(folder, 0, n);
      for (let k = 0; k < n; k++) {
        at = new RealDate(at.getFullYear(), at.getMonth(), at.getDate() - 1 - Math.floor(rnd() * 2.2), 7 + Math.floor(rnd() * 13), Math.floor(rnd() * 60));
        // A template with a `day` comes once a month, about that day; numbered ones until their number runs out.
        let ti = templates.findIndex((tp, i) => tp.day && at.getDate() >= tp.day && at.getDate() <= tp.day + 2 && !monthly.has(`${i}/${at.getFullYear()}/${at.getMonth()}`));
        if (ti >= 0) monthly.add(`${ti}/${at.getFullYear()}/${at.getMonth()}`);
        else {
          const free = templates.map((tp, i) => i).filter((i) => !templates[i].day && (templates[i].start == null || counters[i] > 0));
          const weight = (i) => templates[i].weight ?? 1, total = free.reduce((s, i) => s + weight(i), 0);
          let pick = rnd() * total;
          ti = free.find((i) => (pick -= weight(i)) < 0) ?? free[free.length - 1] ?? 0;
        }
        const tp = templates[ti], nr = counters[ti];
        counters[ti] = nr - (tp.step ?? 1);
        const subjects = tp.subjects || [tp.subject || ''];
        const vars = { n: nr, month: MONTHS[(at.getMonth() + 11) % 12] };
        made.push(message({
          folder, from_name: tp.from_name, from_email: tp.from_email, to: tp.to || account(acct).email, subject: fill(subjects[uses[ti]++ % subjects.length], vars),
          date: fmt(at), size_kb: Math.max(1, Math.round((tp.size_kb || 4) * (0.8 + rnd() * 0.4))), attachments: tp.attachments || [], body: fill(tp.body || '', vars),
        }));
        if (live && k === Math.floor(n / 2)) { emitProgress(folder, k, n); await later(300); }
      }
      msgs.push(...made);
      older[folder] -= n;
      await later(300);
      emitProgress(folder, n, n);
      return { new: n, busy: false };
    } finally { busy = false; }
  }

  /* ---------- settings (settings.rs) ---------- */
  const CHOICES = {
    theme: THEMES, language: ['auto', 'en', 'pl'], density: ['compact', 'comfortable'], text_size: [90, 100, 110, 125, 150, 175, 200],
    mark_read: ['moment', 'at_once', 'manual'], check_every: [0, 5, 10, 15, 30, 60], mail_paper: ['theme', 'white'], compose_format: ['plain', 'html'],
    reply_format: ['same', 'plain', 'html'], undo_send: [0, 5, 10, 20, 30], updates: ['ask', 'daily', 'off'],
  };
  const SWITCHES = ['prefer_text', 'push_mail', 'notify', 'ask_subject', 'spell_check'];
  const settingsView = () => ({ settings: { ...settings }, language: lang(), offer: null, fresh: false });
  function updateSettings({ changes }) {
    for (const [k, v] of Object.entries(changes || {})) {
      if (CHOICES[k]?.includes(v) || (SWITCHES.includes(k) && typeof v === 'boolean')) settings[k] = v;
    }
    return settingsView();
  }

  /* ---------- finding servers (autoconfig.rs) ---------- */
  const PROVIDERS = [
    ['Gmail', ['gmail.com', 'googlemail.com'], 'imap.gmail.com', ['smtp.gmail.com', 465, 'tls'], 'app_password', null, 'https://myaccount.google.com/apppasswords'],
    ['Outlook.com', ['outlook.com', 'hotmail.com', 'live.com', 'msn.com', 'hotmail.co.uk', 'outlook.co.uk', 'live.co.uk'], 'outlook.office365.com',
      ['smtp-mail.outlook.com', 587, 'starttls'], null, 'microsoft_sign_in'],
    ['iCloud Mail', ['icloud.com', 'me.com', 'mac.com'], 'imap.mail.me.com', ['smtp.mail.me.com', 587, 'starttls'], 'app_password'],
    ['Yahoo Mail', ['yahoo.com', 'yahoo.co.uk', 'ymail.com', 'rocketmail.com'], 'imap.mail.yahoo.com', ['smtp.mail.yahoo.com', 465, 'tls'], 'app_password'],
    ['Proton Mail', ['proton.me', 'protonmail.com', 'protonmail.ch', 'pm.me'], '127.0.0.1', ['127.0.0.1', 1025, 'starttls'], null, 'proton_bridge'],
    ['WP Poczta', ['wp.pl'], 'imap.wp.pl', ['smtp.wp.pl', 465, 'tls'], 'turn_on_imap', null, null, true],
    ['o2 Poczta', ['o2.pl', 'go2.pl', 'tlen.pl'], 'poczta.o2.pl', ['poczta.o2.pl', 465, 'tls'], 'turn_on_imap', null, null, true],
    ['Onet Poczta', ['onet.pl', 'onet.eu', 'op.pl', 'vp.pl', 'poczta.onet.pl'], 'imap.poczta.onet.pl', ['smtp.poczta.onet.pl', 465, 'tls'], 'turn_on_imap'],
  ];
  function knownProvider({ email }) {
    const [local, domain = ''] = String(email || '').trim().toLowerCase().split('@');
    const p = PROVIDERS.find((x) => x[1].includes(domain));
    if (!local || !p) return null;
    const [name, , imap, [smtp, smtpPort, smtpSecurity], note = null, unsupported = null, help = null, localPart = false] = p;
    return {
      setup: { name: '', email: String(email).trim(), host: imap, port: 993, username: localPart ? local : String(email).trim(), imap_security: 'tls',
        smtp_host: smtp, smtp_port: smtpPort, smtp_security: smtpSecurity },
      provider: name, source: 'list', note, unsupported, help,
    };
  }
  async function detectSettings({ email }) {
    const known = knownProvider({ email });
    if (known) return known;
    const domain = String(email || '').trim().split('@')[1];
    if (!domain || !domain.includes('.')) return null;
    emit('detect-progress', 'lookup');
    await later(700);
    return {
      setup: { name: '', email, host: `mail.${domain}`, port: 993, username: email, imap_security: 'tls', smtp_host: `mail.${domain}`, smtp_port: 465, smtp_security: 'tls' },
      provider: null, source: 'srv', note: null, unsupported: null, help: null,
    };
  }

  /* ---------- files from this computer ---------- */
  const RISKY = new Set(['exe', 'com', 'scr', 'pif', 'bat', 'cmd', 'msi', 'msp', 'mst', 'msix', 'msixbundle', 'appx', 'appxbundle', 'application', 'appref-ms',
    'gadget', 'cpl', 'msc', 'inf', 'reg', 'scf', 'lnk', 'url', 'hta', 'chm', 'js', 'jse', 'vbs', 'vbe', 'wsf', 'wsh', 'wsc', 'ps1', 'psm1', 'psd1', 'ps1xml',
    'jar', 'iso', 'img', 'vhd', 'vhdx', 'dll', 'sys', 'docm', 'dotm', 'xlsm', 'xltm', 'xlam', 'pptm', 'potm', 'ppam', 'sh', 'bash', 'run', 'desktop',
    'appimage', 'deb', 'rpm', 'py', 'pyw', 'pl', 'rb']);
  const isRisky = (name) => { const n = String(name).trim().replace(/[. ]+$/, '').toLowerCase(), i = n.lastIndexOf('.'); return i >= 0 && RISKY.has(n.slice(i + 1)); };
  // A file the visitor chose or dropped: only its name and size go into the mail; the File stays here.
  function addFile(file) {
    const path = `demo:${nextFile++}`;
    files.set(path, file);
    return { name: file.name || 'file', size_kb: Math.max(1, Math.ceil((file.size || 0) / 1024)), path };
  }
  function pickFiles() {
    return new Promise((resolve) => {
      const input = document.createElement('input');
      input.type = 'file'; input.multiple = true; input.style.display = 'none';
      let done = false;
      const finish = (list) => { if (done) return; done = true; input.remove(); resolve(list); };
      input.addEventListener('change', () => finish([...(input.files || [])].map(addFile)));
      input.addEventListener('cancel', () => finish([]));
      document.body.append(input);
      input.click();
    });
  }
  function openAttachment({ id, index, sure }) {
    const m = need(id), a = m.attachments[index];
    if (!a) fail('message {id} has no attachment {index}', 'wiadomość {id} nie ma załącznika {index}', { id, index });
    if (!sure && isRisky(a.name)) return 'risky';
    if (!sure && kindOf(m.folder) === 'spam') return 'spam';
    return refuse('open');
  }

  /* ---------- the commands ---------- */
  const commands = {
    folder_tree: folderTree,
    list_page: listPage,
    list_ids: listIds,
    get_message: ({ id }) => {
      const m = need(id);
      return { id: m.id, folder: m.folder, from_name: m.from_name, from_email: m.from_email, to: m.to, cc: m.cc,
        bcc: isOutgoing(kindOf(m.folder)) ? m.bcc : '', subject: m.subject, date: m.date, size_kb: m.size_kb, unread: m.unread, flagged: m.flagged,
        remote_images: m.remote_images, note: m.note, attachments: m.attachments, body: m.body };
    },
    set_unread: ({ id, unread }) => { need(id).unread = !!unread; },
    set_flagged: ({ id, flagged }) => { need(id).flagged = !!flagged; },
    set_unread_many: ({ ids, unread }) => setMany(ids, 'unread', !!unread),
    set_flagged_many: ({ ids, flagged }) => setMany(ids, 'flagged', !!flagged),
    mark_folder_read: ({ folder }) => { needFolder(folder); return setMany(msgs.filter((m) => m.folder === folder).map((m) => m.id), 'unread', false); },
    move_message: ({ id, folder }) => { moveMany([id], folder); },
    move_messages: ({ ids, folder }) => moveMany(ids, folder),
    delete_message: ({ id }) => (deleteMany([id]).trashed.length ? 'moved_to_trash' : 'removed'),
    delete_messages: ({ ids }) => deleteMany(ids),
    get_html: getHtml,
    trusted_senders: () => [...trusted].sort((a, b) => a.localeCompare(b)),
    trust_sender: ({ email, trusted: yes }) => { const e = String(email).trim().toLowerCase(); if (yes) trusted.add(e); else trusted.delete(e); },
    open_link: ({ url }) => {
      const u = String(url).trim();
      if (!/^https?:\/\//i.test(u)) fail('Telex opens only web links (http and https).', 'Telex otwiera tylko linki do stron (http i https).');
      window.open(u, '_blank', 'noopener,noreferrer');
    },
    usage,
    recipients,
    start_draft: startDraft,
    save_outgoing: saveOutgoing,
    draft_html: ({ id }) => need(id).clean ?? null,
    text_to_html: ({ text }) => fromText(text),
    html_to_text: ({ html }) => toText(cleanHtml(html)),
    clean_html: ({ html }) => cleanHtml(html),
    send_message: sendMessage,
    hold_message: holdMessage,
    search: ({ query }) => listed({ filter: query, bodies: true }).map((m) => m.id).slice(0, 5000),
    check_mail: checkMail,
    push_changes: () => ({ new: [], new_in: null, sent: [], sorted: 0, errors: [], busy }),
    download_older: downloadOlder,
    list_rules: () => rules,
    save_rule: saveRule,
    delete_rule: ({ id }) => { rules = rules.filter((r) => r.id !== id); },
    move_rule: moveRule,
    run_rules: ({ folder }) => { needFolder(folder); return runRules(msgs.filter((m) => m.folder === folder).map((m) => m.id), false); },
    save_identity: saveIdentity,
    delete_identity: deleteIdentity,
    get_settings: settingsView,
    update_settings: updateSettings,
    update_info: () => ({ version, updatable: false }),
    check_update: () => null,
    install_update: () => refuse('update'),
    known_provider: knownProvider,
    detect_settings: detectSettings,
    // Screenshots show the login still running; the live demo says it can't.
    add_account: () => (live ? later(900).then(() => refuse('servers')) : new Promise(() => {})),
    remove_account: () => refuse('remove'),
    pick_files: pickFiles,
    open_attachment: openAttachment,
    save_attachment: () => refuse('save'),
    save_attachments: () => refuse('save'),
    show_saved: () => refuse('save'),
    export_settings: () => refuse('export'),
    show_export: () => refuse('export'),
    open_import: () => refuse('import'),
    apply_import: () => refuse('import'),
  };
  const asText = (e) => (typeof e === 'string' ? e : String(e?.message ?? e));
  // Like Tauri's invoke: arguments and answers cross as JSON; a failure is the core's sentence.
  // The command starts at once (not after a tick), so a file dialog still counts as the click's.
  function invoke(cmd, args) {
    const run = commands[cmd];
    if (!run) return Promise.reject(tr('The demo doesn’t know “{cmd}”.', 'Demo nie zna polecenia „{cmd}”.', { cmd }));
    let result;
    try { result = run(clone(args ?? {}) ?? {}); }
    catch (e) { if (typeof e !== 'string') console.error(e); return Promise.reject(asText(e)); }
    return Promise.resolve(result).then(clone, (e) => { if (typeof e !== 'string') console.error(e); return Promise.reject(asText(e)); });
  }

  /* ---------- events ---------- */
  const handlers = new Map();
  let eventId = 0;
  function listen(name, handler) {
    if (!handlers.has(name)) handlers.set(name, new Set());
    handlers.get(name).add(handler);
    return Promise.resolve(() => handlers.get(name)?.delete(handler));
  }
  function emit(name, payload) {
    for (const h of [...(handlers.get(name) || [])]) {
      try { h({ event: name, id: ++eventId, payload: clone(payload) }); } catch (e) { console.error(e); }
    }
  }
  // Files dropped on the window go to compose, as in the app.
  const hasFiles = (e) => [...(e.dataTransfer?.types || [])].includes('Files');
  window.addEventListener('dragover', (e) => { if (hasFiles(e)) { e.preventDefault(); e.dataTransfer.dropEffect = 'copy'; } });
  window.addEventListener('drop', (e) => {
    if (!hasFiles(e)) return;
    e.preventDefault();
    const items = [...(e.dataTransfer.items || [])];
    const list = [...e.dataTransfer.files].filter((f, i) => !items[i]?.webkitGetAsEntry?.()?.isDirectory).map(addFile);
    if (list.length) emit('files-dropped', list);
  });

  /* ---------- the window ---------- */
  const inFrame = (() => { try { return window.parent !== window; } catch { return true; } })();
  const appWindow = {
    setTitle(title) {
      document.title = title;
      if (live && inFrame) { try { window.parent.postMessage({ type: 'telex-demo-title', title }, location.origin); } catch { /* another origin */ } }
      return Promise.resolve();
    },
  };
  // Text size (Ctrl+= / Ctrl+− / Ctrl+0): the app zooms its webview, which also narrows the
  // window's CSS width; here the page zooms, and the stylesheets' max-width rules that the
  // narrower width would turn on are copied in, so the toolbar folds up as it does in the app.
  let zoom = 1, zoomStyle = null;
  function zoomedMedia() {
    let css = '';
    if (zoom > 1) {
      const width = innerWidth / zoom;
      for (const sheet of document.styleSheets) {
        if (sheet.ownerNode === zoomStyle) continue;
        let rules = [];
        try { rules = [...sheet.cssRules]; } catch { /* another origin */ }
        for (const r of rules) {
          const limit = r instanceof CSSMediaRule && /^\(max-width:\s*(\d+)px\)$/.exec(r.conditionText ?? r.media.mediaText);
          if (limit && width <= +limit[1] && innerWidth > +limit[1]) css += [...r.cssRules].map((x) => x.cssText).join('\n') + '\n';
        }
      }
    }
    if (!zoomStyle && css) { zoomStyle = document.createElement('style'); zoomStyle.dataset.demo = 'zoom'; }
    if (zoomStyle) { zoomStyle.textContent = css; document.head.append(zoomStyle); }
  }
  addEventListener('resize', () => { if (zoom > 1) zoomedMedia(); });
  const webview = {
    setZoom(z) {
      zoom = Number(z) || 1;
      document.documentElement.style.zoom = zoom === 1 ? '' : String(zoom);
      if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', zoomedMedia, { once: true }); else zoomedMedia();
      return Promise.resolve();
    },
  };
  window.__TAURI__ = {
    core: { invoke },
    event: { listen, emit: (name, payload) => { emit(name, payload); return Promise.resolve(); } },
    window: { getCurrentWindow: () => appWindow },
    webview: { getCurrentWebview: () => webview },
  };
  window.__TELEX_DEMO__ = { emit, addFile, live };

  if (live) {
    // The look boot.js puts on the page before the first paint: this visit's theme and language.
    try {
      localStorage.setItem('telex-look', JSON.stringify({ theme: settings.theme, lang: lang(), zoom: 100 }));
      localStorage.removeItem('telex-ui');
    } catch { /* storage blocked: boot.js guesses, app.js corrects */ }
    // IMAP IDLE: a while after the start, the server says something arrived in an Inbox.
    const idle = data.idle;
    if (idle?.account) {
      setTimeout(() => {
        if (settings.push_mail && incoming.some((m) => acctId(m.folder) === idle.account)) emit('inbox-changed', idle.account);
      }, (idle.after_s ?? 40) * 1000);
    }
  }
}

if (typeof window !== 'undefined' && window.TELEX_DEMO_DATA && !window.TELEX_DEMO_MANUAL) {
  const q = new URLSearchParams(location.search);
  installTelexDemo({
    data: window.TELEX_DEMO_DATA.mailbox, version: window.TELEX_DEMO_DATA.version, live: true,
    theme: q.get('theme') || 'system', lang: q.get('lang') || 'auto',
  });
}
