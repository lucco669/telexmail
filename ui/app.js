// Telex UI. Plain JS, no build step: Tauri serves this folder as-is.
// All mail data comes from the Rust core (crates/telex-core) through `invoke`.
// Every word on screen goes through t() or tn() from i18n.js, in English here; i18n.js has Polish.
(() => {
'use strict';

const invoke = window.__TAURI__?.core?.invoke;
// A message open in a window of its own (index.html?message=<id>): the same page, showing just
// that message. Changes made there reach the main window, which refreshes and tells the server.
const VIEWER = Number(new URLSearchParams(location.search).get('message')) || null;
const tellMain = payload => window.__TAURI__?.event?.emitTo?.('main', 'viewer-did', payload ?? {});
// Events about checks, sending and updates are the main window's business.
const listenMain = (event, fn) => { if (!VIEWER) window.__TAURI__?.event?.listen(event, fn); };
const ownLabel = () => window.__TAURI__?.window?.getCurrentWindow?.().label ?? 'main';
// Events the core sends to one window (emit_to): without a target, every window would hear them.
const listenHere = (event, fn) => window.__TAURI__?.event?.listen(event, fn, {target:ownLabel()});
const I18n = window.TelexI18n;
const {t, tn} = I18n;
// Marks an English text for translation where it is defined; t() or tn() translates it where it is drawn.
const N_ = s => s;

/* ---------- icons ---------- */
const sv = (p, extra = '') => `<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.35" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" ${extra}>${p}</svg>`;
const I = {
  get: sv('<path d="M8 2v7.5M5 6.5l3 3 3-3"/><path d="M2 10v3.5h12V10"/>'),
  neu: sv('<path d="M11 2.5L13.5 5 6 12.5H3.5V10z"/><path d="M9.5 4L12 6.5"/>'),
  reply: sv('<path d="M6.5 4L3 7.5 6.5 11"/><path d="M3.5 7.5H9a4 4 0 014 4v1"/>'),
  replyall: sv('<path d="M8.5 4L5 7.5 8.5 11"/><path d="M5 4L1.5 7.5 5 11"/><path d="M5.5 7.5H10a4 4 0 014 4v1"/>'),
  forward: sv('<path d="M9.5 4L13 7.5 9.5 11"/><path d="M12.5 7.5H7a4 4 0 00-4 4v1"/>'),
  trash: sv('<path d="M2.5 4.5h11M6.5 4.5V2.8h3v1.7M4 4.5l.7 9h6.6l.7-9"/>'),
  flag: sv('<path d="M3.5 14.5V2.5"/><path d="M3.5 3h9l-2 3 2 3h-9"/>'),
  flagOff: sv('<path d="M3.5 14.5V2.5"/><path d="M3.5 3h9l-2 3 2 3h-9"/>', 'class="off"'),
  flagFill: '<svg class="flag" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true"><path d="M3 15V2h10l-2.2 3.5L13 9H4.5v6z"/></svg>',
  clip: sv('<path d="M10.5 5L6 9.5a1.4 1.4 0 002 2L13 6.5a2.8 2.8 0 00-4-4L4 7.5a4.2 4.2 0 006 6l3.5-3.5"/>', 'class="clip"'),
  rows: sv('<path d="M2.5 4h11M2.5 8h11M2.5 12h11"/>'),
  sliders: sv('<path d="M2.5 4.5h6M11.5 4.5h2M2.5 11.5h2M7.5 11.5h6"/><circle cx="10" cy="4.5" r="1.5"/><circle cx="6" cy="11.5" r="1.5"/>'),
  search: sv('<circle cx="7" cy="7" r="4.2"/><path d="M10.2 10.2L14 14"/>'),
  help: sv('<circle cx="8" cy="8" r="6"/><path d="M6.2 6.3a1.9 1.9 0 013.7.5c0 1.3-1.9 1.6-1.9 2.9"/><circle cx="8" cy="11.6" r=".55" fill="currentColor" stroke="none"/>'),
  caret: sv('<path d="M6 3.5L10.5 8 6 12.5"/>'),
  folder: sv('<path d="M2 4.5v8h12V5.8H8L6.5 3.5H2z"/>'),
  inbox: sv('<path d="M2 9h3.5l1 1.5h3l1-1.5H14"/><path d="M4 3.5h8L14 9v3.5H2V9z"/>'),
  drafts: sv('<path d="M4 2h5.5L12 4.5V14H4z"/><path d="M6 8h4M6 10.5h4"/>'),
  sent: sv('<path d="M14 2L2 7l5 2 2 5z"/><path d="M7 9l3-3"/>'),
  archive: sv('<rect x="2" y="3" width="12" height="3" rx=".6"/><path d="M3 6v7h10V6M6.5 8.5h3"/>'),
  spam: sv('<circle cx="8" cy="8" r="5.5"/><path d="M4.2 11.8l7.6-7.6"/>'),
  outbox: sv('<path d="M2 9h3.5l1 1.5h3l1-1.5H14"/><path d="M2 9v4.5h12V9"/><path d="M8 7.5V1.8M5.6 4.2L8 1.8l2.4 2.4"/>'),
  mail: sv('<rect x="1.5" y="3" width="13" height="10" rx="1.2"/><path d="M2 4l6 5 6-5"/>'),
  unread: sv('<path d="M10.5 4H2.7a1.2 1.2 0 00-1.2 1.2v7.6A1.2 1.2 0 002.7 14h10.6a1.2 1.2 0 001.2-1.2V8.5"/><path d="M2 5l6 4.8 3-2.4"/><circle cx="13.2" cy="3.8" r="2.1" fill="currentColor" stroke="none"/>'),
  mailOpen: sv('<path d="M1.5 6.8L8 2.3l6.5 4.5v6.7h-13z"/><path d="M2 7.2l6 4.3 6-4.3"/>'),
  clock: sv('<circle cx="8" cy="8" r="5.8"/><path d="M8 4.8V8l2.2 1.6"/>'),
  person: sv('<circle cx="8" cy="5.5" r="2.7"/><path d="M2.8 14c.6-2.8 2.7-4.3 5.2-4.3s4.6 1.5 5.2 4.3"/>'),
  personAdd: sv('<circle cx="6.5" cy="5.5" r="2.7"/><path d="M1.5 14c.6-2.8 2.6-4.3 5-4.3 1.2 0 2.3.4 3.2 1.1"/><path d="M12.5 8.5v5M10 11h5"/>'),
  people: sv('<circle cx="6" cy="5.8" r="2.3"/><path d="M1.8 13.5c.5-2.4 2.2-3.6 4.2-3.6s3.7 1.2 4.2 3.6"/><path d="M10.2 3.8a2.3 2.3 0 010 4.1M11.8 10.2c1.3.5 2.1 1.6 2.4 3.3"/>'),
  book: sv('<path d="M4 1.8h9v12.4H4z"/><path d="M2.5 4.5h3M2.5 8h3M2.5 11.5h3"/><circle cx="9" cy="6.6" r="1.6"/><path d="M6.6 11.2c.4-1.3 1.3-2 2.4-2s2 .7 2.4 2"/>'),
  plus: sv('<path d="M8 3v10M3 8h10"/>'),
  rules: sv('<path d="M2 3h12l-4.5 5.5V13l-3 1.5V8.5z"/>'),
  sun: sv('<circle cx="8" cy="8" r="2.7"/><path d="M8 1.6v1.5M8 12.9v1.5M1.6 8h1.5M12.9 8h1.5M3.5 3.5l1.05 1.05M11.45 11.45l1.05 1.05M3.5 12.5l1.05-1.05M11.45 4.55l1.05-1.05"/>'),
  moon: sv('<path d="M13.2 9.6A5.6 5.6 0 016.4 2.8a5.6 5.6 0 106.8 6.8z"/>'),
  ul: sv('<path d="M6 4h8M6 8h8M6 12h8"/><circle cx="2.8" cy="4" r=".9" fill="currentColor" stroke="none"/><circle cx="2.8" cy="8" r=".9" fill="currentColor" stroke="none"/><circle cx="2.8" cy="12" r=".9" fill="currentColor" stroke="none"/>'),
  ol: sv('<path d="M6.5 4h7.5M6.5 8h7.5M6.5 12h7.5"/><path d="M2.4 2.9l1-.5v3.2M2.2 9.3c.2-.6 1.7-.7 1.7.3 0 .8-1.6 1.2-1.7 2h1.8" stroke-width="1.1"/>'),
  quote: sv('<path d="M3 3.5v9"/><path d="M6 5h7M6 8h7M6 11h5"/>'),
  link: sv('<path d="M6.8 9.2a2.6 2.6 0 003.7 0l2-2a2.6 2.6 0 00-3.7-3.7l-.8.8"/><path d="M9.2 6.8a2.6 2.6 0 00-3.7 0l-2 2a2.6 2.6 0 003.7 3.7l.8-.8"/>'),
  clear: sv('<path d="M5 3.5h7M8.5 3.5L7 12.5"/><path d="M3 3l10 10"/>'),
  mark: sv('<path d="M9.8 2.6l3.6 3.6-5.6 5.6H4.2V8.2z"/><path d="M7.4 5l3.6 3.6"/>'),
  picture: sv('<rect x="2" y="3" width="12" height="10" rx="1.2"/><circle cx="5.8" cy="6.3" r="1.1"/><path d="M2.6 12.2l3.6-3.6 2.4 2.4 1.9-1.9 2.9 2.9"/>'),
  popout: sv('<path d="M9.5 2.5H13.5V6.5"/><path d="M13.5 2.5L8 8"/><path d="M11.5 9.5v4h-9v-9h4"/>'),
  cal: sv('<rect x="2.5" y="3.5" width="11" height="10" rx="1.2"/><path d="M2.5 6.5h11M5.5 2v3M10.5 2v3"/>', 'class="cal"'),
  copy: sv('<rect x="5.5" y="5.5" width="8" height="8" rx="1.2"/><path d="M10.5 3.5v-.3a.7.7 0 00-.7-.7H3.2a.7.7 0 00-.7.7v6.6a.7.7 0 00.7.7h.3"/>'),
  spell: sv('<path d="M1.8 11.5L4.3 4l2.5 7.5M2.6 9h3.4"/><path d="M8.8 9.6l2 2 3.6-4.6"/>'),
  dots: sv('<circle cx="3.5" cy="8" r=".9" fill="currentColor"/><circle cx="8" cy="8" r=".9" fill="currentColor"/><circle cx="12.5" cy="8" r=".9" fill="currentColor"/>'),
  tag: sv('<path d="M2.5 2.5h5.3l5.7 5.7-5.3 5.3-5.7-5.7z"/><circle cx="5.4" cy="5.4" r="1"/>'),
};
const KIND_ICON = {inbox:'inbox', outbox:'outbox', drafts:'drafts', sent:'sent', archive:'archive', spam:'spam', trash:'trash', custom:'folder', unread:'unread'};
// Special folders have a name in the interface's language, whatever the server calls them.
const KIND_NAME = {inbox:N_('Inbox'), outbox:N_('Outbox'), drafts:N_('Drafts'), sent:N_('Sent'), archive:N_('Archive'), spam:N_('Spam'), trash:N_('Trash'), unread:N_('Unread')};

/* ---------- data (filled from Rust) ---------- */
// The folder tree and its counts; the message list asks the core for pages (see "list").
let ACCOUNTS = [], ACCT = {}, COUNTS = {};
let PROBLEMS = {}; // account id -> why its last check failed (kept by the core until one works)
let AWAY = []; // the away replies that are on: [{account, place, starts, ends}]
let USAGE = null; // the last answer of `usage`, for the Usage tab
let QUOTA = {}, SYNCED = {}; // account id -> its storage on the server; when its last check worked
// Tags (tags.rs), in the window's order, with how many messages carry each; the accounts whose
// server can't keep them, so they stay on this PC.
let TAGS = [], TAGS_HERE = [];
const bodies = new Map(); // id -> full message from get_message

// "2026-10-10T06:12:00" (local time, no zone) -> Date
function parseDate(s){
  const [d, t = '00:00:00'] = s.split('T');
  const [y, mo, da] = d.split('-').map(Number);
  const [h, mi, se] = t.split(':').map(Number);
  return new Date(y, mo - 1, da, h, mi, Math.floor(se || 0));
}
const fromSummary = s => ({
  id:s.id, f:s.folder, from:s.from_name, email:s.from_email, to:s.to, subj:s.subject,
  date:parseDate(s.date), kb:s.size_kb, unread:s.unread, flagged:s.flagged, hasAtt:s.has_attachments,
  thread:s.thread ?? null, goes:s.goes ? parseDate(s.goes) : null, tags:s.tags ?? [],
});
// Accounts, folders and how many messages each folder holds (the core counts them).
async function loadTree(){
  const t = await invoke('folder_tree');
  ACCOUNTS = t.accounts;
  ACCT = Object.fromEntries(ACCOUNTS.map(a => [a.id, a]));
  COUNTS = t.counts;
  PROBLEMS = t.problems ?? {};
  AWAY = t.away ?? [];
  QUOTA = t.quota ?? {}; SYNCED = t.synced ?? {};
  TAGS = t.tags ?? []; TAGS_HERE = t.tags_here ?? [];
  INITIALS = initialsOf(servers());
  try { SAVED_SEARCHES = await invoke('list_saved_searches'); } catch { SAVED_SEARCHES = []; }
}
// Two letters for each account, from its name ("Łukasz Szramuk" → ŁS, "Sales 750xl.com" → S7),
// else its address; two accounts never share them while another pair is free.
let INITIALS = {};
function initialsOf(accounts){
  const used = new Set(), out = {};
  const up = s => s.toLocaleUpperCase();
  for (const a of accounts){
    const words = String(a.name || '').split(/[^\p{L}\p{N}]+/u).filter(Boolean);
    const local = String(a.email).split('@')[0].replace(/[^\p{L}\p{N}]/gu, ''), domain = String(a.email).split('@')[1] ?? '';
    const tries = [words.length > 1 ? words[0][0] + words[1][0] : (words[0] ?? '').slice(0, 2), local[0] + domain[0], local.slice(0, 2), (words[0] ?? local)[0] + (accounts.indexOf(a) + 1)];
    const pick = tries.map(x => up(String(x ?? ''))).find(x => x.length === 2 && !used.has(x)) ?? up(local.slice(0, 2));
    used.add(pick); out[a.id] = pick;
  }
  return out;
}
// The core's errors and notes come in the interface's language already (its tr! macro).
const tr = e => String(e);
// Commands that change mail; the core queues them for the server and we push shortly after.
// send_message counts: a sent message's copy for the server's Sent folder is queued too, and
// save_outgoing: a saved draft goes to the server's Drafts (autosaves stay here).
const CHANGES = new Set(['set_unread', 'set_flagged', 'send_message', 'save_outgoing', 'run_rules',
  'set_unread_many', 'set_flagged_many', 'move_messages', 'delete_messages', 'mark_folder_read', 'mark_spam',
  'send_now', 'unsubscribe', 'set_tag_many', 'delete_tag']);
async function call(cmd, args){
  try {
    const result = await invoke(cmd, args);
    if (CHANGES.has(cmd)) schedulePush();
    return result;
  } catch (e){ say(tr(e), 8000); throw e; }
}

// Changes reach the server a moment after the last one, so a burst of keys is one connection.
// They are saved in the mail base first, so closing Telex or losing the network loses nothing.
let pushTimer;
function schedulePush(delay = 2500){
  if (VIEWER){ tellMain(); return; }
  if (!ACCOUNTS.some(a => a.imap)) return;
  clearTimeout(pushTimer);
  pushTimer = setTimeout(pushNow, delay);
}
async function pushNow(){
  let r;
  try { r = await invoke('push_changes'); } catch (e){ say(tr(e), 8000); return; }
  if (r.busy){ schedulePush(4000); return; } // a check is running; try again after it
  if (r.errors.length) say(r.errors.join(' · '), 15000);
  // A refused move puts the message back; one deleted elsewhere disappears.
  refresh();
}
async function full(id){
  return bodies.has(id) ? bodies.get(id) : keep(bodies, id, await invoke('get_message', {id}), 30);
}
// A cache that holds the last few: the oldest goes first.
function keep(cache, key, value, most){
  cache.set(key, value);
  if (cache.size > most) cache.delete(cache.keys().next().value);
  return value;
}

/* ---------- helpers ---------- */
const $ = id => document.getElementById(id);
const esc = s => String(s).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const pad = n => String(n).padStart(2, '0');
const DOW = [N_('Sun'),N_('Mon'),N_('Tue'),N_('Wed'),N_('Thu'),N_('Fri'),N_('Sat')];
const hm = d => pad(d.getHours()) + ':' + pad(d.getMinutes());
const dmy = d => pad(d.getDate()) + '.' + pad(d.getMonth() + 1) + '.' + d.getFullYear();
function fmtList(d){
  if (colPref.dates === 'full') return dmy(d) + ' ' + hm(d);
  const day = x => new Date(x.getFullYear(), x.getMonth(), x.getDate());
  const diff = Math.round((day(new Date()) - day(d)) / 864e5);
  if (diff <= 0) return hm(d);
  if (diff < 7) return t(DOW[d.getDay()]) + ' ' + hm(d);
  return dmy(d);
}
const fmtFull = d => t(DOW[d.getDay()]) + ', ' + dmy(d) + ' ' + hm(d);
const fmtSize = kb => kb >= 1048576 ? I18n.decimal(kb / 1048576) + ' GB' : kb >= 1024 ? I18n.decimal(kb / 1024) + ' MB' : kb + ' KB';
// When something happened: the time today, the day and time before.
const dm = d => pad(d.getDate()) + '.' + pad(d.getMonth() + 1);
const fmtWhen = d => d.toDateString() === new Date().toDateString() ? hm(d) : dm(d) + ' ' + hm(d);
// When mail sent later goes: the time today, "tomorrow", the weekday within a week, else the date.
function fmtGoes(d){
  const day = x => new Date(x.getFullYear(), x.getMonth(), x.getDate());
  const ahead = Math.round((day(d) - day(new Date())) / 864e5);
  if (ahead <= 0) return hm(d);
  if (ahead === 1) return t('tomorrow') + ' ' + hm(d);
  if (ahead < 7) return t(DOW[d.getDay()]) + ' ' + hm(d);
  return dm(d) + ' ' + hm(d);
}
const nameOf = addr => {
  const s = String(addr || '').trim();
  if (!s) return t('(no recipient)');
  const m = s.match(/^\s*"?([^"<]+?)"?\s*</);
  return m ? m[1] : s;
};
// The core says what was sent as `"Subject" to Name`.
const sentLine = s => String(s).replace(/^"(.*)" to (.*)$/, (_, subject, who) => t('“{subject}” to {who}', {subject, who}));
// Lowercase without diacritics, for matching what is typed: "Łódź" → "lodz".
const fold = s => String(s).normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/ł/g, 'l').replace(/Ł/g, 'L').toLowerCase();
const acctId = key => key.split('/')[0];
// Every account's Drafts, or every Outbox, as one list: "*/drafts", "*/outbox" (EVERY).
const EVERY = '*';
const isEvery = key => acctId(String(key)) === EVERY;
// Unread mail of every folder but Spam and Trash, at the top of the tree. The core gets it as
// all mail with `unread`.
const UNREAD = EVERY + '/unread';
// Lists that name each row's account (Drafts and the Outbox of every account) instead of its folder.
const byAccount = () => isEvery(state.folder) && state.folder !== UNREAD && !allMail();
const folderOf = key => {
  const [a, f] = key.split('/');
  return a === EVERY ? (KIND_NAME[f] ? {id:f, kind:f, name:f} : undefined) : ACCT[a]?.folders.find(x => x.id === f);
};
const folderName = key => { const f = folderOf(key); return !f ? key : KIND_NAME[f.kind] ? t(KIND_NAME[f.kind]) : f.name; };
const kindOf = key => folderOf(key)?.kind;
const isOut = key => ['sent', 'drafts', 'outbox'].includes(kindOf(key));
const keyOfKind = (acct, kind) => { const f = ACCT[acct]?.folders.find(x => x.kind === kind); return f ? acct + '/' + f.id : null; };
const homeOf = acct => keyOfKind(acct, 'inbox') || (ACCT[acct]?.folders[0] ? acct + '/' + ACCT[acct].folders[0].id : '');
// An EVERY list counts what its folders hold together; Unread, the unread mail outside Spam and Trash.
const unreadAll = () => ACCOUNTS.reduce((n, a) => n + a.folders.filter(f => f.kind !== 'spam' && f.kind !== 'trash')
  .reduce((m, f) => m + (COUNTS[a.id + '/' + f.id]?.unread ?? 0), 0), 0);
const countOf = key => !isEvery(key) ? COUNTS[key] ?? {total:0, unread:0}
  : key === UNREAD ? (n => ({total:n, unread:n}))(unreadAll())
  : servers().reduce((n, a) => { const c = COUNTS[keyOfKind(a.id, kindOf(key))]; return c ? {total:n.total + c.total, unread:n.unread + c.unread, stuck:n.stuck + (c.stuck ?? 0)} : n; }, {total:0, unread:0, stuck:0});
const num = n => n.toLocaleString(I18n.lang() === 'pl' ? 'pl-PL' : 'en');
// The folders on this PC ("On this PC"): mail kept only here. The core keeps them as an
// account without servers, with this id, listed last.
const LOCAL = 'local';
const isLocal = key => acctId(String(key)) === LOCAL;
const servers = () => ACCOUNTS.filter(a => a.imap); // the accounts that live on a server
// A POP3 account's server has only an Inbox: all its folders are here, made and removed like
// the folders on this PC. `keepsHere`: a folder of such an account, or one on this PC.
const isPop3 = acct => !!ACCT[acct]?.imap?.pop3;
const keepsHere = key => isLocal(key) || isPop3(acctId(String(key)));
// A folder's name where it could be mistaken for a server folder: "Archive on this PC".
const placeName = key => isLocal(key) ? t('{folder} on this PC', {folder:folderName(key)}) : folderName(key);
// Who a folder belongs to, in words: the account's address, or "On this PC".
const acctLabel = acct => acct === LOCAL ? t('On this PC') : acct === EVERY ? t('All accounts') : ACCT[acct]?.email ?? '';
// Each account has a color, shown as a dot once there is more than one account; the folders
// on this PC have a small screen instead.
// Settings › Appearance says how (account_mark: rail, dot, initials, tint, none).
const acolor = acct => { const i = ACCOUNTS.findIndex(a => a.id === acct); return i < 0 ? '' : 'a' + i % 6; };
const adot = acct => {
  if (acct === LOCAL) return `<i class="adot pc" aria-hidden="true"></i>`;
  const c = acolor(acct);
  return c ? `<i class="adot ${c}" data-i="${esc(INITIALS[acct] ?? '')}" aria-hidden="true"></i>` : '';
};
// The window's own state: pane sizes, the filter's scope, when updates were last checked. Settings are elsewhere.
const store = {
  get(){ try { return JSON.parse(localStorage.getItem('telex-ui') || '{}'); } catch { return {}; } },
  set(v){ try { localStorage.setItem('telex-ui', JSON.stringify(v)); } catch {} },
};

/* ---------- state ---------- */
const saved = store.get();
const state = {
  savedSearch:null,
  folder:'', sel:{}, sort:{key:'date', dir:-1}, q:'', inText:false,
  scope: saved.scope === 'all' ? 'all' : 'folder', // the quick filter looks in this folder or in all mail
  more: saved.more && typeof saved.more === 'object' ? saved.more : {}, // accounts showing all their folders
  collapsed:{}, compose:null, images:new Set(), white:new Set(), // white: HTML shown on white
  undo:null, // what Z takes back (set with setUndo): {kind:'move', items:[{id, from}]} after a delete or move; {kind:'send', id, until}; {kind:'discard', compose}
  picked:new Set(), anchor:null, span:0, // more than one selected message; the Shift selection's start and length
  thread:null, // {id, subject}: the list shows this message's conversation
  expanded:new Set(), // expanded thread ids in this folder
  unread:false, // the list shows only unread mail (O); Unread in the tree always does
  kept:new Set(), // read while an unread list was open: it keeps them until it is left
  recent: Array.isArray(saved.recent) ? saved.recent.slice(0, 8) : [], // folders last moved to or opened with M and G
  pinned: new Set(Array.isArray(saved.pinned) ? saved.pinned : []), // folders shown outside "more"
  searches: Array.isArray(saved.searches) ? saved.searches.slice(0, 8) : [], // what was looked for last
  updateChecked: Number(saved.updateChecked) || 0, // when a check last got an answer (ms)
  offered: saved.offered || null, // the language offered on the first start, once
  helpTold: !!saved.helpTold, // the status bar said once where the keys and tips are
};
// The list's columns: From and Folder as wide as they were dragged (px; null fits them to the
// list), the Size column, and dates short (06:12, Wed 06:12, 10.10.2026) or full.
const colPref = {from:null, where:null, size:true, dates:'short'};
if (saved.cols && typeof saved.cols === 'object'){
  for (const k of ['from', 'where']) if (Number(saved.cols[k]) > 0) colPref[k] = Math.round(saved.cols[k]);
  if (saved.cols.size === false) colPref.size = false;
  if (saved.cols.dates === 'full') colPref.dates = 'full';
}
const sdlg = $('set-dlg'); // the settings window, which also holds the accounts
const win = $('win'), rowsEl = $('rows'), listEl = $('list'), previewEl = $('preview'), treeEl = $('tree'), qEl = $('q');
if (saved.treeW) win.style.setProperty('--tree-w', saved.treeW + 'px');
if (saved.listH) win.style.setProperty('--list-h', saved.listH + '%');
const persist = () => store.set({
  treeW: parseInt(win.style.getPropertyValue('--tree-w')) || undefined,
  listH: parseFloat(win.style.getPropertyValue('--list-h')) || undefined,
  updateChecked: state.updateChecked || undefined,
  offered: state.offered || undefined,
  helpTold: state.helpTold || undefined,
  scope: state.scope === 'all' ? 'all' : undefined,
  more: Object.keys(state.more).length ? state.more : undefined,
  recent: state.recent.length ? state.recent : undefined,
  pinned: state.pinned.size ? [...state.pinned] : undefined,
  searches: state.searches.length ? state.searches : undefined,
  cols: colPref.from || colPref.where || !colPref.size || colPref.dates !== 'short' ? {...colPref} : undefined,
});

/* ---------- settings ---------- */
// Theme, language, density, text size and the rest live in settings.json, through the core
// (get_settings, update_settings). The look is also kept for boot.js, which puts it on the
// page before the first paint.
const THEMES = ['system', 'light', 'dark', 'paper', 'heather', 'fjord', 'phosphor', 'contrast-light', 'contrast-dark'];
const THEME_NAMES = {system:N_('Match Windows'), light:N_('Light'), dark:N_('Dark'), paper:N_('Paper'), heather:N_('Heather'), fjord:N_('Fjord'),
  phosphor:N_('Phosphor'), 'contrast-light':N_('High contrast light'), 'contrast-dark':N_('High contrast dark')};
const LANG_NAMES = {en:'English', pl:'Polski'}; // each in itself, never translated
const TOOLBAR_STYLES = [
  ['classic', N_('Classic|toolbar'), N_('Icons beside labels; adapts to the window.')],
  ['icons', N_('Icons only'), N_('Small buttons, more room for your mail.')],
  ['large', N_('Large icons'), N_('Bigger buttons, easy to see and click.')],
  ['below', N_('Labels below'), N_('Large icons with every action named.')],
  ['text', N_('Text only'), N_('Clear labels without icons.')],
];
const TEXT_SIZES = [90, 100, 110, 125, 150, 175, 200];
const CHECK_EVERY = [5, 10, 15, 30, 60, 0];
const UNDO_SEND = [0, 5, 10, 20, 30];
const READ_DELAYS = [600, 1000, 2000, 3000, 5000, 10000]; // ms on screen before a message counts as read
const darkMode = matchMedia('(prefers-color-scheme: dark)'), forcedColors = matchMedia('(forced-colors: active)');
const shownTheme = id => id === 'system' ? (darkMode.matches ? 'dark' : 'light') : id;
// Themes on dark paper. HTML mail goes on the same paper there, unless the settings say white.
const DARK_THEMES = new Set(['dark', 'fjord', 'phosphor', 'contrast-dark']);
let SETTINGS = {
  settings:{theme:'system', language:'auto', toolbar_style:'classic', density:'compact', account_mark:'rail', unread_style:'quiet', font:'plex', text_size:100, prefer_text:false, mark_read:'moment', read_delay:600, check_every:10,
    threaded:false, push_mail:true, notify:true, mail_paper:'theme', risk_checks:true, spam_filter:'careful', compose_format:'html', reply_format:'html', sign_replies:true, undo_send:5, ask_subject:true, ask_attachment:true, ask_many:true,
    image_size:'large', spell_check:true, updates:'ask', usage_count:'ask', backup_every:'off', backup_dir:'', backup_keep:5},
  language:I18n.lang(), offer:null, fresh:false,
};
const pref = () => SETTINGS.settings;
let zoomed = 100;
let threadSetting = false;
// The colors HTML mail gets on dark paper: the theme's own (themes.css), or null for white.
function darkPaper(id){
  const theme = document.documentElement.dataset.theme;
  if (!DARK_THEMES.has(theme) || pref().mail_paper !== 'theme' || state.white.has(id)) return null;
  const css = getComputedStyle(document.documentElement), v = name => css.getPropertyValue(name).trim();
  return {paper:v('--surface'), ink:v('--fg'), link:v('--accent'), line:v('--line'), muted:v('--muted')};
}
function applySettings(){
  const s = pref(), theme = document.documentElement.dataset.theme;
  if (threadSetting !== !!s.threaded){
    threadSetting = !!s.threaded;
    state.expanded.clear(); unpick();
    if (!VIEWER && list.seq) loadView();
  }
  document.documentElement.dataset.theme = shownTheme(s.theme);
  document.documentElement.dataset.mark = s.account_mark;
  document.documentElement.dataset.unread = s.unread_style;
  document.documentElement.dataset.toolbar = s.toolbar_style || 'classic';
  if (document.documentElement.dataset.font !== s.font){
    document.documentElement.dataset.font = s.font;
    measured = null; colsDone = ''; fitColumns(); // the dates' width is the new face's
  }
  // HTML mail on dark paper takes the theme's colors: show it again in the new ones.
  if (theme !== shownTheme(s.theme) && !state.compose && ACCOUNTS.length) renderPreview();
  if (s.text_size !== zoomed){
    zoomed = s.text_size;
    try { window.__TAURI__?.webview?.getCurrentWebview().setZoom(s.text_size / 100); } catch {}
  }
  const rows = win.dataset.density;
  paintDensity();
  if (win.dataset.density !== rows) drawSoon(); // the list keeps the same rows on screen
  scheduleChecks();
  if (SETTINGS.language !== I18n.lang()){ I18n.setLang(SETTINGS.language); relabel(); }
  renderUpdates();
  try { localStorage.setItem('telex-look', JSON.stringify({theme:s.theme, lang:SETTINGS.language, zoom:s.text_size, mark:s.account_mark, unread:s.unread_style, font:s.font, toolbar:s.toolbar_style})); } catch {}
  fitToolbarSoon(); // labels, language, font and layout all change how wide the actions are
}
darkMode.addEventListener('change', () => { if (pref().theme === 'system') applySettings(); });
// Saved at once; the window follows. Resolves to true when it worked.
async function changeSettings(changes){
  const before = SETTINGS.language;
  let next;
  try { next = await invoke('update_settings', {changes}); }
  catch (e){ say(tr(e), 8000); return false; }
  if (!next?.settings) return false;
  SETTINGS = next;
  applySettings();
  if (sdlg.open) SETTINGS.language === before ? syncSettings() : renderSettings(true);
  window.__TAURI__?.event?.emit('settings-changed', ownLabel());
  return true;
}
// After a language change: everything on screen again, in the new words.
function relabel(){
  I18n.translatePage();
  paintToolbar(); paintDensity(); paintScope(); paintUnreadOnly();
  renderTree(); renderHead(); drawRows(); renderStatus();
  if (helpDlg.open) renderHelp();
  if (!state.compose) renderPreview(); else { readCompose(); renderCompose(); }
  if (rdlg.open){ if (!$('r-list-view').hidden) showRuleList(); else { renderConditions(); renderActions(); } }
  // Thunderbird's five tags are named in the interface's language until renamed: ask again.
  loadTree().then(() => { drawRows(); paintTagChips(); if (sdlg.open && setTab === 'tags') renderTags(); }, () => {});
}
// Older versions kept these in the window; the first start with settings.json brings them over.
async function bootSettings(){
  let got = null;
  try { got = await invoke('get_settings'); } catch {}
  if (got?.settings) SETTINGS = got;
  if (SETTINGS.fresh){
    const changes = {};
    if (['compact', 'comfortable'].includes(saved.density)) changes.density = saved.density;
    if (saved.preferText === true) changes.prefer_text = true;
    if (['daily', 'off'].includes(saved.updates)) changes.updates = saved.updates;
    try { const next = await invoke('update_settings', {changes}); if (next?.settings) SETTINGS = next; } catch {}
  }
  applySettings();
}
let checkTimer = null, checkEvery = null;
function scheduleChecks(){
  const min = pref().check_every;
  if (min === checkEvery) return;
  checkEvery = min;
  clearInterval(checkTimer);
  checkTimer = min ? setInterval(() => getMail({quiet:true}), min * 60e3) : null;
}
function stepTextSize(dir){
  const now = pref().text_size, i = TEXT_SIZES.indexOf(now);
  const next = dir === 0 ? 100 : TEXT_SIZES[clamp((i < 0 ? 1 : i) + dir, 0, TEXT_SIZES.length - 1)];
  if (next === now) return;
  changeSettings({text_size:next}).then(ok => ok && say(t('Text size {n}%. Ctrl+0 goes back to 100%.', {n:next}), 3000));
}

/* ---------- toolbar ---------- */
const TOOLBAR = [['b-get','get',N_('Get mail')],['b-new','neu',N_('New|message')],['b-reply','reply',N_('Reply')],['b-replyall','replyall',N_('Reply all')],
  ['b-forward','forward',N_('Forward')],['b-flag','flag',N_('Flag')],['b-delete','trash',N_('Delete')],['b-rules','rules',N_('Rules')],['b-book','book',N_('Contacts')],['b-settings','sliders',N_('Settings')],
  ['b-help','help',N_('Help')]];
function paintToolbar(){
  TOOLBAR.forEach(([id, ic, label]) => {
    const b = $(id);
    b.innerHTML = I[ic] + `<span class="lbl">${esc(t(label))}</span>`;
    b.setAttribute('aria-label', b.title || t(label));
  });
  $('search-ic').innerHTML = I.search;
}
function paintDensity(){
  win.dataset.density = pref().density;
  $('b-density').innerHTML = I.rows + `<span class="lbl">${esc(pref().density === 'compact' ? t('Compact') : t('Comfortable'))}</span>`;
  $('b-density').setAttribute('aria-label', t('Switch row density') + ': ' + t(pref().density === 'compact' ? 'Compact' : 'Comfortable'));
}
// Classic shows its words while they fit; when they don't (a narrow window, longer Polish words)
// it shows only the icons, each with its tooltip, instead of scrolling. Measured, not guessed.
function fitToolbar(){
  const bar = document.querySelector('.toolbar'), actions = $('toolbar-actions');
  if (!bar) return;
  bar.classList.remove('tight');
  if (document.documentElement.dataset.toolbar !== 'classic' || VIEWER) return;
  bar.classList.toggle('tight', actions.scrollWidth > actions.clientWidth + 1);
}
function fitToolbarSoon(){ requestAnimationFrame(fitToolbar); } // after the new words are drawn
new ResizeObserver(fitToolbarSoon).observe(document.querySelector('.toolbar'));
document.fonts?.ready.then(fitToolbarSoon);
// A vertical mouse wheel can reach the rest of the actions on narrow windows.
$('toolbar-actions').addEventListener('wheel', e => {
  const actions = e.currentTarget;
  if (e.ctrlKey || e.shiftKey || Math.abs(e.deltaX) >= Math.abs(e.deltaY)) return;
  const before = actions.scrollLeft;
  actions.scrollLeft += e.deltaY * (e.deltaMode === 1 ? 24 : e.deltaMode === 2 ? actions.clientWidth : 1);
  if (actions.scrollLeft !== before) e.preventDefault();
}, {passive:false});

/* ---------- status ---------- */
let stTimer, lastCheck = null, lastTitle = '';
// What just happened, on the left of the status bar. A message too long for its side takes the
// keys' place in the middle until it goes.
function say(text, ms = 4500){
  const el = $('st-msg'), bar = el.closest('.status');
  el.textContent = text; el.title = text;
  bar.classList.remove('saying');
  if (text && el.scrollWidth > el.clientWidth) bar.classList.add('saying');
  clearTimeout(stTimer);
  if (ms) stTimer = setTimeout(() => { el.textContent = ''; el.title = ''; bar.classList.remove('saying'); }, ms);
}
// What Z takes back, also as a button next to the status, so other news doesn't hide it: mail
// waiting to go counts down its moment; a move, delete or discard stays a while (Z works later too).
const UNDO_SHOWN = 10000;
let undoTimer;
function setUndo(u){
  state.undo = u ? {at:Date.now(), ...u} : null;
  paintUndo();
}
function paintUndo(){
  const b = $('st-undo'), u = state.undo;
  clearTimeout(undoTimer);
  const left = u ? (u.until ?? u.at + UNDO_SHOWN) - Date.now() : 0;
  b.hidden = left <= 0;
  if (b.hidden) return;
  const n = Math.ceil(left / 1000);
  b.textContent = u.kind === 'send' ? t('Undo send · {n} s', {n}) : t('Undo');
  b.title = t('Undo (Z)');
  undoTimer = setTimeout(paintUndo, u.kind === 'send' ? left - (n - 1) * 1000 : left);
}
$('st-undo').addEventListener('click', () => undo());
// Accounts whose last check failed, on the right until a check works; a click checks again.
function renderSync(){
  const b = $('st-sync'), ids = VIEWER ? [] : ACCOUNTS.filter(a => PROBLEMS[a.id]).map(a => a.id);
  b.hidden = $('st-syncwhy').hidden = !ids.length;
  if (!ids.length) return;
  $('st-syncwhy').title = t('Every step of the last connections: Settings › Connections');
  b.textContent = ids.length === 1 ? t('Check failed · Retry') : tn(ids.length, '{n} check failed · Retry', '{n} checks failed · Retry', {n:num(ids.length)});
  b.title = ids.map(id => `${acctLabel(id)}: ${PROBLEMS[id]}`).join('\n') + '\n' + t('Click to check again.');
}
$('st-sync').addEventListener('click', () => getMail());
$('st-syncwhy').addEventListener('click', () => {
  const ids = ACCOUNTS.filter(a => PROBLEMS[a.id]).map(a => a.id);
  showConnections(ids.length === 1 ? ids[0] : '', ids.length > 1);
});
// Mail that didn't go (refused, a failed try, taken back), whichever folder is open; a click shows it.
function renderUnsent(){
  const n = VIEWER ? 0 : countOf(EVERY + '/outbox').stuck, b = $('st-unsent');
  b.hidden = !n;
  if (!n) return;
  b.textContent = tn(n, '{n} not sent', '{n} not sent', {n:num(n)});
  b.title = t('Not sent: open the message to see why.');
}
$('st-unsent').addEventListener('click', () => setFolder(EVERY + '/outbox'));
// How many older messages the "download" button in the status bar asks for at a time.
const OLDER_BATCH = 1000;
function renderStatus(){
  const {total, unread, older = 0} = countOf(state.folder), found = list.total;
  const picked = state.picked.size > 1 ? ' · ' + esc(tn(state.picked.size, '{n} selected', '{n} selected', {n:num(state.picked.size)})) : '';
  const more = older && !allMail() && !state.q.trim()
    ? ` · ${esc(tn(older, '{n} older on the server', '{n} older on the server', {n:num(older)}))} <button type="button" class="linkbtn" id="st-older" title="${esc(t('The first check downloads the newest 300 messages of each folder.'))}">${esc(older > OLDER_BATCH ? t('Download {n}', {n:num(OLDER_BATCH)}) : t('Download'))}</button>` : '';
  // "n unread" is a switch: only those (O). With it on, "Show all" switches back.
  const unreadBtn = unread ? `<button type="button" class="linkbtn quiet" id="st-unread" title="${esc(t('Show only unread mail (O)'))}">${esc(tn(unread, '{n} unread', '{n} unread', {n:num(unread)}))}</button>` : '';
  $('st-folder').innerHTML = !ACCOUNTS.length ? esc(t('No account yet'))
    : state.thread ? esc(t('Conversation · {n}', {n:tn(found, '{n} message', '{n} messages', {n:num(found)})})) + picked
    : allMail() ? esc(t('All mail') + ' · ' + tn(found, '{n} match', '{n} matches', {n:num(found)})) + picked
    : state.folder === UNREAD ? esc(t('Unread') + ' · ' + (unread ? tn(unread, '{n} message', '{n} messages', {n:num(unread)}) : t('all read'))) + picked
    : state.unread ? `${adot(acctId(state.folder))}${esc(folderName(state.folder))} · ${esc(t('only unread: {n} of {total}', {n:num(unread), total:num(total)}))} · <button type="button" class="linkbtn" id="st-all" title="${esc(t('Show every message (O)'))}">${esc(t('Show all'))}</button>${picked}`
    : `${adot(acctId(state.folder))}${esc(folderName(state.folder))} · ${esc(tn(total, '{n} message', '{n} messages', {n:num(total)}))}${unread ? ', ' + unreadBtn : ''}${picked}${more}`;
  $('st-check').textContent = lastCheck ? t('Last check {time}', {time:hm(lastCheck)}) : '';
  renderSync();
  renderUnsent();
  renderAway();
  renderMeter();
  const title = VIEWER ? `${selRow?.subj || t('Message')} – Telex` : ACCOUNTS.length ? `${folderName(state.folder)} · ${acctLabel(acctId(state.folder))} – Telex` : 'Telex';
  if (title !== lastTitle){
    lastTitle = title;
    document.title = title;
    try { window.__TAURI__?.window?.getCurrentWindow().setTitle(title); } catch {}
  }
}

// Older mail the first download left on the server, a batch at a time.
$('st-folder').addEventListener('click', e => {
  if (e.target.closest('#st-older')) downloadOlder();
  else if (e.target.closest('#st-unread, #st-all')){ toggleUnreadOnly(); listEl.focus({preventScroll:true}); }
});
async function downloadOlder(folder = state.folder, again = false){
  const b = $('st-older');
  if (!folder || (b?.disabled && !again)) return;
  if (b) b.disabled = true;
  say(t('Downloading older messages…'), 0);
  let r;
  try { r = await invoke('download_older', {folder, limit:OLDER_BATCH}); }
  catch (e){ say(tr(e), 10000); if (b) b.disabled = false; return; }
  if (r.busy){ setTimeout(() => downloadOlder(folder, true), 2000); return; } // a check is running: the same folder, in a moment
  await refresh();
  say(tn(r.new, '{n} older message downloaded.', '{n} older messages downloaded.', {n:num(r.new)}));
}

/* ---------- usage meter ---------- */
// The server's storage for the open folder's account, as a small bar in the status bar,
// when the server reports a quota. A nearly full account also says so in the folder tree.
const quotaOf = acct => QUOTA[acct] ?? null;
const pctOf = q => Math.min(100, Math.round(q.used_kb / q.limit_kb * 100));
const levelOf = pct => pct >= 95 ? 'full' : pct >= 80 ? 'warn' : 'ok';
const quotaText = q => t('Server storage: {used} of {limit} ({pct}%)', {used:fmtSize(q.used_kb), limit:fmtSize(q.limit_kb), pct:pctOf(q)});
// The bar is the account's own color, like its dot; next to it, how much room is left.
function renderMeter(){
  const acct = ACCOUNTS.length && state.folder ? acctId(state.folder) : null, q = acct ? quotaOf(acct) : null, m = $('st-usage');
  m.hidden = !q;
  if (!q) return;
  const pct = pctOf(q), free = fmtSize(Math.max(0, q.limit_kb - q.used_kb));
  m.dataset.level = levelOf(pct);
  m.style.setProperty('--fill', `var(--acct-${Math.max(0, ACCOUNTS.findIndex(a => a.id === acct)) % 6})`);
  m.querySelector('i').style.width = pct + '%';
  m.querySelector('.mtxt').textContent = t('{size} free', {size:free});
  m.title = t('{email} on the server: {used} of {limit} used ({pct}%), {free} free.', {email:ACCT[acct]?.email ?? '', used:fmtSize(q.used_kb), limit:fmtSize(q.limit_kb), pct, free})
    + ' ' + t('Click for usage.');
  m.setAttribute('aria-label', m.title);
}
$('st-usage').addEventListener('click', () => openSettings('use'));

/* ---------- tree ---------- */
// Each account shows its Inbox and Sent (quieter: mail that went out), and the open folder.
// The rest wait behind a quiet "more" line, remembered per account. Drafts and the Outbox
// aren't places to keep mail: what isn't sent yet, from any account, waits at the end of the
// tree as one list each (EVERY), only while it holds something.
const MAIN_KINDS = new Set(['inbox', 'sent']);
const UNSENT = ['drafts', 'outbox'];
const COUNT_ALL = new Set(UNSENT); // these count everything, not unread
function folderButton(a, f){
  const key = a.id + '/' + f.id;
  if (state.naming?.key === key) return `<li><span class="fold naming">${I[KIND_ICON[f.kind] || 'folder']}${nameInput()}</span></li>`;
  // A special folder is named by its kind; its own name on the server is in the tooltip, and
  // next to it when another folder has the kind's name ("Sent · Wysłane" next to an unused "Sent").
  const shown = folderName(key), same = (x, y) => x.toLocaleLowerCase() === y.toLocaleLowerCase();
  const own = f.kind !== 'custom' && !keepsHere(key) && f.name && !same(f.name, shown) ? f.name : '';
  const twin = own && a.folders.some(x => x.kind === 'custom' && same(x.name, shown));
  return foldButton(key, f.kind, own, twin);
}
function foldButton(key, kind, own = '', twin = false){
  const c = countOf(key), all = COUNT_ALL.has(kind), on = key === state.folder && !state.savedSearch;
  const n = all ? c.total : c.unread;
  // The Outbox says what didn't go, in the warning colors, before what waits its turn.
  const cnt = c.stuck ? `<span class="qchip" data-level="full" title="${esc(tn(c.stuck, '{n} not sent', '{n} not sent', {n:num(c.stuck)}) + ' · ' + t('Not sent: open the message to see why.'))}">${num(c.stuck)}</span>`
    : n ? `<span class="cnt${all ? ' dim' : ''}">${num(n)}</span>` : '';
  const tip = key === UNREAD ? t('Unread mail from every folder except Spam and Trash. O shows only the unread mail of a folder.')
    : isEvery(key) ? t('{folder} of every account', {folder:folderName(key)}) : own ? t('“{name}” on the server', {name:own}) : '';
  const name = esc(folderName(key)) + (twin ? `<i class="srv"> · ${esc(own)}</i>` : '');
  return `<li><button type="button" class="fold${kind === 'sent' ? ' out' : ''}${on ? ' active' : ''}" data-folder="${key}"${on ? ' aria-current="true"' : ''}${tip ? ` title="${esc(tip)}"` : ''}>${I[KIND_ICON[kind] || 'folder']}<span>${name}</span>${cnt}</button></li>`;
}
// What isn't sent yet, at the end of the tree: every account's drafts, and the Outbox.
const unsentKeys = () => UNSENT.map(kind => EVERY + '/' + kind).filter(key => countOf(key).total || key === state.folder);
function unsentGroup(){
  const keys = unsentKeys();
  return keys.length ? `<ul class="unsent" aria-label="${esc(t('Not sent yet'))}">${keys.map(key => foldButton(key, kindOf(key))).join('')}</ul>` : '';
}
// A new folder being named in this group (the folders on this PC, a POP3 account), first in it.
const newFolderLine = acct => state.naming?.key === 'new' && state.naming.acct === acct ? `<li><span class="fold naming">${I.folder}${nameInput()}</span></li>` : '';
// The box a folder on this PC is named in, in the tree itself.
const nameInput = () => `<input id="nf-name" class="nfname" value="${esc(state.naming.value)}" autocomplete="off" spellcheck="false" aria-label="${esc(t('Folder name'))}" placeholder="${esc(t('Folder name'))}">`;
// The folders on this PC: their own group under the accounts, with a + for a new one. Archive
// and Trash show only when they hold something.
// Mail on this PC exists nowhere else: the group says so while there is no backup, or (with
// backups off) none for a month.
function backupNudge(){
  const held = (ACCT[LOCAL]?.folders ?? []).reduce((n, f) => n + countOf(LOCAL + '/' + f.id).total, 0);
  if (!held || !bk.state?.base) return false;
  const newest = bk.state.backups[0];
  return !newest || (pref().backup_every === 'off' && Date.now() - parseDate(newest.made) > 30 * 864e5);
}
function localGroup(a){
  const open = !state.collapsed[LOCAL];
  let items = '';
  if (open){
    const shown = a.folders.filter(f => f.kind === 'custom' || countOf(LOCAL + '/' + f.id).total > 0 || LOCAL + '/' + f.id === state.folder);
    items = '<ul>' + newFolderLine(LOCAL)
      + shown.map(f => folderButton(a, f)).join('') + '</ul>';
  }
  return `<div class="pcrow"><button type="button" class="acct" data-acct="${LOCAL}" aria-expanded="${open}" title="${esc(t('Mail kept only on this PC, on no server. Right-click a folder for more.'))}">${I.caret}${adot(LOCAL)}<span class="acct-mail">${esc(t('On this PC'))}</span></button>`
    + (backupNudge() ? `<button type="button" class="qchip bkchip" data-act="backups" title="${esc(t('This mail is only on this PC and has no recent backup. Click to back it up.'))}">${esc(t('No backup'))}</button>` : '')
    + `<button type="button" class="pcadd" data-act="new-folder" title="${esc(t('New folder on this PC'))}" aria-label="${esc(t('New folder on this PC'))}">${I.plus}</button></div>${items}`;
}
function renderTree(){
  win.classList.toggle('noacct', !ACCOUNTS.length); // the welcome takes the whole right side
  win.classList.toggle('multi', servers().length > 1);
  // A folder being named keeps what was typed, and the focus, when the tree is drawn again.
  const typing = state.naming && $('nf-name'), had = typing && document.activeElement === typing;
  if (typing) state.naming.value = typing.value;
  const caret = typing ? [typing.selectionStart, typing.selectionEnd] : null;
  // Unread mail of every folder first, under a hairline: what to read, wherever rules put it.
  treeEl.innerHTML = savedSearchGroup() + (ACCOUNTS.length ? `<ul class="unreadg">${foldButton(UNREAD, 'unread')}</ul>` : '') + ACCOUNTS.map(a => {
    if (a.id === LOCAL) return localGroup(a);
    const open = !state.collapsed[a.id], key = f => a.id + '/' + f.id;
    let items = '';
    if (open){
      const own = a.folders.filter(f => !COUNT_ALL.has(f.kind) || key(f) === state.folder);
      const shown = f => MAIN_KINDS.has(f.kind) || state.pinned.has(key(f)) || key(f) === state.folder;
      const main = own.filter(shown), rest = own.filter(f => !shown(f)), more = !!state.more[a.id];
      items = '<ul>' + newFolderLine(a.id) + (more ? own : main).map(f => folderButton(a, f)).join('');
      if (rest.length){
        const unread = more ? 0 : rest.reduce((n, f) => n + countOf(key(f)).unread, 0);
        items += `<li><button type="button" class="fold more" data-more="${esc(a.id)}" aria-expanded="${more}">${I.caret}<span>${esc(more ? t('Fewer folders') : tn(rest.length, '{n} more', '{n} more'))}</span>${unread ? `<span class="cnt">${num(unread)}</span>` : ''}</button></li>`;
      }
      items += '</ul>';
    }
    // A nearly full mailbox on the server says so next to its address.
    const q = quotaOf(a.id), pct = q ? pctOf(q) : 0;
    const full = pct >= 90 ? `<span class="qchip" data-level="${levelOf(pct)}" title="${esc(quotaText(q))}">${pct}%</span>` : '';
    // An account whose last check failed says so until one works (the status bar can retry).
    const failed = PROBLEMS[a.id] ? t('The last check failed: {reason}', {reason:PROBLEMS[a.id]}) : '';
    const bad = failed ? `<span class="qchip" data-level="full" title="${esc(failed)}" aria-hidden="true">!</span><span class="vh">${esc(failed)}</span>` : '';
    return `<div class="agrp ${acolor(a.id)}"><button type="button" class="acct" data-acct="${esc(a.id)}" aria-expanded="${open}" title="${esc(a.name)} · ${esc(a.email)}${failed ? '\n' + esc(failed) : ''}">${I.caret}${adot(a.id)}<span class="acct-mail">${esc(a.email)}</span>${full}${bad}</button>${items}</div>`;
  }).join('') + unsentGroup() + `<button type="button" class="add-acct" data-act="add-account">${I.plus}<span>${esc(servers().length ? t('Add account') : t('Add your account'))}</span></button>`;
  const input = $('nf-name');
  if (input){
    wireNaming(input);
    if (had){ input.focus(); if (caret) input.setSelectionRange(...caret); }
  }
  if (drag?.started) paintDrag(); // the folders under a drag show it again
  const option = key => { const n = countOf(key).unread; return `<option value="${key}"${key === state.folder ? ' selected' : ''}>${esc(folderName(key))}${n ? ` (${n})` : ''}</option>`; };
  const unsent = unsentKeys();
  $('folder-select').innerHTML = (ACCOUNTS.length ? option(UNREAD) : '') + ACCOUNTS.map(a => `<optgroup label="${esc(acctLabel(a.id))}">`
    + a.folders.filter(f => !COUNT_ALL.has(f.kind) || a.id + '/' + f.id === state.folder).map(f => option(a.id + '/' + f.id)).join('') + '</optgroup>').join('')
    + (unsent.length ? `<optgroup label="${esc(t('Not sent yet'))}">${unsent.map(option).join('')}</optgroup>` : '');
}
treeEl.addEventListener('click', e => {
  const search = e.target.closest('[data-search]');
  if (search){ openSavedSearch(Number(search.dataset.search)); return; }
  if (e.target.closest('[data-act="saved-searches"]')){ openSavedSearches(); return; }
  const f = e.target.closest('[data-folder]'), a = e.target.closest('[data-acct]'), m = e.target.closest('[data-more]');
  if (e.target.closest('[data-act="add-account"]')) openAccountDialog();
  else if (e.target.closest('[data-act="new-folder"]')) startNaming('new');
  else if (e.target.closest('[data-act="backups"]')) openSettings('backup');
  else if (f) setFolder(f.dataset.folder);
  else if (m) toggleMore(m.dataset.more);
  else if (a){ state.collapsed[a.dataset.acct] = !state.collapsed[a.dataset.acct]; renderTree(); }
});
// All of an account's folders, or the main ones and "N more".
function toggleMore(acct){
  if (state.more[acct]) delete state.more[acct]; else state.more[acct] = true;
  persist(); renderTree();
}
// A folder kept in view (pinned) shows with the main ones, outside "more".
function togglePin(key){
  if (state.pinned.has(key)) state.pinned.delete(key); else state.pinned.add(key);
  persist(); renderTree();
  say(state.pinned.has(key) ? t('{folder} stays in view.', {folder:folderName(key)}) : t('{folder} goes back under “more”.', {folder:folderName(key)}), 3000);
}
// The accounts' order in the tree (and everywhere they are listed). The folders on this PC stay last.
async function moveAccount(id, delta){
  try { await invoke('move_account', {id, delta}); } catch (e){ say(tr(e), 8000); return; }
  await loadTree();
  renderTree(); renderStatus(); drawRows();
  if (sdlg.open && setTab === 'accts' && accountView === 'home') renderAccountList();
}
// What a right-click (or the menu key) on an account offers.
function accountMenu(id){
  const list = servers(), i = list.findIndex(a => a.id === id);
  if (i < 0) return [];
  return [
    {label:t('Move up'), key:'Alt+↑', disabled:i === 0, run:() => moveAccount(id, -1)},
    {label:t('Move down'), key:'Alt+↓', disabled:i === list.length - 1, run:() => moveAccount(id, 1)},
    {sep:true},
    ...(isPop3(id) ? [{label:t('New folder…'), run:() => startNaming('new', id)}] : []),
    {label:state.more[id] ? t('Show fewer folders') : t('Show all folders'), run:() => toggleMore(id)},
    // A POP3 account's folders are Telex's own: nothing to choose on a server.
    ...(isPop3(id) ? [] : [{label:t('Which folder is which…'), run:() => openFolderChoices(id)}]),
    {label:t('Account settings…'), run:() => { openSettings('accts'); editAccount(id); }},
  ];
}
$('folder-select').addEventListener('change', e => setFolder(e.target.value));

/* ---------- folders on this PC ---------- */
// Made and renamed right in the tree: the name is typed where the folder shows; Enter keeps
// it, Esc doesn't. Removed only when empty. Right-click (or the menu key) for more.
function startNaming(key, acct = key === 'new' ? LOCAL : acctId(key)){
  if (!ACCT[acct]) return;
  delete state.collapsed[acct];
  // A new folder of a POP3 account shows with its others: all of them, not only the main ones.
  if (key === 'new' && acct !== LOCAL) state.more[acct] = true;
  state.naming = {key, acct, value:key === 'new' ? '' : folderName(key), busy:false};
  renderTree();
  const input = $('nf-name');
  input?.focus(); input?.select();
}
function wireNaming(input){
  input.addEventListener('keydown', e => {
    if (e.key === 'Enter'){ e.preventDefault(); e.stopPropagation(); finishNaming(true); }
    else if (e.key === 'Escape'){ e.preventDefault(); e.stopPropagation(); finishNaming(false); }
  });
  input.addEventListener('input', () => { if (state.naming) state.naming.value = input.value; });
  input.addEventListener('blur', () => setTimeout(() => {
    if (state.naming && !state.naming.busy && document.activeElement !== $('nf-name')) finishNaming(!!input.value.trim());
  }, 0));
}
async function finishNaming(save){
  const n = state.naming;
  if (!n || n.busy) return;
  const name = ($('nf-name')?.value ?? n.value).trim();
  const back = () => { state.naming = null; renderTree(); treeEl.querySelector(`[data-folder="${n.key}"]`)?.focus(); };
  if (!save || !name || (n.key !== 'new' && name === folderName(n.key))){ back(); return; }
  n.busy = true;
  try {
    if (n.key === 'new'){
      const key = await invoke('create_local_folder', {name, account:n.acct === LOCAL ? null : n.acct});
      state.naming = null;
      await loadTree();
      await setFolder(key);
      say(n.acct === LOCAL ? t('Made the folder {folder} on this PC. Drag mail onto it, or press M.', {folder:name})
        : t('Made the folder {folder}. Drag mail onto it, or press M.', {folder:name}), 6000);
      treeEl.querySelector(`[data-folder="${key}"]`)?.focus();
    } else {
      await invoke('rename_local_folder', {folder:n.key, name});
      await loadTree();
      back();
      renderStatus();
    }
  } catch (e){
    n.busy = false;
    say(tr(e), 8000);
    $('nf-name')?.focus();
  }
}
async function markFolderRead(key){
  let n;
  try { n = await call('mark_folder_read', {folder:key}); } catch { return; }
  await refresh();
  say(tn(n, 'Marked {n} message as read.', 'Marked {n} messages as read.', {n:num(n)}), 3000);
}
async function deleteFolder(key){
  const name = folderName(key);
  let rules;
  try { rules = await invoke('delete_local_folder', {folder:key}); } catch (e){ say(tr(e), 8000); return; }
  await loadTree();
  if (state.folder === key) setFolder(isLocal(key) ? LOCAL + '/archive' : keyOfKind(acctId(key), 'inbox')); else renderTree();
  say(t('Removed the folder {folder}.', {folder:name}) + (rules ? ' ' + tn(rules, '{n} rule no longer moves mail there.', '{n} rules no longer move mail there.') : ''), 6000);
}
// A folder as one mbox file, which other mail programs read.
async function exportFolder(key){
  let path;
  try {
    path = await invoke('export_folder', {folder:key, words:{title:t('Save {folder} as mbox', {folder:placeName(key)}), filter:t('mbox files'), file_name:folderName(key)}});
  } catch (e){ say(tr(e), 8000); return; }
  if (path) say(t('Saved {folder} to {path}.', {folder:placeName(key), path}), 8000);
}
// .eml and mbox files into a folder on this PC: dropped on it (`paths`), or chosen in Open.
async function importInto(key, paths = null){
  let r;
  try {
    r = await invoke('import_mail', {folder:key, paths, words:{title:t('Bring mail into {folder}', {folder:placeName(key)}), filter:t('Mail files'), all:t('All files')}});
  } catch (e){ say(tr(e), 10000); return; }
  if (!r) return; // cancelled
  await refresh();
  const lines = [tn(r.added, 'Brought {n} message into {folder}.', 'Brought {n} messages into {folder}.', {n:num(r.added), folder:placeName(key)})];
  if (r.already) lines.push(tn(r.already, '{n} was there already.', '{n} were there already.', {n:num(r.already)}));
  say([...lines, ...r.problems].join(' '), r.problems.length ? 15000 : 8000);
}
listenHere('import-progress', ({payload: p}) => say(t('Reading mail from {file}: {n} so far…', {file:p.file, n:num(p.done)}), 0));
// What a right-click (or the menu key) on a folder offers.
function folderMenu(key){
  const f = folderOf(key);
  if (!f || isEvery(key)) return [];
  const local = isLocal(key), here = keepsHere(key), own = here && f.kind === 'custom';
  const sections = [
    here ? [{label:t('New folder…'), run:() => startNaming('new', acctId(key))}] : [],
    // Inbox and Sent are always in view; the folders on this PC too.
    !local && !MAIN_KINDS.has(f.kind) ? [{label:t('Keep in view'), checked:state.pinned.has(key), run:() => togglePin(key)}] : [],
    // A server folder another program kept sent mail in ("Wysłane", "Sent Items").
    !here && f.kind === 'custom' ? [{label:t('Use for sent mail'), run:() => chooseFolder(key, 'sent')},
      {label:t('Which folder is which…'), run:() => openFolderChoices(acctId(key))}] : [],
    own ? [{label:t('Rename…'), key:'F2', run:() => startNaming(key)}, {label:t('Delete folder'), disabled:countOf(key).total > 0, run:() => deleteFolder(key)}] : [],
    [...(countOf(key).unread ? [{label:t('Mark all as read'), key:'Shift+U', run:() => markFolderRead(key)}] : []),
      ...(here && f.kind !== 'trash' && f.kind !== 'outbox' ? [{label:t('Bring mail in from files…'), run:() => importInto(key)}] : []),
      {label:t('Save as mbox…'), run:() => exportFolder(key)}],
    // Its own greeting and reply header (Settings › Writing).
    !['outbox', 'drafts', 'trash', 'spam'].includes(f.kind) ? [{label:t('Templates…'), run:() => openFolderTemplates(key)}] : [],
  ];
  return sections.filter(s => s.length).flatMap((s, i) => i ? [{sep:true}, ...s] : s);
}
// The menu for a folder or an account line in the tree.
function treeMenu(el){
  if (el.dataset.folder) return folderMenu(el.dataset.folder);
  if (el.dataset.acct === LOCAL) return [{label:t('New folder…'), run:() => startNaming('new')}];
  return el.dataset.acct ? accountMenu(el.dataset.acct) : [];
}
treeEl.addEventListener('contextmenu', e => {
  e.preventDefault();
  const el = e.target.closest('[data-folder], [data-acct]');
  const items = el ? treeMenu(el) : [];
  if (items.length) openMenu(e.clientX, e.clientY, items, el);
});
treeEl.addEventListener('keydown', e => {
  const el = e.target.closest?.('[data-folder], [data-acct]');
  if (!el) return;
  if (e.key === 'ContextMenu' || (e.key === 'F10' && e.shiftKey)){
    e.preventDefault(); e.stopPropagation();
    const r = el.getBoundingClientRect(), items = treeMenu(el);
    if (items.length) openMenu(r.left + 24, r.bottom, items, el);
  } else if (e.key === 'F2' && el.dataset.folder && keepsHere(el.dataset.folder) && kindOf(el.dataset.folder) === 'custom'){
    e.preventDefault(); e.stopPropagation(); startNaming(el.dataset.folder);
  } else if (e.altKey && (e.key === 'ArrowUp' || e.key === 'ArrowDown') && el.dataset.acct && el.dataset.acct !== LOCAL){
    // Alt+↑ ↓ on an account: up or down in the tree, keeping the focus on it.
    e.preventDefault(); e.stopPropagation();
    const id = el.dataset.acct;
    moveAccount(id, e.key === 'ArrowUp' ? -1 : 1).then(() => treeEl.querySelector(`[data-acct="${CSS.escape(id)}"]`)?.focus());
  }
});

/* ---------- a small menu ---------- */
// Right-click menus: ↑ ↓ choose, Enter takes, Esc or a click elsewhere closes, and the focus
// goes back where it was.
const menuEl = document.createElement('ul');
menuEl.className = 'cmenu'; menuEl.setAttribute('role', 'menu'); menuEl.hidden = true;
document.body.append(menuEl);
let menuItems = [], menuFrom = null;
// An item with `checked` (true or false) is a choice that is on or off, with a tick when on.
let menuAt = [0, 0]; // where the last menu opened, for one that follows it (Tags…)
function openMenu(x, y, items, from){
  menuItems = items; menuFrom = from; menuAt = [x, y];
  const ticks = items.some(it => 'checked' in it);
  menuEl.innerHTML = items.map((it, i) => it.sep ? '<li class="msep" role="separator"></li>'
    : `<li role="${'checked' in it ? 'menuitemcheckbox' : 'menuitem'}" tabindex="-1" data-i="${i}"${'checked' in it ? ` aria-checked="${!!it.checked}"` : ''}${it.disabled ? ' aria-disabled="true"' : ''}>`
      + `<span${it.cls ? ` class="${it.cls}"` : ''}>${ticks ? `<i class="tick" aria-hidden="true">${it.checked ? '✓' : ''}</i>` : ''}${it.swatch ? `<i class="sw ${it.swatch}" aria-hidden="true"></i>` : ''}${it.color ? `<i class="tdot t-${esc(it.color)}" aria-hidden="true"></i>` : ''}${esc(it.label)}</span>${it.key ? `<kbd>${esc(it.key)}</kbd>` : ''}</li>`).join('');
  menuEl.hidden = false;
  const w = menuEl.offsetWidth, h = menuEl.offsetHeight;
  menuEl.style.left = Math.max(4, Math.min(x, innerWidth - w - 4)) + 'px';
  menuEl.style.top = Math.max(4, Math.min(y, innerHeight - h - 4)) + 'px';
  menuEl.querySelector('[data-i]:not([aria-disabled])')?.focus();
}
function closeMenu(refocus = true){
  if (menuEl.hidden) return;
  menuEl.hidden = true;
  if (refocus) menuFrom?.focus?.();
}
function runMenu(li){
  const it = menuItems[Number(li?.dataset.i)];
  if (!it || it.disabled) return;
  closeMenu(false);
  it.run();
}
menuEl.addEventListener('click', e => runMenu(e.target.closest('[data-i]')));
menuEl.addEventListener('keydown', e => {
  const all = [...menuEl.querySelectorAll('[data-i]:not([aria-disabled])')], i = all.indexOf(document.activeElement);
  if (e.key === 'ArrowDown' || e.key === 'ArrowUp'){ all[(i + (e.key === 'ArrowDown' ? 1 : -1) + all.length) % all.length]?.focus(); }
  else if (e.key === 'Home' || e.key === 'End'){ all[e.key === 'Home' ? 0 : all.length - 1]?.focus(); }
  else if (e.key === 'Enter' || e.key === ' '){ runMenu(document.activeElement); }
  else if (e.key === 'Escape' || e.key === 'Tab'){ closeMenu(); }
  else return;
  e.preventDefault(); e.stopPropagation();
});
document.addEventListener('pointerdown', e => { if (!menuEl.hidden && !menuEl.contains(e.target)) closeMenu(false); }, true);
addEventListener('blur', () => closeMenu(false));

function setFolder(key){
  state.savedSearch = null;
  if (key === UNREAD && state.folder !== UNREAD) state.unreadFrom = state.folder; // where O goes back to
  state.folder = key; state.q = ''; qEl.value = ''; state.inText = false; state.thread = null;
  state.unread = false; state.kept.clear(); state.expanded.clear();
  unpick();
  paintUnreadOnly();
  paintScope(); renderTree(); renderStatus();
  return loadView({top:true});
}

/* ---------- only unread ---------- */
// O, the dot above the list's dots, or "n unread" in the status bar: the list shows only unread
// mail, until O again, Esc or another folder. What is read meanwhile stays in it (`keep`), so
// rows don't jump away under the reader. Unread in the tree is the same for every folder at once;
// there O goes back to the folder that was open before.
function toggleUnreadOnly(){
  if (!ACCOUNTS.length || VIEWER) return;
  if (state.folder === UNREAD){
    const back = state.unreadFrom && folderOf(state.unreadFrom) ? state.unreadFrom : homeOf(ACCOUNTS[0]?.id);
    setFolder(back).then(() => listEl.focus({preventScroll:true}));
    return;
  }
  if (state.thread) leaveThread();
  state.unread = !state.unread; state.kept.clear();
  unpick();
  paintUnreadOnly(); renderStatus();
  loadView().then(() => {
    if (state.unread) say(list.total ? t('Only unread mail. O shows everything again.') : t('Nothing unread here. O shows everything again.'), 3500);
  });
}
function paintUnreadOnly(){
  const b = $('b-unread'), on = state.folder === UNREAD || state.unread;
  b.setAttribute('aria-pressed', String(on));
  b.title = state.folder === UNREAD ? t('Unread mail of every folder. O goes back to the folder before.') : on ? t('Show every message (O)') : t('Show only unread mail (O)');
  b.setAttribute('aria-label', b.title);
}
$('b-unread').addEventListener('click', () => { toggleUnreadOnly(); listEl.focus({preventScroll:true}); });

/* ---------- list ---------- */
// The list holds one stretch of the current view (a folder, or all mail while filtering; an
// order; the quick filter): `rows`, starting at position `start` of `total`. The core sends
// pages of it: the next or previous page while scrolling (keyset, "after this message"), the
// page at a row after a jump with the scroll bar. Only the rows on screen and a margin are
// drawn; spacer rows above and below give the table its full height.
const PAGE = 100, MARGIN = 20, KEEP = 600;
const list = {total:0, start:0, rows:[], seq:0};
let rowH = 21, selRow = null; // selRow: the selected message, also when its row isn't loaded
// All mail (the filter in every account) or a conversation: lists across folders, with a Folder column.
const allMail = () => !!state.thread || (state.scope === 'all' && !!state.q.trim());
// Lists whose rows come from more than one folder: all mail, and every account's Drafts or Outbox.
const across = () => allMail() || isEvery(state.folder);
const selKey = () => state.thread ? '#' : allMail() ? '*' : state.folder;
// An unread list: Unread in the tree, or a folder with O. A conversation shows all of itself.
const unreadOnly = () => !state.thread && (state.folder === UNREAD || state.unread);
const grouped = () => !!pref().threaded && !state.thread && !state.savedSearch && !state.q.trim() && !unreadOnly()
  && !isEvery(state.folder) && !['drafts', 'outbox'].includes(kindOf(state.folder));
const viewArgs = () => ({folder:allMail() || state.folder === UNREAD ? null : state.folder, sort:state.sort.key, ascending:state.sort.dir > 0,
  filter:state.q, bodies:state.inText, thread:state.thread?.id ?? null, unread:unreadOnly(), keep:unreadOnly() ? [...state.kept] : [],
  grouped:grouped(), expanded:grouped() ? [...state.expanded] : []});
// Whether a message belongs in the list on screen.
const inView = f => allMail() || f === state.folder || state.folder === UNREAD || (isEvery(state.folder) && kindOf(f) === kindOf(state.folder));
const current = () => selRow && selRow.id === state.sel[selKey()] && inView(selRow.f) ? selRow : null;
// The messages an action is for: those picked with Shift or Ctrl, else the selected one.
const chosen = () => state.picked.size ? [...state.picked] : current() ? [current().id] : [];
function unpick(){
  if (!state.picked.size && state.anchor == null) return false;
  state.picked.clear(); state.anchor = null; state.span = 0;
  return true;
}

// A page of the current view, or null when the view changed while the core answered.
async function fetchPage(at, limit = PAGE){
  const args = viewArgs(), key = JSON.stringify(args);
  const p = await invoke('list_page', {view:args, at, limit});
  if (key !== JSON.stringify(viewArgs())) return null;
  return {total:p.total, start:p.start, rows:p.rows.map(fromSummary), focus:p.focus};
}
// Put a page in the list: after or before the loaded stretch when it fits there, else instead of it.
function merge(p, how = 'replace'){
  const s = list.start, e = s + list.rows.length;
  list.total = p.total;
  if (how === 'after' && p.start === e) list.rows = list.rows.concat(p.rows);
  else if (how === 'before' && p.start + p.rows.length === s){ list.rows = p.rows.concat(list.rows); list.start = p.start; }
  else { list.rows = p.rows; list.start = p.start; } // a jump, or mail came and went meanwhile
  if (list.rows.length > KEEP){ // keep only the part near the screen
    const from = Math.max(0, Math.min(visibleRange()[0] - MARGIN - list.start, list.rows.length - KEEP));
    list.rows = list.rows.slice(from, from + KEEP); list.start += from;
  }
  const fresh = selRow && list.rows.find(r => r.id === selRow.id);
  if (fresh) selRow = fresh;
}
const posOf = id => { const i = list.rows.findIndex(r => r.id === id); return i < 0 ? -1 : list.start + i; };
const headH = () => listEl.querySelector('thead').offsetHeight;
function visibleRange(){
  const top = listEl.scrollTop, h = Math.max(0, listEl.clientHeight - headH());
  return [Math.floor(top / rowH), Math.ceil((top + h) / rowH)];
}
// Scroll so the row at `pos` is on screen. True when the list moved.
function reveal(pos){
  const top = pos * rowH, h = listEl.clientHeight - headH();
  if (top < listEl.scrollTop) listEl.scrollTop = top;
  else if (top + rowH > listEl.scrollTop + h) listEl.scrollTop = top + rowH - h;
  else return false;
  return true;
}

/* ---------- columns ---------- */
// The columns fit the list's width. The icons, the date and the size are as wide as what they
// show in this language and font, so a date is never cut; From (and Folder, in all mail) takes
// a share of the rest, or the width it was dragged to, and Subject what is left. A narrow list
// lets Size go first, then gives From less. Drag the edge before Subject (or before Folder) to
// set a width, double-click it to fit the names on screen; the header's right-click menu has the
// rest. Kept with the window's other sizes (telex-ui).
const COL = {ic:50, pad:12, minFrom:80, minSubj:140, minWhere:90, narrow:640};
const cssVar = name => getComputedStyle(document.documentElement).getPropertyValue(name).trim();
let measureCtx = null;
function textWidth(text, font){
  measureCtx ??= document.createElement('canvas').getContext('2d');
  measureCtx.font = font;
  return measureCtx.measureText(text).width;
}
// The widest date and size this language can show, measured once per language and date style.
let measured = null;
function measure(){
  const key = I18n.lang() + colPref.dates + document.documentElement.dataset.font;
  if (measured?.key === key) return measured;
  const data = `11.5px ${cssVar('--font-data')}`, head = `500 11.5px ${cssVar('--font-ui')}`, arrow = 14;
  const late = new Date(2026, 11, 28, 23, 59);
  const dates = colPref.dates === 'full' ? [dmy(late) + ' ' + hm(late)] : [hm(late), dmy(late), ...DOW.map(d => t(d) + ' ' + hm(late))];
  const sizes = ['1023 KB', I18n.decimal(999.9) + ' MB', I18n.decimal(99.9) + ' GB'];
  const widest = (list, font) => Math.max(...list.map(s => textWidth(s, font)));
  // The faces load when first used; until then the canvas measures a fallback. Measure again then.
  if (document.fonts && !(document.fonts.check(data) && document.fonts.check(head))){
    Promise.all([document.fonts.load(data), document.fonts.load(head)])
      .then(() => { if (measured?.key === key){ measured = null; colsDone = ''; fitColumns(); } }).catch(() => {});
  }
  measured = {key,
    date:Math.ceil(Math.max(widest(dates, data), textWidth(t('Received'), head) + arrow) + COL.pad + 2),
    size:Math.ceil(Math.max(widest(sizes, data), textWidth(t('Size'), head) + arrow) + COL.pad + 2)};
  return measured;
}
let colCount = 5, colsDone = '';
// Set the widths for the list as it is now: its width, all mail or a folder, the choices.
function fitColumns(){
  const w = listEl.clientWidth;
  if (!w) return;
  const where = across(), size = colPref.size && !where && w >= COL.narrow, m = measure();
  const fixed = COL.ic + m.date + (size ? m.size : 0);
  let flex = Math.max(0, w - fixed);
  let whereW = 0;
  if (where){
    const share = servers().length > 1 && !byAccount() ? 0.3 : 0.2; // the folder and whose it is
    whereW = clamp(colPref.where ?? Math.round(flex * share), COL.minWhere, Math.max(COL.minWhere, Math.round(flex * 0.45)));
    flex -= whereW;
  }
  const fromAuto = clamp(Math.round(flex * 0.3), 120, 280);
  const fromW = Math.max(Math.min(COL.minFrom, Math.round(flex * 0.4)), Math.min(colPref.from ?? fromAuto, flex - COL.minSubj));
  const widths = {ic:COL.ic, from:fromW, where:whereW, date:m.date, size:m.size};
  const done = JSON.stringify([widths, size, where]);
  if (done === colsDone) return;
  colsDone = done;
  for (const [k, px] of Object.entries(widths)){
    const th = listEl.querySelector('th.c-' + k);
    if (th) th.style.width = px + 'px';
  }
  win.classList.toggle('nosize', !size);
  const n = 4 + (where ? 1 : 0) + (size ? 1 : 0);
  if (n !== colCount){ colCount = n; drawRows(); }
}
// A double-click on an edge: From as wide as the widest name on screen (Folder: the widest folder).
function fitToNames(which){
  const font = `600 12.5px ${cssVar('--font-list')}`;
  const texts = list.rows.slice(Math.max(0, visibleRange()[0] - list.start - MARGIN)).slice(0, 200)
    .map(m => which === 'where' ? whereOf(m, servers().length > 1).join(' · ') : whoOf(m));
  const px = Math.ceil(Math.max(0, ...texts.map(s => textWidth(s, font)))) + COL.pad + (which === 'where' ? 14 : 4);
  colPref[which] = Math.max(which === 'where' ? COL.minWhere : COL.minFrom, px);
  colsDone = ''; fitColumns(); persist();
}
function resetColumns(){
  colPref.from = colPref.where = null;
  colsDone = ''; fitColumns(); persist(); renderColumnChoices();
  say(t('Columns fit the list again.'), 2500);
}
// Dragging an edge: the column before it (From) or after it (Folder) follows the pointer.
const theadEl = listEl.querySelector('thead');
theadEl.addEventListener('pointerdown', e => {
  const g = e.target.closest('[data-grip]');
  if (!g || e.button !== 0) return;
  e.preventDefault();
  const which = g.dataset.grip, th = listEl.querySelector('th.c-' + which), start = th.offsetWidth, x0 = e.clientX;
  let moved = false; // a click (or a double-click's first) changes nothing
  g.setPointerCapture(e.pointerId); g.classList.add('drag'); win.classList.add('resizing');
  const mv = ev => {
    if (!moved && Math.abs(ev.clientX - x0) < 3) return;
    moved = true;
    colPref[which] = Math.max(40, start + (which === 'where' ? x0 - ev.clientX : ev.clientX - x0)); colsDone = ''; fitColumns();
  };
  const up = () => {
    g.classList.remove('drag'); win.classList.remove('resizing');
    g.removeEventListener('pointermove', mv); g.removeEventListener('pointerup', up); g.removeEventListener('pointercancel', up);
    if (moved){ colPref[which] = th.offsetWidth; persist(); } // what fits, not where the pointer went
  };
  g.addEventListener('pointermove', mv); g.addEventListener('pointerup', up); g.addEventListener('pointercancel', up);
});
theadEl.addEventListener('dblclick', e => { const g = e.target.closest('[data-grip]'); if (g) fitToNames(g.dataset.grip); });
theadEl.addEventListener('keydown', e => {
  const g = e.target.closest('[data-grip]');
  if (!g) return;
  const which = g.dataset.grip, th = listEl.querySelector('th.c-' + which), sign = which === 'where' ? -1 : 1;
  const d = e.key === 'ArrowLeft' ? -16 * sign : e.key === 'ArrowRight' ? 16 * sign : 0;
  if (d){ colPref[which] = th.offsetWidth + d; colsDone = ''; fitColumns(); colPref[which] = th.offsetWidth; persist(); }
  else if (e.key === 'Enter') fitToNames(which);
  else return;
  e.preventDefault(); e.stopPropagation();
});
// The list's columns: short or full dates, the Size column, widths fitted again. Right-click on
// the headings, or Settings › Appearance.
function setDates(v){ colPref.dates = v; measured = null; colsDone = ''; persist(); fitColumns(); drawRows(); renderColumnChoices(); }
function setSizeColumn(on){ colPref.size = on; persist(); colsDone = ''; fitColumns(); renderColumnChoices(); }
const DATE_DAY = new Date(2026, 9, 7, 6, 12), DATE_OLD = new Date(2026, 8, 30, 6, 12);
const shortDates = () => t('Short dates: {examples}', {examples:`${hm(DATE_DAY)}, ${t(DOW[DATE_DAY.getDay()])} ${hm(DATE_DAY)}, ${dmy(DATE_OLD)}`});
const fullDates = () => t('Full dates: {example}', {example:`${dmy(DATE_OLD)} ${hm(DATE_OLD)}`});
function columnMenu(){
  return [
    {label:shortDates(), checked:colPref.dates === 'short', run:() => setDates('short')},
    {label:fullDates(), checked:colPref.dates === 'full', run:() => setDates('full')},
    {sep:true},
    {label:t('Size column'), checked:colPref.size, disabled:across(), run:() => setSizeColumn(!colPref.size)},
    {sep:true},
    {label:t('Fit columns to the list'), disabled:!colPref.from && !colPref.where, run:resetColumns},
  ];
}
// Settings › Appearance › Message list columns, kept in step with the menu.
function renderColumnChoices(){
  if (!sdlg.open && $('s-dates').childElementCount) return;
  $('s-dates').innerHTML = radios('list_dates', [['short', esc(shortDates())], ['full', esc(fullDates())]], colPref.dates);
  $('s-sizecol').checked = colPref.size;
  $('s-fitcols').disabled = !colPref.from && !colPref.where;
}
$('s-fitcols').addEventListener('click', resetColumns);
theadEl.addEventListener('contextmenu', e => {
  e.preventDefault();
  openMenu(e.clientX, e.clientY, columnMenu(), e.target.closest('button, [data-grip]') ?? listEl);
});
document.fonts?.ready.then(() => { measured = null; colsDone = ''; fitColumns(); });

// Who a row is from, or in Sent and Drafts who it went to (in all mail with "To:").
function whoOf(m){
  if (!isOut(m.f)) return m.from;
  return allMail() ? t('To: {who}', {who:nameOf(m.to)}) : nameOf(m.to);
}
// Where a row of a list that mixes folders is: its folder and, with more than one account, whose
// (the folders on this PC say so in their name), or its account in every account's Drafts.
function whereOf(m, accounts){
  const acct = acctId(m.f);
  if (byAccount()) return [acctLabel(acct)];
  if (acct === LOCAL) return [placeName(m.f)];
  return accounts ? [folderName(m.f), acctLabel(acct)] : [folderName(m.f)];
}
function rowHtml(m, sel, hl, accounts){
  const out = isOut(m.f), tip = out ? m.to : `${m.from} <${m.email}>`;
  const on = m.id === sel || state.picked.has(m.id), acct = acctId(m.f);
  const th = m.thread, head = th && !th.child, expanded = th && state.expanded.has(th.id);
  const cls = (m.unread ? 'unread ' : '') + (th?.child ? 'thread-child ' : head && th.unread ? 'thread-new ' : '') + (m.id === sel ? 'sel ' : '') + (state.picked.has(m.id) ? 'picked ' : '')
    + (acct !== LOCAL ? 'acc ' + acolor(acct) : '');
  const count = head ? tn(th.count, '{n} message', '{n} messages') + (th.unread ? ', ' + tn(th.unread, '{n} unread', '{n} unread') : '') : '';
  const toggleLabel = t(expanded ? N_('Collapse thread') : N_('Expand thread'));
  const toggle = head ? `<button type="button" class="thread-toggle" data-thread="${th.id}" aria-expanded="${expanded}" aria-label="${esc(toggleLabel)}" title="${esc(toggleLabel)}">${expanded ? '▾' : '▸'}</button>` : '<span class="thread-slot" aria-hidden="true"></span>';
  const chips = m.tags?.length ? tagChips(m.tags, 3) : '';
  const subject = grouped() ? `<span class="thread-subject">${toggle}${chips}<span class="subject-text">${marked(m.subj, hl?.subject)}</span>${head ? `<span class="thread-count" title="${esc(count)}" aria-label="${esc(count)}">${num(th.count)}</span>` : ''}</span>` : chips + marked(m.subj, hl?.subject);
  const [where, whose] = whereOf(m, accounts);
  // Mail sent later says in the Outbox when it goes.
  const goes = m.goes ? `<span class="goes">${esc(t('Goes {when}', {when:fmtGoes(m.goes)}))}</span> ` : '';
  // The dot and the flag are switches: a click marks read or unread, flags or unflags.
  const dot = `<b class="mk rd" title="${esc(m.unread ? t('Mark as read (U)') : t('Mark as unread (U)'))}"><i class="dot"></i></b>`;
  const flag = `<b class="mk fl" title="${esc(m.flagged ? t('Remove the flag (F)') : t('Flag (F)'))}">${m.flagged ? I.flagFill.replace('aria-hidden="true"', `role="img" aria-label="${esc(t('Flagged'))}"`) : I.flagOff}</b>`;
  return `<tr data-id="${m.id}" class="${cls}" aria-selected="${on}">
<td class="c-ic"><span class="ic">${dot}${flag}${m.hasAtt ? I.clip : '<i class="sp"></i>'}</span></td>
<td class="c-from" title="${esc(tip)}">${marked(whoOf(m), hl?.[out ? 'to' : 'from'])}</td>
<td class="c-subj" title="${esc(m.subj)}">${goes}${subject}</td>
<td class="c-where" title="${esc(acctLabel(acct))} › ${esc(folderName(m.f))}">${adot(acct)}${esc(whose ? where + ' · ' + whose : where)}</td>
<td class="c-date">${fmtList(m.date)}</td>
<td class="c-size">${fmtSize(m.kb)}</td></tr>`;
}

/* ---------- what the filter found, marked in the rows ---------- */
// The words of the quick filter, folded (no case, no diacritics), for the cells they match:
// plain words in From and Subject, from: (to: in Sent) in From, subject: in Subject.
let hlFor = null;
function hlTerms(){
  const q = state.q.trim();
  if (hlFor?.q === q) return hlFor.terms;
  let terms = null;
  if (q){
    const words = [], from = [], to = [], subject = [];
    for (const tok of q.match(/\S*"[^"]*"?|\S+/g) ?? []){
      const i = tok.indexOf(':'), key = i > 0 ? tok.slice(0, i).toLowerCase() : '';
      const val = fold((i > 0 ? tok.slice(i + 1) : tok).replace(/"/g, '').trim());
      if (val.length < 2) continue;
      if (!key) words.push(val);
      else if (key === 'from') from.push(val);
      else if (key === 'to') to.push(val);
      else if (key === 'subject') subject.push(val);
    }
    terms = {from:[...words, ...from], to:[...words, ...to], subject:[...words, ...subject]};
    if (!words.length && !from.length && !to.length && !subject.length) terms = null;
  }
  hlFor = {q, terms};
  return terms;
}
// Text with the parts that match marked; diacritics and case don't matter ("lodz" marks "Łódź").
function marked(text, words){
  text = String(text ?? '');
  if (!words?.length) return esc(text);
  let folded = '';
  const at = []; // where each folded character came from
  for (let i = 0; i < text.length; i++){
    const f = fold(text[i]);
    for (let j = 0; j < f.length; j++){ folded += f[j]; at.push(i); }
  }
  const spans = [];
  for (const w of words){
    for (let k = folded.indexOf(w); k >= 0; k = folded.indexOf(w, k + w.length)){
      spans.push([at[k], at[k + w.length - 1] + 1]);
    }
  }
  if (!spans.length) return esc(text);
  spans.sort((a, b) => a[0] - b[0]);
  let html = '', pos = 0;
  for (const [s, e] of spans){
    if (e <= pos) continue;
    const from = Math.max(s, pos);
    html += esc(text.slice(pos, from)) + '<mark>' + esc(text.slice(from, e)) + '</mark>';
    pos = e;
  }
  return html + esc(text.slice(pos));
}

const spacer = (px, n) => px > 0 ? `<tr class="pad" aria-hidden="true"><td colspan="${n}" style="height:${px}px"></td></tr>` : '';
// Draw the rows on screen and a margin; ask for the pages that are missing. While scrolling, the
// rows drawn last are kept as long as they still cover the screen with a few to spare.
let drawn = null; // {a, b, start, length}: the stretch drawn last
function drawRows(scrolling = false){
  const n = colCount;
  if (!list.total){
    drawn = null;
    const empty = state.q ? (allMail() ? t('No messages match “{q}” in any account. Press Esc to clear the filter.', {q:esc(state.q)}) : t('No messages match “{q}”. Press Esc to clear the filter.', {q:esc(state.q)}))
      : state.folder === UNREAD && !state.thread ? esc(t('Nothing unread. Everything in every folder is read.'))
      : unreadOnly() ? esc(t('Nothing unread here. Press O to see every message.')) : esc(t('This folder is empty.'));
    rowsEl.innerHTML = !ACCOUNTS.length || list.seq === 0 ? '' : `<tr class="empty"><td colspan="${n}">${empty}</td></tr>`;
    return;
  }
  const [va, vb] = visibleRange(), s = list.start, e = s + list.rows.length;
  if (scrolling && drawn && drawn.start === s && drawn.length === list.rows.length
      && (va >= drawn.a + 5 || drawn.a === 0) && (vb <= drawn.b - 5 || drawn.b === list.total)) return;
  const a = Math.max(0, va - MARGIN), b = Math.min(list.total, vb + MARGIN), sel = state.sel[selKey()], hl = hlTerms(), accounts = servers().length > 1;
  let html = spacer(a * rowH, n);
  for (let i = a; i < b; i++) html += i >= s && i < e ? rowHtml(list.rows[i - s], sel, hl, accounts) : `<tr class="rowph" aria-hidden="true"><td colspan="${n}"></td></tr>`;
  rowsEl.innerHTML = html + spacer((list.total - b) * rowH, n);
  drawn = {a, b, start:s, length:list.rows.length};
  const r = rowsEl.querySelector('tr[data-id]'), h = r?.getBoundingClientRect().height;
  if (h && Math.abs(h - rowH) > 0.01){ // the density changed: keep the same rows on screen
    listEl.scrollTop = listEl.scrollTop * h / rowH; rowH = h; drawRows(); return;
  }
  fill(a, b);
}
let drawQueued = false, drawScroll = true;
// Draw on the next frame; a scroll alone may keep what is drawn.
const drawSoon = (scrolling = false) => {
  drawScroll = drawQueued ? drawScroll && scrolling : scrolling;
  if (!drawQueued){ drawQueued = true; requestAnimationFrame(() => { drawQueued = false; drawRows(drawScroll); }); }
};
listEl.addEventListener('scroll', () => drawSoon(true));
new ResizeObserver(() => { fitColumns(); drawSoon(); }).observe(listEl);

// Load what rows [a, b) need, one page at a time, then draw again.
let filling = false;
async function fill(a, b){
  const s = list.start, e = s + list.rows.length;
  if (filling || (a >= s && b <= e)) return;
  const near = list.rows.length > 0;
  const [at, how] = near && a >= s && a <= e + PAGE && b > e ? [{after:list.rows[list.rows.length - 1].id}, 'after']
    : near && b <= e && b >= s - PAGE && a < s ? [{before:list.rows[0].id}, 'before']
    : [{row:a}, 'replace'];
  const seq = list.seq;
  filling = true;
  let p = null;
  try { p = await fetchPage(at); }
  catch { try { p = await fetchPage({row:a}); } catch {} } // the message we paged from is gone
  filling = false;
  if (!p || seq !== list.seq) return;
  merge(p, p && at.row === undefined ? how : 'replace');
  if (p.rows.length) drawRows();
}
// The row at position `pos`, loading its page if needed.
async function rowAt(pos){
  if (pos < 0 || pos >= list.total) return null;
  const s = list.start, e = s + list.rows.length;
  if (pos >= s && pos < e) return list.rows[pos - s];
  const [at, how] = pos === 0 ? ['first'] : pos === list.total - 1 ? ['last']
    : pos === e && e > s ? [{after:list.rows[list.rows.length - 1].id}, 'after']
    : pos === s - 1 && e > s ? [{before:list.rows[0].id}, 'before']
    : [{row:Math.max(0, pos - MARGIN)}];
  const p = await fetchPage(at);
  if (!p) return null;
  merge(p, how);
  return list.rows[pos - list.start] ?? null;
}

// Load the view again with the selection in it (or from the top), e.g. after a folder
// switch, a new order or filter. `top`: a different folder, so start at its top.
async function loadView({top = false} = {}){
  if (VIEWER) return viewerShow();
  const seq = ++list.seq, key = selKey(), before = selRow?.id;
  let id = state.sel[key], p;
  try { p = await fetchPage(id != null ? {around:id} : 'first'); }
  catch { try { p = await fetchPage('first'); } catch (e){ say(tr(e), 8000); return; } }
  if (!p || seq !== list.seq) return;
  merge(p);
  // Like before: a folder opens on its first message; a filter leaves the selection alone.
  if (p.focus != null && !list.rows.some(r => r.id === id)) id = state.sel[key] = p.focus;
  if (!list.rows.some(r => r.id === id) && (!state.q.trim() || key === '*')) id = state.sel[key] = list.rows[0]?.id;
  selRow = list.rows.find(r => r.id === id) ?? (state.q.trim() ? selRow : null);
  renderHead();
  if (top) listEl.scrollTop = 0;
  drawRows();
  const pos = posOf(id);
  if (pos >= 0 && reveal(pos)) drawRows();
  renderStatus();
  // The message on screen stays as it is while typing a filter or sorting again.
  if (!state.compose && (top || !selRow || selRow.id !== before || state.picked.size > 1)) renderPreview();
}
// After mail changed (a check, rules, sending, a push): new counts, and the open page again,
// keeping the first row on screen where it was. `skip`: a message that just went away.
// `arrived`: mail a check brought; when some of it is in this list, the list goes to its start
// (its end when sorted oldest first), where new mail is, and the selection stays.
async function refresh({skip, arrived} = {}){
  if (VIEWER) return viewerShow();
  try { await loadTree(); } catch { return; }
  if (!folderOf(state.folder)){ setFolder(homeOf(ACCOUNTS[0]?.id)); return; } // e.g. the folder is gone from the server
  if (state.picked.size){ // picked messages that went away (deleted elsewhere, moved by a rule) aren't picked any more
    try {
      const ids = new Set(await invoke('list_ids', {view:viewArgs(), from:null, to:null}));
      for (const id of state.picked) if (!ids.has(id)) state.picked.delete(id);
    } catch {}
  }
  renderTree(); renderStatus();
  const [va] = visibleRange(), seq = ++list.seq;
  const anchor = list.rows.slice(Math.max(0, va - list.start)).find(r => r.id !== skip);
  const before = selRow?.id;
  let p = null, end = null;
  if (arrived?.length){
    end = state.sort.key === 'date' && state.sort.dir > 0 ? 'last' : 'first';
    const fresh = new Set(arrived);
    try { p = await fetchPage(end); } catch {}
    if (p && !p.rows.some(r => fresh.has(r.id))) p = end = null; // none of it is in this list
  }
  if (!p) try { p = await fetchPage(anchor ? {around:anchor.id} : 'first'); } catch {}
  if (!p || seq !== list.seq) return;
  const was = anchor && !end ? posOf(anchor.id) : -1;
  merge(p);
  const now = anchor && !end ? posOf(anchor.id) : -1;
  if (end) listEl.scrollTop = end === 'first' ? 0 : list.total * rowH;
  if (grouped() && selRow?.thread && !state.expanded.has(selRow.thread.id)){
    const head = list.rows.find(r => r.thread?.id === selRow.thread.id && !r.thread.child);
    if (head){ selRow = head; state.sel[selKey()] = head.id; }
  }
  if (was >= 0 && now >= 0) listEl.scrollTop += (now - was) * rowH;
  if (!current()){ // the selection is gone: the first row, as when mail arrives in an empty folder
    selRow = list.rows.find(r => r.id === state.sel[selKey()]) ?? null;
    if (!selRow && !state.q.trim()){ selRow = list.rows[0] ?? null; state.sel[selKey()] = selRow?.id; }
  }
  drawRows(); renderStatus(); paintTagChips();
  // A change may have written a note on any message (rules, a refused send): read them again.
  const shown = selRow && bodies.get(selRow.id);
  bodies.clear();
  if (state.compose) return;
  if (selRow?.id !== before) renderPreview();
  else if (shown) full(shown.id).then(m => { if (m.note !== shown.note && selRow?.id === m.id) renderPreview(); }, () => {});
}

function renderHead(){
  const out = !allMail() && isOut(state.folder);
  $('from-label').textContent = out ? t('To') : t('From');
  $('where-label').textContent = byAccount() ? t('Account') : t('Folder');
  // Above one account's folder, its dot or initials once (with those marks; rows carry the others).
  const a = acctId(state.folder), mk = $('from-mark');
  mk.className = 'adot hmark ' + acolor(a);
  mk.dataset.i = INITIALS[a] ?? '';
  mk.hidden = across() || a === LOCAL || !acolor(a);
  mk.parentElement.title = mk.hidden ? '' : acctLabel(a);
  listEl.className = 'list' + (mk.hidden ? '' : ' acc ' + acolor(a)); // a tint goes under the rows too
  document.querySelectorAll('th button[data-sort]').forEach(b => {
    const on = b.dataset.sort === state.sort.key, th = b.parentElement;
    if (on) th.setAttribute('aria-sort', state.sort.dir > 0 ? 'ascending' : 'descending'); else th.removeAttribute('aria-sort');
    b.querySelector('.arr').textContent = on ? (state.sort.dir > 0 ? '▲' : '▼') : '';
  });
}
// Show the selection without drawing the rows again.
function paintSelection(){
  const sel = state.sel[selKey()];
  rowsEl.querySelectorAll('tr[data-id]').forEach(r => {
    const id = Number(r.dataset.id), picked = state.picked.has(id);
    r.classList.toggle('sel', id === sel);
    r.classList.toggle('picked', picked);
    r.setAttribute('aria-selected', String(id === sel || picked));
  });
}
// Change fields of a message wherever the list keeps it.
function patch(id, fields){
  if (typeof fields.unread === 'boolean'){
    const old = list.rows.find(r => r.id === id) ?? (selRow?.id === id ? selRow : null);
    if (old?.thread && old.unread !== fields.unread){
      const group = old.thread.id, delta = fields.unread ? 1 : -1;
      for (const r of list.rows) if (r.thread?.id === group) r.thread.unread = Math.max(0, r.thread.unread + delta);
    }
  }
  for (const r of list.rows) if (r.id === id) Object.assign(r, fields);
  if (selRow?.id === id) Object.assign(selRow, fields);
}
rowsEl.addEventListener('click', e => {
  const tr = e.target.closest('tr[data-id]');
  if (!tr) return;
  const id = Number(tr.dataset.id);
  if (e.target.closest('[data-thread]')){
    toggleGroup(list.rows.find(r => r.id === id)); return;
  }
  // The dot and the flag switch that one message, wherever the selection is.
  const mk = !e.ctrlKey && !e.metaKey && !e.shiftKey && e.target.closest('.mk');
  const m = mk && list.rows.find(r => r.id === id);
  if (m){
    if (mk.classList.contains('rd')) setUnread(m, !m.unread); else setFlagged(m, !m.flagged);
  } else if (e.ctrlKey || e.metaKey) togglePick(id);
  else if (e.shiftKey) pickTo(id);
  else { unpick(); select(id); }
  listEl.focus({preventScroll:true});
});
rowsEl.addEventListener('dblclick', e => {
  const tr = e.target.closest('tr[data-id]');
  if (tr && !e.target.closest('.mk, [data-thread]')) openWindow(Number(tr.dataset.id));
});
// Right-click on a message (or the menu key in the list): what can be done with it, or with
// all the selected ones, each with its key.
function messageMenu(){
  const m = current(), many = state.picked.size > 1;
  if (!m && !many) return [];
  const rows = many ? pickedRows() : [m];
  const unread = many ? rows.length > 0 && !rows.some(r => r.unread) : !m.unread; // what U does
  const flag = many ? !rows.length || rows.some(r => !r.flagged) : !m.flagged;
  const trash = many ? !allMail() && kindOf(state.folder) === 'trash' : kindOf(m.f) === 'trash';
  // Spam or Not spam for received mail of server accounts; Not spam when all of it is in Spam.
  const spamOk = rows.length > 0 && rows.every(r => acctId(r.f) !== LOCAL && !isOut(r.f));
  const inSpam = spamOk && rows.every(r => kindOf(r.f) === 'spam');
  const sort = [
    {label:many ? (unread ? t('Mark as unread|several') : t('Mark as read|several')) : unread ? t('Mark as unread') : t('Mark as read'), key:'U', run:toggleUnread},
    {label:flag ? t('Flag|rule action') : t('Remove the flag'), key:'F', run:toggleFlag},
    {label:t('Tags…'), key:'1–5', run:() => openTagMenu(...menuAt)},
    {label:t('Move to…'), key:'M', run:() => openPicker('move')},
    ...(spamOk ? [inSpam ? {label:t('Not spam'), run:() => markSpam(false)} : {label:t('Mark as spam'), run:() => markSpam(true)}] : []),
    {label:trash ? t('Delete for good') : t('Move to Trash'), key:'Del', run:del},
  ];
  const md = {label:t('Copy as Markdown'), key:'Ctrl+C', run:() => copyMarkdown()};
  const saveAs = {label:t('Save as…'), key:'Ctrl+S', run:() => saveMessages()};
  if (many) return [...sort, {sep:true}, md, saveAs];
  const out = isOut(m.f), edit = ['drafts', 'outbox'].includes(kindOf(m.f));
  const to = String(m.to).match(/[^\s<>,;"]+@[^\s<>,;"]+/)?.[0];
  // The sender (in Sent: the first recipient) into the address book, or their contact there.
  const who = out ? addresses(m.to)[0] : {name:m.from, email:m.email}, known = contactOf(who?.email);
  // Read or unread always comes first, where the pointer lands.
  return [
    sort[0],
    {sep:true},
    {label:t('Open in its own window'), key:'Enter', run:() => openWindow(m.id)},
    ...(edit ? [{label:t('Edit'), key:'E', run:() => openCompose('edit')}] : []),
    {label:t('Reply'), key:'R', run:() => openCompose('reply')},
    {label:t('Reply all'), key:'Shift+R', run:() => openCompose('replyall')},
    {label:t('Forward'), key:'Shift+F', run:() => openCompose('forward')},
    md,
    ...outItems(m.id, false),
    {sep:true},
    ...sort.slice(1),
    {sep:true},
    {label:t('Show the conversation'), key:'T', run:toggleThread},
    out ? {label:t('All mail to {name}', {name:nameOf(m.to)}), disabled:!to, run:() => filterAllMail('to:' + to)}
      : {label:t('All mail from {name}', {name:m.from || m.email}), run:() => filterAllMail('from:' + m.email)},
    ...(who?.email ? [{label:known ? t('Open {name} in the address book', {name:known.name || who.email}) : t('Add {name} to the address book', {name:who.name || who.email}),
      run:() => openAddressBook(who)}] : []),
    {label:out ? t('New rule for mail to {name}', {name:nameOf(m.to)}) : t('New rule for mail from {name}', {name:m.from || m.email}),
      run:() => { const r = ruleFrom(m); openRules().then(() => showRuleForm(null, r)); }},
  ];
}
function openMessageMenu(x, y){
  const items = messageMenu();
  if (items.length) openMenu(x, y, items, listEl);
}
rowsEl.addEventListener('contextmenu', e => {
  const tr = e.target.closest('tr[data-id]');
  if (!tr) return;
  e.preventDefault();
  const id = Number(tr.dataset.id);
  // A message outside the selection becomes the selection; right-clicking doesn't read it.
  if (!state.picked.has(id)){ if (unpick()) afterPick(); if (current()?.id !== id) select(id, true); }
  listEl.focus({preventScroll:true});
  openMessageMenu(e.clientX, e.clientY);
});
listEl.addEventListener('contextmenu', e => e.preventDefault()); // no browser menu between the rows either
// A message in a window of its own, to read side by side or on another screen.
function openWindow(id){
  if (id == null) return;
  invoke('open_window', {id}).catch(e => say(tr(e), 8000));
}

/* ---------- drag and drop ---------- */
// Messages dragged from the list onto a folder in the tree move there, as with M. So nothing
// moves by accident: a drag starts only after the pointer has gone a few pixels; a folder takes
// the messages only after the pointer has rested on it for a moment (it fills up, then shows
// it is ready), and folders that can't take them say no; letting go anywhere else, or Esc,
// does nothing; and Z puts them back. Resting on a closed account or "more" opens it.
const DRAG_START = 6, ARM_MS = 350, SPRING_MS = 650;
let drag = null; // {pointer, x, y, row, started, ids, folders, over, armed, timers, ghost}
function dragTarget(x, y){
  const el = document.elementFromPoint(x, y);
  if (!el?.closest?.('#tree')) return null;
  const f = el.closest('[data-folder]'), more = el.closest('[data-more]'), acct = el.closest('[data-acct]');
  if (f && !isEvery(f.dataset.folder)){
    const key = f.dataset.folder, kind = kindOf(key);
    const ok = !drag.folders.includes(key) && kind !== 'outbox' && targetsFor(drag.folders).includes(acctId(key));
    return {key, ok, el:f};
  }
  if (more && more.getAttribute('aria-expanded') === 'false') return {spring:() => { state.more[more.dataset.more] = true; persist(); renderTree(); }, el:more, key:'more:' + more.dataset.more};
  if (acct && acct.getAttribute('aria-expanded') === 'false') return {spring:() => { delete state.collapsed[acct.dataset.acct]; renderTree(); }, el:acct, key:'acct:' + acct.dataset.acct};
  return null;
}
// What the ghost under the pointer says, and which folder shows what.
function paintDrag(){
  if (!drag?.started) return;
  treeEl.querySelectorAll('.dragover, .dropok, .dropno').forEach(el => el.classList.remove('dragover', 'dropok', 'dropno'));
  const o = drag.over, el = o && !o.spring ? treeEl.querySelector(`[data-folder="${o.key}"]`) : null;
  if (el) el.classList.add(o.ok ? (drag.armed ? 'dropok' : 'dragover') : 'dropno');
  const what = drag.subject || tn(drag.ids.length, '{n} message', '{n} messages', {n:num(drag.ids.length)});
  const hint = !o && invoke ? t('Rest on a folder, then let go. Out of the window: save as a file.')
    : !o || o.spring ? t('Rest on a folder for a moment, then let go.')
    : !o.ok ? (drag.folders.includes(o.key) ? t('It is in this folder already.') : t('Not to another account: put it in a folder on this PC first.'))
    : drag.armed ? t('Let go to move it to {folder}.', {folder:placeName(o.key)}) : t('Hold on {folder}…', {folder:placeName(o.key)});
  drag.ghost.innerHTML = `<b>${esc(what)}</b><span>${esc(hint)}</span>`;
  drag.ghost.classList.toggle('ready', !!(o?.ok && drag.armed));
}
function startDrag(){
  const d = drag, row = list.rows.find(r => r.id === d.row) ?? (selRow?.id === d.row ? selRow : null);
  if (!row) { drag = null; return; }
  // Dragging a message that isn't among those picked takes just that one, selected.
  if (!state.picked.has(row.id)){ if (unpick()) paintSelection(); if (current()?.id !== row.id) select(row.id); }
  d.ids = chosen();
  const rows = state.picked.size > 1 ? pickedRows() : [row];
  d.folders = [...new Set(rows.map(r => r.f))];
  d.subject = d.ids.length === 1 ? row.subj : '';
  d.started = true; d.over = null; d.armed = false; d.timers = [];
  d.ghost = document.createElement('div');
  d.ghost.className = 'dragghost'; d.ghost.setAttribute('aria-hidden', 'true');
  document.body.append(d.ghost);
  win.classList.add('dragging');
  try { listEl.setPointerCapture(d.pointer); } catch {}
  paintDrag();
}
function endDrag(){
  const d = drag; drag = null;
  if (!d?.started) return;
  d.timers.forEach(clearTimeout); cancelAnimationFrame(d.scroll);
  d.ghost.remove();
  win.classList.remove('dragging');
  treeEl.querySelectorAll('.dragover, .dropok, .dropno').forEach(el => el.classList.remove('dragover', 'dropok', 'dropno'));
  try { listEl.releasePointerCapture(d.pointer); } catch {}
  return d;
}
// Near the tree's top or bottom edge, the tree scrolls by itself.
function dragScroll(){
  const d = drag; if (!d?.started) return;
  const r = treeEl.getBoundingClientRect(), edge = 28;
  const step = d.x < r.left || d.x > r.right ? 0 : d.y < r.top + edge ? -6 : d.y > r.bottom - edge ? 6 : 0;
  if (step){ treeEl.scrollTop += step; d.scroll = requestAnimationFrame(dragScroll); } else d.scroll = 0;
}
rowsEl.addEventListener('pointerdown', e => {
  if (e.target.closest('[data-thread]')) return;
  const tr = e.target.closest('tr[data-id]');
  if (!tr || e.button !== 0 || e.pointerType === 'touch') return;
  drag = {pointer:e.pointerId, x0:e.clientX, y0:e.clientY, x:e.clientX, y:e.clientY, row:Number(tr.dataset.id), started:false};
});
listEl.addEventListener('pointermove', e => {
  const d = drag;
  if (!d || e.pointerId !== d.pointer) return;
  // The button went up somewhere this list didn't hear about: no drag.
  if (!(e.buttons & 1)){ endDrag(); drag = null; return; }
  d.x = e.clientX; d.y = e.clientY;
  if (!d.started){
    if (Math.hypot(d.x - d.x0, d.y - d.y0) < DRAG_START) return;
    startDrag();
    if (!drag) return;
  }
  // Past the window's edge the messages leave as .eml files, for Explorer or the desktop.
  if (invoke && (d.x < 0 || d.y < 0 || d.x >= innerWidth || d.y >= innerHeight)){ dragOut(); return; }
  // Above the pointer, so the folders below it stay in sight; below it at the top of the window.
  const gh = d.ghost.offsetHeight, gy = d.y - gh - 10 < 4 ? d.y + 18 : d.y - gh - 10;
  d.ghost.style.transform = `translate(${d.x + 14}px, ${gy}px)`;
  const over = dragTarget(d.x, d.y);
  if ((over?.key ?? null) !== (d.over?.key ?? null)){
    d.timers.forEach(clearTimeout); d.timers = [];
    d.over = over; d.armed = false;
    if (over?.ok) d.timers.push(setTimeout(() => { if (drag === d){ d.armed = true; paintDrag(); } }, ARM_MS));
    if (over?.spring) d.timers.push(setTimeout(() => { if (drag === d){ over.spring(); d.over = null; paintDrag(); } }, SPRING_MS));
    paintDrag();
  }
  if (!d.scroll) dragScroll();
});
listEl.addEventListener('pointerup', e => {
  if (!drag || e.pointerId !== drag.pointer) return;
  const d = endDrag();
  if (!d) return;
  if (d.over?.ok && d.armed){ moveChosen(d.over.key); listEl.focus({preventScroll:true}); }
  else if (d.over?.ok) say(t('Not moved: rest on the folder until it fills, then let go.'), 5000);
});
// The system's own drag takes over: Telex writes the messages as .eml files under the
// temporary folder and hands them on; wherever they are dropped gets copies.
function dragOut(){
  const d = endDrag();
  if (!d) return;
  const n = d.ids.length;
  say(tn(n, 'Drop it in a folder or on the desktop to save it as an .eml file.', 'Drop them in a folder or on the desktop to save them as .eml files.'), 0);
  invoke('drag_out', {ids:d.ids})
    .then(dropped => say(dropped ? tn(n, 'Saved as an .eml file where you dropped it.', '{n} messages saved as .eml files where you dropped them.', {n:num(n)}) : t('Nothing saved.'), 5000))
    .catch(e => say(tr(e), 8000));
}
listEl.addEventListener('pointercancel', () => endDrag());
listEl.addEventListener('lostpointercapture', () => { if (drag?.started) endDrag(); });
// Esc while dragging: nothing moves.
document.addEventListener('keydown', e => {
  if (e.key === 'Escape' && drag?.started){ e.preventDefault(); e.stopPropagation(); endDrag(); say(t('Nothing moved.'), 2500); }
}, true);
document.querySelectorAll('th button[data-sort]').forEach(b => b.addEventListener('click', () => {
  const k = b.dataset.sort;
  state.sort = state.sort.key === k ? {key:k, dir:-state.sort.dir} : {key:k, dir:(k === 'date' || k === 'size' || k === 'flag') ? -1 : 1};
  loadView();
}));
// All mail: go to the selected message in its folder.
function openInFolder(){
  const m = current(); if (!m) return;
  state.sel[m.f] = m.id;
  setFolder(m.f).then(() => listEl.focus({preventScroll:true}));
}

// Open a message by id in its folder, e.g. from Settings › Usage.
function openMessage(id, folder){
  if (state.compose) return;
  state.sel[folder] = id;
  setFolder(folder).then(() => listEl.focus({preventScroll:true}));
}

// Mark as read after the message has been on screen for a moment (Settings › Reading says how
// long), so arrowing past doesn't; or at once; or only when it is opened in its own window, or
// marked by hand.
let readTimer;
function select(id, silent){
  state.sel[selKey()] = id;
  selRow = list.rows.find(r => r.id === id) ?? (selRow?.id === id ? selRow : null);
  clearTimeout(readTimer);
  const m = current(), how = pref().mark_read;
  if (m && m.unread && !silent && (how === 'moment' || how === 'at_once')){
    readTimer = setTimeout(() => {
      call('set_unread', {id:m.id, unread:false}).then(() => { changed(m, {unread:false}); reorder(); }).catch(() => {});
    }, how === 'at_once' ? 0 : pref().read_delay ?? 600);
  }
  paintSelection();
  if (!state.compose) renderPreview();
}
// A message's read or flag state changed here: its row, the counts, the envelope above it.
function changed(m, fields){
  if (fields.unread === false && unreadOnly()) state.kept.add(m.id);
  if ('unread' in fields && fields.unread !== m.unread){
    const c = COUNTS[m.f] ??= {total:0, unread:0};
    c.unread = Math.max(0, c.unread + (fields.unread ? 1 : -1));
  }
  patch(m.id, fields);
  renderTree(); renderStatus(); drawRows();
  if (m.id === current()?.id) paintReadButton();
}
// In the order by status, a changed message moves: once the core has the change, load again.
const reorder = () => { if (state.sort.key === 'flag') loadView(); };
// Arrow keys, Home and End. Steps wait for each other, so a held key never skips a row.
let moving = Promise.resolve();
const move = delta => { moving = moving.then(() => moveBy(delta)).catch(() => {}); };
const pickMove = delta => { moving = moving.then(() => pickBy(delta)).catch(() => {}); };
async function moveBy(delta){
  if (!list.total) return;
  if (unpick()) paintSelection();
  const id = state.sel[selKey()];
  let pos = posOf(id);
  if (pos < 0 && current() && Number.isFinite(delta)){ // scrolled far from the selection: find it again
    const p = await fetchPage({around:id});
    if (p){ merge(p); pos = posOf(id); }
  }
  const to = delta === -Infinity ? 0 : delta === Infinity ? list.total - 1 : pos < 0 ? 0 : Math.max(0, Math.min(list.total - 1, pos + delta));
  const row = await rowAt(to);
  if (!row) return;
  select(row.id);
  if (reveal(to)) drawRows();
}

/* ---------- Space: read on, then the next unread ---------- */
// Space pages down the message on screen; at its end it goes to the next unread message in the
// list, wherever its page is (the core finds it). After the last one, it names the next folder
// with unread mail, and Space again goes there. Shift+Space pages up, then goes to the previous
// unread. Selecting it counts as reading it, as the settings say.
let spaceOffer = null; // {key, at}: the folder a second Space opens
const spaceFolders = () => ACCOUNTS.flatMap(a => a.folders.filter(f => !['spam', 'trash', 'outbox', 'drafts', 'sent'].includes(f.kind)).map(f => a.id + '/' + f.id));
// The next folder after this one, in the tree's order and round again, with unread mail.
function nextUnreadFolder(from){
  const all = spaceFolders(), i = all.indexOf(from);
  return [...all.slice(i + 1), ...all.slice(0, Math.max(0, i))].find(k => countOf(k).unread > 0) ?? null;
}
// Select an unread message the core found, loading its page.
async function goToUnread(u){
  if (grouped()){
    if (u.thread != null) state.expanded.add(u.thread);
    const p = await fetchPage({around:u.id});
    if (!p) return;
    merge(p);
    // Expansion changes the scroll height. Draw it before revealing the hidden message.
    drawRows();
  } else {
    const row = await rowAt(u.row);
    if (row?.id !== u.id){ const p = await fetchPage({around:u.id}); if (p) merge(p); }
  }
  select(u.id);
  const pos = posOf(u.id);
  if (pos >= 0 && reveal(pos)) drawRows(); else drawRows();
}
async function space(back){
  const p = previewEl, step = Math.max(40, p.clientHeight - 48);
  if (current() && state.picked.size <= 1 && !state.compose){
    if (!back && p.scrollTop + p.clientHeight < p.scrollHeight - 4){ p.scrollTop += step; return; }
    if (back && p.scrollTop > 4){ p.scrollTop -= step; return; }
  }
  if (VIEWER || state.compose) return;
  if (unpick()) paintSelection();
  let u;
  try { u = await invoke('next_unread', {view:viewArgs(), after:current()?.id ?? null, back}); } catch (e){ say(tr(e), 8000); return; }
  if (u){ spaceOffer = null; await goToUnread(u); return; }
  if (back){ say(t('No unread message above this one.'), 3000); return; }
  if (allMail() || state.folder === UNREAD){ say(t('No more unread messages here.'), 3000); return; }
  const next = nextUnreadFolder(state.folder);
  if (!next){ say(t('No unread mail in any folder.'), 3000); return; }
  if (spaceOffer?.key !== next || Date.now() - spaceOffer.at > 10e3){
    spaceOffer = {key:next, at:Date.now()};
    say(t('No more unread in {folder}. Space goes to {next} ({n} unread).', {folder:placeName(state.folder), next:`${placeName(next)} · ${acctLabel(acctId(next))}`, n:num(countOf(next).unread)}), 10e3);
    return;
  }
  spaceOffer = null;
  await setFolder(next);
  listEl.focus({preventScroll:true});
  try { u = await invoke('next_unread', {view:viewArgs(), after:null, back:false}); } catch { return; }
  if (u) await goToUnread(u);
}

/* ---------- picking several messages ---------- */
// Ctrl+click adds or takes away one; Shift+click and Shift+Home/End take everything from where
// picking started (the anchor) to there, the core saying which messages lie between; Shift+↑/↓
// grow or shrink that by a row; Ctrl+A takes the whole list. F, U, Del, M and Z work on them all.
function startPicking(){
  const m = current();
  if (!state.picked.size && m) state.picked.add(m.id);
  if (state.anchor == null){ state.anchor = m?.id ?? null; state.span = 0; }
}
function afterPick(){
  paintSelection(); renderStatus();
  if (!state.compose) renderPreview();
}
function togglePick(id){
  startPicking();
  if (state.picked.has(id) && state.picked.size > 1) state.picked.delete(id); else state.picked.add(id);
  state.anchor = id; state.span = 0;
  select(id, true); // picking doesn't mark anything read
  afterPick();
}
async function pickTo(id){
  startPicking();
  const from = state.anchor ?? id;
  let ids;
  try { ids = await invoke('list_ids', {view:viewArgs(), from, to:id}); } catch (e){ say(tr(e), 8000); return; }
  state.picked = new Set(ids);
  state.span = ids[0] === from ? ids.length - 1 : 1 - ids.length;
  select(id, true);
  afterPick();
}
// Shift+↑/↓: away from the anchor the selection grows, towards it it shrinks.
async function pickBy(delta){
  const m = current();
  if (!list.total || !m) return;
  startPicking();
  let pos = posOf(m.id);
  if (pos < 0){ const p = await fetchPage({around:m.id}); if (p){ merge(p); pos = posOf(m.id); } }
  if (pos < 0) return;
  const to = Math.max(0, Math.min(list.total - 1, pos + delta));
  if (to === pos) return;
  const row = await rowAt(to);
  if (!row) return;
  if (state.span === 0 || Math.sign(state.span) === Math.sign(delta)) state.picked.add(row.id);
  else state.picked.delete(m.id);
  state.span += delta;
  select(row.id, true);
  if (reveal(to)) drawRows();
  afterPick();
}
async function pickEnd(last){
  const row = await rowAt(last ? list.total - 1 : 0);
  if (!row) return;
  await pickTo(row.id);
  if (reveal(posOf(row.id))) drawRows();
}
async function pickAll(){
  if (!list.total) return;
  let ids;
  try { ids = await invoke('list_ids', {view:viewArgs(), from:null, to:null}); } catch (e){ say(tr(e), 8000); return; }
  state.picked = new Set(ids); state.anchor = current()?.id ?? ids[0] ?? null; state.span = 0;
  afterPick();
  say(tn(ids.length, '{n} message selected.', '{n} messages selected.', {n:num(ids.length)}), 3000);
}
// The rows of the picked messages that are loaded: enough to decide what F and U do.
const pickedRows = () => list.rows.filter(r => state.picked.has(r.id));
// After the picked messages went away: the first row below them, or above.
function rowAfterPicked(){
  const at = list.rows.map((r, i) => state.picked.has(r.id) ? i : -1).filter(i => i >= 0);
  if (!at.length) return null;
  const below = list.rows.slice(at[at.length - 1] + 1).find(r => !state.picked.has(r.id));
  return below ?? list.rows.slice(0, at[0]).reverse().find(r => !state.picked.has(r.id)) ?? null;
}

/* ---------- preview ---------- */
// Quoted mail in plain text reads as blocks: each quoted message in the text's own color with a
// bar for its depth, the line that introduced it ("Dnia … napisał(a):", "On … wrote:") on top,
// and an old message after "-----Original Message-----" as one block too. A long block shows its
// first lines and folds the rest; Q opens or folds every quote of the message.
const QUOTE_FOLD = 8, QUOTE_PEEK = 3;
const quotesOpen = new Set(); // messages whose quotes Q opened
const HISTORY_LINE = /^\s*-{2,}\s*(original message|forwarded message|wiadomość oryginalna|oryginalna wiadomość|przekazana wiadomość|wiadomość przekazana)\s*-{2,}\s*$/i;
// The lines as a tree of blocks: {head, items: [line text | block], n: lines in all}.
function quoteTree(body){
  let history = 0;
  const rows = body.split('\n').map(line => {
    if (!history && HISTORY_LINE.test(line)){ history = 1; return {depth:1, text:line.trim(), head:true}; }
    const m = line.match(/^((?:>[ \t]?)+)/);
    return {depth:(m ? m[1].split('>').length - 1 : 0) + history, text:m ? line.slice(m[1].length) : line};
  });
  const blank = r => !r.head && !r.text.trim();
  // A blank line inside a quote belongs to it.
  rows.forEach((r, i) => {
    if (!blank(r)) return;
    const prev = rows.slice(0, i).findLast(x => !blank(x)), next = rows.slice(i + 1).find(x => !blank(x));
    r.depth = Math.min(prev?.depth ?? 0, next?.depth ?? 0);
  });
  // The line that introduces a quote heads it (blank lines between them go in with it).
  rows.forEach((r, i) => {
    if (r.head || !r.text.trim().endsWith(':')) return;
    const j = rows.findIndex((x, k) => k > i && !blank(x));
    if (j < 0 || rows[j].depth <= r.depth) return;
    for (let k = i + 1; k < j; k++) rows[k].depth = rows[j].depth;
    Object.assign(r, {head:true, depth:rows[j].depth});
  });
  const root = {head:null, items:[], n:0}, stack = [root];
  for (const r of rows){
    // A head opens a block of its own, even right after another at the same depth.
    while (stack.length - 1 > r.depth - (r.head ? 1 : 0)) stack.pop();
    while (stack.length - 1 < r.depth - (r.head ? 1 : 0)){ const b = {head:null, items:[], n:0}; stack.at(-1).items.push(b); stack.push(b); }
    if (r.head){ const b = {head:r.text, items:[], n:0}; stack.at(-1).items.push(b); stack.push(b); }
    else stack.at(-1).items.push(r.text);
  }
  const count = b => (b.n = b.items.reduce((n, x) => n + (typeof x === 'string' ? 1 : count(x)), 0));
  count(root);
  return root;
}
function bodyHtml(m){
  const loaded = state.images.has(m.id), open = quotesOpen.has(m.id);
  const line = text => {
    const img = text.match(/^\[img:(.+)\]$/);
    if (img) return `<div class="imgph${loaded ? ' loaded' : ''}">${loaded ? esc(img[1]) : t('Blocked image: {name}', {name:esc(img[1])})}</div>`;
    return `<div class="ln">${linked(text)}</div>`;
  };
  const block = (b, depth) => {
    const inner = b.items.map(x => typeof x === 'string' ? line(x) : block(x, depth + 1)).join('');
    if (!depth) return inner;
    const fold = !open && b.n > QUOTE_FOLD;
    return `<div class="qb q${(depth - 1) % 4 + 1}${fold ? ' folded' : ''}">${b.head ? `<div class="qh">${linked(b.head)}</div>` : ''}`
      + `<div class="qin">${inner}</div>`
      + (fold ? `<button type="button" class="qmore" data-act="quote">${esc(tn(b.n - QUOTE_PEEK, 'Show {n} more line', 'Show {n} more lines'))}</button>` : '') + '</div>';
  };
  return block(quoteTree(m.body), 0);
}
// Q: every quote of the open message, opened or folded again.
function toggleQuotes(){
  const s = current();
  if (!s || state.compose) return;
  const open = !quotesOpen.has(s.id);
  if (open) quotesOpen.add(s.id); else quotesOpen.delete(s.id);
  const body = previewEl.querySelector('.pbody:not(.html)');
  if (body) full(s.id).then(m => { if (current()?.id !== m.id) return; body.innerHTML = bodyHtml(m); markLinks(body, kindOf(s.f) === 'spam'); }, () => {});
  const d = previewEl.querySelector('.hframe')?.contentDocument;
  d?.querySelectorAll('details.tq').forEach(x => { x.open = open; });
  const any = !!previewEl.querySelector('.qb') || !!d?.querySelector('details.tq');
  say(!any ? t('This message quotes nothing.') : open ? t('Quotes open. Q folds them again.') : t('Quotes folded. Q opens them.'), 2500);
}
// HTML mail: quoted mail (Gmail's and Thunderbird's quote blocks, Apple's cite, everything
// from Outlook's reply divider on) folds behind one line when it is long: the frame has no
// scripts, so <details> opens and closes it.
function foldFrameQuotes(d, id){
  const roots = [];
  const outer = el => !roots.some(r => r.contains(el));
  // Outlook: the divider and all that follows it in its parent.
  const divider = d.getElementById('divRplyFwdMsg');
  if (divider){
    const box = d.createElement('div');
    const hr = divider.previousElementSibling?.tagName === 'HR' ? divider.previousElementSibling : null;
    (hr ?? divider).before(box);
    for (let n = hr ?? divider; n;){ const next = n.nextSibling; box.append(n); n = next; }
    roots.push(box);
  }
  d.querySelectorAll('div.gmail_quote, blockquote').forEach(el => { if (outer(el) && !el.parentElement.closest('div.gmail_quote, blockquote')) roots.push(el); });
  const lines = el => el.innerText.split('\n').filter(l => l.trim()).length;
  let folded = 0;
  for (const el of roots){
    const n = lines(el);
    if (n <= QUOTE_FOLD) continue;
    // Its introduction ("On … wrote:"): Gmail's inside it, Thunderbird's or a short line before it.
    const prev = el.previousElementSibling;
    const intro = el.querySelector(':scope > .gmail_attr')
      ?? (prev && (prev.classList.contains('moz-cite-prefix') || (prev.innerText.trim().endsWith(':') && prev.innerText.length < 300)) ? prev : null);
    const head = intro?.innerText.trim().replace(/\s+/g, ' ') || t('Quoted text');
    const box = d.createElement('details');
    box.className = 'tq'; box.open = quotesOpen.has(id);
    const sum = d.createElement('summary');
    sum.textContent = `${head} · ${tn(n, '{n} line', '{n} lines')}`;
    el.before(box); box.append(sum, el);
    if (intro && intro !== el.querySelector(':scope > .gmail_attr')) intro.remove();
    folded++;
  }
  if (!folded) return;
  const style = d.createElement('style');
  style.textContent = 'details.tq{margin:8px 0}details.tq>summary{cursor:pointer;font-weight:600;padding:3px 0;opacity:.85}'
    + 'details.tq>summary:hover{opacity:1}details.tq[open]>summary{margin-bottom:4px}';
  d.head.append(style);
}
// Plain text with its web addresses as links. An address ends before a space, <, > or a
// quote, and without the full stop or bracket that ends the sentence around it.
function linked(text){
  let out = '', at = 0;
  for (const found of text.matchAll(/https?:\/\/[^\s<>"']+/gi)){
    let url = found[0];
    for (;;){
      const last = url.at(-1);
      if (last === ')' && url.split('(').length >= url.split(')').length) break; // its own: wiki/Foo_(bar)
      if (!'.,;:!?)]}'.includes(last)) break;
      url = url.slice(0, -1);
    }
    out += esc(text.slice(at, found.index)) + `<a href="${esc(url)}">${esc(url)}</a>`;
    at = found.index + url.length;
  }
  return out + esc(text.slice(at));
}
// The HTML view of a message, cached per images setting and paper. load: true loads remote
// images, false never does (only asks whether there is HTML), null lets the core decide
// (trusted sender, not spam). dark: the theme's colors for dark paper, or null for white.
// The core does any downloading; the window never talks to the network.
const htmlViews = new Map();
async function htmlView(id, load, dark = null){
  const key = id + ':' + load + ':' + (dark ? Object.values(dark).join() : 'white');
  // A few only: with its images loaded a view can be megabytes.
  return htmlViews.has(key) ? htmlViews.get(key) : keep(htmlViews, key, await invoke('get_html', {id, loadImages:load, dark}), 8);
}
// Links never navigate inside Telex: web links open in the browser, mailto opens compose.
function followLink(href, spam){
  const info = window.TelexLinks.inspect(href);
  if (!info) return;
  const h = info.href;
  if (spam){ say(t('Links are off in Spam. Move the message out of Spam to use them.')); return; }
  if (/^mailto:/i.test(h)){
    const raw = h.slice(7).split('?')[0];
    let to = raw; try { to = decodeURIComponent(raw); } catch {} // a stray % is taken as it is
    openCompose('new', {to});
  } else if (/^https?:\/\//i.test(h)){
    call('open_link', {url:h}).then(() => say(t('Opened in your browser.'), 2500)).catch(() => {});
  }
}
// The address under the pointer, like a browser's status bubble: one line at the foot of the
// message pane, outside the sender's document, never covering the message. Nothing is fetched.
const linkTip = document.createElement('div');
linkTip.className = 'linktip'; linkTip.hidden = true; linkTip.setAttribute('role', 'status');
document.body.append(linkTip);
let tipFor = null;
function markLinks(root, spam){
  root.querySelectorAll('a[href]').forEach(a => {
    a.removeAttribute('title'); // the sender's tooltip could name another address
    if (spam) a.setAttribute('aria-disabled', 'true');
  });
}
function showLink(a, spam){
  const info = window.TelexLinks.inspect(a.getAttribute('href'));
  if (!info){ hideLink(); return; }
  tipFor = a;
  const warn = info.credentials ? t('Disguised link, it goes to')
    : info.hidden ? t('Hidden characters in the address.') : '';
  linkTip.classList.toggle('warn', !!warn);
  linkTip.innerHTML = (spam ? `<span class="lt-off">${esc(t('Off in Spam'))}</span>` : '')
    + (warn ? `<span class="lt-warn">${esc(warn)}</span>` : '')
    + `<bdi class="lt-host">${esc(info.destination)}</bdi>`
    + (info.web ? `<bdi class="lt-url">${esc(info.display)}</bdi>` : '');
  const pane = previewEl.getBoundingClientRect(), r = a.getBoundingClientRect();
  const frame = a.ownerDocument.defaultView?.frameElement;
  const bottom = r.bottom + (frame ? frame.getBoundingClientRect().top : 0);
  const left = r.left + (frame ? frame.getBoundingClientRect().left : 0);
  linkTip.style.maxWidth = Math.max(160, pane.width * 0.66) + 'px';
  linkTip.style.bottom = Math.max(0, innerHeight - pane.bottom) + 'px';
  linkTip.hidden = false;
  // Like a browser: bottom left, unless the link itself sits there.
  const right = bottom > pane.bottom - 40 && left < pane.left + linkTip.offsetWidth;
  linkTip.style.left = right ? 'auto' : pane.left + 'px';
  linkTip.style.right = right ? Math.max(0, innerWidth - pane.right) + 'px' : 'auto';
}
function hideLink(a = null){
  if (a && a !== tipFor) return;
  tipFor = null; linkTip.hidden = true;
}
previewEl.addEventListener('scroll', () => hideLink(), {passive:true});
// Text onto the clipboard; false when neither way worked.
async function copyText(text){
  try { await navigator.clipboard.writeText(text); return true; }
  catch {
    // WebView clipboard support differs between platforms. Keep a native copy fallback.
    const from = document.activeElement, field = document.createElement('textarea');
    field.value = text; field.readOnly = true; field.className = 'copyfield';
    document.body.append(field); field.select();
    let copied = false;
    try { copied = document.execCommand('copy'); } catch {}
    field.remove(); from?.focus?.({preventScroll:true});
    return copied;
  }
}
// Copy as Markdown (Ctrl+C, the message menu, the button above a message): the selected
// messages, or the open one, oldest first, to paste into notes or an AI chat. Reads nothing.
const MD_MAX = 200;
async function copyMarkdown(ids = state.picked.size > 1 ? [...state.picked] : current() ? [current().id] : []){
  if (!ids.length) return;
  if (ids.length > MD_MAX){ say(t('Copy up to {n} messages at a time.', {n:num(MD_MAX)}), 4000); return; }
  let md;
  try { md = await call('message_markdown', {ids}); } catch { return; }
  if (await copyText(md)) say(tn(ids.length, 'Copied as Markdown.', '{n} messages copied as Markdown.', {n:num(ids.length)}), 2500);
  else say(t('Couldn’t copy to the clipboard.'), 4000);
}

/* ---------- a message out of Telex ---------- */
// Save as… (Ctrl+S): the selected messages as .eml files, byte for byte as they came; one
// opens Save, several a folder for them all. Print… (Ctrl+P) and Save as PDF…: the message as
// it shows (HTML or text) under its header. View source (Ctrl+U) and All headers: as it came.
// `full`: the … button above a message has All headers too; the message menu is long enough.
function outItems(id, full = true){
  return [
    {label:t('Save as…'), key:'Ctrl+S', run:() => saveMessages([id])},
    {label:t('Print…'), key:'Ctrl+P', run:() => printMessage(id)},
    {label:t('Save as PDF…'), run:() => savePdf(id)},
    ...(full ? [{sep:true}] : []),
    {label:t('View source'), key:'Ctrl+U', run:() => showSource(id)},
    ...(full ? [{label:t('All headers'), run:() => showSource(id, 'headers')}] : []),
  ];
}
async function saveMessages(ids = chosen()){
  if (!ids.length) return;
  const words = {title:ids.length > 1 ? t('Save {n} messages as .eml files in', {n:num(ids.length)}) : t('Save the message as'), filter:t('Mail messages')};
  let path;
  try { path = await invoke('save_messages', {ids, words}); } catch (e){ say(tr(e), 8000); return; }
  if (path) say(ids.length > 1 ? t('Saved {n} messages in {path}.', {n:num(ids.length), path}) : t('Saved to {path}.', {path}), 6000);
}
// What goes on paper waits in #printout, which only print shows; the core's document goes into
// a shadow root there, so the mail's markup and styles stay apart from the window's.
const printEl = document.createElement('div');
printEl.id = 'printout'; printEl.setAttribute('aria-hidden', 'true');
document.body.append(printEl);
let printTitle = null;
async function preparePrint(id){
  const doc = await call('print_document', {id, html:!pref().prefer_text, loadImages:state.images.has(id) ? true : null});
  const page = new DOMParser().parseFromString(doc, 'text/html');
  const root = printEl.shadowRoot ?? printEl.attachShadow({mode:'open'});
  root.replaceChildren(...page.head.querySelectorAll('style'), ...page.body.childNodes);
  await Promise.all([...root.querySelectorAll('img')].map(img => img.decode().catch(() => {})));
  // The document's title names the PDF and the print job.
  if (printTitle == null) printTitle = document.title;
  document.title = page.title || 'Telex';
  document.body.classList.add('printing');
}
function endPrint(){
  document.body.classList.remove('printing');
  printEl.shadowRoot?.replaceChildren();
  if (printTitle != null){ document.title = printTitle; printTitle = null; }
}
window.addEventListener('afterprint', endPrint);
async function printMessage(id = current()?.id){
  if (id == null) return;
  try { await preparePrint(id); } catch { endPrint(); return; }
  window.print();
}
async function savePdf(id = current()?.id){
  if (id == null) return;
  try { await preparePrint(id); } catch { endPrint(); return; }
  let done;
  try { done = await invoke('save_pdf', {id, words:{title:t('Save as PDF'), filter:t('PDF documents')}}); }
  catch (e){ endPrint(); say(tr(e), 8000); return; }
  // No PDF of its own here (Linux): the print dialog's printers include one that writes a file.
  if (done === 'print_dialog'){ say(t('Choose “Print to File” (PDF) as the printer.'), 8000); window.print(); return; }
  endPrint();
  if (done?.saved) say(t('Saved to {path}.', {path:done.saved}), 6000);
}
// View source and All headers: the message as it came, to read and copy; nothing in it runs.
const srcDlg = $('src-dlg');
let srcOf = null; // {id, source, tab}
async function showSource(id = current()?.id, tab = 'source'){
  if (id == null) return;
  let source;
  try { source = await invoke('message_source', {id}); } catch (e){ say(tr(e), 8000); return; }
  srcOf = {id, source, tab};
  $('src-text').textContent = source.text;
  $('src-headers').innerHTML = source.headers.map(h => `<dt>${esc(h.name)}</dt><dd>${esc(h.value)}</dd>`).join('');
  $('src-note').textContent = source.cut ? t('A long message: the first megabyte is shown. Save as… keeps all of it.') : '';
  srcTab(tab);
  srcDlg.showModal();
  (tab === 'source' ? $('src-text') : $('src-headers')).focus();
}
function srcTab(tab){
  srcOf.tab = tab;
  srcDlg.querySelectorAll('[data-src]').forEach(b => b.setAttribute('aria-pressed', String(b.dataset.src === tab)));
  $('src-text').hidden = tab !== 'source';
  $('src-headers').hidden = tab !== 'headers';
  $('src-title').textContent = tab === 'source' ? t('Message source') : t('All headers');
}
srcDlg.addEventListener('click', async e => {
  const b = e.target.closest('button');
  if (!b || !srcOf) return;
  if (b.dataset.src){ srcTab(b.dataset.src); return; }
  if (b.id === 'src-close') srcDlg.close();
  if (b.id === 'src-save') saveMessages([srcOf.id]);
  if (b.id === 'src-copy'){
    const text = srcOf.tab === 'source' ? srcOf.source.text : srcOf.source.headers.map(h => `${h.name}: ${h.value}`).join('\n');
    say(await copyText(text) ? (srcOf.tab === 'source' ? t('Source copied.') : t('Headers copied.')) : t('Couldn’t copy to the clipboard.'), 3000);
  }
});
srcDlg.addEventListener('close', () => { srcOf = null; if (!VIEWER) listEl.focus({preventScroll:true}); });
async function copyLink(info){
  if (!info) return;
  if (await copyText(info.href)) say(t('Link address copied.'), 2500);
  else say(t('Couldn’t copy the link address.'), 4000);
}
function linkMenu(e, a, spam, frame = null){
  const info = window.TelexLinks.inspect(a.getAttribute('href'));
  if (!info) return;
  e.preventDefault(); showLink(a, spam);
  const offset = frame?.getBoundingClientRect() ?? {left:0, top:0};
  const r = a.getBoundingClientRect();
  const x = offset.left + (e.clientX || r.left), y = offset.top + (e.clientY || r.bottom);
  openMenu(x, y, [
    info.web ? {label:t('Open in the browser'), disabled:spam, run:() => followLink(info.href, spam)}
      : {label:t('Write to {email}', {email:info.destination}), disabled:spam, run:() => followLink(info.href, spam)},
    {label:info.web ? t('Copy link address') : t('Copy email address'), run:() => copyLink(info.web ? info : {href:info.destination})},
  ], a);
}
function bindPreviewLinks(root, spam, frame = null){
  const link = e => e.target.closest?.('a[href]');
  root.addEventListener('pointerover', e => { const a = link(e); if (a) showLink(a, spam); });
  root.addEventListener('pointerout', e => { const a = link(e); if (a && !a.contains(e.relatedTarget)) hideLink(a); });
  root.addEventListener('focusin', e => { const a = link(e); if (a) showLink(a, spam); });
  root.addEventListener('focusout', e => { const a = link(e); if (a) hideLink(a); });
  root.addEventListener('contextmenu', e => { const a = link(e); if (a) linkMenu(e, a, spam, frame); });
  root.addEventListener('keydown', e => {
    const a = link(e);
    if (a && (e.key === 'ContextMenu' || (e.key === 'F10' && e.shiftKey))){ e.stopPropagation(); linkMenu(e, a, spam, frame); }
  });
  // Middle-click must not bypass the app's link policy or open a tab in the mail frame.
  root.addEventListener('auxclick', e => { if (link(e)) e.preventDefault(); });
  root.addEventListener('pointerdown', e => {
    if (e.button === 1 && link(e)) e.preventDefault();
    if (!menuEl.hidden) closeMenu(false);
  });
}
// The frame shows the core's inert document: no scripts may run in it (sandbox without
// allow-scripts). allow-same-origin only lets this page size it and catch link clicks.
function mountFrame(frame, documentHtml, spam, id){
  frame.addEventListener('load', () => {
    const d = frame.contentDocument;
    if (!d || !frame.isConnected) return;
    foldFrameQuotes(d, id);
    markLinks(d, spam);
    bindPreviewLinks(d, spam, frame);
    const fit = () => { frame.style.height = d.documentElement.scrollHeight + 'px'; };
    fit();
    new ResizeObserver(fit).observe(d.documentElement);
    d.querySelectorAll('img').forEach(img => img.addEventListener('load', fit));
    d.addEventListener('click', e => {
      const a = e.target.closest?.('a[href]');
      if (!a) return;
      e.preventDefault();
      followLink(a.getAttribute('href'), spam);
    });
    d.addEventListener('keydown', onKey); // keys still work after clicking into the message
  }, {once:true});
  frame.srcdoc = documentHtml;
}
function imagesBar(m, hv, spam){
  const who = esc(hv.sender);
  if (hv.blocked && !hv.loaded){
    const n = esc(tn(hv.blocked, 'A remote image is blocked so the sender can’t see when you open this.', '{n} remote images are blocked so the sender can’t see when you open this.'));
    const always = spam ? '' : ` <button type="button" class="linkbtn" data-act="trust">${t('Always load from {who}', {who})}</button>`;
    return `<div class="bar">${n} <button type="button" class="linkbtn" data-act="images">${esc(t('Load images'))}</button>${always}</div>`;
  }
  if (hv.loaded && hv.trusted && !spam){
    return `<div class="bar quiet">${t('Images from {who} load automatically.', {who})} <button type="button" class="linkbtn" data-act="untrust">${esc(t('Stop'))}</button></div>`;
  }
  if (hv.loaded && hv.blocked) return `<div class="bar quiet">${esc(tn(hv.blocked, 'One image couldn’t be loaded.', '{n} images couldn’t be loaded.'))}</div>`;
  return '';
}
// A message's attachments: click opens one in its app, the arrow saves it. In compose
// (`writing`), × takes one off, and the total says when it gets large.
const BIG_KB = 20 * 1024; // above this, some servers refuse a message
function attsHtml(atts, writing = false){
  if (!atts.length) return '';
  const chips = atts.map((a, i) => writing
    ? `<span class="att" title="${esc(a.name)}">${I.clip}<span class="nm">${esc(a.name)}</span><small>${fmtSize(a.size_kb)}</small><button type="button" class="attx" data-unattach="${i}" title="${esc(t('Remove {name}', {name:a.name}))}" aria-label="${esc(t('Remove {name}', {name:a.name}))}">×</button></span>`
    : `<span class="att"><button type="button" class="attopen" data-att="${i}" title="${esc(t('Open {name}', {name:a.name}))}">${I.clip}<span class="nm">${esc(a.name)}</span><small>${fmtSize(a.size_kb)}</small></button><button type="button" class="attx" data-save="${i}" title="${esc(t('Save {name}…', {name:a.name}))}" aria-label="${esc(t('Save {name}…', {name:a.name}))}">${I.get}</button></span>`).join('');
  const kb = atts.reduce((n, a) => n + a.size_kb, 0);
  const tail = writing
    ? (kb > BIG_KB ? `<small class="attwarn">${esc(t('{size} in all: some servers refuse mail over 25 MB.', {size:fmtSize(kb)}))}</small>` : '')
    : atts.length > 1 ? `<button type="button" class="linkbtn" data-saveall>${esc(t('Save all…'))}</button>` : '';
  return `<div class="atts">${chips}${tail}</div>`;
}
// Open an attachment in its app. Programs and anything from Spam open on a second click.
let armedAtt = null; // {id, index, at}
async function openAttachment(id, index){
  const m = await full(id), a = m.attachments[index];
  if (!a) return;
  const sure = armedAtt?.id === id && armedAtt.index === index && Date.now() - armedAtt.at < 8000;
  let r;
  try { r = await invoke('open_attachment', {id, index, sure}); } catch (e){ say(tr(e), 8000); return; }
  armedAtt = null;
  if (r === 'opened'){ say(t('Opened {name}.', {name:a.name}), 3000); return; }
  armedAtt = {id, index, at:Date.now()};
  say(r === 'risky' ? t('“{name}” can run programs on this PC. Open it only if you trust the sender: click it again to open it.', {name:a.name})
    : t('This came in Spam. Click the attachment again to open it anyway.'), 8000);
}
async function saveAttachment(id, index){
  let path;
  try { path = await invoke('save_attachment', {id, index, words:{title:t('Save attachment')}}); } catch (e){ say(tr(e), 8000); return; }
  if (path) say(t('Saved to {path}.', {path}), 6000);
}
async function saveAttachments(id){
  const n = (await full(id)).attachments.length;
  let dir;
  try { dir = await invoke('save_attachments', {id, words:{title:t('Save attachments to')}}); } catch (e){ say(tr(e), 8000); return; }
  if (dir) say(tn(n, 'Saved {n} file to {dir}.', 'Saved {n} files to {dir}.', {dir}), 6000);
}
// An address line ("Anna Kowalska <anna@x.pl>, "Nowak, Jan" <jan@y.pl>") as people: commas inside quotes don't split.
function addresses(line){
  const parts = [];
  let cur = '', quoted = false;
  for (const ch of String(line || '')){
    if (ch === '"') quoted = !quoted;
    if (!quoted && (ch === ',' || ch === ';')){ parts.push(cur); cur = ''; } else cur += ch;
  }
  parts.push(cur);
  return parts.map(s => s.trim()).filter(Boolean).map(s => {
    const m = s.match(/^(.*?)\s*<([^<>]+)>$/);
    return m ? {name:m[1].replace(/^"(.*)"$/, '$1').trim(), email:m[2].trim()} : {name:'', email:s};
  });
}
// A person: the name, then the address in a quieter color; the address alone when there is no name.
const person = (name, email) => name && name !== email ? `<span class="pn">${esc(name)}</span> <span class="pa">${esc(email)}</span>` : `<span class="pn">${esc(email || name)}</span>`;
// `rename` gives the address book's name for an address, when it has one.
const people = (line, rename) => addresses(line).map(p => `<span class="pp">${person(rename ? rename(p.name, p.email) : p.name, p.email)}</span>`).join(', ');
// The message's header: the subject with its buttons (`controls`), who it is from (a click shows all
// their mail; the person button adds them to the address book or opens their contact), when and
// how large, who it went to, and its attachments. People in the address book go by its names.
function headerHtml(m, controls){
  const saved = new Map((m.contacts ?? []).map(c => [c.email.toLowerCase(), c.name]));
  const named = (name, email) => saved.get(String(email).toLowerCase()) || name;
  const from = named(m.from_name, m.from_email);
  const sender = m.from_email && !VIEWER
    ? `<button type="button" class="ph-who" data-act="sender" title="${esc(t('Show all mail from {email}', {email:m.from_email}))}">${person(from, m.from_email)}</button>`
    : `<span class="ph-who">${person(from, m.from_email)}</span>`;
  const known = saved.has(String(m.from_email).toLowerCase()), bookTip = known ? t('In your address book: open the contact') : t('Add to address book');
  const book = m.from_email && !VIEWER && !isOut(m.folder)
    ? `<button type="button" class="pbtn ph-ab${known ? ' known' : ''}" data-act="contact" title="${esc(bookTip)}" aria-label="${esc(bookTip)}">${known ? I.person : I.personAdd}</button>` : '';
  return `<header class="ph">
  <div class="ph-top"><h2 class="ph-subj">${esc(m.subject)}</h2>${controls}</div>
  <div class="ph-tags">${tagChips(m.tags)}</div>
  <dl class="ph-grid">
    <dt>${esc(t('From'))}</dt><dd class="ph-from">${sender}${book}<span class="ph-when" title="${esc(t('Date'))}">${fmtFull(parseDate(m.date))} · ${fmtSize(m.size_kb)}</span></dd>
    <dt>${esc(t('To'))}</dt><dd>${m.to ? people(m.to, named) : esc(t('(no recipient)'))}</dd>
    ${m.cc ? `<dt>${esc(t('Cc'))}</dt><dd>${people(m.cc, named)}</dd>` : ''}
    ${m.bcc ? `<dt>${esc(t('Bcc'))}</dt><dd>${people(m.bcc, named)}</dd>` : ''}
  </dl>
  ${attsHtml(m.attachments)}
</header>`;
}
// The envelope above a message marks it read or unread, like U; it shows what a click does.
const readTip = m => m.unread ? t('Mark as read (U)') : t('Mark as unread (U)');
const readButton = m => `<button type="button" class="pbtn" data-act="read" title="${esc(readTip(m))}" aria-label="${esc(readTip(m))}">${m.unread ? I.mailOpen : I.mail}</button>`;
// The flag next to it, like F: a switch that shows the flag's color while the message has one.
const flagTip = m => m.flagged ? t('Remove the flag (F)') : t('Flag (F)');
const flagButton = m => `<button type="button" class="pbtn pflag" data-act="flag" aria-pressed="${!!m.flagged}" title="${esc(flagTip(m))}" aria-label="${esc(flagTip(m))}">${I.flag}</button>`;
// Tags, like 1–5: the menu of every tag, ticked where the message has it.
const tagButton = () => `<button type="button" class="pbtn" data-act="tags" aria-haspopup="menu" title="${esc(t('Tags (1–5)'))}" aria-label="${esc(t('Tags (1–5)'))}">${I.tag}</button>`;
// Copy as Markdown, like Ctrl+C: the message ready to paste into notes or an AI chat.
const mdButton = () => `<button type="button" class="pbtn" data-act="markdown" title="${esc(t('Copy as Markdown (Ctrl+C)'))}" aria-label="${esc(t('Copy as Markdown (Ctrl+C)'))}">${I.copy}</button>`;
// Save as, print, save as PDF, the source and all headers, in a menu.
const outButton = () => `<button type="button" class="pbtn" data-act="out" aria-haspopup="menu" title="${esc(t('Save, print, source'))}" aria-label="${esc(t('Save, print, source'))}">${I.dots}</button>`;
function paintReadButton(){
  const b = previewEl.querySelector('[data-act="read"]'), m = current();
  if (!b || !m) return;
  b.innerHTML = m.unread ? I.mailOpen : I.mail;
  b.title = readTip(m); b.setAttribute('aria-label', b.title);
}
/* ---------- send later, in the open message ---------- */
// Mail sent later says when it goes, with Send now and Hold; E changes it.
function goesBar(m){
  if (!m.goes || kindOf(m.folder) !== 'outbox') return '';
  return `<div class="bar quiet goesbar"><span>${esc(t('Goes {when}.', {when:fmtFull(parseDate(m.goes))}))}</span>`
    + `<button type="button" class="linkbtn" data-act="send-now">${esc(t('Send now'))}</button>`
    + `<button type="button" class="linkbtn" data-act="hold">${esc(t('Hold'))}</button>`
    + `<span>${esc(t('E changes it.'))}</span></div>`;
}
async function sendNow(m){
  say(t('Sending…'), 0);
  let r;
  try { r = await call('send_now', {id:m.id}); } catch { return; }
  bodies.delete(m.id);
  await refresh();
  const sent = sentText(r.sent);
  if (r.errors.length) say([sent, r.errors.join(' · ')].filter(Boolean).join(' '), 15000);
  else say(sent || t('In the Outbox. It goes out with the next check.'), 6000);
}
async function holdLater(m){
  try { await invoke('hold_message', {id:m.id}); } catch (e){ say(tr(e), 8000); return; }
  bodies.delete(m.id);
  await refresh();
  if (!state.compose) renderPreview();
  say(t('Held. It stays in the Outbox: press E to change it and send it.'), 6000);
}

/* ---------- unsubscribe ---------- */
// Mail from a list offers a quiet Unsubscribe above it: one click where the list takes it (the
// core posts the request), else a mail to the list or its page in the browser, each after a
// line that says where it goes. Never in Spam. Once done, every message of the list says so,
// and the sender's mail can go to Trash in one go (Z brings it back).
let unsubAsk = null;   // the message whose confirmation line is open
let unsubOffer = null; // {id, email, n}: after unsubscribing, the sender's mail to Trash
function unsubButton(m){
  const u = m.unsubscribe;
  if (!u) return '';
  if (u.spam){
    const why = t('Not offered in Spam: unsubscribing would tell the sender your address is read. Delete the message instead.');
    return `<button type="button" class="pbtn ptext" aria-disabled="true" title="${esc(why)}" aria-label="${esc(t('Unsubscribe') + '. ' + why)}">${esc(t('Unsubscribe'))}</button>`;
  }
  if (u.done){
    const when = dmy(parseDate(u.done)), tip = t('You left this list on {date}. Click to ask again.', {date:when});
    return `<button type="button" class="pbtn ptext" data-act="unsub" title="${esc(tip)}">${esc(t('Unsubscribed {date}', {date:dm(parseDate(u.done))}))}</button>`;
  }
  const tip = {one_click:t('Leave this mailing list with one click: Telex asks {target} to take you off.', u),
    mail:t('Leave this mailing list: Telex mails {target} from your account.', u),
    link:t('Leave this mailing list on its page at {target}, in your browser.', u)}[u.method];
  return `<button type="button" class="pbtn ptext" data-act="unsub" title="${esc(tip)}">${esc(t('Unsubscribe'))}</button>`;
}
function unsubBar(m){
  if (unsubAsk === m.id && m.unsubscribe){
    const u = m.unsubscribe;
    const what = u.method === 'mail' ? t('Telex sends an unsubscribe mail to {target} from your account.', u)
      : t('The list’s page at {target} opens in your browser, where you finish unsubscribing.', u);
    return `<div class="bar quiet unsubbar" role="alert"><span>${esc(what)}</span>`
      + `<button type="button" class="btn primary" data-act="unsub-go">${esc(u.method === 'mail' ? t('Send it') : t('Open the page'))}</button>`
      + `<button type="button" class="btn" data-act="unsub-no">${esc(t('Cancel'))}</button></div>`;
  }
  if (unsubOffer?.id === m.id && unsubOffer.n){
    const o = unsubOffer;
    return `<div class="bar quiet unsubbar"><span>${esc(tn(o.n, 'Move the {n} message from {email} to Trash?', 'Move the {n} messages from {email} to Trash?', {n:num(o.n), email:o.email}))}</span>`
      + `<button type="button" class="btn" data-act="unsub-trash">${esc(t('Move to Trash'))}</button>`
      + `<button type="button" class="linkbtn" data-act="unsub-keep">${esc(t('Keep them'))}</button></div>`;
  }
  return '';
}
async function startUnsubscribe(m){
  const u = m.unsubscribe;
  if (!u || u.spam) return;
  unsubOffer = null;
  if (u.method === 'one_click' || unsubAsk === m.id){ unsubAsk = null; unsubscribeNow(m); return; }
  unsubAsk = m.id; renderPreview();
}
// The sender's mail in every folder but Spam and Trash: the list the trash offer works on.
const senderView = email => ({folder:null, sort:'date', ascending:false, filter:'from:' + email, bodies:false, thread:null, unread:false, keep:[]});
async function unsubscribeNow(m){
  const u = m.unsubscribe;
  say(u.method === 'mail' ? t('Sending…') : t('Unsubscribing…'), 0);
  let r;
  try { r = await call('unsubscribe', {id:m.id}); } catch { renderPreview(); return; }
  bodies.delete(m.id);
  if (!r.done){ // the mail waits in the Outbox: it isn't done until it goes
    say([t('The unsubscribe mail waits in the Outbox.'), r.errors.join(' · ')].filter(Boolean).join(' '), 15000);
    await refresh(); renderPreview(); return;
  }
  say(r.method === 'one_click' ? t('Unsubscribed. Mail already on its way may still arrive.')
    : r.method === 'mail' ? t('Sent the unsubscribe mail to {target}.', u)
    : t('The list’s page is open in your browser.'), 8000);
  const email = m.from_email;
  try {
    const n = (await invoke('list_page', {view:senderView(email), at:'first', limit:1})).total;
    unsubOffer = n ? {id:m.id, email, n} : null;
  } catch { unsubOffer = null; }
  await refresh();
  if (current()?.id === m.id && !state.compose) renderPreview();
}
async function trashSenderMail(){
  const o = unsubOffer; unsubOffer = null;
  if (!o) return;
  let ids, r;
  try { ids = await invoke('list_ids', {view:senderView(o.email), from:null, to:null}); } catch (e){ say(tr(e), 8000); return; }
  if (!ids.length){ renderPreview(); return; }
  try { r = await call('delete_messages', {ids}); } catch { return; }
  ids.forEach(id => bodies.delete(id));
  setUndo(r.trashed.length ? {kind:'move', items:r.trashed.map(([id, from]) => ({id, from}))} : null);
  say(tn(ids.length, 'Moved {n} message to Trash. Press Z to undo.', 'Moved {n} messages to Trash. Press Z to undo.', {n:num(ids.length)}));
  await refresh();
  if (!state.compose) renderPreview();
}

let pvSeq = 0;
function riskHtml(m){
  const r = m.risk;
  const score = Math.max(0, Math.min(99, Number(r?.score) || 0));
  // Only mail with warning signs gets a line; everything else reads undisturbed.
  if (!score || isOut(m.folder) || pref().risk_checks === false) return '';
  const level = score >= 70 ? 'high' : score >= 40 ? 'elevated' : 'low';
  const label = score >= 70 ? t('High scam risk') : score >= 40 ? t('Suspicious') : t('Some warning signs');
  const reasons = {
    sender_brand_mismatch:s => t('The sender claims to be {claimed}, but uses an unlisted domain ({actual}).', s),
    sender_lookalike:s => t('The sender domain {actual} resembles {claimed}, but is outside its listed domains.', s),
    link_lookalike:s => t('A link domain {actual} resembles {claimed}, but is outside its listed domains.', s),
    misleading_userinfo:s => t('A link puts {claimed} before @; its actual destination is {actual}.', s),
    link_text_mismatch:s => t('Link text names {claimed}, but the address points to {actual}. Tracking links can also do this.', s),
    hidden_characters:s => t('Invisible or text-direction characters appear in a sender or link address ({actual}).', s),
    offsite_account_action:s => t('An account or payment button claiming {claimed} points to an unlisted domain ({actual}).', s),
    reply_to_mismatch:s => t('Replies go to {actual}, a different domain from the sender ({claimed}).', s),
    sensitive_request:() => t('The message also asks for account access or payment details.'),
  };
  const signals = (r.signals || []).filter(s => reasons[s.kind]).slice().sort((a, b) => b.points - a.points);
  return `<details class="risk-panel" data-level="${level}"${score >= 70 ? ' open' : ''}><summary><b>${esc(t('Risk {score}/100', {score}))}</b><span>${esc(label)}</span><small>${esc(t('Local check'))}</small></summary>`
    + `<div class="risk-details">${signals.length ? `<ul>${signals.map(s => {
      const evidence = {claimed:window.TelexLinks.visible(s.claimed || ''), actual:window.TelexLinks.visible(s.actual || '')};
      return `<li><span>${esc(reasons[s.kind](evidence))}</span><b>+${Number(s.points) || 0}</b></li>`;
    }).join('')}</ul>` : ''}<p>${esc(t('This is a heuristic risk score, not a measured probability. A low score does not establish safety.'))}</p>`
    + `<p>${esc(t('Checked locally without opening links. Authentication, redirects and reputation are not verified.'))}</p>`
    + (r.limited ? `<p>${esc(t('Some content was unavailable or exceeded the check limits.'))}</p>` : '') + '</div></details>';
}
previewEl.addEventListener('keydown', e => {
  if (e.target.closest?.('.risk-panel') && ['Enter', ' '].includes(e.key)) e.stopPropagation();
  if (e.target.closest?.('.invite') && ['Enter', ' '].includes(e.key)) e.stopPropagation();
  if (e.target.classList?.contains('iv-comment') && e.key === 'Escape'){ e.stopPropagation(); e.target.blur(); }
});

/* ---------- calendar invitations ---------- */
// The card above mail that holds an event (calendar/invites.rs): what, when in this PC's time
// (and in the zone it was written in, when that clock differs), where, who and their answers.
// Accept, Maybe and Decline send the reply to the organizer at once; Spam can't answer.
const ANSWER = {accepted:N_('Accepted'), tentative:N_('Maybe'), declined:N_('Declined'), needs_action:N_('No answer yet'), delegated:N_('Delegated')};
const ANSWER_MARK = {accepted:'✓', tentative:'?', declined:'✕', needs_action:'·', delegated:'→'};
const IV_BLOCKED = {
  spam:N_('Invitations in Spam aren’t answered. Move it out of Spam first.'),
  cancelled:N_('This meeting was cancelled.'),
  outdated:N_('A newer version of this invitation came. Answer that one.'),
  organizer:N_('You organize this meeting.'),
  no_organizer:N_('The invitation doesn’t say who organizes it, so it can’t be answered.'),
  no_account:N_('Add an account to answer from.'),
};
const IV_YOU = {
  accepted:N_('You accepted on {when}.'), tentative:N_('You answered maybe on {when}.'), declined:N_('You declined on {when}.'),
};
const IV_EARLIER = {
  accepted:N_('You accepted an earlier version on {when}.'), tentative:N_('You answered maybe to an earlier version on {when}.'), declined:N_('You declined an earlier version on {when}.'),
};
const IV_THEY = {accepted:N_('{name} accepted.'), tentative:N_('{name} may attend.'), declined:N_('{name} declined.'), delegated:N_('{name} passed it on.'), needs_action:N_('{name} answered.')};
const utcOffset = min => `UTC${min < 0 ? '−' : '+'}${Math.floor(Math.abs(min) / 60)}${Math.abs(min) % 60 ? ':' + pad(Math.abs(min) % 60) : ''}`;
const ivDay = d => `${t(DOW[d.getDay()])} ${dmy(d)}`;
function eventWhen(ev){
  if (!ev.start) return '';
  const s = parseDate(ev.start), e = ev.end ? parseDate(ev.end) : null;
  if (ev.all_day) return (e && dmy(e) !== dmy(s) ? `${ivDay(s)} – ${ivDay(e)}` : ivDay(s)) + ` · ${t('all day')}`;
  if (!e) return `${ivDay(s)}, ${hm(s)}`;
  return dmy(e) === dmy(s) ? `${ivDay(s)}, ${hm(s)}–${hm(e)}` : `${ivDay(s)}, ${hm(s)} – ${ivDay(e)}, ${hm(e)}`;
}
function zoneWhen(ev){
  const z = ev.zone;
  if (!z || !ev.start) return '';
  const s = parseDate(z.start), e = z.end ? parseDate(z.end) : null;
  const day = dmy(s) !== dmy(parseDate(ev.start)) ? `${ivDay(s)}, ` : '';
  return t('{time} in {zone} ({offset})', {time:day + hm(s) + (e ? '–' + hm(e) : ''), zone:z.name, offset:utcOffset(z.offset)});
}
const ivName = p => p.name || p.email;
function ivPerson(a){
  const answer = t(ANSWER[a.answer] ?? ANSWER.needs_action);
  const extra = [a.me && t('you'), a.role === 'optional' && t('optional'), a.role === 'chair' && t('organizer'), a.room && t('room')].filter(Boolean);
  return `<span class="iv-p" data-a="${esc(a.answer)}" title="${esc([answer, a.email].filter(Boolean).join(' · '))}"><i aria-hidden="true">${ANSWER_MARK[a.answer] ?? '·'}</i>${esc(ivName(a))}${extra.length ? ` <small>(${esc(extra.join(', '))})</small>` : ''}<span class="vh"> – ${esc(answer)}</span></span>`;
}
const IV_SHOWN = 8; // invited people shown before "n more"
function inviteHtml(m, spam){
  const iv = m.invite;
  if (!iv) return '';
  const ev = iv.event, off = iv.state === 'cancelled';
  const kind = {request:t('Invitation'), cancel:t('Cancelled'), reply:t('Answer|invitation'), publish:t('Event'), counter:t('New time proposed'), other:t('Event')}[iv.method] ?? t('Event');
  const rows = [];
  const when = eventWhen(ev);
  if (when) rows.push([t('When'), `<span class="iv-when">${esc(when)}</span>`
    + (ev.zone ? `<small>${esc(zoneWhen(ev))}</small>` : '')
    + (ev.repeats ? `<small>${esc(ev.repeats)}</small>` : '')
    + (ev.one_date ? `<small>${esc(t('One date of a repeating event'))}</small>` : '')]);
  const links = ev.links.map(l => spam
    ? `<span class="iv-link">${esc(l.label)}</span>`
    : `<button type="button" class="linkbtn iv-link" data-meet="${esc(l.url)}" title="${esc(l.url)}">${esc(t('Join on {service}', {service:l.label}))}</button>`).join(' ');
  if (ev.location || links) rows.push([t('Where'), `${ev.location ? `<span>${esc(ev.location)}</span>` : ''}${links}`]);
  if (ev.organizer) rows.push([t('Organizer'), `<span title="${esc(ev.organizer.email)}">${esc(ivName(ev.organizer))}</span>`]);
  const people = ev.attendees.filter(a => !(ev.organizer && a.email === ev.organizer.email && a.role !== 'optional'));
  if (people.length){
    const more = people.length - IV_SHOWN + ev.more_attendees;
    rows.push([t('Invited'), `<span class="iv-people">${people.map((a, i) => i < IV_SHOWN ? ivPerson(a) : ivPerson(a).replace('class="iv-p"', 'class="iv-p iv-extra"')).join('')}`
      + (more > 0 ? `<button type="button" class="linkbtn iv-morep" data-act="iv-people">${esc(tn(more, '{n} more', '{n} more', {n:num(more)}))}</button>` : '') + '</span>']);
  }
  if (!m.body.trim() && ev.description) rows.push([t('Details'), `<span class="iv-desc">${esc(ev.description)}</span>`]);
  // What the user did, or why they can't.
  const lines = [];
  if (iv.method === 'reply' && iv.from) lines.push(`<p class="iv-line">${esc(t(IV_THEY[iv.from.answer] ?? IV_THEY.needs_action, {name:ivName(iv.from)}))}${ev.comment ? ` <q>${esc(ev.comment)}</q>` : ''}</p>`);
  if (iv.method === 'counter') lines.push(`<p class="iv-line">${esc(t('{name} proposes another time. Telex can’t answer that; answer from your calendar.', {name:ivName(ev.attendees[0] ?? ev.organizer ?? {email:m.from_email})}))}</p>`);
  const answered = iv.answered && (iv.answered.earlier ? IV_EARLIER : IV_YOU)[iv.answered.answer];
  if (answered) lines.push(`<p class="iv-line iv-you">${esc(t(answered, {when:`${dmy(parseDate(iv.answered.at))} ${hm(parseDate(iv.answered.at))}`}))}</p>`);
  else if (iv.method === 'request' && iv.mine && iv.mine !== 'needs_action') lines.push(`<p class="iv-line iv-you">${esc(t('Your answer: {answer}', {answer:t(ANSWER[iv.mine])}))}</p>`);
  if (iv.blocked && IV_BLOCKED[iv.blocked] && iv.method === 'request') lines.push(`<p class="iv-line iv-warn${iv.blocked === 'cancelled' || iv.blocked === 'spam' ? ' iv-off' : ''}">${esc(t(IV_BLOCKED[iv.blocked]))}</p>`);
  else if (off) lines.push(`<p class="iv-line iv-warn iv-off">${esc(t('This meeting was cancelled.'))}</p>`);
  const current = iv.answered && !iv.answered.earlier ? iv.answered.answer : iv.mine;
  const answer = iv.method === 'request' && !iv.blocked
    ? `<div class="iv-answer"><span class="seg" role="group" aria-label="${esc(t('Answer the invitation'))}">${[['accepted', N_('Accept')], ['tentative', N_('Maybe|answer')], ['declined', N_('Decline')]].map(([a, l]) =>
      `<button type="button" data-answer="${a}" aria-pressed="${current === a}">${esc(t(l))}</button>`).join('')}</span>`
      + `<input class="iv-comment" type="text" maxlength="2000" spellcheck="true" placeholder="${esc(t('Comment for the organizer (optional)'))}" aria-label="${esc(t('Comment for the organizer (optional)'))}"></div>` : '';
  return `<section class="invite${off ? ' off' : ''}" aria-label="${esc(kind)}">
  <div class="iv-top">${I.cal}<span class="iv-kind">${esc(kind)}</span><b class="iv-what">${esc(ev.summary || t('(no title)'))}</b>
    <span class="iv-tools"><button type="button" class="btn" data-act="iv-add" title="${esc(t('Open it in your calendar app'))}">${esc(t('Add to calendar'))}</button><button type="button" class="pbtn" data-act="iv-save" title="${esc(t('Save as .ics…'))}" aria-label="${esc(t('Save as .ics…'))}">${I.get}</button></span></div>
  ${rows.length ? `<dl class="iv-grid">${rows.map(([k, v]) => `<dt>${esc(k)}</dt><dd>${v}</dd>`).join('')}</dl>` : ''}
  ${lines.join('')}${answer}
</section>`;
}
let answering = false;
async function answerInvite(answer){
  const s = current();
  if (!s || answering) return;
  const comment = previewEl.querySelector('.iv-comment')?.value ?? '';
  const buttons = [...previewEl.querySelectorAll('[data-answer]')];
  answering = true; buttons.forEach(b => b.disabled = true);
  say(t('Sending your answer…'), 0);
  let r;
  try { r = await call('answer_invite', {id:s.id, answer, comment}); }
  catch { buttons.forEach(b => b.disabled = false); return; }
  finally { answering = false; }
  bodies.delete(s.id);
  const sent = sentText(r.sent);
  const text = r.errors.length ? [sent, r.errors.join(' · ')].filter(Boolean).join(' ') : sent || t('In the Outbox. It goes out with the next check.');
  const ms = r.errors.length ? 15000 : 6000;
  if (VIEWER){ tellMain({say:text, ms}); renderPreview(); return; }
  schedulePush();
  await refresh();
  renderPreview();
  say(text, ms);
}
async function renderPreview(){
  const seq = ++pvSeq, s = current();
  hideLink(); closeMenu(false);
  if (!ACCOUNTS.length){
    // Another mail program on this PC: everything can come from there in one go.
    if (!moveIn.sources) findPrograms().then(() => { if (!ACCOUNTS.length) renderPreview(); });
    const from = moveIn.sources?.find(s => s.accounts.length || s.folders.length);
    previewEl.innerHTML = `<div class="welcome">${I.mail}<h2>${esc(t('Welcome to Telex'))}</h2>
<p>${esc(t('Add your mail account to start. Telex keeps your mail on this PC and talks only to your mail servers.'))}</p>
<div class="wbtns"><button type="button" class="btn primary" data-act="add-account">${esc(t('Add your mail account'))}</button>
${from ? `<button type="button" class="btn" data-act="move-in" data-path="${esc(from.path)}">${esc(t('Bring everything from {program}', {program:programName(from.program)}))}</button>` : ''}</div>
<p class="wkeys">${esc(t('Telex works from the keyboard.'))} <button type="button" class="linkbtn" data-act="help" data-help-open><kbd>?</kbd> ${esc(t('Every key and tip'))}</button></p></div>`;
    return;
  }
  if (state.picked.size > 1){
    const keys = [['Del', N_('to Trash')], ['M', N_('move')], ['F', N_('flag|several')], ['U', N_('read or unread')], ['Ctrl+C', N_('copy as Markdown')], ['Esc', N_('clear the selection')]];
    previewEl.innerHTML = `<div class="pempty">${I.mail}<div><b>${esc(tn(state.picked.size, '{n} message selected', '{n} messages selected', {n:num(state.picked.size)}))}</b></div>`
      + `<div class="mkeys">${keys.map(([k, l]) => `<span><kbd>${k}</kbd> ${esc(t(l))}</span>`).join('')}</div></div>`;
    return;
  }
  if (!s){
    previewEl.innerHTML = `<div class="pempty">${I.mail}<div>${list.total || allMail() ? esc(t('Select a message to read it here.')) : t('Nothing in {folder}.', {folder:esc(folderName(state.folder))})}</div></div>`;
    return;
  }
  let m, hv;
  const dark = pref().prefer_text ? null : darkPaper(s.id);
  try {
    m = await full(s.id);
    hv = await htmlView(s.id, pref().prefer_text ? false : state.images.has(s.id) ? true : null, dark);
  } catch (e){ if (seq === pvSeq) previewEl.innerHTML = `<div class="pempty">${esc(tr(e))}</div>`; return; }
  if (seq !== pvSeq || state.compose) return;
  const spam = kindOf(s.f) === 'spam'; // links and trusted images stay off in Spam
  const asHtml = !!hv && !pref().prefer_text;
  const views = hv ? `<span class="seg" role="group" aria-label="${esc(t('Show as'))}"><button type="button" data-view="html" aria-pressed="${asHtml}" title="${esc(t('Show the HTML (H)'))}">HTML</button><button type="button" data-view="text" aria-pressed="${!asHtml}" title="${esc(t('Show plain text (H)'))}">${esc(t('Text|view'))}</button></span>` : '';
  // In a dark theme, one click puts this message on white paper, as the sender designed it.
  const onWhite = state.white.has(s.id);
  const paper = asHtml && DARK_THEMES.has(document.documentElement.dataset.theme) && pref().mail_paper === 'theme'
    ? `<button type="button" class="pbtn" data-act="paper" aria-pressed="${onWhite}" title="${esc(onWhite ? t('Show on dark paper') : t('Show on white, as sent'))}" aria-label="${esc(onWhite ? t('Show on dark paper') : t('Show on white, as sent'))}">${onWhite ? I.moon : I.sun}</button>` : '';
  const images = asHtml ? imagesBar(m, hv, spam)
    : !hv && m.remote_images && !state.images.has(m.id) ? `<div class="bar">${esc(t('Remote images are blocked so the sender can’t see when you open this.'))} <button type="button" class="linkbtn" data-act="images">${esc(t('Load images'))}</button></div>` : '';
  const popout = VIEWER ? '' : `<button type="button" class="pbtn" data-act="window" title="${esc(t('Open in its own window (Enter)'))}" aria-label="${esc(t('Open in its own window (Enter)'))}">${I.popout}</button>`;
  previewEl.innerHTML = `<article>
${headerHtml({...m, tags:s.tags ?? m.tags}, `${unsubButton(m)}${readButton(s)}${flagButton(s)}${tagButton()}${popout}${mdButton()}${outButton()}${paper}${views}`)}
${inviteHtml(m, spam)}
${riskHtml(m)}
${goesBar(m)}${unsubBar(m)}${pgpHtml(m)}
${m.note ? `<div class="bar note${spam || kindOf(s.f) === 'outbox' ? ' warn' : ''}">${esc(m.note)}</div>` : ''}
${images}
${asHtml ? `<div class="pbody html${dark ? ' dark' : ''}"><iframe class="hframe" sandbox="allow-same-origin" referrerpolicy="no-referrer" title="${esc(t('Message'))}"></iframe></div>` : `<div class="pbody" lang="">${bodyHtml(m)}</div>`}
</article>`;
  if (asHtml) mountFrame(previewEl.querySelector('.hframe'), hv.document, spam, s.id);
  else {
    const body = previewEl.querySelector('.pbody');
    markLinks(body, spam); bindPreviewLinks(body, spam);
  }
  previewEl.scrollTop = 0;
}
async function setView(text){
  if (!await changeSettings({prefer_text:text})) return;
  if (!state.compose) renderPreview();
  say(text ? t('Showing plain text. Press H for HTML.') : t('Showing HTML. Press H for plain text.'), 2500);
}
previewEl.addEventListener('click', async e => {
  const link = e.target.closest('.pbody a[href]');
  if (link){ e.preventDefault(); const s = current(); followLink(link.getAttribute('href'), s && (kindOf(s.f) === 'spam')); return; }
  const view = e.target.closest('[data-view]');
  if (view){ setView(view.dataset.view === 'text'); return; }
  const rk = e.target.closest('[data-rk]');
  if (rk && state.compose){ switchReply(rk.dataset.rk === 'all'); return; }
  const more = e.target.closest('.qmore');
  if (more){ more.closest('.qb').classList.remove('folded'); more.remove(); return; }
  const att = e.target.closest('[data-att]'), save = e.target.closest('[data-save]'), act = e.target.closest('[data-act]');
  if (!state.compose && (att || save || e.target.closest('[data-saveall]'))){
    const m = current(); if (!m) return;
    if (att) openAttachment(m.id, Number(att.dataset.att));
    else if (save) saveAttachment(m.id, Number(save.dataset.save));
    else saveAttachments(m.id);
    return;
  }
  const answer = e.target.closest('[data-answer]'), meet = e.target.closest('[data-meet]');
  if (answer && !state.compose){ answerInvite(answer.dataset.answer); return; }
  if (meet){ call('open_link', {url:meet.dataset.meet}).then(() => say(t('Opened in your browser.'), 2500)).catch(() => {}); return; }
  const off = e.target.closest('[data-unattach]');
  if (off && state.compose){ state.compose.attachments.splice(Number(off.dataset.unattach), 1); renderComposeAtts(); return; }
  if (!act) return;
  const m = current();
  if (act.dataset.act === 'images' && m){
    state.images.add(m.id); say(t('Loading images…'), 0);
    await renderPreview(); say(t('Images loaded for this message only.'));
  }
  if ((act.dataset.act === 'trust' || act.dataset.act === 'untrust') && m){
    const trust = act.dataset.act === 'trust', email = (await full(m.id)).from_email;
    try { await call('trust_sender', {email, trusted:trust}); } catch { return; }
    htmlViews.clear();
    if (trust) state.images.add(m.id); else state.images.delete(m.id);
    if (trust) say(t('Loading images…'), 0);
    await renderPreview();
    say(trust ? t('Images from {email} will load automatically, except in Spam.', {email}) : t('Images from {email} are blocked again.', {email}));
  }
  if (act.dataset.act === 'window' && m){ openWindow(m.id); return; }
  if (act.dataset.act === 'markdown' && m){ copyMarkdown([m.id]); return; }
  if (act.dataset.act === 'out' && m){ const r = act.getBoundingClientRect(); openMenu(r.left, r.bottom + 2, outItems(m.id), act); return; }
  if ((act.dataset.act === 'iv-add' || act.dataset.act === 'iv-save') && m){
    const iv = (await full(m.id)).invite;
    if (iv) (act.dataset.act === 'iv-add' ? openAttachment : saveAttachment)(m.id, iv.file);
    return;
  }
  if (act.dataset.act === 'iv-people'){ act.closest('.iv-people').classList.add('open'); act.remove(); return; }
  if (act.dataset.act === 'read' && m){ setUnread(m, !m.unread); return; }
  if (act.dataset.act === 'flag' && m){ setFlagged(m, !m.flagged); return; }
  if (act.dataset.act === 'tags' && m){ const r = act.getBoundingClientRect(); openTagMenu(r.left, r.bottom + 2, act); return; }
  if (act.dataset.act === 'sender' && m){ filterAllMail((await full(m.id)).from_email); return; }
  if (act.dataset.act === 'unsub' && m && act.getAttribute('aria-disabled') !== 'true'){ startUnsubscribe(await full(m.id)); return; }
  if (act.dataset.act === 'unsub-go' && m){ unsubAsk = null; unsubscribeNow(await full(m.id)); return; }
  if (act.dataset.act === 'unsub-no'){ unsubAsk = null; renderPreview(); return; }
  if (act.dataset.act === 'unsub-trash'){ trashSenderMail(); return; }
  if (act.dataset.act === 'unsub-keep'){ unsubOffer = null; renderPreview(); return; }
  if (act.dataset.act === 'send-now' && m){ sendNow(m); return; }
  if (act.dataset.act === 'hold' && m){ holdLater(m); return; }
  if (act.dataset.act === 'later' && state.compose){ laterMenu(act); return; }
  if (act.dataset.act === 'later-at' && state.compose){ sendAtPicked(); return; }
  if (act.dataset.act === 'later-cancel' && state.compose){ $('c-later').hidden = true; $('c-when-btn')?.focus(); return; }
  if (act.dataset.act === 'contact' && m){ const f = await full(m.id); openAddressBook({name:f.from_name, email:f.from_email}); return; }
  if (act.dataset.act === 'paper' && m){
    if (state.white.has(m.id)) state.white.delete(m.id); else state.white.add(m.id);
    renderPreview();
  }
  if (act.dataset.act === 'bcc' && state.compose){
    readCompose(); state.compose.showBcc = true; renderCompose(); $('c-bcc')?.focus();
  }
  if (act.dataset.act === 'attach' && state.compose) pickFiles();
  if (act.dataset.act === 'others-add' && state.compose) switchReply(true);
  if (act.dataset.act === 'origfiles' && state.compose) attachOriginal();
  if (act.dataset.act === 'ask-send' && state.compose) send();
  if (act.dataset.act === 'ask-one' && state.compose){ switchReply(false); say(t('Only to the sender now. Send when ready.'), 4000); }
  if (act.dataset.act === 'spell' && state.compose) spellLanguageMenu(act);
  if (act.dataset.act === 'discard') discardCompose();
  if (act.dataset.act === 'draft') closeCompose().then(ok => ok && say(t('Saved to Drafts.')));
  if (act.dataset.act === 'add-account') openAccountDialog();
  if (act.dataset.act === 'help') openHelp();
  if (act.dataset.act === 'move-in') openMoveIn(act.dataset.path);
});

/* ---------- compose ---------- */
// kind: new, reply, replyall or forward, started by the core with the identity's signature
// and templates; or edit, which reopens a draft or an unsent message in the Outbox.
// A message is plain text (`body`) or HTML (`html`, edited in place); Settings › Writing
// says which a new message, a reply and a forward start in, and Text | HTML switches.
// `of`: the message to edit when it isn't the selected one (one just taken back from sending).
async function openCompose(kind, preset = {}, of = null){
  if (!servers().length){ say(t('Add a mail account first.')); openAccountDialog(); return; }
  const s = of != null ? {id:of, f:(await full(of).catch(() => null))?.folder} : current();
  if (kind !== 'new' && !s?.f){ say(t('Select a message first.')); return; }
  if (kind === 'edit' && !['drafts', 'outbox'].includes(kindOf(s.f))){ say(t('Only drafts and unsent messages in the Outbox can be edited.')); return; }
  const open = state.compose;
  if (kind === 'edit' && open && [open.replaces, open.autosaved].includes(s.id)){ say(t('You are writing that message already.')); focusCompose(); return; }
  if (kind !== 'new' && state.picked.size > 1){ unpick(); paintSelection(); }
  loadRecipients();
  let c;
  try {
    if (kind === 'edit'){
      bodies.delete(s.id); // its note may have changed (taken back, refused)
      const m = await full(s.id), account = acctId(s.f), ids = ACCT[account]?.identities ?? [];
      // Encrypted and still locked: its text isn't here to edit yet.
      if (m.pgp?.encrypted && !m.pgp.decrypted){ say(t('It is encrypted: unlock it above the message first.'), 6000); return; }
      const me = ids.find(i => i.email.toLowerCase() === m.from_email.toLowerCase()) ?? ids.find(i => i.is_default) ?? ids[0];
      // Mail sent later is held while it is changed, so it can't go meanwhile; Send keeps its time.
      const later = m.goes && parseDate(m.goes) - Date.now() > 60000 ? m.goes : null;
      if (later){ await invoke('hold_message', {id:s.id}); bodies.delete(s.id); }
      // Attachments come from the old message itself, which Send replaces.
      c = {kind, account, identity:me?.id ?? null, to:m.to, cc:m.cc, bcc:m.bcc ?? '', subject:m.subject, body:m.body,
        html:await invoke('draft_html', {id:s.id}), attachments:m.attachments.slice(), replyTo:null, forwardOf:m.id, replaces:m.id, later, pgpWas:m.pgp ?? null};
      if (later) refresh();
    } else {
      // A new message takes the open folder's templates (a reply, its original's folder's).
      const folder = state.folder && !state.folder.startsWith('*/') ? state.folder : null;
      const d = await invoke('start_draft', {kind:kind === 'replyall' ? 'reply_all' : kind, of:kind === 'new' ? null : s.id, account:acctId(state.folder), folder});
      c = {kind, account:d.account, identity:d.identity, to:d.to, cc:d.cc, bcc:'', subject:d.subject, body:d.body, html:null, caretLine:d.caret_line,
        attachments:d.attachments, replyTo:d.reply_to, forwardOf:d.forward_of, replaces:null};
      if (kind === 'reply' || kind === 'replyall'){
        const [other, orig] = await Promise.all([
          invoke('start_draft', {kind:kind === 'reply' ? 'reply_all' : 'reply', of:s.id, account:acctId(state.folder), folder}).catch(() => null),
          full(s.id).catch(() => null)]);
        const [one, all] = kind === 'reply' ? [d, other] : [other, d];
        // Only when the original had others besides the sender is there a choice to make.
        const count = x => addressesIn(x.to + ',' + x.cc).length;
        if (one && all && count(all) > count(one)) c.recipients = {one:{to:one.to, cc:one.cc}, all:{to:all.to, cc:all.cc}};
        if (orig?.attachments?.length) c.originalFiles = {id:s.id, list:orig.attachments};
      }
      const p = pref();
      const html = kind === 'new' ? p.compose_format === 'html'
        : p.reply_format === 'same' ? !!(await htmlView(s.id, false)) : p.reply_format === 'html';
      // The core's HTML: the signature as written, a reply's original in its own HTML.
      if (html) c.html = d.html;
    }
  } catch (e){ say(tr(e), 8000); return; }
  Object.assign(c, preset); // e.g. {to} from a mailto: link
  const kept = state.compose;
  if (kept && !await saveCompose()) return; // the message being written goes to Drafts first
  state.compose = c;
  renderCompose();
  c.saved = shapeOf(readCompose()); // as it opened: nothing to autosave yet
  if (kept){ draftsChanged(); say(t('The message you were writing is in Drafts.'), 6000); }
  else if (c.later) say(t('Held while you change it. Send keeps its time, {when}; Esc keeps it as a draft.', {when:fmtFull(parseDate(c.later))}), 8000);
}
const focusCompose = () => $('c-to')?.focus();
// After a draft was saved or dropped: the Drafts folder's count and list (the main window's).
const draftsChanged = () => VIEWER ? tellMain() : refresh();
const allIdentities = () => servers().flatMap(a => (a.identities || []).map(i => ({...i, account:a.id})));
const composeNote = c => !ACCT[c.account]?.smtp ? t('This account has no SMTP server yet: add one in Settings › Accounts to send.') : '';
function fromOptions(c){
  return servers().map(a => `<optgroup label="${esc(a.name)}">` + (a.identities || []).map(i =>
    `<option value="${i.id}"${i.id === c.identity ? ' selected' : ''}>${esc(i.name)} &lt;${esc(i.email)}&gt;</option>`).join('') + '</optgroup>').join('');
}
// Another identity: the account follows it, and its signature replaces the old one.
async function changeIdentity(id){
  const c = readCompose(), next = allIdentities().find(i => i.id === id);
  if (!next) return;
  const prev = allIdentities().find(i => i.id === c.identity);
  const before = (prev?.signature || '').trim(), after = (next.signature || '').trim();
  Object.assign(c, {identity:next.id, account:next.account});
  previewEl.querySelector('.cnote').textContent = composeNote(c);
  if (c.html == null){
    if (before && before !== after && c.body.includes(before)){ c.body = c.body.replace(before, () => after); $('c-body').value = c.body; }
    return;
  }
  // In HTML the signature has a block of its own: its content is swapped, as written.
  const block = $('c-html')?.querySelector('.telex-signature');
  if (!block || signatureHtml(prev) === signatureHtml(next)) return;
  if (after) block.innerHTML = signatureHtml(next); else block.remove();
}
// The HTML editor's formatting bar: the browser's own editing commands, the palette's colors and
// sizes, and pictures. Used by compose and by the signature editors (Settings).
const FORMATS = [['bold', '<b>B</b>', N_('Bold (Ctrl+B)')], ['italic', '<i>I</i>', N_('Italic (Ctrl+I)')], ['underline', '<u>U</u>', N_('Underline (Ctrl+U)')],
  ['strikeThrough', '<s>S</s>', N_('Strikethrough')], ['color', '<b>A</b><i class="sw"></i>', N_('Text color')], ['mark', I.mark + '<i class="sw"></i>', N_('Highlight')],
  ['size', '<span>a<b>A</b></span>', N_('Text size')], ['insertUnorderedList', I.ul, N_('Bulleted list')], ['insertOrderedList', I.ol, N_('Numbered list')],
  ['quote', I.quote, N_('Quote')], ['link', I.link, N_('Link (Ctrl+K)')], ['picture', I.picture, N_('Insert picture…')], ['removeFormat', I.clear, N_('Clear formatting')]];
const MENU_CMDS = new Set(['color', 'mark', 'size']);
function fbarHtml(urlId = ''){
  return `<div class="fbar" role="toolbar" aria-label="${esc(t('Formatting'))}">${FORMATS.map(([cmd, label, tip]) =>
    `<button type="button" class="fb${cmd === 'color' ? ' fcolor' : cmd === 'mark' ? ' fmark' : ''}" data-cmd="${cmd}" title="${esc(t(tip))}" aria-label="${esc(t(tip))}"${MENU_CMDS.has(cmd) ? ' aria-haspopup="menu"' : ''}>${label}</button>`).join('')}`
    + `<span class="flink" hidden><input${urlId ? ` id="${urlId}"` : ''} type="url" placeholder="https://" spellcheck="false" aria-label="${esc(t('Link address'))}"><small>${esc(t('Enter sets the link, Esc cancels'))}</small></span></div>`;
}
// The palette, as the core sends it (html::TEXT_COLORS, MARK_COLORS); the editor shows each in the
// theme's shade (--pal-n, --mark-n in themes.css). The "none" colors are stand-ins the stylesheet
// shows as the text's own and the core leaves out.
const TEXT_COLORS = [['#c00000', N_('Red')], ['#b45309', N_('Orange')], ['#15803d', N_('Green')], ['#1d4ed8', N_('Blue')], ['#7e22ce', N_('Purple')], ['#6b7280', N_('Gray')]];
const MARK_COLORS = [['#fff59d', N_('Yellow')], ['#c8f7c5', N_('Green')], ['#cfe8ff', N_('Blue')], ['#ffd6e7', N_('Pink')]];
const NO_COLOR = '#010203', NO_MARK = '#010204';
// Text in the palette's colors, highlighted or sized. styleWithCSS makes the browser write spans
// with styles (color, background-color, font-size: small or x-large) instead of <font>.
function formatWith(ed, kept, cmd, value){
  ed.focus();
  if (kept){ const sel = getSelection(); sel.removeAllRanges(); sel.addRange(kept); }
  document.execCommand('styleWithCSS', false, true);
  try { document.execCommand(cmd, false, value); } finally { document.execCommand('styleWithCSS', false, false); }
}
function paletteMenu(ed, btn, kind){
  const sel = getSelection(), kept = sel.rangeCount && ed.contains(sel.anchorNode) ? sel.getRangeAt(0).cloneRange() : null;
  const items = kind === 'color'
    ? [{label:t('Automatic|text color'), swatch:'none', run:() => formatWith(ed, kept, 'foreColor', NO_COLOR)},
      ...TEXT_COLORS.map(([hex, name], i) => ({label:t(name), swatch:'p' + (i + 1), run:() => formatWith(ed, kept, 'foreColor', hex)}))]
    : kind === 'mark'
    ? [{label:t('No highlight'), swatch:'none', run:() => formatWith(ed, kept, 'hiliteColor', NO_MARK)},
      ...MARK_COLORS.map(([hex, name], i) => ({label:t(name), swatch:'m' + (i + 1), run:() => formatWith(ed, kept, 'hiliteColor', hex)}))]
    : [['2', t('Small|text size'), 'fs-small'], ['3', t('Normal|text size'), ''], ['5', t('Large|text size'), 'fs-large']]
      .map(([size, label, cls]) => ({label, cls, run:() => formatWith(ed, kept, 'fontSize', size)}));
  const r = btn.getBoundingClientRect();
  openMenu(r.left, r.bottom + 2, items, btn);
}
// Pictures in the HTML editor, pasted, dropped from Explorer or chosen with Insert picture…: the
// core makes them the size Settings › Writing says (a signature's smaller) and they go in where
// the caret is, as data: URIs; sending makes them parts of the message.
async function insertPictures(ed, sources, signature = false){
  for (const source of sources){
    let p;
    try { p = await invoke('editor_image', {...source, signature}); } catch (e){ say(tr(e), 9000); continue; }
    if (!ed.isConnected) return;
    ed.focus();
    document.execCommand('insertHTML', false, `<img src="${p.src}" alt="">`);
    if (p.smaller) say(t('Picture made smaller: {width} × {height} pixels.', {width:p.width, height:p.height}), 4000);
  }
}
const isPicture = name => /\.(png|jpe?g|gif|webp)$/i.test(name);
async function choosePictures(ed, signature){
  let files;
  try { files = await invoke('pick_files', {words:{title:t('Insert picture'), pictures:t('Pictures')}}); } catch (e){ say(tr(e), 8000); return; }
  await insertPictures(ed, files.map(f => ({path:f.path})), signature);
}
const fileData = file => new Promise((ok, fail) => { const r = new FileReader(); r.onload = () => ok(r.result); r.onerror = fail; r.readAsDataURL(file); });
// A plain-text signature in the editor: a line per <div>, as the core writes it.
const linesHtml = text => (text || '').split('\n').map(l => `<div>${l ? esc(l) : '<br>'}</div>`).join('');
const signatureHtml = i => i?.signature_html ?? linesHtml(i?.signature ?? '');
function renderCompose(){
  const c = state.compose, html = c.html != null;
  const title = {new:t('New message'), reply:t('Reply|compose title'), replyall:t('Reply to all|compose title'), forward:t('Forward|compose title'), edit:t('Edit|compose title')}[c.kind];
  const note = composeNote(c);
  const bcc = !!c.bcc || c.showBcc;
  previewEl.innerHTML = `<form class="compose" id="cform" novalidate>
<div class="ctitle">${c.recipients && (c.kind === 'reply' || c.kind === 'replyall') ? `<span class="seg crk" role="group" aria-label="${esc(t('Reply to'))}"><button type="button" data-rk="one" aria-pressed="${c.kind === 'reply'}">${esc(t('Reply|compose switch'))}</button><button type="button" data-rk="all" aria-pressed="${c.kind === 'replyall'}">${esc(t('Reply all|compose switch'))}</button></span>` : esc(title)}<label class="cfrom">${esc(t('from|compose'))} <select id="c-from" aria-label="${esc(t('From'))}">${fromOptions(c)}</select></label>
<span class="seg cfmt" role="group" aria-label="${esc(t('Format'))}"><button type="button" data-fmt="plain" aria-pressed="${!html}" title="${esc(t('Plain text'))}">${esc(t('Text|view'))}</button><button type="button" data-fmt="html" aria-pressed="${html}" title="${esc(t('HTML: bold, lists, links'))}">HTML</button></span></div>
<div class="cf">
  <label for="c-to">${esc(t('To'))}</label><input id="c-to" value="${esc(c.to)}" autocomplete="off" spellcheck="false" aria-autocomplete="list">
  <label for="c-cc">${esc(t('Cc'))}</label><div class="crow"><input id="c-cc" value="${esc(c.cc)}" autocomplete="off" spellcheck="false" aria-autocomplete="list">${bcc ? '' : `<button type="button" class="linkbtn cbcc" data-act="bcc" title="${esc(t('Blind copy: they get it, the others don’t see them'))}">${esc(t('Bcc'))}</button>`}</div>
  <div class="cothers" id="c-others" hidden></div>
  ${bcc ? `<label for="c-bcc">${esc(t('Bcc'))}</label><input id="c-bcc" value="${esc(c.bcc)}" autocomplete="off" spellcheck="false" aria-autocomplete="list">` : ''}
  <label for="c-subj">${esc(t('Subject'))}</label><div class="spellfield"><input id="c-subj" value="${esc(c.subject)}" autocomplete="off" spellcheck="false"><div class="spellmarks" aria-hidden="true"></div></div>
</div>
<div id="c-attbox">${composeAttsHtml(c)}</div>
${html ? `${fbarHtml('c-url')}
<div id="c-html" class="hedit" contenteditable="true" role="textbox" aria-multiline="true" aria-label="${esc(t('Message text'))}" spellcheck="false"></div>`
  : `<div class="spellfield body"><textarea id="c-body" aria-label="${esc(t('Message text'))}" spellcheck="false">
${esc(c.body)}</textarea><div class="spellmarks" aria-hidden="true"></div></div>`}
<div class="bar pgp warn cpgp" id="c-pgp" hidden></div>
<div class="cask" id="c-ask" role="alert" hidden></div>
<div class="cask clater" id="c-later" hidden><label for="c-when">${esc(t('Send at'))}</label><input id="c-when" type="datetime-local" step="60">
  <button class="btn primary" type="button" data-act="later-at">${esc(t('Send later|button'))}</button><button class="btn" type="button" data-act="later-cancel">${esc(t('Cancel'))}</button></div>
<div class="cbar">
  <button class="btn primary csend" type="submit"><span>${esc(laterLabel(c))}</span> <kbd>Ctrl+Enter</kbd></button><button class="btn primary cwhen" id="c-when-btn" type="button" data-act="later" aria-haspopup="menu" title="${esc(t('Send later (Ctrl+Shift+Enter)'))}" aria-label="${esc(t('Send later (Ctrl+Shift+Enter)'))}">${I.clock}</button>
  <button class="btn" type="button" data-act="draft">${esc(t('Save draft'))} <kbd>Esc</kbd></button>
  <button class="btn" type="button" data-act="attach" title="${esc(t('Attach files from this PC. You can also drop them on the window.'))}">${I.clip}<span>${esc(t('Attach…'))}</span></button>
  <button class="btn" type="button" data-act="discard">${esc(t('Discard'))}</button>
  <span class="cnote">${esc(note)}</span>
  <span class="csaved" aria-live="polite"></span>
  ${pgpSetHtml()}
  <button class="btn spellchip" type="button" data-act="spell" aria-haspopup="menu">${I.spell}<span></span></button>
</div></form>`;
  $('cform').addEventListener('submit', e => { e.preventDefault(); send(); });
  // Enter in a header field doesn't send: Ctrl+Enter does.
  $('cform').addEventListener('keydown', e => { if (e.key === 'Enter' && !e.ctrlKey && !e.metaKey && e.target.tagName === 'INPUT') e.preventDefault(); });
  $('c-from').addEventListener('change', e => changeIdentity(Number(e.target.value)));
  // The picked time: Enter sends it then, Esc puts the line away (and doesn't close the message).
  $('c-when').addEventListener('keydown', e => {
    if (e.key === 'Enter' && !e.ctrlKey && !e.metaKey){ e.preventDefault(); e.stopPropagation(); sendAtPicked(); }
    else if (e.key === 'Escape'){ e.preventDefault(); e.stopPropagation(); $('c-later').hidden = true; $('c-when-btn').focus(); }
  });
  previewEl.querySelector('.cfmt').addEventListener('click', e => { const b = e.target.closest('[data-fmt]'); if (b) setFormat(b.dataset.fmt); });
  ['c-to', 'c-cc', 'c-bcc'].forEach(id => { if ($(id)) wireSuggestions($(id)); });
  const body = html ? $('c-html') : $('c-body');
  if (html){ body.innerHTML = c.html; wireEditor(body); if (c.kind !== 'new') foldComposeQuote(body); }
  ['c-to', 'c-cc', 'c-bcc'].forEach(id => $(id)?.addEventListener('input', paintOthers));
  paintOthers();
  if (c.kind === 'new' || c.kind === 'forward' || (c.kind === 'edit' && !c.to)) $('c-to').focus();
  else focusStart(body, c.caretLine);
  wireSpelling();
  paintSpellChip();
  wirePgp();
}
/* ---------- reply or reply all ---------- */
// A reply knows both answers the core gave: to the sender (`one`) and to everyone (`all`). The
// switch at the top swaps between them without touching the text; with Reply, a line under Cc
// names who else was on the original, one click from being added. Addresses typed by hand stay.
const REPLY_MANY = 8; // a reply to all with this many people asks once before it goes
// "Anna <a@x.pl>, b@y.pl" → [{text, email}], split at commas and semicolons outside quotes.
function addressesIn(line){
  const out = [];
  let from = 0, quoted = false;
  const take = to => {
    const text = line.slice(from, to).trim();
    if (text) out.push({text, email:(text.match(/<([^>]+)>/)?.[1] ?? text).trim().toLowerCase()});
    from = to + 1;
  };
  for (let i = 0; i < line.length; i++){
    if (line[i] === '"') quoted = !quoted;
    else if (!quoted && (line[i] === ',' || line[i] === ';')) take(i);
  }
  take(line.length);
  return out;
}
const nameOfAddress = a => a.text.replace(/<[^>]*>/, '').replace(/"/g, '').trim() || a.email;
// Who was on the original but isn't on this reply now.
function othersLeftOut(c){
  if (!c.recipients) return [];
  const now = new Set(['c-to', 'c-cc', 'c-bcc'].flatMap(id => addressesIn($(id)?.value ?? '')).map(a => a.email));
  const all = [...addressesIn(c.recipients.all.to), ...addressesIn(c.recipients.all.cc)];
  return all.filter((a, i) => !now.has(a.email) && all.findIndex(b => b.email === a.email) === i);
}
function paintOthers(){
  const c = state.compose, row = $('c-others');
  if (!c || !row) return;
  const left = c.kind === 'reply' ? othersLeftOut(c) : [];
  row.hidden = !left.length;
  if (!left.length) return;
  const names = left.slice(0, 4).map(nameOfAddress).join(', ') + (left.length > 4 ? ' ' + tn(left.length - 4, 'and {n} more', 'and {n} more') : '');
  row.innerHTML = `${esc(t('Also on the original: {names}', {names}))} · <button type="button" class="linkbtn" data-act="others-add">${esc(t('Add them'))}</button>`;
}
// Reply ↔ Reply all: everyone from the original goes in (To to To, Cc to Cc), or out again.
function switchReply(all){
  const c = state.compose;
  if (!c?.recipients || (c.kind === 'replyall') === all) return;
  const fields = {to:$('c-to'), cc:$('c-cc')};
  if (all){
    const have = new Set(['c-to', 'c-cc', 'c-bcc'].flatMap(id => addressesIn($(id)?.value ?? '')).map(a => a.email));
    for (const k of ['to', 'cc']){
      const add = addressesIn(c.recipients.all[k]).filter(a => !have.has(a.email) && have.add(a.email));
      if (add.length) fields[k].value = [...addressesIn(fields[k].value), ...add].map(a => a.text).join(', ');
    }
  } else {
    const keep = new Set([...addressesIn(c.recipients.one.to), ...addressesIn(c.recipients.one.cc)].map(a => a.email));
    const drop = new Set([...addressesIn(c.recipients.all.to), ...addressesIn(c.recipients.all.cc)].map(a => a.email).filter(e => !keep.has(e)));
    for (const k of ['to', 'cc']) fields[k].value = addressesIn(fields[k].value).filter(a => !drop.has(a.email)).map(a => a.text).join(', ');
  }
  c.kind = all ? 'replyall' : 'reply';
  c.manyOk = false;
  previewEl.querySelectorAll('[data-rk]').forEach(b => b.setAttribute('aria-pressed', String((b.dataset.rk === 'all') === all)));
  hideComposeAsk();
  paintOthers();
}
// The original's files: a reply goes without them, one click puts them in (carried from the
// original like a forward's).
function originalFilesHtml(c){
  const o = c.originalFiles;
  if (!o || c.forwardOf === o.id) return '';
  const names = o.list.slice(0, 3).map(a => a.name).join(', ') + (o.list.length > 3 ? ' …' : '');
  return `<div class="cfiles">${esc(tn(o.list.length, 'The original had a file: {names}', 'The original had {n} files: {names}', {names}))} · <button type="button" class="linkbtn" data-act="origfiles">${esc(tn(o.list.length, 'Attach it', 'Attach them'))}</button></div>`;
}
const composeAttsHtml = c => originalFilesHtml(c) + attsHtml(c.attachments, true);
function attachOriginal(){
  const c = state.compose, o = c?.originalFiles;
  if (!o) return;
  c.forwardOf = o.id;
  const have = new Set(c.attachments.filter(a => !a.path).map(a => a.name));
  c.attachments.push(...o.list.filter(a => !have.has(a.name)));
  renderComposeAtts();
  hideComposeAsk();
  say(tn(o.list.length, 'Attached the original’s file.', 'Attached the original’s {n} files.'), 3000);
}
// The text the user wrote: the HTML editor's without its quotes and links, or the plain text.
function ownText(){
  const ed = $('c-html');
  return ed ? editorText(ed, 'blockquote, a, .telex-signature').text : $('c-body')?.value ?? '';
}
// Send's questions, in a line above the buttons: a file the text mentions but that isn't
// attached, and a reply to all that goes to many people. Send again goes ahead.
function showComposeAsk(kind, n = 0){
  const box = $('c-ask'), c = state.compose;
  if (!box || !c) return;
  box.dataset.kind = kind;
  if (kind === 'files'){
    box.innerHTML = `<span>${esc(t('You mention an attachment, but nothing is attached.'))}</span>`
      + (c.originalFiles && c.forwardOf !== c.originalFiles.id ? `<button type="button" class="btn" data-act="origfiles">${esc(tn(c.originalFiles.list.length, 'Attach the original’s file', 'Attach the original’s files'))}</button>` : '')
      + `<button type="button" class="btn" data-act="attach">${I.clip}<span>${esc(t('Attach…'))}</span></button>`
      + `<button type="button" class="btn" data-act="ask-send">${esc(t('Send anyway'))}</button>`;
  } else {
    const sender = c.recipients ? addressesIn(c.recipients.one.to).map(nameOfAddress).join(', ') : '';
    box.innerHTML = `<span>${esc(tn(n, 'This reply goes to {n} person.', 'This reply goes to {n} people.'))}</span>`
      + `<button type="button" class="btn" data-act="ask-send">${esc(tn(n, 'Send to all {n}', 'Send to all {n}'))}</button>`
      + (sender ? `<button type="button" class="btn" data-act="ask-one">${esc(t('Reply to {name} only', {name:sender}))}</button>` : '');
  }
  box.hidden = false;
  box.querySelector('button')?.focus();
}
function hideComposeAsk(){
  const box = $('c-ask');
  if (box){ box.hidden = true; delete box.dataset.kind; }
}
// Before Send: true when it may go now; false when it asked something (or the answer was no).
async function sendChecks(){
  const c = state.compose, asked = $('c-ask')?.hidden === false ? $('c-ask').dataset.kind : null;
  // Send while the question shows: go ahead, as with the subject.
  if (asked === 'many') c.manyOk = true;
  if (asked === 'files') c.filesOk = true;
  hideComposeAsk();
  if (pref().ask_many !== false && c.kind === 'replyall' && !c.manyOk){
    const n = new Set(['c-to', 'c-cc', 'c-bcc'].flatMap(id => addressesIn($(id)?.value ?? '')).map(a => a.email)).size;
    if (n >= REPLY_MANY){ showComposeAsk('many', n); return false; }
  }
  if (pref().ask_attachment !== false && !c.attachments.length && !c.filesOk){
    const signature = allIdentities().find(i => i.id === c.identity)?.signature ?? '';
    let mentions = false;
    try { mentions = await invoke('mentions_attachment', {text:ownText(), signature}); } catch {}
    if (state.compose !== c) return false;
    if (mentions){ showComposeAsk('files'); return false; }
  }
  return true;
}
// A reply's quoted original folds to its first lines in the HTML editor; a click or the caret
// in it opens it. The fold is the editor's only, never saved or sent.
function foldComposeQuote(ed){
  ed.querySelectorAll(':scope > blockquote').forEach(q => { if (q.innerText.split('\n').filter(l => l.trim()).length > QUOTE_FOLD) q.classList.add('cfold'); });
  const open = () => {
    const n = getSelection().anchorNode;
    (n?.nodeType === 1 ? n : n?.parentElement)?.closest?.('blockquote.cfold')?.classList.remove('cfold');
  };
  ed.addEventListener('click', e => { e.target.closest?.('blockquote.cfold')?.classList.remove('cfold'); });
  ed.addEventListener('keyup', open);
}
const unfolded = html => html.replace(/ class="cfold"/g, '');
/* ---------- recipient suggestions ---------- */
// People come from the address book and the mail base (recipients): contacts first (marked),
// then those written to, then those who write most. Typing in To, Cc or Bcc shows the ones whose
// name or address has a word starting with what was typed since the last comma, after the
// address book's groups whose name does; ↓ ↑ choose, Enter or Tab takes one, Esc closes. A group
// goes in as its people's addresses, at once, so what is sent (and counted) is plain to see.
let RECIPS = null, GROUPS = [], recipsAsked = null;
function loadRecipients(){
  if (!recipsAsked) recipsAsked = Promise.all([invoke('recipients'), loadBook()]).then(([r]) => {
    RECIPS = r.map(withWords);
    GROUPS = (BOOK?.groups ?? []).filter(g => g.members.some(m => m.email)).map(g => ({...g, group:true, words:fold(g.name).split(/\s+/), folded:fold(g.name)}));
  }).catch(() => { recipsAsked = null; });
  return recipsAsked;
}
const forgetRecipients = () => { RECIPS = null; GROUPS = []; recipsAsked = null; };
// A group's people as an address line, leaving out those in `taken`.
const groupLine = (g, taken = new Set()) => g.members.filter(m => m.email && !taken.has(m.email.toLowerCase())).map(addressOf).join(', ');
// A group's name typed whole in To, Cc or Bcc (and left, or sent) goes in as its people.
function expandGroupNames(input){
  if (!input || !GROUPS.length || !input.value.trim()) return;
  let changed = false;
  const parts = addressesIn(input.value).map(a => {
    if (a.text.includes('@')) return a.text;
    const g = GROUPS.find(x => x.folded === fold(a.text.trim()));
    if (!g) return a.text;
    changed = true;
    return groupLine(g);
  });
  if (changed) input.value = parts.filter(Boolean).join(', ') + ', ';
}
// "Kowalska, Anna" needs quotes in an address line; a plain name doesn't.
const addressOf = p => !p.name ? p.email : /[",;<>@]/.test(p.name) ? `"${p.name.replace(/"/g, "'")}" <${p.email}>` : `${p.name} <${p.email}>`;
// Where the address being typed starts: after the last comma or semicolon outside quotes.
function tokenStart(text){
  let start = 0, quoted = false;
  for (let i = 0; i < text.length; i++){
    if (text[i] === '"') quoted = !quoted;
    else if (!quoted && (text[i] === ',' || text[i] === ';')) start = i + 1;
  }
  return start;
}
// Each person's words, folded once when the list comes: typing only compares.
const withWords = p => ({...p, words:fold(p.name + ' ' + p.email.replace(/[@.]/g, ' ')).split(/\s+/), folded:fold(p.email)});
const matchesTyped = (p, typed) => p.words.some(w => w.startsWith(typed)) || p.folded.startsWith(typed);
const sugg = {input:null, list:[], at:0, el:null};
function closeSuggestions(){
  sugg.el?.remove();
  if (sugg.input) sugg.input.removeAttribute('aria-activedescendant');
  sugg.input = null; sugg.list = []; sugg.el = null;
}
function showSuggestions(input){
  const text = input.value.slice(0, input.selectionStart ?? input.value.length);
  const typed = fold(text.slice(tokenStart(text)).trim());
  if (!typed || !RECIPS){ closeSuggestions(); return; }
  const taken = new Set(['c-to', 'c-cc', 'c-bcc'].flatMap(id => ($(id)?.value ?? '').toLowerCase().match(/[^\s<>,;"]+@[^\s<>,;"]+/g) ?? []));
  const found = GROUPS.filter(g => (g.words.some(w => w.startsWith(typed)) || g.folded.startsWith(typed)) && groupLine(g, taken)).slice(0, 3);
  for (const p of RECIPS){
    if (found.length >= 8) break;
    if (!taken.has(p.email.toLowerCase()) && matchesTyped(p, typed)) found.push(p);
  }
  if (!found.length){ closeSuggestions(); return; }
  if (sugg.input !== input) closeSuggestions();
  sugg.input = input; sugg.list = found; sugg.at = Math.min(sugg.at, found.length - 1);
  if (!sugg.el){
    sugg.el = document.createElement('ul');
    sugg.el.className = 'sugg'; sugg.el.id = 'sugg'; sugg.el.setAttribute('role', 'listbox');
    sugg.el.addEventListener('mousedown', e => {
      const li = e.target.closest('[data-i]'); if (!li) return;
      e.preventDefault(); takeSuggestion(Number(li.dataset.i));
    });
    input.closest('.cf').append(sugg.el);
    input.setAttribute('aria-controls', 'sugg');
  }
  sugg.el.style.cssText = `top:${input.offsetTop + input.offsetHeight + 2}px;left:${input.offsetLeft}px;width:${Math.max(input.offsetWidth, 260)}px`;
  sugg.el.innerHTML = found.map((p, i) => `<li role="option" id="sugg-${i}" data-i="${i}" aria-selected="${i === sugg.at}">`
    + (p.group ? `<span class="nm"><span class="ct">${I.people}</span>${esc(p.name)}</span><span class="em">${esc(t('group') + ' · ' + tn(p.members.length, '{n} person', '{n} people'))}</span>`
      : `<span class="nm">${p.contact ? `<span class="ct" title="${esc(t('In your address book'))}">${I.person}</span>` : ''}${esc(p.name || p.email)}</span>${p.name ? `<span class="em">${esc(p.email)}</span>` : ''}`)
    + '</li>').join('');
  input.setAttribute('aria-activedescendant', 'sugg-' + sugg.at);
}
function takeSuggestion(i){
  const input = sugg.input, p = sugg.list[i];
  if (!input || !p) return;
  const caret = input.selectionStart ?? input.value.length;
  const before = input.value.slice(0, caret), after = input.value.slice(caret).replace(/^[^,;]*[,;]?\s*/, '');
  const start = tokenStart(before);
  const head = before.slice(0, start).trimEnd();
  const taken = new Set(['c-to', 'c-cc', 'c-bcc'].flatMap(id => ($(id)?.value ?? '').toLowerCase().match(/[^\s<>,;"]+@[^\s<>,;"]+/g) ?? []));
  const value = (head ? head + ' ' : '') + (p.group ? groupLine(p, taken) : addressOf(p)) + ', ';
  input.value = value + after;
  input.setSelectionRange(value.length, value.length);
  closeSuggestions();
}
function wireSuggestions(input){
  input.addEventListener('input', () => { sugg.at = 0; loadRecipients().then(() => { if (document.activeElement === input) showSuggestions(input); }); });
  input.addEventListener('blur', () => setTimeout(() => { if (sugg.input === input) closeSuggestions(); if (document.activeElement !== input) expandGroupNames(input); }, 120));
  input.addEventListener('keydown', e => {
    if (sugg.input !== input || !sugg.list.length) return;
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp'){
      e.preventDefault();
      sugg.at = (sugg.at + (e.key === 'ArrowDown' ? 1 : -1) + sugg.list.length) % sugg.list.length;
      showSuggestions(input);
    } else if ((e.key === 'Enter' && !e.ctrlKey && !e.metaKey) || (e.key === 'Tab' && !e.shiftKey)){
      e.preventDefault(); e.stopPropagation(); takeSuggestion(sugg.at);
    } else if (e.key === 'Escape'){
      e.preventDefault(); e.stopPropagation(); closeSuggestions(); // Esc here doesn't close the message
    }
  });
}

// The attachments of the message being written, drawn again without touching the text.
function renderComposeAtts(){
  const box = $('c-attbox');
  if (box && state.compose) box.innerHTML = composeAttsHtml(state.compose);
}
// Files chosen in the Open dialog or dropped on the window. Dropped with no message open, they
// start a new one.
async function addFiles(files){
  if (!files?.length) return;
  if (!state.compose){
    await openCompose('new');
    if (!state.compose) return;
  }
  const c = state.compose, have = new Set(c.attachments.map(a => a.path).filter(Boolean));
  const fresh = files.filter(f => !have.has(f.path));
  c.attachments.push(...fresh);
  renderComposeAtts();
  hideComposeAsk();
  say(fresh.length === 1 ? t('Attached {name}.', {name:fresh[0].name}) : tn(fresh.length, 'Attached {n} file.', 'Attached {n} files.'), 3000);
}
async function pickFiles(){
  let files;
  try { files = await invoke('pick_files', {words:{title:t('Attach files')}}); } catch (e){ say(tr(e), 8000); return; }
  await addFiles(files);
}
// Files dragged in from Explorer. Over a folder on this PC it lights up, and dropped there they
// are read into it as mail (.eml, mbox); anywhere else they are attached to a message. The
// core says where in physical pixels.
let fileTarget = null;
function dropSpot(x, y){
  const el = document.elementFromPoint(x / (devicePixelRatio || 1), y / (devicePixelRatio || 1));
  if (!el?.closest?.('#tree')) return null;
  const f = el.closest('[data-folder]');
  return {folder:f && keepsHere(f.dataset.folder) && !['trash', 'outbox'].includes(kindOf(f.dataset.folder)) ? f : null};
}
listenHere('files-over', ({payload}) => {
  const f = payload ? dropSpot(payload[0], payload[1])?.folder ?? null : null;
  if (f === fileTarget) return;
  fileTarget?.classList.remove('dropok');
  f?.classList.add('dropok');
  fileTarget = f;
});
listenHere('files-dropped', ({payload}) => {
  fileTarget?.classList.remove('dropok'); fileTarget = null;
  const p = Array.isArray(payload) ? {files:payload} : payload; // the demo sends just the files
  if (dropIntoEditor(p)) return; // pictures into an HTML editor, also a signature's in Settings
  if (document.querySelector('dialog[open]')) return; // not while a dialog is open
  const spot = p.x != null ? dropSpot(p.x, p.y) : null;
  if (spot?.folder){ importInto(spot.folder.dataset.folder, p.files.map(f => f.path)); return; }
  if (spot){ say(t('To bring in mail files (.eml, mbox), drop them on a folder on this PC. Dropped anywhere else, files are attached to a message.'), 9000); return; }
  addFiles(p.files);
});
// The caret at the start of the line typing begins on (0, or below a template's greeting), above
// the quote, as in the plain-text editor.
function focusStart(body, line = 0){
  body.focus();
  if (body.setSelectionRange){
    const at = line ? body.value.split('\n').slice(0, line).join('\n').length + 1 : 0;
    body.setSelectionRange(at, at);
  } else {
    const r = document.createRange(); r.setStart(body.children[line] ?? body, 0); r.collapse(true);
    const sel = getSelection(); sel.removeAllRanges(); sel.addRange(r);
  }
  body.scrollTop = 0;
}
// The HTML editor: formatting buttons, Ctrl+K for a link, colors and sizes from the palette,
// pictures, pasted HTML cleaned by the core. `bar` is its formatting bar (compose's by default; a
// signature editor has its own, drawn again with the dialog); the editor's own listeners go on once.
function wireEditor(ed, bar = previewEl.querySelector('.fbar'), {signature = false} = {}){
  document.execCommand('defaultParagraphSeparator', false, 'div');
  document.execCommand('styleWithCSS', false, false);
  const box = bar.querySelector('.flink'), url = box.querySelector('input');
  let kept = null; // the selection while the link box has the focus
  bar.addEventListener('mousedown', e => { if (e.target.closest('.fb')) e.preventDefault(); }); // keeps the selection
  bar.addEventListener('click', e => {
    const b = e.target.closest('[data-cmd]'); if (!b) return;
    ed.focus();
    if (b.dataset.cmd === 'link') openLinkBox();
    else if (MENU_CMDS.has(b.dataset.cmd)) paletteMenu(ed, b, b.dataset.cmd);
    else if (b.dataset.cmd === 'picture') choosePictures(ed, signature);
    else if (b.dataset.cmd === 'quote') document.execCommand('formatBlock', false, document.queryCommandValue('formatBlock') === 'blockquote' ? 'div' : 'blockquote');
    else document.execCommand(b.dataset.cmd);
  });
  function openLinkBox(){
    const sel = getSelection();
    kept = sel.rangeCount ? sel.getRangeAt(0).cloneRange() : null;
    const text = sel.toString().trim(), a = sel.anchorNode?.parentElement?.closest('a');
    url.value = a?.getAttribute('href') ?? (/^https?:\/\//i.test(text) ? text : '');
    box.hidden = false; url.focus(); url.select();
  }
  function closeLinkBox(apply){
    box.hidden = true; ed.focus();
    if (kept){ const sel = getSelection(); sel.removeAllRanges(); sel.addRange(kept); }
    if (!apply) return;
    let href = url.value.trim();
    if (!href){ document.execCommand('unlink'); return; }
    if (!/^(https?:|mailto:)/i.test(href)) href = (href.includes('@') && !href.includes('/') ? 'mailto:' : 'https://') + href;
    if (getSelection().isCollapsed) document.execCommand('insertHTML', false, `<a href="${esc(href)}">${esc(href.replace(/^mailto:/i, ''))}</a>`);
    else document.execCommand('createLink', false, href);
  }
  url.addEventListener('keydown', e => {
    if (e.key !== 'Enter' && e.key !== 'Escape') return;
    e.preventDefault(); e.stopPropagation(); // Esc here doesn't close the message
    closeLinkBox(e.key === 'Enter');
  });
  ed.linkBox = openLinkBox; // the bar drawn last
  if (ed.dataset.wired) return;
  ed.dataset.wired = '1';
  ed.addEventListener('keydown', e => {
    if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 'k'){ e.preventDefault(); ed.linkBox(); }
  });
  // A pasted picture goes in made the right size. A pasted web page brings styles, scripts and
  // pictures: only what Telex sends goes in, and pictures from the web are left out, not fetched
  // (fetching would tell their server; sending their addresses would let it see who reads).
  ed.addEventListener('paste', async e => {
    const pictures = [...(e.clipboardData?.files ?? [])].filter(f => f.type.startsWith('image/'));
    const pasted = e.clipboardData?.getData('text/html'), plain = e.clipboardData?.getData('text/plain') ?? '';
    if (pictures.length){
      e.preventDefault();
      const read = await Promise.all(pictures.map(f => fileData(f).catch(() => null)));
      await insertPictures(ed, read.filter(Boolean).map(data => ({data})), signature);
      return;
    }
    if (!pasted) return;
    e.preventDefault();
    const remote = (pasted.match(/<img\b[^>]*\bsrc\s*=\s*["']?\s*https?:/gi) ?? []).length;
    try { document.execCommand('insertHTML', false, await invoke('clean_html', {html:pasted})); }
    catch { document.execCommand('insertText', false, plain); }
    if (remote) say(tn(remote, 'A picture from the web was left out: save it, then use Insert picture.', '{n} pictures from the web were left out: save them, then use Insert picture.'), 9000);
  });
}
// Files dropped from Explorer onto an HTML editor (compose's, or a signature's in Settings): the
// pictures go in where they were dropped. True when it took them; other files are the caller's.
function dropIntoEditor(p){
  const dpr = devicePixelRatio || 1, x = p.x / dpr, y = p.y / dpr;
  const ed = p.x != null ? document.elementFromPoint(x, y)?.closest?.('.hedit[contenteditable="true"]') : null;
  const pictures = p.files.filter(f => isPicture(f.name));
  if (!ed || !pictures.length) return false;
  const r = document.caretRangeFromPoint?.(x, y);
  ed.focus();
  if (r && ed.contains(r.startContainer)){ const sel = getSelection(); sel.removeAllRanges(); sel.addRange(r); }
  insertPictures(ed, pictures.map(f => ({path:f.path})), ed.dataset.signature === '1');
  const others = p.files.filter(f => !isPicture(f.name));
  if (others.length && ed.id === 'c-html') addFiles(others);
  return true;
}
/* ---------- spelling ---------- */
// Telex's own checker (spell.rs), in the languages of Settings › Writing: the words none of them
// knows get a red wavy line. A text field can't style its words, so in inputs and textareas a
// layer over the field (the same face, wrapping and scroll, and no pointer) draws the lines; the
// HTML editor uses the CSS Custom Highlight API. The lines move with the text as it is typed, and
// the word being typed gets its line only once the caret leaves it. A click on a marked word
// shows its suggestions under it; right-click, the menu key or F7 (the next one) open them as a
// menu. Compose's fields, the signatures and the away reply are checked; the webview's own
// checker (Windows' languages only) is off everywhere (spellcheck="false" on <body>).
const spellOf = new WeakMap(); // field → {seq, timer, text, found: [{s, l, word, range?}], typing}
const spellLive = new Set(); // the fields wired, while they are on the page
const spellIgnored = new Set(); // "Ignore", for this session
const spellSuggested = new Map(); // word → suggestions, until the languages change
const spellMarks = window.Highlight && CSS.highlights ? new Highlight() : null;
if (spellMarks) CSS.highlights.set('spell', spellMarks);
const spellFields = () => ['c-subj', 'c-body', 'c-html'].map($).filter(Boolean);
// An HTML editor (compose's or a signature's): its marks are highlights, not a layer.
const rich = el => el.getAttribute?.('contenteditable') === 'true';
const spellResize = new ResizeObserver(entries => entries.forEach(e => syncLayer(e.target)));
// Telex's checking for one field. An input or textarea gets a box with the layer of lines.
function spellField(el){
  if (!el || spellOf.has(el)) return;
  if (!rich(el) && !el.parentElement.classList.contains('spellfield')){
    const box = document.createElement('div');
    box.className = 'spellfield';
    el.before(box); box.append(el);
    box.insertAdjacentHTML('beforeend', '<div class="spellmarks" aria-hidden="true"></div>');
  }
  spellOf.set(el, {seq:0, timer:0, text:'', found:[], typing:false});
  spellLive.add(el);
  el.addEventListener('input', () => { hideSpellTip(); spellEdited(el); scheduleSpelling(el); });
  el.addEventListener('contextmenu', e => spellMenuAt(el, e));
  el.addEventListener('keydown', e => spellKeys(el, e));
  // The caret moved by hand: the word it was in may get its line now.
  el.addEventListener('pointerdown', () => { spellOf.get(el).typing = false; });
  el.addEventListener('click', () => { paintSpelling(el); spellTipAt(el); });
  el.addEventListener('blur', () => { spellOf.get(el).typing = false; paintSpelling(el); hideSpellTip(); });
  if (!rich(el)){
    // An input scrolls sideways with the caret, without a scroll event.
    for (const ev of ['scroll', 'keyup', 'click', 'select']) el.addEventListener(ev, () => scrollLayer(el));
    el.addEventListener('scroll', hideSpellTip);
    spellResize.observe(el);
  } else el.addEventListener('scroll', hideSpellTip);
  scheduleSpelling(el, 0);
}
// Fields that left the page are let go.
function spellPrune(){
  for (const el of spellLive) if (!el.isConnected){ spellLive.delete(el); spellResize.unobserve(el); }
}
function wireSpelling(){
  spellPrune();
  spellFields().forEach(spellField);
  spellNudge();
}
const respell = () => {
  spellPrune(); spellSuggested.clear();
  spellLive.forEach(el => scheduleSpelling(el, 0));
};
// Compose closed: its suggestions go, and the editor's marks let go of its words.
function leftCompose(){
  closeSuggestions();
  hideSpellTip();
  spellMarks?.clear();
}
function scheduleSpelling(el, ms = 400){
  const st = spellOf.get(el);
  if (!st) return;
  clearTimeout(st.timer);
  st.timer = setTimeout(() => checkSpelling(el), ms);
}
// The HTML editor's words as one text, each text node on a line of its own (quotes and links
// left out), and where each node starts in it.
function editorText(ed, skip = 'blockquote, a'){
  const nodes = [];
  let text = '';
  const walker = document.createTreeWalker(ed, NodeFilter.SHOW_TEXT);
  for (let n = walker.nextNode(); n; n = walker.nextNode()){
    if (n.parentElement.closest(skip)) continue;
    if (text) text += '\n';
    nodes.push({node:n, at:text.length});
    text += n.data;
  }
  return {text, nodes};
}
async function checkSpelling(el){
  const st = spellOf.get(el);
  if (!st || !el.isConnected) return;
  const seq = ++st.seq, html = rich(el);
  const {text, nodes} = html ? editorText(el) : {text:el.value, nodes:null};
  let found = [];
  if (pref().spell_check && text.trim()){
    try { found = await invoke('spell_check', {text}); } catch { return; }
  }
  if (seq !== st.seq || !el.isConnected) return;
  st.text = text;
  st.found = found.map(([s, l]) => ({s, l, word:text.slice(s, s + l)})).filter(m => !spellIgnored.has(m.word)).sort((a, b) => a.s - b.s);
  if (html) for (const m of st.found){
    const at = nodes.findLast(n => n.at <= m.s);
    if (!at || m.s + m.l > at.at + at.node.data.length) continue;
    m.range = new Range();
    m.range.setStart(at.node, m.s - at.at); m.range.setEnd(at.node, m.s - at.at + m.l);
  }
  paintSpelling(el);
}
// Typed: the lines stay on their words until the next check. In a text field the edit is found
// by what the text before and after it kept; a mistake it touched goes. The editor's ranges
// follow the text by themselves; one whose word changed goes.
function spellEdited(el){
  const st = spellOf.get(el);
  st.typing = true;
  if (!st.found.length) return;
  if (rich(el)){
    st.found = st.found.filter(m => m.range && m.range.toString() === m.word);
  } else {
    const old = st.text, now = el.value, max = Math.min(old.length, now.length);
    let p = 0, q = 0;
    while (p < max && old[p] === now[p]) p++;
    while (q < max - p && old[old.length - 1 - q] === now[now.length - 1 - q]) q++;
    const end = old.length - q, d = now.length - old.length;
    st.found = st.found.filter(m => m.s + m.l < p || m.s > end).map(m => m.s > end ? {...m, s:m.s + d} : m);
    st.text = now;
  }
  paintSpelling(el);
}
// The mistake the caret is in or at the end of, in the text the checker got.
function mistakeAtCaret(el){
  const found = spellOf.get(el)?.found ?? [];
  if (rich(el)){
    const sel = getSelection();
    if (!sel.rangeCount || !el.contains(sel.anchorNode)) return null;
    return found.find(m => { try { return m.range?.comparePoint(sel.anchorNode, sel.anchorOffset) === 0; } catch { return false; } }) ?? null;
  }
  const at = el.selectionStart ?? -1;
  return found.find(m => at >= m.s && at <= m.s + m.l) ?? null;
}
// Draw the lines, leaving out the word being typed.
function paintSpelling(el){
  const st = spellOf.get(el);
  if (!st || !el.isConnected) return;
  const held = st.typing && document.activeElement === el ? mistakeAtCaret(el) : null;
  const shown = st.found.filter(m => m !== held);
  if (rich(el)){
    if (!spellMarks) return;
    // One highlight for every editor: only this one's ranges change.
    for (const r of st.painted ?? []) spellMarks.delete(r);
    st.painted = shown.map(m => m.range).filter(Boolean);
    for (const r of st.painted) spellMarks.add(r);
  } else paintLayer(el, shown);
}
function paintLayer(el, found){
  const layer = el.parentElement.querySelector('.spellmarks');
  if (!layer) return;
  const text = el.value;
  let html = '', at = 0;
  for (const m of found){
    if (m.s < at || text.slice(m.s, m.s + m.l) !== m.word) continue;
    html += esc(text.slice(at, m.s)) + `<mark data-s="${m.s}">` + esc(m.word) + '</mark>';
    at = m.s + m.l;
  }
  // A last empty line still takes room in a textarea.
  layer.innerHTML = html + esc(text.slice(at)) + (el.tagName === 'TEXTAREA' ? '\n ' : '');
  syncLayer(el);
}
// The layer covers the field's text box exactly: inside its border, beside its scroll bar. An
// input puts its line in the middle of its height; its layer reaches a little lower, so the wavy
// line under a word isn't cut off.
function syncLayer(el){
  const layer = el.parentElement?.querySelector('.spellmarks');
  if (!layer) return;
  const cs = getComputedStyle(el), input = el.tagName === 'INPUT';
  const inner = el.clientHeight - parseFloat(cs.paddingTop) - parseFloat(cs.paddingBottom);
  Object.assign(layer.style, {left:el.offsetLeft + el.clientLeft + 'px', top:el.offsetTop + el.clientTop + 'px',
    width:el.clientWidth + 'px', height:el.clientHeight + (input ? 3 : 0) + 'px', padding:cs.padding, font:cs.font, letterSpacing:cs.letterSpacing,
    lineHeight:input ? inner + 'px' : cs.lineHeight, tabSize:cs.tabSize, textIndent:cs.textIndent});
  scrollLayer(el);
}
function scrollLayer(el){
  const layer = el.parentElement?.querySelector('.spellmarks');
  if (layer){ layer.scrollTop = el.scrollTop; layer.scrollLeft = el.scrollLeft; }
}
// Where a mistake is on screen.
function mistakeRect(el, m){
  if (rich(el)) return m.range?.getClientRects()[0] ?? null;
  return el.parentElement.querySelector(`.spellmarks mark[data-s="${m.s}"]`)?.getClientRects()[0] ?? null;
}
const within = (r, x, y) => x >= r.left && x <= r.right && y >= r.top && y <= r.bottom;
// The marked word under the pointer, if there is one.
function spellHit(el, x, y){
  const found = spellOf.get(el)?.found ?? [];
  if (rich(el)) return found.find(m => m.range && [...m.range.getClientRects()].some(r => within(r, x, y)));
  const mark = [...(el.parentElement.querySelector('.spellmarks')?.querySelectorAll('mark') ?? [])]
    .find(mk => [...mk.getClientRects()].some(r => within(r, x, y)));
  return mark ? found.find(m => m.s === Number(mark.dataset.s)) : null;
}
async function suggestionsFor(word){
  if (!spellSuggested.has(word)){
    let list = [];
    try { list = await invoke('spell_suggest', {word}); } catch {}
    spellSuggested.set(word, list);
  }
  return spellSuggested.get(word);
}
// A click on a marked word: its suggestions in a small strip under it. The focus stays in the
// field, so typing goes on; typing, Esc or a click elsewhere closes it.
const spellTip = document.createElement('div');
spellTip.className = 'spelltip'; spellTip.hidden = true; spellTip.setAttribute('role', 'toolbar');
document.body.append(spellTip);
let spellTipFor = null;
async function spellTipAt(el){
  const sel = rich(el) ? getSelection() : null;
  if (sel ? !sel.isCollapsed : el.selectionStart !== el.selectionEnd) return hideSpellTip();
  const m = mistakeAtCaret(el);
  if (!m || spellTipFor?.m === m) return m ? null : hideSpellTip();
  spellTipFor = {el, m};
  const list = await suggestionsFor(m.word);
  if (spellTipFor?.m !== m) return;
  const r = mistakeRect(el, m);
  if (!r) return hideSpellTip();
  spellTip.setAttribute('aria-label', t('Spelling: {word}', {word:m.word}));
  spellTip.innerHTML = (list.length ? list.slice(0, 5).map((s, i) => `<button type="button" class="sfix" data-fix="${i}">${esc(s)}</button>`).join('')
    : `<span class="snone">${esc(t('No suggestions'))}</span>`)
    + `<span class="ssep"></span><button type="button" data-add>${esc(t('Add to dictionary'))}</button><button type="button" data-ignore>${esc(t('Ignore|spelling'))}</button>`;
  spellTip.hidden = false;
  const w = spellTip.offsetWidth, h = spellTip.offsetHeight;
  spellTip.style.left = Math.max(4, Math.min(r.left - 6, innerWidth - w - 4)) + 'px';
  spellTip.style.top = (r.bottom + 4 + h > innerHeight - 4 ? r.top - h - 4 : r.bottom + 4) + 'px';
  spellTip.list = list;
}
function hideSpellTip(){
  spellTipFor = null;
  spellTip.hidden = true;
}
// The field keeps the focus: the strip's buttons don't take it.
spellTip.addEventListener('pointerdown', e => e.preventDefault());
spellTip.addEventListener('click', e => {
  const b = e.target.closest('button');
  if (!b || !spellTipFor) return;
  const {el, m} = spellTipFor, list = spellTip.list ?? [];
  hideSpellTip();
  if (b.dataset.fix) fixMistake(el, m, list[Number(b.dataset.fix)]);
  else if ('add' in b.dataset) addWord(el, m.word);
  else if ('ignore' in b.dataset) ignoreWord(el, m.word);
});
document.addEventListener('pointerdown', e => { if (!spellTip.hidden && !spellTip.contains(e.target) && e.target !== spellTipFor?.el) hideSpellTip(); }, true);
addEventListener('blur', hideSpellTip);
addEventListener('resize', hideSpellTip);
function spellMenuAt(el, e){
  const m = spellHit(el, e.clientX, e.clientY);
  if (!m) return; // the browser's own menu: cut, copy, paste
  e.preventDefault();
  hideSpellTip();
  openSpellMenu(el, m, e.clientX, e.clientY);
}
async function openSpellMenu(el, m, x, y){
  const list = await suggestionsFor(m.word);
  const items = list.length ? list.map(s => ({label:s, run:() => fixMistake(el, m, s)}))
    : [{label:t('No suggestions'), disabled:true, run(){}}];
  items.push({sep:true},
    {label:t('Add “{word}” to the dictionary', {word:m.word}), run:() => addWord(el, m.word)},
    {label:t('Ignore|spelling'), run:() => ignoreWord(el, m.word)});
  openMenu(x, y, items, el);
}
// The word as the menu found it, if the text still has it there.
function selectWord(el, m){
  el.focus();
  if (rich(el)){
    if (!m.range || m.range.toString() !== m.word) return false;
    const sel = getSelection(); sel.removeAllRanges(); sel.addRange(m.range);
    return true;
  }
  if (el.value.slice(m.s, m.s + m.l) !== m.word) return false;
  el.setSelectionRange(m.s, m.s + m.l);
  return true;
}
function fixMistake(el, m, word){
  // insertText keeps Ctrl+Z working, and the input event checks again. The word put in is done:
  // its line, if any, shows at once.
  if (selectWord(el, m)){ document.execCommand('insertText', false, word); spellOf.get(el).typing = false; }
}
function ignoreWord(el, word){
  spellIgnored.add(word);
  el.focus();
  respell();
}
async function addWord(el, word){
  el.focus();
  try { await invoke('spell_add', {word}); } catch (e){ say(tr(e), 8000); return; }
  respell();
  say(t('“{word}” is in your dictionary now.', {word}));
}
// Where the selection starts (or ends), in the text the checker got.
function caretAt(el, end){
  if (!rich(el)) return (end ? el.selectionEnd : el.selectionStart) ?? 0;
  const sel = getSelection();
  if (!sel.rangeCount) return 0;
  const r = sel.getRangeAt(0), node = end ? r.endContainer : r.startContainer;
  const n = editorText(el).nodes.find(x => x.node === node);
  return n ? n.at + (end ? r.endOffset : r.startOffset) : 0;
}
function menuForMistake(el, m){
  if (!selectWord(el, m)) return;
  spellOf.get(el).typing = false; paintSpelling(el);
  const r = mistakeRect(el, m);
  if (r) openSpellMenu(el, m, r.left, r.bottom + 2);
}
function spellKeys(el, e){
  const found = spellOf.get(el)?.found ?? [];
  if (e.key === 'Escape' && !spellTip.hidden){
    e.preventDefault(); e.stopPropagation();
    hideSpellTip();
  } else if (e.key === 'F7' && !e.ctrlKey && !e.altKey){
    // The next mistake after the caret, then from the top.
    e.preventDefault(); e.stopPropagation();
    hideSpellTip();
    const at = caretAt(el, true), next = found.find(m => m.s >= at) ?? found[0];
    if (next) menuForMistake(el, next); else say(t('No spelling mistakes here.'));
  } else if (e.key === 'ContextMenu' || (e.key === 'F10' && e.shiftKey)){
    const m = mistakeAtCaret(el);
    if (!m) return; // the browser's own menu
    e.preventDefault(); e.stopPropagation();
    hideSpellTip();
    menuForMistake(el, m);
  } else if (/^(Arrow|Home|End|Page)/.test(e.key)){
    // The caret moves away from the word typed: its line may show.
    const st = spellOf.get(el);
    if (st?.typing){ st.typing = false; setTimeout(() => paintSpelling(el)); }
    hideSpellTip();
  }
}
// The other fields where people write prose: the away reply (signatures get theirs as they are drawn).
['away-body'].map($).forEach(spellField);
// Once a session, when the language Telex speaks has no dictionary yet but one can be fetched.
let spellNudged = false;
async function spellNudge(){
  if (spellNudged || !pref().spell_check || VIEWER) return;
  spellNudged = true;
  let info;
  try { info = await invoke('spell_languages'); } catch { return; }
  const missing = info.missing?.[0];
  if (missing) say(t('{language} spelling isn’t on this PC yet: Settings › Writing can fetch it.', {language:langName(missing)}), 9000);
}
// Compose's bar says which languages spelling is checked in: all of them at once, a word any of
// them knows being right. Its menu ticks them as Settings › Writing does.
async function spellLanguagesNow(){
  if (!spellInfo){ try { spellInfo = await invoke('spell_languages'); } catch { return null; } }
  return spellInfo;
}
async function paintSpellChip(){
  if (!previewEl.querySelector('.spellchip')) return;
  const on = pref().spell_check, info = on ? await spellLanguagesNow() : null;
  const b = previewEl.querySelector('.spellchip');
  if (!b) return;
  const codes = [...new Set((info?.chosen ?? []).map(tag => tag.split('-')[0].toUpperCase()))];
  b.querySelector('span').textContent = !on ? t('Spelling off') : codes.length ? codes.join(' · ') : t('No dictionary');
  b.title = !on ? t('Spelling isn’t checked. Click to turn it on.')
    : codes.length ? t('Spelling is checked in {languages} at once: a word any of them knows is right. Click to choose.', {languages:info.chosen.map(langName).join(', ')})
    : t('There is no spelling dictionary for your languages yet. Click to fetch one.');
}
async function spellLanguageMenu(btn){
  const info = await spellLanguagesNow(), on = pref().spell_check;
  const chosen = new Set(info?.chosen ?? []);
  const set = list => changeSettings({spell_languages:list}).then(() => { spellInfo = null; respell(); paintSpellChip(); });
  const items = (info?.languages ?? []).map(l => ({label:langName(l.tag), checked:on && chosen.has(l.tag), disabled:!on,
    run:() => { const next = new Set(chosen); if (next.has(l.tag)) next.delete(l.tag); else next.add(l.tag); set([...next]); }}));
  if (items.length) items.push({label:t('Automatic: the languages of Windows and of Telex'), checked:on && !!info?.auto, disabled:!on || !!info?.auto, run:() => set([])}, {sep:true});
  items.push({label:t('Underline spelling mistakes'), checked:on, run:() => changeSettings({spell_check:!on}).then(() => { respell(); paintSpellChip(); })},
    {label:t('Dictionaries…'), run:() => openSettings('write')});
  const r = btn.getBoundingClientRect();
  openMenu(r.left, r.top, items, btn);
  menuEl.style.top = Math.max(4, r.top - menuEl.offsetHeight - 4) + 'px'; // above the button
}
// "pl-PL" → "polski (Polska)", in the interface's language.
function langName(tag){
  try { return new Intl.DisplayNames([I18n.lang()], {type:'language'}).of(tag) ?? tag; } catch { return tag; }
}

// Switch the message between plain text and HTML. Plain text drops the formatting.
async function setFormat(fmt){
  const c = readCompose();
  try {
    if (fmt === 'html' && c.html == null) c.html = await invoke('text_to_html', {text:c.body});
    else if (fmt === 'plain' && c.html != null){
      c.body = await invoke('html_to_text', {html:c.html}); c.html = null;
      say(t('Plain text now: the formatting is gone.'));
    } else return;
  } catch (e){ say(tr(e), 8000); return; }
  if (state.compose !== c) return;
  renderCompose();
  focusStart(c.html != null ? $('c-html') : $('c-body'));
}
const draftOf = c => ({account:c.account, to:c.to, cc:c.cc, bcc:c.bcc, subject:c.subject || t('(no subject)'),
  body:c.html != null ? '' : c.body.replace(/^\n+/, ''), html:c.html,
  attachments:c.attachments, reply_to:c.replyTo, forward_of:c.forwardOf, replaces:c.replaces, autosaved:c.autosaved ?? null, identity:c.identity});
// What a save would keep: an autosave happens only when this changed.
const shapeOf = c => JSON.stringify([c.identity, c.to, c.cc, c.bcc, c.subject, c.html ?? c.body, c.attachments.map(a => a.path ?? a.name)]);
// Saves of the message being written go one at a time, so each knows the draft the one before
// made (the autosave it replaces).
let draftSaves = Promise.resolve();
function inTurn(fn){ const p = draftSaves.then(fn); draftSaves = p.catch(() => {}); return p; }
// Autosave: while a message is being written, what changed is kept every few seconds as a draft
// of its own, each replacing the one before, so a crash or a frozen window loses little. The
// draft it was opened from stays as it was until Esc, Save draft or Send. Autosaves stay on
// this PC; what the user saves goes to the server's Drafts too (the core keeps the draft's
// Message-ID and thread from the one it was opened from).
const AUTOSAVE_MS = 15000;
async function autosave(){
  const c = state.compose;
  if (!c || !$('cform') || shapeOf(readCompose()) === c.saved) return;
  try {
    await inTurn(async () => {
      if (state.compose !== c || c.done) return; // closed meanwhile, and saved or discarded
      const shape = shapeOf(c);
      c.autosaved = await invoke('save_outgoing', {draft:{...draftOf(c), autosave:true}, pgp:pgpChoice(c)});
      c.saved = shape;
      const note = previewEl.querySelector('.csaved');
      if (note && state.compose === c) note.textContent = t('Draft saved {time}', {time:hm(new Date())});
    });
  } catch { return; } // quietly: the next change tries again, and Esc still saves
  draftsChanged();
}
setInterval(autosave, AUTOSAVE_MS);
// Saves the message being written as a draft, in place of the draft it was edited from and its
// last autosave. Resolves to true when it worked; the message stays on screen.
function saveCompose(){
  const c = readCompose();
  return inTurn(async () => {
    await call('save_outgoing', {draft:draftOf(c), pgp:pgpChoice(c)});
    c.done = true;
    return true;
  }).catch(() => false);
}
// Discard: the message closes unsaved and its autosave goes; Z brings it back.
async function discardCompose(){
  const c = readCompose();
  state.compose = null;
  leftCompose();
  renderPreview();
  listEl.focus();
  const autosaved = c.autosaved;
  await inTurn(async () => {
    c.done = true;
    if (c.autosaved != null) await invoke('discard_draft', {id:c.autosaved});
    c.autosaved = null;
  }).catch(() => {});
  if (autosaved != null) draftsChanged();
  setUndo({kind:'discard', compose:c});
  say(t('Message discarded. Press Z to undo.'));
}
// Z after Discard: the message comes back as it was (one being written meanwhile goes to Drafts).
async function restoreCompose(c){
  setUndo(null);
  const kept = state.compose;
  if (kept && !await saveCompose()) return;
  Object.assign(c, {done:false, autosaved:null, saved:null}); // its autosave went: the next one makes it again
  state.compose = c;
  renderCompose();
  if (kept) draftsChanged();
  say(kept ? t('The message is back. The one you were writing is in Drafts.') : t('The message is back.'));
}
const trimList = s => s.trim().replace(/[,;]\s*$/, '');
function readCompose(){
  const c = state.compose;
  c.to = trimList($('c-to').value); c.cc = trimList($('c-cc').value); c.subject = $('c-subj').value.trim();
  if ($('c-bcc')) c.bcc = trimList($('c-bcc').value);
  if (c.html != null) c.html = unfolded($('c-html').innerHTML); else c.body = $('c-body').value;
  return c;
}
// Saves the compose content as a draft and closes it. Resolves to true when it worked.
async function closeCompose(){
  if (!await saveCompose()) return false;
  state.compose = null;
  leftCompose();
  renderPreview();
  await refresh();
  listEl.focus();
  return true;
}
const sentText = sent => sent.length === 1 ? t('Sent {what}.', {what:sentLine(sent[0])}) : sent.length ? tn(sent.length, 'Sent {n} message.', 'Sent {n} messages.') : '';
// Real mail goes to the Outbox and out at once. It's only called sent once the SMTP
// server has taken it; otherwise it waits in the Outbox and the status says why.
let askedSubject = 0; // when Send last asked about a missing subject
// Send later: `c.later` is the time a message edited from the Outbox was to go, kept by Send.
const laterKept = c => c.later && parseDate(c.later) - Date.now() > 60000 ? c.later : null;
const laterLabel = c => laterKept(c) ? t('Send {when}', {when:fmtGoes(parseDate(c.later))}) : t('Send');
const LATER_NAMES = {hour:N_('In an hour'), tomorrow:N_('Tomorrow morning'), monday:N_('Monday morning')};
// The clock next to Send (or Ctrl+Shift+Enter): when the message goes, then it waits in the Outbox.
async function laterMenu(from = $('c-when-btn')){
  let choices;
  try { choices = await invoke('send_later_choices'); } catch (e){ say(tr(e), 8000); return; }
  const c = state.compose;
  if (!c || !from) return;
  const items = [
    ...(laterKept(c) ? [{label:t('Send now'), run:() => send(null)}, {sep:true}] : []),
    ...choices.map(ch => {
      const d = parseDate(ch.when), today = d.toDateString() === new Date().toDateString();
      return {label:t(LATER_NAMES[ch.kind]), key:today ? hm(d) : t(DOW[d.getDay()]) + ' ' + hm(d), run:() => send({kind:ch.kind})};
    }),
    {sep:true},
    {label:t('Pick a date and time…'), run:pickLater},
  ];
  const r = from.getBoundingClientRect();
  openMenu(r.left, r.bottom, items, from);
  menuEl.style.top = Math.max(4, r.top - menuEl.offsetHeight - 4) + 'px'; // above the bar
}
// A time of the user's own: the line above the buttons, a minute from now at the earliest.
function pickLater(){
  const box = $('c-later'), input = $('c-when'), c = state.compose;
  if (!box || !c) return;
  const local = d => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${hm(d)}`;
  const tomorrow = new Date(); tomorrow.setDate(tomorrow.getDate() + 1); tomorrow.setHours(8, 0, 0, 0);
  input.min = local(new Date(Date.now() + 60000));
  input.value = laterKept(c) ? c.later.slice(0, 16) : local(tomorrow);
  box.hidden = false;
  input.focus();
}
function sendAtPicked(){
  const v = $('c-when')?.value;
  if (!v){ say(t('Pick a date and time.')); $('c-when')?.focus(); return; }
  send({kind:'at', at:v.length === 16 ? v + ':00' : v});
}
// later: undefined sends as the message says (now, or at the time it was to go), null sends now,
// {kind, at} sends later.
async function send(later){
  const sc = state.compose;
  [$('c-to'), $('c-cc'), $('c-bcc')].forEach(expandGroupNames);
  if (!sc) return;
  // Send again after a question goes the way the first press chose.
  if (later === undefined) later = sc.askLater !== undefined ? sc.askLater : laterKept(sc) ? {kind:'at', at:sc.later} : null;
  sc.askLater = later;
  if (![$('c-to'), $('c-cc'), $('c-bcc')].some(f => trimList(f?.value ?? ''))){ say(t('Add at least one recipient in To.')); $('c-to').focus(); return; }
  if (pref().ask_subject && !$('c-subj').value.trim() && Date.now() - askedSubject > 8000){
    askedSubject = Date.now();
    say(t('No subject. Press Send or Ctrl+Enter again to send it without one.'), 8000);
    $('c-subj').focus();
    return;
  }
  askedSubject = 0;
  if (!await pgpReady()) return;
  if (!await sendChecks()) return;
  const btn = document.querySelector('#cform .csend');
  if (!btn || btn.disabled) return;
  const label = btn.querySelector('span');
  btn.disabled = true; label.textContent = later ? t('Saving…') : t('Sending…');
  say(later ? t('Saving…') : t('Sending…'), 0);
  let r;
  const c = readCompose();
  try { r = await inTurn(async () => { const r = await call('send_message', {draft:draftOf(c), later, pgp:pgpChoice(c)}); c.done = true; return r; }); }
  catch { btn.disabled = false; label.textContent = laterLabel(c); return; } // e.g. a bad address: fix it and try again
  state.compose = null;
  leftCompose();
  forgetRecipients(); // someone new may be in it
  // Sent later: it waits in the Outbox, where it says when it goes.
  const scheduled = r.goes ? t('In the Outbox: it goes {when}. E changes it.', {when:fmtFull(parseDate(r.goes))}) : '';
  if (VIEWER){
    const sent = sentText(r.sent);
    tellMain(scheduled ? {say:scheduled, ms:8000}
      : r.wait ? {say:t('Sending in {n} s. Press Z to undo.', {n:r.wait}), ms:r.wait * 1000 + 500, undo:{kind:'send', id:r.queued, until:Date.now() + r.wait * 1000}}
      : {say:r.errors.length ? [sent, r.errors.join(' · ')].filter(Boolean).join(' ') : sent || t('In the Outbox. It goes out with the next check.'), ms:r.errors.length ? 15000 : 6000});
    closeViewer();
    return;
  }
  renderPreview();
  await refresh();
  listEl.focus();
  if (scheduled){ say(scheduled, 8000); return; }
  if (r.wait){ // it waits a moment in the Outbox, so Z can still take it back
    setUndo({kind:'send', id:r.queued, until:Date.now() + r.wait * 1000});
    say(t('Sending in {n} s. Press Z to undo.', {n:r.wait}), r.wait * 1000 + 500);
    return;
  }
  const sent = sentText(r.sent);
  if (r.errors.length) say([sent, r.errors.join(' · ')].filter(Boolean).join(' '), 15000);
  else say(sent || t('In the Outbox. It goes out with the next check.'), 6000);
}
// Mail that waited its moment has gone out (or not): what the SMTP server said.
listenMain('outbox-sent', ({payload: r}) => {
  if (state.undo?.kind === 'send' && state.undo.id === r.queued) setUndo(null);
  refresh();
  const sent = sentText(r.sent);
  if (r.errors.length) say([sent, r.errors.join(' · ')].filter(Boolean).join(' '), 15000);
  else if (sent) say(sent, 6000);
});
// A message open in its own window changed something, or sent mail: say so here, where Z works.
listenMain('viewer-did', ({payload: p}) => {
  if (p?.undo) setUndo(p.undo);
  if (p?.say) say(p.say, p.ms ?? 6000);
  refresh(); schedulePush();
});
// Z within the moment: the message stays in the Outbox, held, and opens again to be changed.
async function unsend(u){
  setUndo(null);
  if (Date.now() >= u.until){ say(t('Too late to undo: the message is on its way.')); return; }
  try { await invoke('hold_message', {id:u.id}); } catch (e){ say(tr(e), 8000); return; }
  await refresh();
  await openCompose('edit', {}, u.id);
  say(t('Not sent. Change it and send it again, or press Esc to keep it as a draft.'), 8000);
}

/* ---------- actions ---------- */
// F, U and Del work on every picked message, or on the selected one.
function toggleFlag(){
  if (state.picked.size > 1){
    const rows = pickedRows(), flagged = !rows.length || rows.some(r => !r.flagged), n = state.picked.size;
    call('set_flagged_many', {ids:chosen(), flagged}).then(() => {
      say(flagged ? tn(n, 'Flagged {n} message.', 'Flagged {n} messages.', {n:num(n)}) : tn(n, 'Removed the flag from {n} message.', 'Removed the flag from {n} messages.', {n:num(n)}), 3000);
      refresh();
    }).catch(() => {});
    return;
  }
  const m = current(); if (m) setFlagged(m, !m.flagged);
}
// One message flagged or not: F, the flag in its row, the menu.
function setFlagged(m, flagged){
  // The flag button above the message, without drawing the message again.
  const shown = on => {
    const b = m.id === current()?.id && previewEl.querySelector('.pflag');
    if (!b) return;
    b.setAttribute('aria-pressed', String(on));
    b.title = flagTip({flagged:on}); b.setAttribute('aria-label', b.title);
  };
  changed(m, {flagged}); shown(flagged);
  call('set_flagged', {id:m.id, flagged})
    .then(() => { say(flagged ? t('Flagged.') : t('Flag removed.'), 2000); reorder(); })
    .catch(() => { changed(m, {flagged:!flagged}); shown(!flagged); });
}
// One message read or unread: U, the dot in its row, the envelope above it, the menu. Marked
// unread by hand, it stays so while it is on screen.
function setUnread(m, unread){
  if (m.id === current()?.id) clearTimeout(readTimer);
  return call('set_unread', {id:m.id, unread}).then(() => {
    changed(m, {unread}); reorder();
    say(unread ? t('Marked as unread.') : t('Marked as read.'), 2000);
  }).catch(() => {});
}
function toggleUnread(){
  clearTimeout(readTimer);
  if (state.picked.size > 1){
    // Any unread among them: all read. All read: all unread.
    const rows = pickedRows(), unread = rows.length > 0 && !rows.some(r => r.unread), n = state.picked.size;
    if (!unread && unreadOnly()) chosen().forEach(id => state.kept.add(id));
    call('set_unread_many', {ids:chosen(), unread}).then(() => {
      say(unread ? tn(n, 'Marked {n} message as unread.', 'Marked {n} messages as unread.', {n:num(n)}) : tn(n, 'Marked {n} message as read.', 'Marked {n} messages as read.', {n:num(n)}), 3000);
      refresh();
    }).catch(() => {});
    return;
  }
  const m = current(); if (m) setUnread(m, !m.unread);
}
/* ---------- tags ---------- */
// Tags are IMAP keywords (tags.rs): the core keeps them and puts them on the server. The first
// five have keys 1–5, which put a tag on the selection, or take it off when all of it has it.
const TAG_COLORS = ['red', 'orange', 'yellow', 'green', 'teal', 'blue', 'purple', 'gray'];
const COLOR_NAMES = {red:N_('Red'), orange:N_('Orange'), yellow:N_('Yellow'), green:N_('Green'), teal:N_('Teal'), blue:N_('Blue'), purple:N_('Purple'), gray:N_('Gray')};
const tagOf = k => TAGS.find(x => x.keyword === k);
// A message's tags in the window's order as chips; in a row at most `most`, then "+n".
function tagChips(keys, most = Infinity){
  const tags = TAGS.filter(x => keys?.includes(x.keyword));
  if (!tags.length) return '';
  const shown = tags.slice(0, most), rest = tags.slice(most);
  return `<span class="tgs">${shown.map(x => `<span class="tg t-${esc(x.color)}">${esc(x.name)}</span>`).join('')}`
    + (rest.length ? `<span class="tg more" title="${esc(rest.map(x => x.name).join(', '))}">+${rest.length}</span>` : '') + '</span>';
}
// The chips above the open message, without drawing it again.
function paintTagChips(){
  const el = previewEl.querySelector('.ph-tags'), m = current();
  if (el && m) el.innerHTML = tagChips(m.tags);
}
const tagRows = () => state.picked.size > 1 ? pickedRows() : [current()].filter(Boolean);
function toggleTag(keyword){
  const rows = tagRows();
  if (rows.length) setTag(rows, keyword, !rows.every(r => r.tags?.includes(keyword)));
}
// A tag on messages or off, at once on screen; the core keeps it and tells the server.
function setTag(rows, keyword, on){
  const tag = tagOf(keyword);
  if (!tag) return;
  const ids = state.picked.size > 1 ? chosen() : rows.map(r => r.id), n = ids.length;
  const before = new Map(rows.map(r => [r.id, r.tags ?? []]));
  const show = r => {
    const tags = before.get(r.id).filter(k => k !== keyword);
    if (on) tags.push(keyword);
    return tags.sort();
  };
  const paint = get => { for (const r of rows){ patch(r.id, {tags:get(r)}); const b = bodies.get(r.id); if (b) b.tags = get(r); } drawRows(); paintTagChips(); };
  paint(show);
  call('set_tag_many', {ids, keyword, on}).then(() => {
    const name = tag.name;
    say(n > 1 ? (on ? tn(n, 'Tagged {n} message “{name}”.', 'Tagged {n} messages “{name}”.', {n:num(n), name}) : tn(n, 'Took “{name}” off {n} message.', 'Took “{name}” off {n} messages.', {n:num(n), name}))
      : on ? t('Tagged “{name}”.', {name}) : t('Took “{name}” off.', {name}), 2500);
    if (n > 1) refresh(); else loadTree().catch(() => {});
  }).catch(() => paint(r => before.get(r.id)));
}
// The tags of the selection, ticked where all of it has one; from the message menu or the tag
// button above a message.
function openTagMenu(x, y, from = listEl){
  const rows = tagRows();
  if (!rows.length) return;
  const here = rows.some(r => TAGS_HERE.includes(acctId(r.f)));
  openMenu(x, y, [
    ...TAGS.map((x, i) => ({label:x.name, color:x.color, checked:rows.every(r => r.tags?.includes(x.keyword)), key:i < 5 ? String(i + 1) : '', run:() => toggleTag(x.keyword)})),
    ...(TAGS.length ? [{sep:true}] : []),
    ...(here ? [{label:t('On this account tags stay on this PC: its server can’t keep them.'), disabled:true}] : []),
    {label:t('Manage tags…'), run:() => openSettings('tags')},
  ], from);
}

// Settings › Tags: name, color and order; a new tag; deleting one asks first, since it comes off
// every message, here and on the server.
let tagEdit = {adding:false, armed:null, error:''};
function renderTags(){
  const el = $('s-tags');
  if (!el) return;
  const row = (x, i) => `<div class="srow stag" data-k="${esc(x.keyword)}">
<span class="tkey">${i < 5 ? `<kbd>${i + 1}</kbd>` : ''}</span>
<button type="button" class="tswatch t-${esc(x.color)}" data-tg="color" aria-haspopup="menu" title="${esc(t('Color: {color}', {color:t(COLOR_NAMES[x.color] ?? x.color)}))}" aria-label="${esc(t('Color: {color}', {color:t(COLOR_NAMES[x.color] ?? x.color)}))}"></button>
<input class="tgname" data-tg="name" value="${esc(x.name)}" maxlength="40" spellcheck="false" autocomplete="off" aria-label="${esc(t('Tag name'))}">
<span class="em" title="${esc(x.keyword)}">${esc(tn(x.count, '{n} message', '{n} messages', {n:num(x.count)}))}</span>
<button type="button" class="btn icon" data-tg="up" title="${esc(t('Earlier'))}" aria-label="${esc(t('Earlier'))}"${i ? '' : ' disabled'}>↑</button>
<button type="button" class="btn icon" data-tg="down" title="${esc(t('Later|order'))}" aria-label="${esc(t('Later|order'))}"${i < TAGS.length - 1 ? '' : ' disabled'}>↓</button>
<button type="button" class="btn danger" data-tg="delete">${esc(tagEdit.armed === x.keyword ? t('Delete it') : t('Delete'))}</button></div>`
    + (tagEdit.armed === x.keyword ? `<p class="dnote tarmed">${esc(x.count ? tn(x.count, 'It comes off {n} message, here and on the server.', 'It comes off {n} messages, here and on the server.', {n:num(x.count)}) : t('No message has it.'))}</p>` : '');
  const free = TAG_COLORS.find(c => !TAGS.some(x => x.color === c)) ?? 'gray';
  el.innerHTML = TAGS.map(row).join('') + (tagEdit.adding ? `<div class="srow stag" data-k="">
<span class="tkey"></span><button type="button" class="tswatch t-${free}" data-tg="color" data-color="${free}" aria-haspopup="menu" aria-label="${esc(t('Color: {color}', {color:t(COLOR_NAMES[free])}))}"></button>
<input class="tgname" id="s-tag-name" data-tg="new" maxlength="40" spellcheck="false" autocomplete="off" placeholder="${esc(t('Name of the new tag'))}" aria-label="${esc(t('Name of the new tag'))}">
<button type="button" class="btn primary" data-tg="add">${esc(t('Add'))}</button><button type="button" class="btn" data-tg="cancel">${esc(t('Cancel'))}</button></div>` : '');
  $('s-tag-new').hidden = tagEdit.adding;
  $('s-tag-err').textContent = tagEdit.error;
  const here = TAGS_HERE.map(id => ACCT[id]?.email ?? id);
  $('s-tag-here').hidden = !here.length;
  $('s-tag-here').textContent = here.length ? t('Tags of {accounts} stay on this PC: the server can’t keep them.', {accounts:here.join(', ')}) : '';
}
async function tagsChanged(){
  tagEdit.error = '';
  await refresh();
  renderTags();
}
async function saveTag(x, changes){
  try { await invoke('save_tag', {tag:{...x, ...changes}}); }
  catch (e){ tagEdit.error = tr(e); renderTags(); return false; }
  await tagsChanged();
  return true;
}
async function addTag(){
  const name = $('s-tag-name')?.value ?? '', color = $('s-tags').querySelector('[data-k=""] .tswatch')?.dataset.color ?? 'gray';
  tagEdit.adding = false;
  if (await saveTag({keyword:'', name, color, count:0}, {})) say(t('Tag “{name}” added.', {name:name.trim()}));
  else { tagEdit.adding = true; renderTags(); const f = $('s-tag-name'); if (f){ f.value = name; f.focus(); } }
}
function colorMenu(button, x){
  const r = button.getBoundingClientRect(), now = x?.color ?? button.dataset.color;
  openMenu(r.left, r.bottom + 2, TAG_COLORS.map(c => ({label:t(COLOR_NAMES[c]), color:c, checked:c === now, run:() => {
    if (x) saveTag(x, {color:c});
    else { button.dataset.color = c; button.className = 'tswatch t-' + c; button.setAttribute('aria-label', t('Color: {color}', {color:t(COLOR_NAMES[c])})); button.focus(); }
  }})), button);
}
$('s-tag-new').addEventListener('click', () => { tagEdit = {adding:true, armed:null, error:''}; renderTags(); $('s-tag-name')?.focus(); });
$('s-tags').addEventListener('click', async e => {
  const b = e.target.closest('[data-tg]'), row = e.target.closest('[data-k]');
  if (!b || !row || b.tagName === 'INPUT') return;
  const x = tagOf(row.dataset.k), act = b.dataset.tg;
  if (act === 'color'){ colorMenu(b, x); return; }
  if (act === 'add'){ addTag(); return; }
  if (act === 'cancel'){ tagEdit.adding = false; renderTags(); $('s-tag-new').focus(); return; }
  if (!x) return;
  if (act === 'up' || act === 'down'){
    try { await invoke('move_tag', {keyword:x.keyword, delta:act === 'up' ? -1 : 1}); } catch (err){ tagEdit.error = tr(err); renderTags(); return; }
    await tagsChanged();
    $('s-tags').querySelector(`[data-k="${CSS.escape(x.keyword)}"] [data-tg="${act}"]:not([disabled])`)?.focus();
  }
  if (act === 'delete'){
    if (tagEdit.armed !== x.keyword){ tagEdit.armed = x.keyword; renderTags(); $('s-tags').querySelector(`[data-k="${CSS.escape(x.keyword)}"] [data-tg="delete"]`)?.focus(); return; }
    tagEdit.armed = null;
    let n;
    try { n = await call('delete_tag', {keyword:x.keyword}); } catch (err){ tagEdit.error = tr(err); renderTags(); return; }
    await tagsChanged();
    say(n ? tn(n, 'Tag “{name}” deleted; it came off {n} message.', 'Tag “{name}” deleted; it came off {n} messages.', {n:num(n), name:x.name}) : t('Tag “{name}” deleted.', {name:x.name}));
    $('s-tag-new').focus();
  }
});
// A name is saved when it is left or Enter is pressed; Esc in it puts the old one back.
$('s-tags').addEventListener('change', e => {
  const row = e.target.closest('[data-k]'), x = row && tagOf(row.dataset.k);
  if (x && e.target.dataset.tg === 'name' && e.target.value.trim() !== x.name) saveTag(x, {name:e.target.value});
});
$('s-tags').addEventListener('keydown', e => {
  if (e.target.tagName !== 'INPUT') return;
  if (e.key === 'Enter'){ e.preventDefault(); if (e.target.dataset.tg === 'new') addTag(); else e.target.blur(); }
  else if (e.key === 'Escape'){
    e.preventDefault(); e.stopPropagation();
    if (e.target.dataset.tg === 'new'){ tagEdit.adding = false; renderTags(); $('s-tag-new').focus(); }
    else { const x = tagOf(e.target.closest('[data-k]').dataset.k); e.target.value = x?.name ?? ''; }
  }
});

// Shift+U: every message in the folder (or in the list, in all mail) read.
async function markAllRead(){
  if (!list.total) return;
  state.kept.clear(); // all read: an unread list empties
  let n;
  try {
    if (across()) n = await call('set_unread_many', {ids:await invoke('list_ids', {view:viewArgs(), from:null, to:null}), unread:false});
    else n = await call('mark_folder_read', {folder:state.folder});
  } catch { return; }
  await refresh();
  say(n ? tn(n, 'Marked {n} message as read.', 'Marked {n} messages as read.', {n:num(n)}) : t('Everything here was read already.'), 3000);
}
// Deleting from Trash can't be undone (on the server either), so it takes a second Del: a press
// of its own, at least a moment after the first, so a double-click or a bounce isn't one.
let armedDelete = null; // {key, at}
const DELETE_GAP = 400;
async function del(){
  const ids = chosen(); if (!ids.length) return;
  const many = state.picked.size > 1, m = current();
  const inTrash = many ? !allMail() && kindOf(state.folder) === 'trash' : m && kindOf(m.f) === 'trash';
  if (inTrash){
    const key = ids.join(',');
    if (armedDelete?.key === key && Date.now() - armedDelete.at < DELETE_GAP) return;
    if (armedDelete?.key !== key || Date.now() - armedDelete.at > 5000){
      armedDelete = {key, at:Date.now()};
      say(many ? tn(ids.length, 'Press Del again to delete {n} message for good.', 'Press Del again to delete these {n} messages for good.', {n:num(ids.length)})
        : t('Press Del again to delete this message for good.'), 5000);
      return;
    }
    armedDelete = null;
  }
  // The next one gets the selection, or the one above at the end of the list.
  let next;
  if (many) next = rowAfterPicked();
  else { const pos = m ? posOf(m.id) : -1; next = pos < 0 ? null : (await rowAt(pos + 1)) ?? (await rowAt(pos - 1)); }
  let r;
  try { r = await call('delete_messages', {ids}); } catch { return; }
  ids.forEach(id => bodies.delete(id));
  setUndo(r.trashed.length ? {kind:'move', items:r.trashed.map(([id, from]) => ({id, from}))} : null);
  const n = ids.length;
  say(r.trashed.length ? (n === 1 ? t('Moved to Trash. Press Z to undo.') : tn(n, 'Moved {n} message to Trash. Press Z to undo.', 'Moved {n} messages to Trash. Press Z to undo.', {n:num(n)}))
    : n === 1 ? t('Deleted for good.') : tn(n, 'Deleted {n} message for good.', 'Deleted {n} messages for good.', {n:num(n)}));
  if (VIEWER){ tellMain({say:$('st-msg').textContent, undo:state.undo}); closeViewer(); return; }
  unpick();
  state.sel[selKey()] = next?.id;
  selRow = next;
  if (!state.compose) renderPreview();
  await refresh({skip:ids.length === 1 ? ids[0] : undefined});
}
// Move the picked messages (or the selected one) to a folder of their account; Z moves them back.
async function moveChosen(folder){
  const ids = chosen(); if (!ids.length) return;
  const next = state.picked.size > 1 ? rowAfterPicked() : null;
  let places;
  try { places = await call('move_messages', {ids, folder}); } catch { return; }
  setUndo({kind:'move', items:places.filter(([, from]) => from !== folder).map(([id, from]) => ({id, from}))});
  const n = ids.length, where = placeName(folder);
  say(n === 1 ? t('Moved to {folder}. Press Z to undo.', {folder:where}) : tn(n, 'Moved {n} message to {folder}. Press Z to undo.', 'Moved {n} messages to {folder}. Press Z to undo.', {n:num(n), folder:where}));
  unpick();
  if (next){ state.sel[selKey()] = next.id; selRow = next; }
  await refresh({skip:n === 1 ? ids[0] : undefined});
  if (!state.compose) renderPreview();
}
// The menu's Mark as spam and Not spam: to the account's Spam folder, or back to its Inbox. The
// spam filter learns from either, and from Z, which moves them back.
async function markSpam(spam){
  const ids = chosen(); if (!ids.length) return;
  const next = state.picked.size > 1 ? rowAfterPicked() : null;
  let places;
  try { places = await call('mark_spam', {ids, spam}); } catch { return; }
  setUndo({kind:'move', items:places.map(([id, from]) => ({id, from}))});
  const n = ids.length;
  say(spam ? (n === 1 ? t('Moved to Spam; the spam filter learns from it. Press Z to undo.')
      : tn(n, 'Moved {n} message to Spam; the spam filter learns from it. Press Z to undo.', 'Moved {n} messages to Spam; the spam filter learns from them. Press Z to undo.', {n:num(n)}))
    : n === 1 ? t('Back in the Inbox; the spam filter learns it isn’t spam. Press Z to undo.')
      : tn(n, 'Moved {n} message back to the Inbox; the spam filter learns it isn’t spam. Press Z to undo.', 'Moved {n} messages back to the Inbox; the spam filter learns they aren’t spam. Press Z to undo.', {n:num(n)}));
  if (VIEWER){ tellMain({say:$('st-msg').textContent, undo:state.undo}); closeViewer(); return; }
  unpick();
  if (next){ state.sel[selKey()] = next.id; selRow = next; }
  await refresh({skip:n === 1 ? ids[0] : undefined});
  if (!state.compose) renderPreview();
}
// Z: put back what was deleted or moved last, or take back mail still waiting to be sent.
async function undo(){
  const u = state.undo; if (!u) return;
  if (u.kind === 'send'){ await unsend(u); return; }
  if (u.kind === 'discard'){ await restoreCompose(u.compose); return; }
  const groups = new Map();
  for (const it of u.items){ if (!groups.has(it.from)) groups.set(it.from, []); groups.get(it.from).push(it.id); }
  try { for (const [folder, ids] of groups) await call('move_messages', {ids, folder}); } catch { return; }
  setUndo(null);
  const first = u.items[0];
  if (first && inView(first.from)){ state.sel[selKey()] = first.id; selRow = null; }
  await loadTree().catch(() => {});
  renderTree();
  await loadView();
  say(u.items.length === 1 ? t('Restored to {folder}.', {folder:placeName(first.from)}) : tn(u.items.length, 'Restored {n} message.', 'Restored {n} messages.', {n:num(u.items.length)}));
}
/* ---------- folder picker: M moves, G goes ---------- */
// Type part of a folder's name (diacritics don't matter), ↑ ↓ to choose, Enter. Folders used
// last come first. M lists the folders of the selected messages' account; G every folder.
const pdlg = $('pick-dlg');
let pick = null; // {mode, items:[{key, name, acct}], shown:[], at}
// Where messages from these folders can go: a folder of their own account, or one on this
// PC; mail on this PC can go anywhere. Messages of two server accounts go to this PC only.
function targetsFor(folders){
  const from = [...new Set(folders.map(acctId))].filter(a => a !== LOCAL);
  const accts = from.length > 1 ? [] : from.length === 1 ? from : servers().map(a => a.id);
  return ACCT[LOCAL] ? [...accts, LOCAL] : accts;
}
function openPicker(mode){
  if (state.compose || !ACCOUNTS.length) return;
  let accts = ACCOUNTS.map(a => a.id), skip = null;
  if (mode === 'move'){
    const ids = chosen();
    if (!ids.length){ say(t('Select a message first.')); return; }
    const rows = state.picked.size > 1 ? pickedRows() : [current()];
    const folders = [...new Set(rows.map(r => r.f))];
    accts = targetsFor(folders);
    if (folders.length === 1) skip = folders[0];
  }
  // G goes to every account's drafts and to the Outbox as one list each, as the tree shows them.
  const items = (mode === 'move' ? [] : ['unread', ...UNSENT].map(kind => ({key:EVERY + '/' + kind, name:t(KIND_NAME[kind]), acct:EVERY})))
    .concat(accts.flatMap(a => (ACCT[a]?.folders ?? [])
      .filter(f => mode === 'move' ? f.kind !== 'outbox' : !COUNT_ALL.has(f.kind))
      .map(f => ({key:a + '/' + f.id, name:folderName(a + '/' + f.id), acct:a}))
      .filter(it => it.key !== skip)));
  pick = {mode, items, shown:[], at:0};
  const n = chosen().length;
  $('pk-title').textContent = mode === 'move' ? tn(n, 'Move {n} message to', 'Move {n} messages to', {n:num(n)}) : t('Go to folder');
  $('pk-q').value = '';
  filterPicker();
  pdlg.showModal();
  $('pk-q').focus();
}
function filterPicker(){
  const q = fold($('pk-q').value.trim());
  const ranked = pick.items.map((it, i) => {
    const name = fold(it.name), recent = state.recent.indexOf(it.key);
    const r = !q ? (recent >= 0 ? recent : 100 + i)
      : name.startsWith(q) ? (recent >= 0 ? recent : 20) + i / 1000
      : name.split(/[\s/._-]+/).some(w => w.startsWith(q)) ? 40 + i / 1000
      : name.includes(q) ? 60 + i / 1000
      : fold(acctLabel(it.acct)).startsWith(q) ? 80 + i / 1000 : -1;
    return {it, r};
  }).filter(x => x.r >= 0).sort((a, b) => a.r - b.r);
  pick.shown = ranked.map(x => x.it);
  pick.at = Math.min(pick.at, Math.max(0, pick.shown.length - 1));
  const many = new Set(pick.items.map(it => it.acct)).size > 1; // say whose folder only when it isn't obvious
  $('pk-list').innerHTML = pick.shown.length ? pick.shown.map((it, i) => {
    const f = folderOf(it.key);
    return `<li role="option" id="pk-${i}" data-i="${i}" aria-selected="${i === pick.at}">${I[KIND_ICON[f?.kind] || 'folder']}${many ? adot(it.acct) : ''}<span class="nm">${esc(it.name)}</span>${many ? `<span class="em">${esc(acctLabel(it.acct))}</span>` : ''}</li>`;
  }).join('') : `<li class="none">${esc(t('No folder matches.'))}</li>`;
  $('pk-q').setAttribute('aria-activedescendant', pick.shown.length ? 'pk-' + pick.at : '');
  $('pk-list').querySelector('[aria-selected="true"]')?.scrollIntoView({block:'nearest'});
}
function choosePicked(i){
  const it = pick?.shown[i];
  if (!it) return;
  const mode = pick.mode;
  pdlg.close();
  state.recent = [it.key, ...state.recent.filter(k => k !== it.key)].slice(0, 8); persist();
  if (mode === 'move') moveChosen(it.key);
  else setFolder(it.key).then(() => listEl.focus({preventScroll:true}));
}
$('pk-q').addEventListener('input', () => { pick.at = 0; filterPicker(); });
$('pk-q').addEventListener('keydown', e => {
  if (e.key === 'ArrowDown' || e.key === 'ArrowUp'){
    e.preventDefault();
    const n = pick.shown.length; if (!n) return;
    pick.at = (pick.at + (e.key === 'ArrowDown' ? 1 : -1) + n) % n;
    filterPicker();
  } else if (e.key === 'Enter'){ e.preventDefault(); choosePicked(pick.at); }
});
$('pk-list').addEventListener('click', e => { const li = e.target.closest('[data-i]'); if (li) choosePicked(Number(li.dataset.i)); });
pdlg.addEventListener('close', () => listEl.focus({preventScroll:true}));

/* ---------- conversations: T ---------- */
let groupBusy = false;
async function toggleGroup(m, open){
  if (!grouped() || !m?.thread || groupBusy) return;
  const th = m.thread, on = open ?? !state.expanded.has(th.id);
  if (on === state.expanded.has(th.id)) return;
  groupBusy = true;
  const selected = current(), collapsingSelected = !on && selected?.thread?.id === th.id && selected.thread.child;
  on ? state.expanded.add(th.id) : state.expanded.delete(th.id);
  if (collapsingSelected) state.sel[selKey()] = th.head;
  unpick();
  try { await loadView(); }
  finally { groupBusy = false; listEl.focus({preventScroll:true}); }
}
// The selected message with everything it answers and everything that answers it, from every
// folder of its account (not Spam or Trash). T or Esc goes back to the folder.
function toggleThread(){
  if (state.thread){ leaveThread(); return; }
  const m = current();
  if (!m){ say(t('Select a message first.')); return; }
  unpick();
  state.thread = {id:m.id, folder:state.folder, scope:state.scope};
  state.sel['#'] = m.id;
  state.q = ''; qEl.value = ''; state.inText = false;
  paintScope(); renderHead();
  loadView({top:true}).then(() => {
    say(list.total > 1 ? t('The conversation. T or Esc goes back.') : t('No other messages in this conversation. T or Esc goes back.'), 4000);
  });
}
function leaveThread(){
  const back = state.thread; if (!back) return;
  const m = current();
  state.thread = null;
  // Back in the folder, on the same message if it is there.
  if (m && m.f === back.folder) state.sel[back.folder] = m.id;
  paintScope(); renderHead();
  loadView();
}

// quiet: a background check, which speaks up only for new mail or errors.
// first: the download right after adding an account.
// account: only that account's Inbox, because IDLE said something changed there.
let checkAgain = null; // asked for while a check was running; runs when it ends
async function getMail({quiet = false, first = false, account = null} = {}){
  const n = servers().length;
  if (!n){ if (!quiet) openAccountDialog(); return; } // nothing to check yet
  const b = $('b-get');
  if (b.classList.contains('busy')){
    // A full check waiting covers an Inbox one.
    if (first || !quiet || account) checkAgain = checkAgain && !checkAgain.account ? checkAgain : {quiet, first, account};
    return;
  }
  b.classList.add('busy');
  if (!quiet) say(tn(n, 'Checking {n} account…', 'Checking {n} accounts…'), 0);
  try {
    const r = await invoke('check_mail', {account});
    if (r.busy){ setTimeout(() => getMail({quiet, first, account}), 2000); return; } // changes are being sent
    forgetRecipients();
    lastCheck = new Date();
    await refresh({arrived:r.new}); // the quota and how the check went come with the tree
    const ids = r.new;
    let text = '';
    if (first) text = tn(ids.length, '{n} message downloaded.', '{n} messages downloaded.');
    else if (ids.length){
      const where = r.new_in && folderOf(r.new_in) ? `${ACCT[acctId(r.new_in)].email} › ${folderName(r.new_in)}` : '';
      text = where ? tn(ids.length, '{n} new message in {where}.', '{n} new messages in {where}.', {where})
        : tn(ids.length, '{n} new message.', '{n} new messages.');
    } else if (!quiet) text = t('No new messages.');
    if (r.spam) text = [text, tn(r.spam, 'The spam filter moved {n} message to Spam.', 'The spam filter moved {n} messages to Spam.')].filter(Boolean).join(' ');
    if (r.sorted) text = [text, tn(r.sorted, 'Rules sorted {n} message.', 'Rules sorted {n} messages.')].filter(Boolean).join(' ');
    if (r.sent?.length) text = [sentText(r.sent), text].filter(Boolean).join(' ');
    if (r.errors.length) text = [r.errors.join(' · '), text].filter(Boolean).join(' · ');
    say(text, r.errors.length ? 15000 : 6000);
  } catch (e){ if (!quiet) say(tr(e), 8000); }
  finally {
    b.classList.remove('busy');
    if (checkAgain){ const next = checkAgain; checkAgain = null; getMail(next); }
  }
}

// IDLE: the server said something changed in an account's Inbox. Several notices in a row are
// one check.
let inboxTimer;
listenMain('inbox-changed', ({payload: account}) => {
  clearTimeout(inboxTimer);
  inboxTimer = setTimeout(() => getMail({quiet:true, account}), 800);
});

// Progress while a folder downloads; each finished folder shows up right away.
let reloadTimer;
listenMain('sync-progress', ({payload: p}) => {
  say(t('{account} › {folder}: {done} of {total}', {...p, folder:p.key && folderOf(p.key) ? folderName(p.key) : p.folder}), 0);
  if (p.done === p.total){
    clearTimeout(reloadTimer);
    reloadTimer = setTimeout(refresh, 200);
  }
});

$('b-get').addEventListener('click', () => getMail());
$('b-new').addEventListener('click', () => openCompose('new'));
$('b-reply').addEventListener('click', () => openCompose('reply'));
$('b-replyall').addEventListener('click', () => openCompose('replyall'));
$('b-forward').addEventListener('click', () => openCompose('forward'));
$('b-flag').addEventListener('click', toggleFlag);
$('b-delete').addEventListener('click', del);
$('b-density').addEventListener('click', () => changeSettings({density:pref().density === 'compact' ? 'comfortable' : 'compact'}));
$('b-settings').addEventListener('click', () => openSettings());
// The filter matches senders, recipients and subjects at once, then also message text a
// moment later (the core's full-text index). In all mail it looks in every account.
let searchTimer;
function setFilter(text){
  state.savedSearch = null;
  state.q = text; state.inText = false;
  if (state.thread && text.trim()) state.thread = null; // the filter looks in the folder or all mail again
  unpick();
  paintScope(); renderHead();
  clearTimeout(searchTimer);
  loadView();
  if (state.q.trim()) searchTimer = setTimeout(() => { state.inText = true; loadView(); }, 150);
}
qEl.addEventListener('input', () => { setFilter(qEl.value); qs.at = -1; suggest(); });

/* ---------- search: suggestions under the box ---------- */
// While the box has the focus it offers, for an empty box, the searches made before and the
// operators; for the word being typed, the operators it could start and the people it could be
// (from:, or to: in Sent and Drafts). ↓ ↑ choose, Enter or a click takes one. Enter with none
// chosen (or ↓ with none to choose) goes to the results; Esc clears the search.
const qPop = $('q-pop');
const OPERATORS_ALL = ['from:', 'to:', 'subject:', 'is:unread', 'is:read', 'is:flagged', 'is:unflagged', 'tag:', 'is:tagged', 'has:file', 'after:', 'before:', 'larger:', 'smaller:'];
const qs = {items:[], at:-1};
// The word the caret is in: where it starts and ends, and the word.
function wordAtCaret(){
  const v = qEl.value, c = qEl.selectionStart ?? v.length;
  let a = c, b = c;
  while (a > 0 && !/\s/.test(v[a - 1])) a--;
  while (b < v.length && !/\s/.test(v[b])) b++;
  return [a, b, v.slice(a, b)];
}
function suggest(){
  if (document.activeElement !== qEl || !ACCOUNTS.length){ closeSuggest(); return; }
  const items = [], [, , word] = wordAtCaret(), low = word.toLowerCase();
  if (!qEl.value.trim()) state.searches.forEach(text => items.push({kind:'recent', text}));
  else if (word){
    const i = low.indexOf(':'), key = i > 0 ? low.slice(0, i) : '';
    if ((key || low.length >= 2) && !OPERATORS_ALL.includes(low)){
      for (const op of OPERATORS_ALL) if (op.startsWith(low) && items.length < 4) items.push({kind:'op', text:op});
    }
    if (key === 'tag'){
      const typed = fold(word.slice(i + 1).replace(/"/g, ''));
      for (const x of TAGS) if (items.length < 8 && fold(x.name).startsWith(typed)) items.push({kind:'tag', text:'tag:' + (/\s/.test(x.name) ? `"${x.name}"` : x.name), tag:x});
    }
    const who = fold((key ? word.slice(i + 1) : word).replace(/"/g, ''));
    if ((!key || key === 'from' || key === 'to') && who.length >= 2 && RECIPS){
      const op = key || (isOut(state.folder) && !allMail() ? 'to' : 'from');
      for (const p of RECIPS){
        if (items.length >= 8) break;
        if (matchesTyped(p, who)) items.push({kind:'person', text:`${op}:${p.email}`, name:p.name, email:p.email, op});
      }
    }
  }
  qs.items = items; qs.at = Math.min(qs.at, items.length - 1);
  const empty = !qEl.value.trim();
  if (!items.length && !empty){ closeSuggest(); return; }
  const row = (it, i) => {
    const body = it.kind === 'recent' ? `${I.clock}<span class="nm">${esc(it.text)}</span>`
      : it.kind === 'op' ? `${I.search}<span class="nm"><code>${esc(it.text)}</code></span><span class="em">${esc(opHint(it.text))}</span>`
      : it.kind === 'tag' ? `<i class="tdot t-${esc(it.tag.color)}" aria-hidden="true"></i><span class="nm">${esc(it.tag.name)}</span><span class="em"><code>${esc(it.text)}</code></span>`
      : `${I.person}<span class="nm">${esc(it.name || it.email)}</span><span class="em">${esc(it.op === 'to' ? t('to {email}', {email:it.email}) : t('from {email}', {email:it.email}))}</span>`;
    return `<li role="option" id="q-opt-${i}" data-i="${i}" aria-selected="${i === qs.at}">${body}</li>`;
  };
  qPop.innerHTML = (empty && items.length ? `<div class="qhead"><span>${esc(t('Recent searches'))}</span><button type="button" class="linkbtn" data-forget>${esc(t('Clear'))}</button></div>` : '')
    + (items.length ? `<ul id="q-list" role="listbox" aria-label="${esc(t('Suggestions'))}">${items.map(row).join('')}</ul>` : '')
    + (empty ? `<div class="qops"><span>${esc(t('Narrow it down:'))}</span>${OPERATORS_ALL.filter(o => o !== 'is:read' && o !== 'is:unflagged').map(o => `<button type="button" data-op="${esc(o)}">${esc(o)}</button>`).join('')}</div>` : '')
    + `<div class="qfoot"><span><kbd>Enter</kbd> ${esc(t('the results'))}</span><span><kbd>Tab</kbd> ${esc(t('this folder or all mail'))}</span><span><kbd>Esc</kbd> ${esc(t('clear'))}</span></div>`;
  const r = qEl.closest('.search').getBoundingClientRect(), w = Math.min(innerWidth - 8, Math.max(r.width, 340));
  qPop.style.left = Math.max(4, Math.min(r.left, innerWidth - w - 4)) + 'px';
  qPop.style.top = (r.bottom + 3) + 'px';
  qPop.style.width = w + 'px';
  qPop.hidden = false;
  qEl.setAttribute('aria-expanded', 'true');
  qEl.setAttribute('aria-activedescendant', qs.at >= 0 ? 'q-opt-' + qs.at : '');
}
// What an operator does, in a few words.
function opHint(op){
  return {'from:':t('sender'), 'to:':t('recipient'), 'subject:':t('words in the subject'), 'is:unread':t('unread only'), 'is:read':t('read only'),
    'is:flagged':t('flagged only'), 'is:unflagged':t('without a flag'), 'tag:':t('a tag, e.g. tag:work'), 'is:tagged':t('with any tag'), 'has:file':t('with attachments'), 'after:':t('2026, 2026-09 or 15.09.2026'),
    'before:':t('2026, 2026-09 or 15.09.2026'), 'larger:':t('size, e.g. 5m'), 'smaller:':t('size, e.g. 100k')}[op] ?? '';
}
function closeSuggest(){
  if (qPop.hidden) return;
  qPop.hidden = true; qs.items = []; qs.at = -1;
  qEl.setAttribute('aria-expanded', 'false'); qEl.removeAttribute('aria-activedescendant');
}
// Put text where the word at the caret is (an operator, a person), and look again.
function replaceWord(text){
  const [a, b] = wordAtCaret(), v = qEl.value, tail = v.slice(b).replace(/^\s*/, '');
  const head = v.slice(0, a) + text + (text.endsWith(':') ? '' : ' ');
  qEl.value = head + tail;
  qEl.setSelectionRange(head.length, head.length);
  setFilter(qEl.value); qs.at = -1; suggest();
}
function takeQuery(i){
  const it = qs.items[i];
  if (!it) return;
  if (it.kind === 'recent'){ qEl.value = it.text; setFilter(it.text); toResults(); }
  else replaceWord(it.text);
}
// The search is what was wanted: keep it among the recent ones and go to the results.
function rememberSearch(){
  const q = state.q.trim();
  if (q.length < 2) return;
  state.searches = [q, ...state.searches.filter(s => s !== q)].slice(0, 8); persist();
}
async function toResults(){
  rememberSearch(); closeSuggest();
  listEl.focus({preventScroll:true});
  if (!current() && list.total){ const row = await rowAt(0); if (row){ select(row.id); if (reveal(0)) drawRows(); } }
}
qEl.addEventListener('keydown', e => {
  if (e.key === 'ArrowDown' || e.key === 'ArrowUp'){
    const n = qs.items.length;
    if (!n || qPop.hidden){ if (e.key === 'ArrowDown' && list.total){ e.preventDefault(); toResults(); } return; }
    e.preventDefault();
    qs.at = e.key === 'ArrowDown' ? (qs.at + 1) % n : qs.at <= 0 ? n - 1 : qs.at - 1;
    suggest();
    $('q-opt-' + qs.at)?.scrollIntoView({block:'nearest'});
  } else if (e.key === 'Enter' && !e.ctrlKey && !e.altKey){
    e.preventDefault();
    if (qs.at >= 0 && !qPop.hidden) takeQuery(qs.at); else toResults();
  }
});
qEl.addEventListener('focus', () => { loadRecipients().then(suggest); suggest(); });
qEl.addEventListener('click', suggest);
qEl.addEventListener('blur', () => setTimeout(() => {
  if (document.activeElement === qEl) return;
  closeSuggest();
  if (state.q.trim() && list.total) rememberSearch(); // the results were used
}, 150));
// Clicks in the box below keep the focus in the search box.
qPop.addEventListener('mousedown', e => e.preventDefault());
qPop.addEventListener('click', e => {
  const li = e.target.closest('[data-i]'), op = e.target.closest('[data-op]');
  if (li) takeQuery(Number(li.dataset.i));
  else if (op){ // after what is there
    const v = qEl.value.trimEnd();
    qEl.value = v ? v + ' ' : '';
    qEl.setSelectionRange(qEl.value.length, qEl.value.length);
    replaceWord(op.dataset.op);
  }
  else if (e.target.closest('[data-forget]')){ state.searches = []; persist(); suggest(); }
});
addEventListener('resize', () => { if (!qPop.hidden) suggest(); });
function paintScope(){
  const all = state.scope === 'all';
  win.classList.toggle('allmail', across());
  $('q').closest('.search').classList.toggle('active', !!state.q);
  $('q-scope').textContent = all ? t('All mail') : t('This folder');
  $('q-scope').setAttribute('aria-pressed', String(all));
  $('q-save').disabled = !state.q.trim();
  fitColumns();
}
// This folder or all mail. Remembered; matters only while there is something to filter by.
function toggleScope(){
  state.scope = state.scope === 'all' ? 'folder' : 'all'; persist();
  paintScope();
  if (state.q.trim()) loadView();
}
$('q-scope').addEventListener('click', () => { toggleScope(); qEl.focus(); });
// All mail from one sender, e.g. from Settings › Usage.
function filterAllMail(text){
  state.scope = 'all'; persist();
  qEl.value = text; setFilter(text);
}

/* ---------- accounts ---------- */
// Settings › Accounts: the list, and per account its server settings and identities.
// Adding is a wizard: name, email and password. The core finds the servers (its provider
// list, the domain's setup file, its DNS zone, the usual names) and add_account logs in to
// both before anything is saved. When nothing is found or the login fails, the settings
// form opens, filled in, with the reason. Editing an account goes straight to the form.
const aForm = $('acct-form'), aErr = $('a-err'), wForm = $('w-start'), wErr = $('w-err');
let editing = null;     // id of the account being edited, or null when adding
let wizardSeq = 0;      // each run gets a number; answers for an older run are dropped
let wizardBusy = null;  // 'find' or 'login' while the wizard works
let accountView = 'home';
const ACCOUNT_VIEWS = {home:'a-home', start:'w-start', work:'w-work', form:'acct-form', ids:'i-list-view', idform:'i-form', folders:'f-view', away:'away-view'};
function showAccountView(view){
  accountView = view;
  for (const [v, id] of Object.entries(ACCOUNT_VIEWS)) $(id).hidden = v !== view;
  if (view === 'home') renderAccountList();
  if (view === 'start') offerMoveIn();
  sdlg.querySelector('.setpanes').scrollTop = 0;
}

// Adding an account while another mail program on this PC has some: all of them can come at once.
function offerMoveIn(){
  const el = $('w-move');
  const show = () => {
    const from = moveIn.sources?.find(src => src.accounts.some(canAdd));
    el.hidden = !from || accountView !== 'start';
    if (!from) return;
    const n = from.accounts.filter(canAdd).length;
    el.innerHTML = esc(tn(n, '{program} is on this PC with {n} account.', '{program} is on this PC with {n} accounts.', {program:programName(from.program)}))
      + ` <button type="button" class="linkbtn" data-path="${esc(from.path)}">${esc(t('Bring them all, with their mail'))}</button>`;
  };
  if (moveIn.sources) show(); else findPrograms().then(show);
}
$('w-move').addEventListener('click', e => { const b = e.target.closest('[data-path]'); if (b) openMoveIn(b.dataset.path); });

// The core sends tips and reasons as codes; the words are here, to translate with the rest.
const NOTE_TEXT = {
  app_password: p => t('{p} takes an app password here, not your usual password. You make one in your {p} account’s security settings.', {p}),
  turn_on_imap: p => t('Turn on access for mail programs (IMAP) in your {p} settings first.', {p}),
  turn_on_imap_and_app_password: p => t('Turn on IMAP in your {p} settings first, and use an app password made there, not your usual password.', {p}),
};
const UNSUPPORTED_TEXT = {
  microsoft_sign_in: p => t('{p} lets other mail apps in only with Microsoft sign-in, which Telex doesn’t have yet. It’s next on the roadmap.', {p}),
  proton_bridge: () => t('Proton Mail works with other mail apps only through Proton Mail Bridge, which Telex can’t use yet.'),
};
const SOURCE_TEXT = {
  list: d => t('Telex knows {provider}', {provider:d.provider}),
  autoconfig: d => d.provider ? t('from {provider}’s setup file', {provider:d.provider}) : t('from your provider’s setup file'),
  srv: () => t('from your domain’s DNS records'),
  mx: d => t('your domain’s mail is handled by {provider}', {provider:d.provider}),
  guess: () => t('found by trying the usual server names'),
};
function hint(el, d){
  const p = d?.provider ?? t('your provider');
  const text = d?.unsupported ? UNSUPPORTED_TEXT[d.unsupported]?.(p) : d?.note ? NOTE_TEXT[d.note]?.(p) : '';
  el.hidden = !text;
  el.classList.toggle('warn', !!d?.unsupported);
  el.innerHTML = text ? esc(text) + (d.help ? ` <button type="button" class="linkbtn" data-help="${esc(d.help)}">${esc(t('Open that page'))}</button>` : '') : '';
}
// "Add account" anywhere: Settings › Accounts, with the wizard.
function openAccountDialog(){
  wForm.reset(); aForm.reset();
  editing = null; wizardBusy = null; ++wizardSeq;
  wErr.textContent = ''; hint($('w-hint'), null);
  // A second account is usually the same person: start from their name.
  const me = ACCOUNTS.flatMap(a => a.identities || []).find(i => i.is_default)?.name ?? '';
  $('w-name').value = me;
  openSettings('accts');
  renderLangPick();
  showAccountView('start');
  $(me ? 'w-email' : 'w-name').focus();
}
// The first start: the language, right where Telex opens. Windows in English with Polish
// formats gets a nudge, in Polish.
function renderLangPick(){
  const first = !ACCOUNTS.length, lang = I18n.lang();
  $('w-lang').hidden = !first;
  $('w-lang').querySelectorAll('[data-lang]').forEach(b => b.setAttribute('aria-pressed', String(b.dataset.lang === lang)));
  $('w-langhint').hidden = !(first && SETTINGS.offer === 'pl' && lang !== 'pl' && state.offered !== 'pl');
}
$('w-lang').addEventListener('click', async e => {
  const b = e.target.closest('[data-lang]');
  if (!b || b.dataset.lang === I18n.lang()) return;
  state.offered = SETTINGS.offer || state.offered; persist();
  if (await changeSettings({language:b.dataset.lang})) renderLangPick();
});
// Each account: who it is, where its servers are, how the last check went.
function renderAccountList(){
  const real = servers();
  $('a-list').innerHTML = real.length ? real.map((a, i) => {
    const where = `${a.email} · ${a.imap.pop3 ? 'POP3 ' : ''}${a.imap.host}${a.smtp ? '' : ' · ' + t('no SMTP server yet')}`, u = {sync_error:PROBLEMS[a.id], synced_at:SYNCED[a.id]};
    // Their order in the folder tree, when there is more than one.
    const order = real.length < 2 ? '' : `<button type="button" class="btn icon" data-up="${esc(a.id)}" title="${esc(t('Move up'))}" aria-label="${esc(t('Move {email} up', {email:a.email}))}"${i ? '' : ' disabled'}>↑</button>`
      + `<button type="button" class="btn icon" data-down="${esc(a.id)}" title="${esc(t('Move down'))}" aria-label="${esc(t('Move {email} down', {email:a.email}))}"${i < real.length - 1 ? '' : ' disabled'}>↓</button>`;
    const sync = u?.sync_error ? `<button type="button" class="st warn cnwhy" data-conn="${esc(a.id)}" title="${esc(u.sync_error + '\n' + t('Click for every step of its last connections.'))}">${esc(t('Last check failed: {reason}', {reason:u.sync_error}))}</button>`
      : u?.synced_at ? `<div class="st">${esc(t('Checked {when}', {when:fmtWhen(parseDate(u.synced_at))}))}</div>` : '';
    // The account and how its last check went on top; what can be done with it in a row below.
    return `<div class="sacct">${adot(a.id)}<div class="satext"><div class="sahead"><span class="nm">${esc(a.name)}</span>${sync}</div><div class="em" title="${esc(where)}">${esc(where)}</div>`
      + `<div class="sacts"><button type="button" class="btn" data-edit="${esc(a.id)}">${esc(t('Edit'))}</button><button type="button" class="btn" data-ids="${esc(a.id)}">${esc(t('Identities'))}</button>`
      // A POP3 account's folders are Telex's own: nothing to choose among a server's.
      + (a.imap.pop3 ? '' : `<button type="button" class="btn" data-folders="${esc(a.id)}">${esc(t('Folders'))}</button>`)
      + `<button type="button" class="btn${awayOn(a.id) ? ' on' : ''}" data-away="${esc(a.id)}" title="${esc(awayOn(a.id) ? t('Auto-reply is on') : t('Auto-reply while you’re away'))}">${esc(t('Auto-reply'))}</button>`
      + `<span class="sgap"></span>${order}<button type="button" class="btn danger" data-remove="${esc(a.id)}">${esc(t('Remove'))}</button></div></div></div>`;
  }).join('') : `<p class="dnote">${esc(t('No accounts yet. Add one to start: Telex finds the servers itself.'))}</p>`;
  $('a-list-err').textContent = '';
}

// Tips while the address is typed, from the provider list only (nothing goes online).
let hintTimer;
$('w-email').addEventListener('input', () => {
  clearTimeout(hintTimer);
  hintTimer = setTimeout(async () => {
    let d = null;
    try { d = await invoke('known_provider', {email:$('w-email').value}); } catch {}
    hint($('w-hint'), d);
  }, 200);
});

// The wizard's progress: one line per step.
let steps = [];
function step(text){ steps.push({text, detail:'', status:'work'}); renderSteps(); }
function finishStep(status, text, detail = ''){
  const s = steps[steps.length - 1];
  if (s) Object.assign(s, {status, detail}, text ? {text} : {});
  renderSteps();
}
function renderSteps(){
  $('w-steps').innerHTML = steps.map(s => `<li class="${s.status}"><span>${esc(s.text)}</span>${s.detail ? `<small>${esc(s.detail)}</small>` : ''}</li>`).join('');
}
listenMain('detect-progress', ({payload}) => {
  if (wizardBusy !== 'find' || !steps.length) return;
  const domain = $('w-email').value.trim().split('@')[1] || '';
  steps[steps.length - 1].detail = payload === 'guess' ? t('Trying the usual server names…') : t('Asking {domain} and its DNS records…', {domain});
  renderSteps();
});
const serverNames = s => (names => s.pop3 ? t('{servers} (POP3)', {servers:names}) : names)(s.host === s.smtp_host ? s.host : t('{a} and {b}', {a:s.host, b:s.smtp_host}));

wForm.addEventListener('submit', async e => {
  e.preventDefault();
  if (wizardBusy) return;
  const name = $('w-name').value.trim(), email = $('w-email').value.trim(), password = $('w-pass').value;
  const domain = email.split('@')[1] || '';
  wErr.textContent = '';
  if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email)){ wErr.textContent = t('Enter your email address, e.g. anna@example.com.'); $('w-email').focus(); return; }
  if (!password){ wErr.textContent = t('Enter the password.'); $('w-pass').focus(); return; }
  const seq = ++wizardSeq;
  steps = [];
  $('w-work-title').textContent = t('Setting up {email}', {email});
  $('w-stop').disabled = false;
  showAccountView('work');
  wizardBusy = 'find';
  step(t('Looking for the mail servers of {domain}', {domain}));
  let d = null;
  try { d = await invoke('detect_settings', {email}); } catch {}
  if (seq !== wizardSeq) return; // cancelled meanwhile
  wizardBusy = null;
  if (!d){
    openForm({name, email, password}, t('Telex couldn’t find the mail servers for {domain}. Enter them here: your provider’s help pages list them as IMAP and SMTP servers.', {domain}));
    return;
  }
  if (d.unsupported){ showAccountView('start'); hint($('w-hint'), d); $('w-email').focus(); return; }
  finishStep('done', t('Found {servers}', {servers:serverNames(d.setup)}), SOURCE_TEXT[d.source]?.(d) ?? '');
  const setup = {...d.setup, name, email};
  // Servers outside the address's domain that only DNS named: nothing is sent until the user
  // has looked (a forged DNS answer could have named them).
  if (d.confirm){
    openForm({...setup, password}, '', d, t('Telex found {servers} through your domain’s DNS. They aren’t part of {domain}: check with your provider that they are its servers, then Connect.', {servers:serverNames(d.setup), domain}));
    return;
  }
  wizardBusy = 'login';
  $('w-stop').disabled = true; // the account is saved the moment the login works
  step(t('Logging in to {servers}', {servers:serverNames(d.setup)}));
  let account;
  try { account = await invoke('add_account', {setup, password}); }
  catch (err){
    wizardBusy = null;
    openForm({...setup, password}, tr(err), d);
    return;
  }
  wizardBusy = null;
  finishStep('done');
  await connected(account, false);
});
$('w-manual').addEventListener('click', () => openForm({name:$('w-name').value, email:$('w-email').value, password:$('w-pass').value}));
$('w-cancel').addEventListener('click', () => { ++wizardSeq; wizardBusy = null; showAccountView('home'); $('a-add').focus(); });
$('w-stop').addEventListener('click', () => {
  if (wizardBusy !== 'find') return;
  ++wizardSeq; wizardBusy = null;
  showAccountView('start');
  $('w-email').focus();
});
$('a-add').addEventListener('click', openAccountDialog);

// The settings form, filled in from what the wizard found or typed (v), with a reason.
// note: a tip above the fields, e.g. that the servers came from an imported file.
function openForm(v, error = '', d = null, note = ''){
  aForm.reset();
  $('a-title').textContent = t('Server settings');
  $('a-go').querySelector('span').textContent = t('Connect');
  $('a-email').value = v.email ?? ''; $('a-name').value = v.name ?? '';
  $('a-host').value = v.host ?? ''; $('a-port').value = v.port ?? (v.pop3 ? 995 : 993);
  $('a-isec').value = $('a-psec').value = v.imap_security ?? 'tls';
  setProto(v.pop3);
  $('a-user').value = v.username && v.username !== v.email ? v.username : '';
  $('a-pass').value = v.password ?? ''; $('a-pass').placeholder = '';
  $('a-smtp').value = v.smtp_host ?? ''; $('a-sport').value = v.smtp_port || 465; $('a-sec').value = v.smtp_security ?? 'tls';
  placeholders();
  aErr.textContent = error;
  hint($('a-hint'), d);
  if (note){ $('a-hint').hidden = false; $('a-hint').classList.add('warn'); $('a-hint').textContent = note; }
  if (!sdlg.open || setTab !== 'accts') openSettings('accts');
  showAccountView('form');
  $(!v.email ? 'a-email' : !v.host ? 'a-host' : note ? 'a-pass' : 'a-go').focus();
}
// Fill the form with an account's settings; the password stays empty and is kept if left so.
function editAccount(id){
  const a = ACCT[id]; if (!a?.imap) return;
  editing = id;
  aForm.reset();
  aErr.textContent = ''; hint($('a-hint'), null);
  $('a-title').textContent = t('Edit {name}', {name:a.name});
  $('a-go').querySelector('span').textContent = t('Save');
  $('a-email').value = a.email; $('a-name').value = a.name;
  $('a-host').value = a.imap.host; $('a-port').value = a.imap.port; $('a-user').value = a.imap.username;
  $('a-isec').value = $('a-psec').value = a.imap.security ?? 'tls';
  setProto(a.imap.pop3, true);
  $('a-smtp').value = a.smtp?.host ?? ''; $('a-sec').value = a.smtp?.security ?? 'tls';
  $('a-sport').value = a.smtp?.port ?? 465;
  $('a-pass').value = ''; $('a-pass').placeholder = t('Leave empty to keep the saved password');
  placeholders();
  showAccountView('form');
  $(a.smtp ? 'a-pass' : 'a-smtp').focus();
}
function placeholders(){
  const email = $('a-email').value.trim(), domain = email.split('@')[1] || '';
  $('a-host').placeholder = domain.includes('.') ? t('e.g. mail.{domain}', {domain}) : 'mail.example.com';
  $('a-user').placeholder = email || t('Same as email');
  $('a-smtp').placeholder = $('a-proto').value === 'pop3' ? t('Same as POP3 server') : t('Same as IMAP server');
}
$('a-email').addEventListener('input', placeholders);
// The usual ports follow the security choice, unless the user typed another one.
$('a-sec').addEventListener('change', () => {
  const p = $('a-sport');
  if (['465', '587', ''].includes(p.value)) p.value = $('a-sec').value === 'starttls' ? 587 : 465;
});
$('a-isec').addEventListener('change', () => {
  const p = $('a-port');
  if (['993', '143', ''].includes(p.value)) p.value = $('a-isec').value === 'starttls' ? 143 : 993;
});
$('a-psec').addEventListener('change', () => {
  const p = $('a-port');
  if (['995', '110', ''].includes(p.value)) p.value = $('a-psec').value === 'starttls' ? 110 : 995;
});
// IMAP or POP3: the form shows the fields of the one chosen, and for POP3 what stays on the
// server (`keep`: the account's, or Telex's default: everything, until deleted here). An
// account keeps the protocol it has (`fixed`): its folders are the server's, or here.
function setProto(keep, fixed = false){
  const pop3 = !!keep;
  $('a-proto').value = pop3 ? 'pop3' : 'imap';
  $('a-proto').disabled = fixed;
  aForm.classList.toggle('pop3', pop3);
  const k = keep || {leave:true, days:null, until_deleted:true};
  $('a-leave').checked = k.leave; $('a-until').checked = k.until_deleted;
  $('a-days-on').checked = k.days != null; $('a-days').value = k.days ?? 14;
  paintKeep();
}
// Without leaving mail on the server, how long it stays there doesn't arise.
function paintKeep(){
  const leave = $('a-leave').checked;
  for (const id of ['a-days-on', 'a-until']) $(id).disabled = !leave;
  $('a-days').disabled = !leave || !$('a-days-on').checked;
}
$('a-leave').addEventListener('change', paintKeep);
$('a-days-on').addEventListener('change', paintKeep);
$('a-proto').addEventListener('change', () => {
  const pop3 = $('a-proto').value === 'pop3', p = $('a-port');
  // The usual port of the other protocol, unless the user typed another one.
  if (['993', '143', '995', '110', ''].includes(p.value)){
    const starttls = (pop3 ? $('a-isec') : $('a-psec')).value === 'starttls';
    p.value = pop3 ? (starttls ? 110 : 995) : (starttls ? 143 : 993);
  }
  $('a-isec').value = $('a-psec').value = (pop3 ? $('a-isec') : $('a-psec')).value;
  aForm.classList.toggle('pop3', pop3);
  placeholders();
});
$('a-back').addEventListener('click', () => {
  const back = editing || ACCOUNTS.some(a => a.imap) ? 'home' : 'start';
  editing = null;
  showAccountView(back);
  if (back === 'start') $('w-email').focus();
});
$('s-pane-accts').addEventListener('click', e => {
  const help = e.target.closest('[data-help]');
  if (help) call('open_link', {url:help.dataset.help}).catch(() => {});
});
aForm.addEventListener('submit', async e => {
  e.preventDefault();
  const go = $('a-go'), label = go.querySelector('span');
  if (go.disabled) return;
  const port = Number($('a-port').value), smtpPort = Number($('a-sport').value || 0);
  const badPort = p => !Number.isInteger(p) || p < 0 || p > 65535;
  const pop3 = $('a-proto').value === 'pop3';
  if (badPort(port) || !port){ aErr.textContent = pop3 ? t('The POP3 port is a number from 1 to 65535: usually 995 for SSL/TLS, 110 for STLS.') : t('The IMAP port is a number from 1 to 65535: usually 993 for SSL/TLS, 143 for STARTTLS.'); $('a-port').focus(); return; }
  const days = Number($('a-days').value);
  if (pop3 && $('a-leave').checked && $('a-days-on').checked && (!Number.isInteger(days) || days < 1 || days > 3650)){ aErr.textContent = t('The days are a number from 1 to 3650.'); $('a-days').focus(); return; }
  if (badPort(smtpPort)){ aErr.textContent = t('The SMTP port is a number from 1 to 65535: usually 465 for SSL/TLS, 587 for STARTTLS.'); $('a-sport').focus(); return; }
  const setup = {name:$('a-name').value, email:$('a-email').value, host:$('a-host').value, port, username:$('a-user').value,
    imap_security:(pop3 ? $('a-psec') : $('a-isec')).value, smtp_host:$('a-smtp').value, smtp_port:smtpPort, smtp_security:$('a-sec').value,
    pop3:pop3 ? {leave:$('a-leave').checked, days:$('a-days-on').checked ? days : null, until_deleted:$('a-until').checked} : null};
  const wasEditing = editing;
  aErr.textContent = '';
  go.disabled = true; label.textContent = t('Connecting…');
  wizardBusy = 'login';
  let account;
  try { account = await invoke('add_account', {setup, password:$('a-pass').value}); }
  catch (err){ aErr.textContent = tr(err); return; }
  finally { wizardBusy = null; go.disabled = false; label.textContent = wasEditing ? t('Save') : t('Connect'); }
  await connected(account, !!wasEditing);
});
async function connected(account, wasEditing){
  wForm.reset(); aForm.reset();
  editing = null;
  await loadTree();
  if (wasEditing){
    showAccountView('home');
    refresh();
    say(t('Saved {name}.', {name:account.name}));
    getMail({quiet:true}); // the Outbox may have waited for these settings
    return;
  }
  accountView = 'home';
  sdlg.close();
  setFolder(homeOf(account.id));
  say(t('Connected to {email}. Downloading mail…', {email:account.email}), 0);
  getMail({first:true});
}
$('a-list').addEventListener('click', async e => {
  const why = e.target.closest('[data-conn]');
  if (why){ showConnections(why.dataset.conn, false); return; }
  const up = e.target.closest('[data-up]'), down = e.target.closest('[data-down]');
  if (up || down){
    const id = (up ?? down).dataset[up ? 'up' : 'down'];
    await moveAccount(id, up ? -1 : 1);
    // The focus stays on the button that moved, or on its twin at either end.
    const b = $('a-list').querySelector(`[data-${up ? 'up' : 'down'}="${CSS.escape(id)}"]`);
    (b?.disabled ? $('a-list').querySelector(`[data-${up ? 'down' : 'up'}="${CSS.escape(id)}"]`) : b)?.focus();
    return;
  }
  const ed = e.target.closest('[data-edit]');
  if (ed){ editAccount(ed.dataset.edit); return; }
  const ids = e.target.closest('[data-ids]');
  if (ids){ openIdentities(ids.dataset.ids); return; }
  const fo = e.target.closest('[data-folders]');
  if (fo){ openFolderChoices(fo.dataset.folders); return; }
  const aw = e.target.closest('[data-away]');
  if (aw){ openAway(aw.dataset.away); return; }
  const b = e.target.closest('[data-remove]');
  if (!b) return;
  if (b.dataset.armed !== '1'){ b.dataset.armed = '1'; b.textContent = t('Remove it and its local mail'); return; }
  try { await invoke('remove_account', {id:b.dataset.remove}); } catch (err){ $('a-list-err').textContent = tr(err); return; }
  await refresh();
  renderAccountList();
  say(t('Account removed from Telex. The mail on the server is untouched.'));
});

/* ---------- away reply ---------- */
// Per account, what people get while the user is away. Saved, it goes onto the server where the
// server takes it (ManageSieve): it answers even while Telex is closed, and keeps to the days
// itself. Elsewhere Telex answers new mail when it checks. The status bar says while one is on.
const AWAY_DAYS = [1, 2, 3, 4, 7, 14, 30];
let awayOf = null, awayData = null, awayBusy = false;
const ymd = d => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
// On, and today within its days (or before them: it is set).
const awayOn = id => AWAY.some(a => a.account === id && (!a.ends || a.ends >= ymd(new Date())));
async function openAway(account){
  if (!ACCT[account]){ showAccountView('home'); return; }
  awayOf = account; awayData = null;
  if (!sdlg.open || setTab !== 'accts') openSettings('accts');
  showAccountView('away');
  $('away-title').textContent = t('Auto-reply') + ' · ' + ACCT[account].email;
  $('away-err').textContent = '';
  try { awayData = await invoke('get_away', {account}); } catch (e){ $('away-err').textContent = tr(e); return; }
  if (awayOf !== account) return;
  fillAway(awayData, null);
  $('away-on').focus();
}
function fillAway(a, why){
  $('away-on').checked = a.enabled;
  $('away-from').value = a.starts ?? ''; $('away-until').value = a.ends ?? '';
  $('away-subj').value = a.subject;
  $('away-body').value = a.body || [t('Hello,|away'), '', t('I’m away and will answer your message when I’m back.'), '', t('Best regards|away')].join('\n');
  scheduleSpelling($('away-body'), 0);
  $('away-days').innerHTML = AWAY_DAYS.map(n => `<option value="${n}"${n === a.days ? ' selected' : ''}>${esc(tn(n, 'at most once a day', 'at most once in {n} days'))}</option>`).join('');
  paintAwayWhere(a, why);
}
// Why the server didn't take it, in words.
function awayWhy(why){
  const words = {no_server:t('your server doesn’t take automatic replies from mail programs'), no_tls:t('your server’s filter service doesn’t encrypt the connection'),
    no_vacation:t('your server’s filters can’t send automatic replies'), no_dates:t('your server’s filters can’t keep to dates')}[why.code];
  return words ?? tr(why.detail);
}
function paintAwayWhere(a, why){
  const el = $('away-where');
  let text = '';
  if (why && !a.enabled && a.place === 'server') text = t('Telex couldn’t take it off the server: {why}. Save again to try once more.', {why:awayWhy(why)});
  else if (a.enabled && a.place === 'server') text = t('On the server: it answers even while Telex is closed, and keeps to the days by itself.');
  else if (a.enabled) text = why ? t('Telex answers new mail when it checks, while it is open: {why}.', {why:awayWhy(why)}) : t('Telex answers new mail when it checks, while it is open.');
  el.hidden = !text; el.textContent = text;
  el.classList.toggle('warn', !!why && a.place === 'server' && !a.enabled);
}
$('away-view').addEventListener('submit', async e => {
  e.preventDefault();
  if (awayBusy || !awayData) return;
  const a = {...awayData, enabled:$('away-on').checked, starts:$('away-from').value || null, ends:$('away-until').value || null,
    subject:$('away-subj').value.trim(), body:$('away-body').value, days:Number($('away-days').value)};
  awayBusy = true; $('away-save').disabled = true; $('away-err').textContent = '';
  const where = $('away-where');
  where.hidden = false; where.classList.remove('warn');
  where.textContent = a.enabled ? t('Asking your server whether it can answer for you…') : t('Saving…');
  let r;
  try { r = await invoke('save_away', {away:a}); }
  catch (err){ $('away-err').textContent = tr(err); paintAwayWhere(awayData, null); return; }
  finally { awayBusy = false; $('away-save').disabled = false; }
  awayData = r.away;
  paintAwayWhere(r.away, r.why);
  await loadTree(); renderStatus();
  say(!r.away.enabled ? t('Auto-reply is off.') : r.away.place === 'server' ? t('Auto-reply is on, on the server.') : t('Auto-reply is on: Telex answers while it is open.'));
});
$('away-back').addEventListener('click', () => showAccountView('home'));
// While a reply is on: a chip on the right; a click opens it.
function renderAway(){
  const b = $('st-away'), on = VIEWER ? [] : AWAY.filter(a => awayOn(a.account) && ACCT[a.account]);
  b.hidden = !on.length;
  if (!on.length) return;
  b.textContent = on.length === 1 ? t('Auto-reply on') : tn(on.length, 'Auto-reply on · {n} account', 'Auto-reply on · {n} accounts', {n:num(on.length)});
  b.title = on.map(a => {
    const days = a.starts && a.ends ? t('{from} to {to}', {from:fmtDay(a.starts), to:fmtDay(a.ends)}) : a.ends ? t('until {day}', {day:fmtDay(a.ends)}) : a.starts ? t('from {day}', {day:fmtDay(a.starts)}) : '';
    return [ACCT[a.account].email, days, a.place === 'server' ? t('on the server') : t('Telex answers')].filter(Boolean).join(' · ');
  }).join('\n');
}
const fmtDay = s => new Date(s + 'T12:00:00').toLocaleDateString(I18n.lang() === 'pl' ? 'pl-PL' : 'en-GB');
$('st-away').addEventListener('click', () => { const a = AWAY.find(x => awayOn(x.account)); if (a) openAway(a.account); });

/* ---------- which folder is which ---------- */
// Per account, the server folder Telex uses for sent mail, drafts, archive, spam and trash.
// The core finds them (the server's flags, the names, which holds mail); a choice here wins.
const CHOOSABLE = [['sent', N_('Sent mail')], ['drafts', N_('Drafts')], ['archive', N_('Archive')], ['spam', N_('Spam')], ['trash', N_('Trash')]];
// Who puts a copy of sent mail into the server's Sent: Gmail and Outlook.com file it themselves.
const SENT_COPY = [['auto', N_('Automatically')], ['server', N_('The server keeps one')], ['telex', N_('Telex uploads one')]];
function sentCopyNote(a){
  if (a.sent_copy === 'server') return t('Telex uploads no copy of the mail you send; the server’s own shows in Sent after the next check.');
  if (a.sent_copy === 'telex') return t('Telex uploads a copy of every message you send to Sent.');
  return a.server_files_sent === true ? t('This server files the mail you send in Sent itself, so Telex uploads no copy.')
    : a.server_files_sent === false ? t('This server doesn’t file the mail you send, so Telex uploads a copy to Sent.')
    : t('Telex finds out with the first message you send: when the server files it in Sent itself, Telex uploads no copy.');
}
let foldersOf = null;
function openFolderChoices(account){
  foldersOf = account;
  if (!sdlg.open || setTab !== 'accts') openSettings('accts');
  showFolderChoices();
  $('f-list').querySelector('select')?.focus();
}
function showFolderChoices(){
  const a = ACCT[foldersOf];
  if (!a){ showAccountView('home'); return; }
  showAccountView('folders');
  $('f-err').textContent = '';
  $('f-title').textContent = t('Folders') + ' · ' + a.email;
  // Each folder by its own name on the server, with what it holds here.
  const server = a.folders.filter(f => f.kind !== 'inbox' && f.kind !== 'outbox');
  const label = f => { const n = countOf(a.id + '/' + f.id).total; return f.name + (n ? ` (${num(n)})` : ''); };
  $('f-list').innerHTML = CHOOSABLE.map(([kind, name]) => {
    const now = server.find(f => f.kind === kind);
    const opts = (now ? '' : `<option value="" selected disabled>${esc(t('None on the server'))}</option>`)
      + server.map(f => `<option value="${esc(a.id + '/' + f.id)}"${f === now ? ' selected' : ''}>${esc(label(f))}</option>`).join('');
    return `<label for="f-${kind}">${esc(t(name))}</label><select id="f-${kind}" class="sselect" data-kind="${kind}">${opts}</select>`;
  }).join('')
    + `<label for="f-sentcopy">${esc(t('Copy of sent mail'))}</label><select id="f-sentcopy" class="sselect">`
    + SENT_COPY.map(([mode, name]) => `<option value="${mode}"${(a.sent_copy ?? 'auto') === mode ? ' selected' : ''}>${esc(t(name))}</option>`).join('') + '</select>';
  $('f-sentnote').textContent = sentCopyNote(a);
}
async function chooseFolder(key, kind){
  const name = folderOf(key)?.name ?? key;
  try { await invoke('choose_folder', {folder:key, kind}); } catch (e){ say(tr(e), 8000); return false; }
  await loadTree();
  renderTree(); renderStatus(); renderHead(); drawRows();
  say(kind === 'sent' ? t('Sent mail goes to {folder} from now on.', {folder:name}) : t('{folder} is the {kind} folder from now on.', {folder:name, kind:t(KIND_NAME[kind])}), 5000);
  return true;
}
$('f-list').addEventListener('change', async e => {
  const sel = e.target.closest('select[data-kind]');
  if (!sel?.value) return;
  if (await chooseFolder(sel.value, sel.dataset.kind)) showFolderChoices();
  else $('f-err').textContent = t('That didn’t work. Try again.');
  $('f-' + sel.dataset.kind)?.focus();
});
$('f-list').addEventListener('change', async e => {
  if (e.target.id !== 'f-sentcopy') return;
  try { await invoke('set_sent_copy', {account:foldersOf, mode:e.target.value}); }
  catch (err){ $('f-err').textContent = tr(err); return; }
  await loadTree();
  showFolderChoices();
  $('f-sentcopy')?.focus();
});
$('f-close').addEventListener('click', () => showAccountView('home'));

/* ---------- identities ---------- */
let idAccount = null, identity = null; // identity: the one being edited
function openIdentities(account){
  idAccount = account;
  if (!sdlg.open || setTab !== 'accts') openSettings('accts');
  showIdentityList();
}
function showIdentityList(){
  const a = ACCT[idAccount];
  if (!a){ showAccountView('home'); return; }
  showAccountView('ids');
  $('i-err').textContent = '';
  $('ids-title').textContent = t('Identities') + ' · ' + a.name;
  $('i-list').innerHTML = (a.identities || []).map(i => `<div class="rrule" data-id="${i.id}">
<div class="rtext"><div class="rname">${esc(i.name)}${i.is_default ? ` <span class="tag">${esc(t('Default|identity'))}</span>` : ''}</div>
<div class="rsum">${esc(i.email)}${i.signature ? ' · ' + esc(i.signature.split('\n')[0]) : ''}</div></div>
<button type="button" class="btn" data-i="edit">${esc(t('Edit'))}</button><button type="button" class="btn danger" data-i="delete">${esc(t('Delete'))}</button></div>`).join('');
  $('i-new').focus();
}
function showIdentityForm(existing){
  const a = ACCT[idAccount], base = a.identities.find(i => i.is_default) ?? a.identities[0] ?? {};
  identity = existing ? structuredClone(existing) : {id:0, account:idAccount, name:'', email:a.email, signature:base.signature ?? '',
    signature_html:base.signature_html ?? null, new_template:base.new_template ?? '', reply_template:base.reply_template ?? '',
    reply_header:base.reply_header ?? '', forward_header:base.forward_header ?? '', is_default:false};
  showAccountView('idform');
  $('i-form-err').textContent = '';
  $('i-form-title').textContent = existing ? t('Edit identity') : t('New identity');
  $('i-name').value = identity.name; $('i-email').value = identity.email;
  signatureEditor($('i-sigedit'), identity).setAttribute('aria-labelledby', 'i-sig-label');
  $('i-newt').value = identity.new_template ?? ''; $('i-replyt').value = identity.reply_template ?? '';
  $('i-reply').value = identity.reply_header; $('i-fwd').value = identity.forward_header;
  $('i-default').checked = identity.is_default;
  $('i-default').disabled = existing?.is_default === true; // switch by making another one the default
  $('i-name').focus();
}
$('i-form').addEventListener('submit', async e => {
  e.preventDefault();
  Object.assign(identity, {name:$('i-name').value, email:$('i-email').value, signature_html:$('i-sigedit').querySelector('.hedit').innerHTML,
    new_template:$('i-newt').value, reply_template:$('i-replyt').value,
    reply_header:$('i-reply').value, forward_header:$('i-fwd').value, is_default:$('i-default').checked});
  try { await invoke('save_identity', {identity}); } catch (err){ $('i-form-err').textContent = tr(err); return; }
  await loadTree();
  say(t('Saved {name}.', {name:identity.name.trim()}));
  showIdentityList();
});
$('i-list').addEventListener('click', async e => {
  const b = e.target.closest('[data-i]'), row = e.target.closest('[data-id]');
  if (!b || !row) return;
  const i = ACCT[idAccount].identities.find(x => x.id === Number(row.dataset.id));
  if (!i) return;
  if (b.dataset.i === 'edit'){ showIdentityForm(i); return; }
  if (b.dataset.armed !== '1'){ b.dataset.armed = '1'; b.textContent = t('Delete it'); return; }
  try { await invoke('delete_identity', {id:i.id}); } catch (err){ $('i-err').textContent = tr(err); return; }
  await loadTree();
  showIdentityList();
});
// Settings › Writing: every identity's signature, edited in place with the HTML editor (formatting,
// colors, a picture) and saved as it is typed. One without formatting is kept as plain text.
const sigSaves = new Map(); // identity id → timer
// A signature editor: its formatting bar and the editor, for `box` (a .sigedit).
function signatureEditor(box, identity){
  box.innerHTML = fbarHtml() + `<div class="hedit" contenteditable="true" role="textbox" aria-multiline="true" data-signature="1" spellcheck="false"
 aria-label="${esc(t('Signature for {email}', {email:identity.email}))}" data-placeholder="${esc(t('No signature'))}"></div>`;
  const ed = box.querySelector('.hedit');
  ed.innerHTML = signatureHtml(identity);
  if (!ed.textContent.trim() && !ed.querySelector('img')) ed.innerHTML = '';
  wireEditor(ed, box.querySelector('.fbar'), {signature:true});
  spellField(ed);
  return ed;
}
function renderSignatures(){
  const el = $('s-sigs');
  const ids = allIdentities();
  if (!ids.length){ el.innerHTML = `<p class="dnote">${esc(t('Add an account first: each address you write from gets its signature here.'))}</p>`; return; }
  const several = servers().length > 1;
  el.innerHTML = ids.map(i => `<div class="sig" data-sig="${i.id}" data-account="${esc(i.account)}">
<div class="sighead"><b>${esc(i.name || i.email)}</b><span class="sdim">${esc(i.email)}${several && i.email !== ACCT[i.account]?.email ? ' · ' + esc(ACCT[i.account]?.email ?? '') : ''}</span>
<span class="sigstate" aria-live="polite"></span><button type="button" class="linkbtn" data-sig-edit>${esc(t('Edit identity|button'))}</button></div>
<div class="sigedit"></div></div>`).join('');
  el.querySelectorAll('.sig').forEach(card => signatureEditor(card.querySelector('.sigedit'), ids.find(i => i.id === Number(card.dataset.sig))));
}
async function saveSignature(card){
  clearTimeout(sigSaves.get(card.dataset.sig)); sigSaves.delete(card.dataset.sig);
  const own = ACCT[card.dataset.account]?.identities?.find(i => i.id === Number(card.dataset.sig));
  const html = card.querySelector('.hedit').innerHTML, state = card.querySelector('.sigstate');
  if (!own || signatureHtml(own) === html) return;
  try {
    const saved = await invoke('save_identity', {identity:{...own, signature_html:html}});
    // Compose takes it from here, no reload needed.
    Object.assign(own, {signature:saved?.signature ?? own.signature, signature_html:saved ? saved.signature_html : html});
    state.textContent = t('Saved');
  } catch (err){ state.textContent = tr(err); }
}
$('s-sigs').addEventListener('input', e => {
  const card = e.target.closest('.sig');
  if (!card) return;
  card.querySelector('.sigstate').textContent = '';
  clearTimeout(sigSaves.get(card.dataset.sig));
  sigSaves.set(card.dataset.sig, setTimeout(() => saveSignature(card), 700));
});
$('s-sigs').addEventListener('focusout', e => { const card = e.target.closest('.sig'); if (card && e.target.matches('.hedit')) saveSignature(card); });
$('s-sigs').addEventListener('click', e => {
  const card = e.target.closest('[data-sig-edit]')?.closest('.sig');
  if (!card) return;
  saveSignature(card);
  const i = ACCT[card.dataset.account]?.identities?.find(x => x.id === Number(card.dataset.sig));
  openIdentities(card.dataset.account);
  if (i) showIdentityForm(i);
});
/* ---------- folder templates ---------- */
// Settings › Writing: a folder's own greeting for new messages written while it is open and for
// replies to mail in it, and its own reply header; what is left empty is the identity's. Saved as
// typed; all empty takes them away. From a folder's menu: Templates….
let FTPL = null, ftplAsked = null; // [{folder, new_template, reply_template, reply_header}], from the core
const loadTemplates = () => ftplAsked ??= invoke('folder_templates').catch(() => []).then(r => { FTPL = r; });
const forgetTemplates = () => { FTPL = null; ftplAsked = null; };
const ftplSaves = new Map(); // folder key → timer
const templateFolders = () => [...servers(), ...ACCOUNTS.filter(a => a.id === LOCAL)].flatMap(a => a.folders
  .filter(f => !['outbox', 'drafts', 'trash', 'spam'].includes(f.kind)).map(f => a.id + '/' + f.id));
const folderWhere = key => isLocal(key) ? placeName(key) : `${folderName(key)} · ${ACCT[acctId(key)]?.email ?? ''}`;
async function renderFolderTemplates(focusOn = null){
  const el = $('s-ftpl'), add = $('s-ftpl-add');
  await loadTemplates();
  if (focusOn && !FTPL.some(x => x.folder === focusOn)) FTPL.push({folder:focusOn, new_template:'', reply_template:'', reply_header:''});
  el.innerHTML = FTPL.map(x => `<div class="sig ftpl" data-ftpl="${esc(x.folder)}">
<div class="sighead"><b>${esc(placeName(x.folder))}</b><span class="sdim">${esc(isLocal(x.folder) ? '' : ACCT[acctId(x.folder)]?.email ?? '')}</span>
<span class="sigstate" aria-live="polite"></span><button type="button" class="linkbtn" data-ftpl-drop>${esc(t('Remove|templates'))}</button></div>
<div class="ffields"><label>${esc(t('New messages open with'))}</label><textarea rows="2" data-k="new_template" spellcheck="false" placeholder="${esc(t('As the identity says'))}">${esc(x.new_template)}</textarea>
<label>${esc(t('Replies open with'))}</label><textarea rows="2" data-k="reply_template" spellcheck="false" placeholder="${esc(t('As the identity says'))}">${esc(x.reply_template)}</textarea>
<label>${esc(t('Reply header'))}</label><input data-k="reply_header" autocomplete="off" spellcheck="false" placeholder="${esc(t('As the identity says'))}" value="${esc(x.reply_header)}"></div></div>`).join('')
    || `<p class="dnote">${esc(t('No folder has templates of its own.'))}</p>`;
  const taken = new Set(FTPL.map(x => x.folder));
  add.innerHTML = `<option value="">${esc(t('Templates for a folder…'))}</option>` + templateFolders().filter(k => !taken.has(k))
    .map(k => `<option value="${esc(k)}">${esc(folderWhere(k))}</option>`).join('');
  if (focusOn){
    const card = el.querySelector(`[data-ftpl="${CSS.escape(focusOn)}"]`);
    card?.scrollIntoView({block:'nearest'}); card?.querySelector('textarea')?.focus();
  }
}
async function saveFolderTemplate(card){
  const key = card.dataset.ftpl;
  clearTimeout(ftplSaves.get(key)); ftplSaves.delete(key);
  const x = FTPL?.find(f => f.folder === key);
  if (!x) return;
  const next = {folder:key};
  card.querySelectorAll('[data-k]').forEach(f => { next[f.dataset.k] = f.value; });
  if (['new_template', 'reply_template', 'reply_header'].every(k => next[k] === x[k])) return;
  try { await invoke('save_folder_template', {template:next}); Object.assign(x, next); card.querySelector('.sigstate').textContent = t('Saved'); }
  catch (err){ card.querySelector('.sigstate').textContent = tr(err); }
}
$('s-ftpl').addEventListener('input', e => {
  const card = e.target.closest('.ftpl');
  if (!card) return;
  card.querySelector('.sigstate').textContent = '';
  clearTimeout(ftplSaves.get(card.dataset.ftpl));
  ftplSaves.set(card.dataset.ftpl, setTimeout(() => saveFolderTemplate(card), 700));
});
$('s-ftpl').addEventListener('focusout', e => { const card = e.target.closest('.ftpl'); if (card && e.target.dataset.k) saveFolderTemplate(card); });
$('s-ftpl').addEventListener('click', async e => {
  const card = e.target.closest('[data-ftpl-drop]')?.closest('.ftpl');
  if (!card) return;
  const key = card.dataset.ftpl;
  clearTimeout(ftplSaves.get(key));
  try { await invoke('save_folder_template', {template:{folder:key, new_template:'', reply_template:'', reply_header:''}}); } catch (err){ card.querySelector('.sigstate').textContent = tr(err); return; }
  FTPL = FTPL.filter(x => x.folder !== key);
  renderFolderTemplates();
});
$('s-ftpl-add').addEventListener('change', e => { const key = e.target.value; if (key) renderFolderTemplates(key); });
// A folder's menu › Templates…: Settings › Writing, at that folder's templates.
function openFolderTemplates(key){
  openSettings('write');
  renderFolderTemplates(key);
}
$('i-new').addEventListener('click', () => showIdentityForm(null));
$('i-back').addEventListener('click', showIdentityList);
$('i-close').addEventListener('click', () => showAccountView('home'));

/* ---------- rules ---------- */
/* Saved searches use the same list queries as the search box, stored by the core. */
let SAVED_SEARCHES = [], editingSearch = null;
function savedSearchGroup(){
  if (!ACCOUNTS.length) return '';
  return `<div class="agrp saved-searches"><button type="button" class="acct" data-act="saved-searches">${I.search}<span>${esc(t('Saved searches'))}</span></button><ul>`
    + SAVED_SEARCHES.map(s => `<li><button type="button" class="fold${state.savedSearch === s.id ? ' sel' : ''}" data-search="${s.id}" title="${esc(s.view.filter)}"${state.savedSearch === s.id ? ' aria-current="page"' : ''}>${I.search}<span>${esc(s.name)}</span></button></li>`).join('') + '</ul></div>';
}
async function openSavedSearch(id){
  const search = SAVED_SEARCHES.find(s => s.id === id);
  if (!search) return;
  const v = search.view;
  if (v.folder && !folderOf(v.folder)){ say(t('The folder for this saved search no longer exists.'), 7000); return; }
  state.savedSearch = id;
  state.folder = v.folder ?? (folderOf(state.folder) ? state.folder : homeOf(ACCOUNTS[0]?.id));
  state.scope = v.folder ? 'folder' : 'all';
  state.q = v.filter; qEl.value = v.filter; state.inText = v.bodies;
  state.thread = null; state.unread = v.unread; state.kept.clear(); state.expanded.clear(); unpick();
  state.sort = {key:v.sort,dir:v.ascending ? 1 : -1};
  paintUnreadOnly(); paintScope(); renderTree(); renderStatus();
  await loadView({top:true}); listEl.focus({preventScroll:true});
}
function showSavedSearchList(){
  $('ss-form').hidden = true; $('ss-list').hidden = false; $('ss-new').hidden = false;
  $('ss-error').textContent = '';
  $('ss-list').innerHTML = SAVED_SEARCHES.length ? SAVED_SEARCHES.map(s => `<div class="rrule" data-id="${s.id}"><div class="rtext"><b>${esc(s.name)}</b><div class="rsum">${esc(s.view.filter)} · ${esc(s.view.folder ? placeName(s.view.folder) : t('All mail'))}</div></div><button type="button" class="btn" data-ss="open">${esc(t('Open'))}</button><button type="button" class="btn" data-ss="edit">${esc(t('Edit'))}</button><button type="button" class="btn danger" data-ss="delete">${esc(t('Delete'))}</button></div>`).join('') : `<p class="dnote">${esc(t('Save a search to open it again from the folder list.'))}</p>`;
}
function editSavedSearch(existing = null){
  editingSearch = existing ? structuredClone(existing) : {id:0,name:'',view:{...viewArgs(),filter:state.q || 'is:unread',bodies:true,thread:null,keep:[],expanded:[],grouped:false}};
  const v = editingSearch.view;
  $('ss-name').value = editingSearch.name; $('ss-query').value = v.filter;
  $('ss-bodies').checked = v.bodies; $('ss-unread').checked = v.unread;
  const folder = v.folder ?? (state.folder === UNREAD ? null : state.folder);
  $('ss-scope').innerHTML = `<option value="">${esc(t('All mail'))}</option>` + (folder ? `<option value="${esc(folder)}">${esc(placeName(folder))}</option>` : '');
  $('ss-scope').value = v.folder ?? '';
  $('ss-list').hidden = true; $('ss-form').hidden = false; $('ss-new').hidden = true; $('ss-error').textContent = '';
  $('ss-name').focus();
}
async function openSavedSearches(create = false){
  closeSuggest();
  try { SAVED_SEARCHES = await invoke('list_saved_searches'); } catch (e){ say(tr(e),7000); return; }
  $('search-dlg').showModal();
  if (create) editSavedSearch(); else showSavedSearchList();
}
$('q-save').addEventListener('click', () => openSavedSearches(true));
$('ss-new').addEventListener('click', () => editSavedSearch());
$('ss-back').addEventListener('click', showSavedSearchList);
$('ss-close').addEventListener('click', () => $('search-dlg').close());
$('search-dlg').addEventListener('close', () => listEl.focus({preventScroll:true}));
$('ss-form').addEventListener('submit', async e => {
  e.preventDefault();
  const search = {...editingSearch,name:$('ss-name').value,view:{...editingSearch.view,filter:$('ss-query').value,folder:$('ss-scope').value || null,bodies:$('ss-bodies').checked,unread:$('ss-unread').checked}};
  try { await invoke('save_search',{search}); SAVED_SEARCHES = await invoke('list_saved_searches'); }
  catch (error){ $('ss-error').textContent = tr(error); return; }
  renderTree(); showSavedSearchList();
});
$('ss-list').addEventListener('click', async e => {
  const button = e.target.closest('[data-ss]'), row = e.target.closest('[data-id]');
  if (!button || !row) return;
  const search = SAVED_SEARCHES.find(s => s.id === Number(row.dataset.id)); if (!search) return;
  if (button.dataset.ss === 'edit'){ editSavedSearch(search); return; }
  if (button.dataset.ss === 'open'){ $('search-dlg').close(); await openSavedSearch(search.id); return; }
  if (button.dataset.armed !== '1'){ button.dataset.armed = '1'; button.textContent = t('Delete it'); return; }
  try { await invoke('delete_search',{id:search.id}); SAVED_SEARCHES = await invoke('list_saved_searches'); }
  catch (error){ $('ss-error').textContent = tr(error); return; }
  if (state.savedSearch === search.id) state.savedSearch = null;
  renderTree(); showSavedSearchList();
});

/* ---------- address book ---------- */
// B, the toolbar's Contacts, or the person button by a sender. Groups and contacts on the left,
// the one chosen on the right: / searches, N adds, Del removes (Z puts back), ↑ ↓ choose, Enter
// goes to its fields, where Enter (Ctrl+Enter in the note) saves; leaving a changed one saves it.
// People written to who aren't contacts are offered at the end, never added by themselves.
const abdlg = $('ab-dlg');
let BOOK = null, bookAsked = null; // {contacts, groups} from the core
function loadBook(){
  if (!bookAsked) bookAsked = Promise.all([invoke('contacts'), invoke('contact_groups')])
    .then(([contacts, groups]) => { BOOK = {contacts, groups}; }).catch(() => { bookAsked = null; });
  return bookAsked;
}
// After a change: the book again, compose's suggestions, and the names above messages.
async function bookChanged(){
  bookAsked = null; forgetRecipients(); bodies.clear();
  await loadBook();
  if (!state.compose && current()) renderPreview();
}
const sameEmail = (a, b) => String(a).toLowerCase() === String(b).toLowerCase();
const contactOf = email => email ? BOOK?.contacts.find(c => c.emails.some(e => sameEmail(e, email))) ?? null : null;
// sel: the entry on the right ('c12', 'g3', 's' + an address offered, 'new', 'newgroup');
// picked: more contacts for Del and Export (Ctrl+click, Shift+click).
const ab = {sel:null, picked:new Set(), q:'', draft:null, dirty:false, undo:null, offers:[], err:'', gs:{list:[], at:0}};
let abMsgTimer;
function abSay(text, ms = 6000){
  const el = $('ab-msg');
  el.textContent = text; el.title = text;
  clearTimeout(abMsgTimer);
  if (ms) abMsgTimer = setTimeout(() => { el.textContent = ''; el.title = ''; }, ms);
}
// The list's entries in order: groups, contacts, then people to offer.
function abEntries(){
  const q = fold(ab.q.trim()), hit = (...s) => !q || s.some(x => fold(x ?? '').includes(q));
  const groups = (BOOK?.groups ?? []).filter(g => hit(g.name)).map(g => ({key:'g' + g.id, kind:'group', g}));
  const contacts = (BOOK?.contacts ?? []).filter(c => hit(c.name, c.organization, c.phone, ...c.emails)).map(c => ({key:'c' + c.id, kind:'contact', c}));
  const offers = ab.offers.filter(p => !contactOf(p.email) && hit(p.name, p.email)).map(p => ({key:'s' + p.email.toLowerCase(), kind:'offer', p}));
  return {groups, contacts, offers, all:[...groups, ...contacts, ...offers]};
}
const abFind = key => abEntries().all.find(e => e.key === key) ?? null;
function abLoadDraft(person){
  const s = ab.sel ?? '';
  ab.err = ''; ab.dirty = false; ab.gs = {list:[], at:0};
  if (s === 'new') ab.draft = {id:0, uid:'', name:person?.name && !sameEmail(person.name, person.email) ? person.name : '', emails:[person?.email ?? ''], phone:'', organization:'', note:'', groups:[]};
  else if (s === 'newgroup') ab.draft = {id:0, name:'', members:[]};
  else if (s[0] === 'c') ab.draft = structuredClone(BOOK?.contacts.find(c => 'c' + c.id === s) ?? null);
  else if (s[0] === 'g') ab.draft = structuredClone(BOOK?.groups.find(g => 'g' + g.id === s) ?? null);
  else ab.draft = null;
}
function renderAbList(){
  const {groups, contacts, offers, all} = abEntries();
  const head = (label, n) => `<li class="abh" role="presentation">${esc(label)}${n != null ? ` <small>${esc(num(n))}</small>` : ''}</li>`;
  let i = 0;
  const item = e => {
    const [nm, em] = e.kind === 'group' ? [e.g.name, tn(e.g.members.length, '{n} person', '{n} people')]
      : e.kind === 'contact' ? [e.c.name || e.c.emails[0] || e.c.phone, e.c.name ? e.c.emails[0] ?? e.c.organization : e.c.organization]
      : [e.p.name || e.p.email, e.p.name ? e.p.email : ''];
    const on = e.key === ab.sel || ab.picked.has(e.key);
    return `<li role="option" id="ab-i-${i++}" data-k="${esc(e.key)}" aria-selected="${on}" class="${e.kind}">${e.kind === 'group' ? I.people : ''}`
      + `<span class="nm">${esc(nm)}</span>${em ? `<span class="em">${esc(em)}</span>` : ''}`
      + (e.kind === 'offer' ? `<button type="button" class="linkbtn" data-ab="take" tabindex="-1">${esc(t('Add'))}</button>` : '') + '</li>';
  };
  const parts = [];
  if (groups.length) parts.push(head(t('Groups')), ...groups.map(item));
  if (contacts.length || !all.length) parts.push(head(t('Contacts'), BOOK?.contacts.length ?? 0), ...contacts.map(item));
  if (offers.length) parts.push(head(t('People you write to')), ...offers.map(item));
  if (!all.length) parts.push(`<li class="none" role="presentation">${esc(ab.q.trim() ? t('Nothing matches.') : t('No contacts yet. N adds one; Import vCard brings them from a file.'))}</li>`);
  const list = $('ab-items');
  list.innerHTML = parts.join('');
  const sel = list.querySelector(`[data-k="${CSS.escape(ab.sel ?? '')}"]`);
  if (sel){ list.setAttribute('aria-activedescendant', sel.id); sel.scrollIntoView({block:'nearest'}); }
  else list.removeAttribute('aria-activedescendant');
  const picked = [...ab.picked].filter(k => k[0] === 'c').length;
  $('ab-export').textContent = picked ? tn(picked, 'Export {n} selected…', 'Export {n} selected…') : t('Export vCard…');
}
function contactForm(d){
  const groups = BOOK?.groups ?? [];
  const mails = (d.emails.length ? d.emails : ['']).map((e, i) => `<div class="abmail"><input data-mail="${i}" value="${esc(e)}" aria-label="${esc(i ? t('Address {n}', {n:i + 1}) : t('Primary address'))}" autocomplete="off" spellcheck="false">`
    + (i ? `<button type="button" class="linkbtn" data-ab="primary" data-i="${i}">${esc(t('Make primary'))}</button>` : `<span class="abtag">${esc(t('primary|address'))}</span>`)
    + `<button type="button" class="btn icon" data-ab="rm-mail" data-i="${i}" title="${esc(t('Remove this address'))}" aria-label="${esc(t('Remove this address'))}">×</button></div>`).join('');
  return `<h3 class="rh">${esc(d.id ? t('Contact') : t('New contact'))}</h3>
<div class="cf df abform">
  <label for="ab-name">${esc(t('Name|person'))}</label><input id="ab-name" value="${esc(d.name)}" autocomplete="off" spellcheck="false">
  <label>${esc(t('Addresses'))}</label><div class="abmails">${mails}<button type="button" class="linkbtn radd" data-ab="add-mail">${esc(t('+ Address'))}</button></div>
  <label for="ab-phone">${esc(t('Phone'))}</label><input id="ab-phone" value="${esc(d.phone)}" autocomplete="off" spellcheck="false">
  <label for="ab-org">${esc(t('Organization'))}</label><input id="ab-org" value="${esc(d.organization)}" autocomplete="off" spellcheck="false">
  <label for="ab-note">${esc(t('Note'))}</label><textarea id="ab-note" rows="3">${esc(d.note)}</textarea>
  ${groups.length ? `<label>${esc(t('Groups'))}</label><div class="abgroups">${groups.map(g => `<label class="sopt"><input type="checkbox" data-group="${g.id}"${d.groups.includes(g.id) ? ' checked' : ''}><span>${esc(g.name)}</span></label>`).join('')}</div>` : ''}
</div>`;
}
function groupForm(d){
  const members = d.members.map((m, i) => `<li><span class="nm">${esc(m.name || m.email)}</span>${m.name && m.email ? `<span class="em">${esc(m.email)}</span>` : ''}`
    + `${m.contact ? '' : `<span class="abtag">${esc(t('not a contact'))}</span>`}<button type="button" class="btn icon" data-ab="rm-member" data-i="${i}" title="${esc(t('Take out of the group'))}" aria-label="${esc(t('Take {name} out of the group', {name:m.name || m.email}))}">×</button></li>`).join('');
  return `<h3 class="rh">${esc(d.id ? t('Group') : t('New group'))}</h3>
<div class="cf df abform">
  <label for="ab-gname">${esc(t('Name'))}</label><input id="ab-gname" value="${esc(d.name)}" autocomplete="off" spellcheck="false">
  <label for="ab-gadd">${esc(t('Add'))}</label><div class="abadd"><input id="ab-gadd" role="combobox" aria-expanded="false" aria-controls="ab-gsugg" aria-autocomplete="list" placeholder="${esc(t('A contact’s name or an address, then Enter'))}" autocomplete="off" spellcheck="false"><ul class="sugg" id="ab-gsugg" role="listbox" hidden></ul></div>
</div>
<ul class="abmembers">${members || `<li class="none">${esc(t('No one in it yet.'))}</li>`}</ul>
<p class="dnote">${esc(t('Type the group’s name in To, Cc or Bcc and it goes in as these addresses.'))}</p>`;
}
function offerPane(p){
  return `<h3 class="rh">${esc(t('Someone you write to'))}</h3>
<div class="abperson"><b>${esc(p.name || p.email)}</b>${p.name ? ` <span class="em">${esc(p.email)}</span>` : ''}</div>
<p class="dnote">${esc(tn(p.sent, 'You wrote to them {n} time.', 'You wrote to them {n} times.', {n:num(p.sent)}))} ${esc(t('Telex offers the people you write to; it adds no one by itself.'))}</p>
<div class="sbtns"><button type="button" class="btn primary" data-ab="take">${esc(t('Add to address book'))}</button><button type="button" class="btn" data-ab="write">${esc(t('Write to them'))}</button></div>`;
}
function renderAbEdit(){
  const el = $('ab-edit'), d = ab.draft, s = ab.sel ?? '';
  if (s[0] === 's'){ const e = abFind(s); el.innerHTML = e ? offerPane(e.p) : ''; return; }
  if (!d){ el.innerHTML = `<p class="dnote abempty">${esc(t('Choose someone on the left, or press N for a new contact.'))}</p>`; return; }
  const group = 'members' in d;
  const reach = group ? d.members.some(m => m.email) : d.emails.some(e => e.trim());
  el.innerHTML = (group ? groupForm(d) : contactForm(d)) + `<p class="derr" role="alert">${esc(ab.err)}</p>
<div class="sbtns abbtns"><button type="button" class="btn primary" data-ab="save">${esc(t('Save'))}</button>`
    + (reach ? `<button type="button" class="btn" data-ab="write">${esc(group ? t('Write to the group') : t('Write to them'))}</button>` : '')
    + (d.id ? `<button type="button" class="btn danger" data-ab="delete">${esc(group ? t('Delete group') : t('Delete'))} <kbd>Del</kbd></button>` : '') + '</div>';
}
function renderAb(){ renderAbList(); renderAbEdit(); }
// What the fields say now, into the draft.
function abRead(){
  const d = ab.draft, el = $('ab-edit');
  if (!d || !el.querySelector('.abform')) return;
  if ('members' in d){ d.name = $('ab-gname').value; return; }
  d.name = $('ab-name').value; d.phone = $('ab-phone').value; d.organization = $('ab-org').value; d.note = $('ab-note').value;
  d.emails = [...el.querySelectorAll('[data-mail]')].map(i => i.value);
  d.groups = [...el.querySelectorAll('[data-group]')].filter(c => c.checked).map(c => Number(c.dataset.group));
}
async function abSave(){
  if (!ab.draft) return true;
  abRead();
  const d = ab.draft, group = 'members' in d;
  let saved;
  try {
    saved = group ? await invoke('save_group', {group:{id:d.id, name:d.name, members:d.members.map(m => ({contact:m.contact ?? null, name:m.name ?? '', email:m.email ?? ''}))}})
      : await invoke('save_contact', {contact:{...d, emails:d.emails.filter(e => e.trim())}});
  } catch (e){ ab.err = tr(e); renderAbEdit(); return false; }
  ab.dirty = false; ab.err = '';
  ab.sel = (group ? 'g' : 'c') + saved.id;
  await bookChanged();
  abLoadDraft();
  renderAb();
  abSay(t('Saved.'), 3000);
  return true;
}
// Another entry on the right; a changed one is saved first (and stays when it can't be).
async function abSelect(key){
  if (key === ab.sel) return true;
  if (ab.dirty && !await abSave()) return false;
  ab.sel = key; abLoadDraft(); renderAb();
  return true;
}
async function abNew(kind, person){
  if (ab.dirty && !await abSave()) return;
  ab.picked.clear(); ab.sel = kind; abLoadDraft(person); renderAb();
  $(kind === 'new' ? 'ab-name' : 'ab-gname')?.focus();
}
async function abDelete(){
  const keys = [...new Set([...ab.picked, ab.sel].filter(Boolean))];
  const ids = keys.filter(k => k[0] === 'c').map(k => Number(k.slice(1)));
  const group = keys.find(k => k[0] === 'g');
  const before = abEntries().all.findIndex(e => e.key === ab.sel);
  if (ids.length){
    let gone;
    try { gone = await invoke('delete_contacts', {ids}); } catch (e){ abSay(tr(e), 8000); return; }
    ab.undo = {contacts:gone};
    abSay(gone.length === 1 ? t('Deleted {name}. Press Z to undo.', {name:gone[0].name || gone[0].emails[0] || ''}) : tn(gone.length, 'Deleted {n} contact. Press Z to undo.', 'Deleted {n} contacts. Press Z to undo.'), 10000);
  } else if (group){
    let gone;
    try { gone = await invoke('delete_group', {id:Number(group.slice(1))}); } catch (e){ abSay(tr(e), 8000); return; }
    if (gone){ ab.undo = {group:gone}; abSay(t('Deleted the group {name}. Press Z to undo.', {name:gone.name}), 10000); }
  } else if (ab.sel === 'new' || ab.sel === 'newgroup'){ ab.dirty = false; }
  else return;
  ab.picked.clear(); ab.dirty = false;
  await bookChanged();
  const after = abEntries().all;
  ab.sel = after[Math.max(0, Math.min(before, after.length - 1))]?.key ?? null;
  abLoadDraft(); renderAb();
  $('ab-items').focus();
}
async function abUndo(){
  const u = ab.undo; if (!u) return;
  try { if (u.contacts) await invoke('restore_contacts', {contacts:u.contacts}); else await invoke('restore_group', {group:u.group}); }
  catch (e){ abSay(tr(e), 8000); return; }
  ab.undo = null;
  await bookChanged();
  const back = u.contacts ? contactOf(u.contacts[0].emails[0]) ?? BOOK?.contacts.find(c => c.id === u.contacts[0].id) : BOOK?.groups.find(g => g.name === u.group.name);
  if (back) ab.sel = (u.contacts ? 'c' : 'g') + back.id;
  abLoadDraft(); renderAb();
  abSay(u.contacts ? tn(u.contacts.length, 'Put back {n} contact.', 'Put back {n} contacts.') : t('Put back the group {name}.', {name:u.group.name}), 4000);
}
// The person offered (or the one in a message) as a new contact, ready to save.
function abTake(key){
  const e = abFind(key);
  if (e?.kind === 'offer') abNew('new', {name:e.p.name, email:e.p.email});
}
function abWrite(){
  const d = ab.draft, s = ab.sel ?? '';
  const to = s[0] === 's' ? [abFind(s)?.p].filter(Boolean)
    : !d ? [] : 'members' in d ? d.members.filter(m => m.email) : d.emails.filter(e => e.trim()).slice(0, 1).map(email => ({name:d.name, email}));
  if (!to.length) return;
  abdlg.close();
  openCompose('new', {to:to.map(addressOf).join(', ')});
}
async function abImport(){
  let r;
  try { r = await invoke('import_vcards', {words:{title:t('Import contacts'), filter:t('vCard files')}}); }
  catch (e){ abSay(tr(e), 8000); return; }
  if (!r) return; // cancelled
  await bookChanged();
  if (!ab.sel || !abFind(ab.sel)){ ab.sel = abEntries().all[0]?.key ?? null; abLoadDraft(); }
  renderAb();
  abSay(bookReport(r), 10000);
}
const bookReport = r => [r.added ? tn(r.added, '{n} new contact', '{n} new contacts', {n:num(r.added)}) : '',
  r.updated ? tn(r.updated, '{n} contact filled in', '{n} contacts filled in', {n:num(r.updated)}) : '',
  r.groups ? tn(r.groups, '{n} new group', '{n} new groups') : '',
  r.same ? tn(r.same, '{n} was in the address book already', '{n} were in the address book already', {n:num(r.same)}) : ''].filter(Boolean).join(', ') || t('Nothing new.');
async function abExport(){
  const ids = [...ab.picked].filter(k => k[0] === 'c').map(k => Number(k.slice(1)));
  if (!ids.length && !BOOK?.contacts.length){ abSay(t('There are no contacts to export yet.')); return; }
  let path;
  try { path = await invoke('export_vcards', {ids, words:{title:t('Export contacts'), filter:t('vCard files'), file_name:t('Contacts {date}', {date:ymd(new Date())}) + '.vcf'}}); }
  catch (e){ abSay(tr(e), 8000); return; }
  if (path) abSay(t('Saved to {path}.', {path}), 8000);
}
// Adding to a group: contacts and people written to whose name or address starts with what is
// typed; Enter takes the one chosen, or the address typed.
function abGroupSuggest(){
  const input = $('ab-gadd'), box = $('ab-gsugg');
  if (!input || !box) return;
  const typed = fold(input.value.trim()), d = ab.draft;
  const have = new Set(d.members.map(m => m.contact ? 'c' + m.contact : String(m.email).toLowerCase()));
  const list = [];
  if (typed){
    for (const c of BOOK?.contacts ?? []){
      if (list.length >= 8) break;
      if (!c.emails.length || have.has('c' + c.id)) continue;
      const words = fold(c.name + ' ' + c.emails.join(' ').replace(/[@.]/g, ' ')).split(/\s+/);
      if (words.some(w => w.startsWith(typed))) list.push({contact:c.id, name:c.name, email:c.emails[0]});
    }
    for (const p of RECIPS ?? []){
      if (list.length >= 8) break;
      if (p.contact || have.has(p.email.toLowerCase()) || !matchesTyped(p, typed)) continue;
      list.push({contact:null, name:p.name, email:p.email});
    }
  }
  ab.gs = {list, at:Math.min(ab.gs.at, Math.max(0, list.length - 1))};
  box.hidden = !list.length;
  input.setAttribute('aria-expanded', String(!!list.length));
  box.innerHTML = list.map((m, i) => `<li role="option" id="ab-gs-${i}" data-i="${i}" aria-selected="${i === ab.gs.at}"><span class="nm">${m.contact ? `<span class="ct" title="${esc(t('In your address book'))}">${I.person}</span>` : ''}${esc(m.name || m.email)}</span>${m.name ? `<span class="em">${esc(m.email)}</span>` : ''}</li>`).join('');
  if (list.length) input.setAttribute('aria-activedescendant', 'ab-gs-' + ab.gs.at); else input.removeAttribute('aria-activedescendant');
}
function abAddMember(m){
  abRead();
  const d = ab.draft;
  if (!m){
    const typed = $('ab-gadd').value.trim(), parsed = addresses(typed)[0];
    if (!parsed || !parsed.email.includes('@')){ ab.err = typed ? t('Choose a contact, or type an address.') : ''; renderAbEdit(); $('ab-gadd')?.focus(); return; }
    const c = contactOf(parsed.email);
    m = c ? {contact:c.id, name:c.name, email:c.emails[0]} : {contact:null, name:parsed.name, email:parsed.email};
  }
  if (!d.members.some(x => m.contact ? x.contact === m.contact : !x.contact && sameEmail(x.email, m.email))) d.members.push(m);
  ab.dirty = true; ab.err = ''; ab.gs = {list:[], at:0};
  renderAbEdit();
  $('ab-gadd')?.focus();
}
async function openAddressBook(person){
  if (VIEWER) return;
  closeSuggestions();
  await loadBook();
  if (!BOOK){ say(t('The address book didn’t open. Try again.')); return; }
  ab.q = ''; $('ab-q').value = ''; ab.picked.clear(); ab.err = ''; ab.dirty = false;
  invoke('suggested_contacts').then(list => { ab.offers = list; if (abdlg.open) renderAbList(); }).catch(() => {});
  loadRecipients();
  const known = contactOf(person?.email);
  ab.sel = known ? 'c' + known.id : person?.email ? 'new' : ab.sel && abFind(ab.sel) ? ab.sel : abEntries().all[0]?.key ?? null;
  abLoadDraft(person);
  if (!abdlg.open) abdlg.showModal();
  renderAb();
  if (ab.sel === 'new') $('ab-name')?.focus(); else $('ab-items').focus();
}
$('ab-close').addEventListener('click', async () => { if (!ab.dirty || await abSave()) abdlg.close(); });
abdlg.addEventListener('cancel', e => {
  if (ab.gs.list.length && !$('ab-gsugg')?.hidden){ e.preventDefault(); ab.gs = {list:[], at:0}; $('ab-gsugg').hidden = true; return; }
  if (!ab.dirty) return;
  e.preventDefault();
  abSave().then(ok => ok && abdlg.close());
});
abdlg.addEventListener('close', () => { ab.dirty = false; if (!state.compose) listEl.focus({preventScroll:true}); });
$('ab-new').addEventListener('click', () => abNew('new'));
$('ab-newgroup').addEventListener('click', () => abNew('newgroup'));
$('ab-import').addEventListener('click', abImport);
$('ab-export').addEventListener('click', abExport);
$('ab-q').addEventListener('input', () => {
  ab.q = $('ab-q').value;
  renderAbList();
});
$('ab-items').addEventListener('click', e => {
  const li = e.target.closest('[data-k]');
  if (!li) return;
  const key = li.dataset.k;
  if (e.target.closest('[data-ab="take"]')){ abTake(key); return; }
  if ((e.ctrlKey || e.metaKey) && key[0] === 'c'){
    if (!ab.picked.size && ab.sel?.[0] === 'c') ab.picked.add(ab.sel);
    ab.picked.has(key) ? ab.picked.delete(key) : ab.picked.add(key);
    renderAbList(); return;
  }
  if (e.shiftKey && key[0] === 'c' && ab.sel?.[0] === 'c'){
    const keys = abEntries().contacts.map(x => x.key), a = keys.indexOf(ab.sel), b = keys.indexOf(key);
    keys.slice(Math.min(a, b), Math.max(a, b) + 1).forEach(k => ab.picked.add(k));
    renderAbList(); return;
  }
  ab.picked.clear();
  abSelect(key).then(() => renderAbList());
});
$('ab-items').addEventListener('dblclick', e => { if (e.target.closest('[data-k]')) $('ab-edit').querySelector('input')?.focus(); });
$('ab-edit').addEventListener('input', e => {
  if (e.target.id === 'ab-gadd'){ ab.gs.at = 0; abGroupSuggest(); return; }
  ab.dirty = true;
});
$('ab-edit').addEventListener('change', e => { if (e.target.dataset.group) ab.dirty = true; });
$('ab-edit').addEventListener('mousedown', e => {
  const li = e.target.closest('#ab-gsugg [data-i]');
  if (li){ e.preventDefault(); abAddMember(ab.gs.list[Number(li.dataset.i)]); }
});
$('ab-edit').addEventListener('click', e => {
  const b = e.target.closest('[data-ab]');
  if (!b) return;
  const act = b.dataset.ab, i = Number(b.dataset.i);
  if (act === 'save') abSave();
  else if (act === 'delete') abDelete();
  else if (act === 'write') abWrite();
  else if (act === 'take') abTake(ab.sel);
  else if (act === 'add-mail' || act === 'rm-mail' || act === 'primary' || act === 'rm-member'){
    abRead();
    const d = ab.draft;
    if (act === 'add-mail') d.emails.push('');
    else if (act === 'rm-mail') d.emails.splice(i, 1);
    else if (act === 'primary') d.emails.unshift(...d.emails.splice(i, 1));
    else d.members.splice(i, 1);
    ab.dirty = true; renderAbEdit();
    const focus = act === 'add-mail' ? `[data-mail="${d.emails.length - 1}"]` : act === 'primary' ? '[data-mail="0"]' : act === 'rm-member' ? '#ab-gadd' : '[data-ab="add-mail"]';
    $('ab-edit').querySelector(focus)?.focus();
  }
});
// ↑ ↓ Home End in the list, skipping its headings.
function abStep(k){
  const all = abEntries().all;
  if (!all.length) return;
  const at = all.findIndex(e => e.key === ab.sel);
  const to = k === 'Home' ? 0 : k === 'End' ? all.length - 1 : Math.max(0, Math.min(all.length - 1, (at < 0 ? -1 : at) + (k === 'ArrowDown' ? 1 : -1)));
  ab.picked.clear();
  abSelect(all[to].key).then(() => renderAbList());
}
abdlg.addEventListener('keydown', e => {
  const tg = e.target;
  if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)){ e.preventDefault(); abSave(); return; }
  if (tg.id === 'ab-q'){
    if (e.key === 'Escape' && tg.value){ e.preventDefault(); e.stopPropagation(); tg.value = ''; ab.q = ''; renderAbList(); }
    else if (e.key === 'ArrowDown' || e.key === 'Enter'){
      e.preventDefault();
      const first = abEntries().all[0];
      if (first) abSelect(first.key).then(() => { renderAbList(); $('ab-items').focus(); });
    }
    return;
  }
  if (tg.id === 'ab-gadd'){
    const n = ab.gs.list.length;
    if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && n){ e.preventDefault(); ab.gs.at = (ab.gs.at + (e.key === 'ArrowDown' ? 1 : -1) + n) % n; abGroupSuggest(); }
    else if (e.key === 'Enter'){ e.preventDefault(); abAddMember(n ? ab.gs.list[ab.gs.at] : null); }
    return;
  }
  if (tg.closest?.('input, textarea, select')){
    if (e.key === 'Enter' && tg.tagName === 'INPUT' && tg.type !== 'checkbox'){ e.preventDefault(); abSave(); }
    return;
  }
  if (e.ctrlKey || e.metaKey || e.altKey) return;
  const k = e.key.length === 1 ? e.key.toLowerCase() : e.key;
  if ((k === 'Enter' || k === ' ') && tg.closest?.('button, label')) return;
  if (k === '/') $('ab-q').focus();
  else if (k === 'n') abNew('new');
  else if (k === 'Delete'){ if (!e.repeat) abDelete(); }
  else if (k === 'z') abUndo();
  else if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(k) && tg.id === 'ab-items') abStep(k);
  else if (k === 'Enter' && tg.id === 'ab-items'){ if (ab.sel?.[0] === 's') abTake(ab.sel); else $('ab-edit').querySelector('input, button')?.focus(); }
  else return;
  e.preventDefault();
});
$('b-book').addEventListener('click', () => openAddressBook());

const rdlg = $('rules-dlg');
let RULES = [], rule = null; // rule: the one being edited
// Labels are English keys, translated where they are drawn.
const FIELDS = [['from', N_('From')], ['to', N_('To or Cc')], ['subject', N_('Subject')], ['body', N_('Text|rule field')], ['anywhere', N_('Anywhere')], ['header', N_('Header')], ['attachment_name', N_('Attachment name')], ['attachment_type', N_('Attachment type')],
  ['received', N_('Date received')], ['size', N_('Size')], ['state', N_('Status|rule field')], ['tag', N_('Tag')]];
// A condition on tags: has one (`value` its keyword), hasn't, has any, has none.
const TAG_OPS = [['is', N_('has|tag')], ['not_contains', N_('doesn’t have|tag')], ['exists', N_('has any|tag')], ['not_exists', N_('has none|tag')]];
const tagOptions = v => TAGS.map(x => `<option value="${esc(x.keyword)}"${x.keyword === v ? ' selected' : ''}>${esc(x.name)}</option>`).join('')
  + (v && !tagOf(v) ? `<option value="${esc(v)}" selected>${esc(t('(deleted tag)'))}</option>` : '');
const OPS = [['contains', N_('contains')], ['not_contains', N_('doesn’t contain')], ['is', N_('is')], ['starts_with', N_('starts with')], ['ends_with', N_('ends with')], ['exists', N_('exists')], ['not_exists', N_('does not exist')], ['matches', N_('matches pattern')]];
// Date, size and status compare their own ways, each with a value to start from.
const FACT_OPS = {received:[['older_than', N_('older than')], ['newer_than', N_('newer than')]], size:[['larger_than', N_('larger than')], ['smaller_than', N_('smaller than')]], state:[['is', N_('is')]]};
const FACT_START = {received:'30', size:'5m', state:'read'};
const STATES = [['read', N_('read|rule state')], ['unread', N_('unread|rule state')], ['flagged', N_('flagged|rule state')], ['unflagged', N_('not flagged|rule state')]];
const RULE_EVERY = [['hour', N_('Every hour')], ['day', N_('Every day')], ['week', N_('Every week')]];
const ALL_OPS = [...OPS, ...FACT_OPS.received, ...FACT_OPS.size];
const opsFor = field => field === 'tag' ? TAG_OPS : FACT_OPS[field] ?? OPS;
// A condition's way of comparing and value follow its field: a date takes days, not text.
function fitCondition(c, before){
  const ops = opsFor(c.field), kind = f => FACT_START[f] ? f : f === 'tag' ? 'tag' : 'text';
  if (kind(before) !== kind(c.field) || !ops.some(([o]) => o === c.op)){ c.op = ops[0][0]; c.value = FACT_START[c.field] ?? (c.field === 'tag' ? TAGS[0]?.keyword ?? '' : ''); }
}
const ACTIONS = [['move', N_('Move to folder')], ['copy', N_('Copy to folder')], ['forward', N_('Forward automatically')], ['mark_read', N_('Mark as read')], ['flag', N_('Flag|rule action')], ['add_tag', N_('Add a tag')], ['remove_tag', N_('Take a tag off')], ['delete', N_('Move to Trash')], ['auto_reply', N_('Answer automatically')]];
const labelOf = (list, v) => { const l = list.find(([k]) => k === v)?.[1]; return l ? t(l) : v; };
const options = (list, v) => list.map(([k, l]) => `<option value="${esc(k)}"${k === v ? ' selected' : ''}>${esc(t(l))}</option>`).join('');
// Folders a rule can move mail to: its account's own but the Outbox, by their names on the
// server, and the folders on this PC (not its Trash), which take mail from any account.
const moveTargets = acct => [
  ...(acct && acct !== LOCAL ? (ACCT[acct]?.folders ?? []).filter(f => f.kind !== 'outbox').map(f => [acct + '/' + f.id, f.name, ACCT[acct].email]) : []),
  ...(ACCT[LOCAL]?.folders ?? []).filter(f => f.kind !== 'trash').map(f => [LOCAL + '/' + f.id, folderName(LOCAL + '/' + f.id), t('On this PC')]),
];
const folderOptions = (list, v) => [...new Set(list.map(x => x[2]))].map(g => `<optgroup label="${esc(g)}">`
  + list.filter(x => x[2] === g).map(([k, l]) => `<option value="${esc(k)}"${k === v ? ' selected' : ''}>${esc(l)}</option>`).join('') + '</optgroup>').join('');

function ruleSummary(r){
  const conds = r.conditions.map(c => c.field === 'tag' ? `${labelOf(FIELDS, 'tag')} ${labelOf(TAG_OPS, c.op)}${['exists','not_exists'].includes(c.op) ? '' : ' “' + tagName(c.value) + '”'}`
    : ['exists','not_exists'].includes(c.op) ? `${labelOf(FIELDS,c.field)}${c.header ? ' ' + c.header : ''} ${labelOf(OPS,c.op)}`
    : c.field === 'received' ? tn(+c.value || 0, '{field} {op} {n} day', '{field} {op} {n} days', {field:labelOf(FIELDS, c.field), op:labelOf(ALL_OPS, c.op)})
    : c.field === 'state' ? t('{field} {op} {value}', {field:labelOf(FIELDS, c.field), op:labelOf(ALL_OPS, c.op), value:labelOf(STATES, c.value)})
    : t('{field} {op} “{value}”', {field:labelOf(FIELDS, c.field) + (c.header ? ' ' + c.header : ''), op:labelOf(ALL_OPS, c.op), value:c.value}))
    .join(r.match_all ? t(' and ') : t(' or '));
  const acts = r.actions.map(a => a.type === 'move' ? t('move to {folder}', {folder:placeName(a.folder)}) : a.type === 'copy' ? t('copy to {folder}', {folder:placeName(a.folder)}) : a.type === 'forward' ? t('forward to {to}',{to:a.to})
    : a.type === 'add_tag' ? t('tag it “{name}”', {name:tagName(a.tag)}) : a.type === 'remove_tag' ? t('take “{name}” off', {name:tagName(a.tag)}) : labelOf(ACTIONS, a.type).toLowerCase()).join(', ');
  return t('If {conds}: {acts}.', {conds, acts}) + (r.account ? ' ' + t('{name} only.', {name:ACCT[r.account]?.email ?? r.account}) : '') + (r.stop ? ' ' + t('Then stop.') : '');
}
// When a rule runs, for the list: "New mail · Every day on Inbox · last run 17.10 9:00".
function ruleWhen(r){
  const s = r.schedule, parts = r.on_arrival !== false ? [t('New mail')] : [];
  if (s) parts.push(t('{every} on {folders}', {every:labelOf(RULE_EVERY, s.every), folders:s.folders.map(placeName).join(', ')}),
    r.last_run ? t('last run {when}', {when:fmtWhen(parseDate(r.last_run))}) : t('not run yet'));
  return parts.join(' · ');
}
// Folders a schedule can run on: the rule's account's (every account's for a rule on all of
// them, and then the folders on this PC), not the Outbox.
const scheduleTargets = acct => (acct ? [acct] : [...servers().map(a => a.id), LOCAL]).flatMap(id =>
  (ACCT[id]?.folders ?? []).filter(f => f.kind !== 'outbox').map(f => [id + '/' + f.id, id === LOCAL ? folderName(id + '/' + f.id) : f.name, id === LOCAL ? t('On this PC') : ACCT[id].email]));
async function openRules(){
  closeSuggest();
  rdlg.showModal();
  await showRuleList();
}
async function showRuleList(){
  $('r-form').hidden = true; $('r-list-view').hidden = false; $('r-err').textContent = '';
  try { RULES = await invoke('list_rules'); } catch (e){ $('r-err').textContent = tr(e); return; }
  $('r-run').textContent = t('Run on {folder} now', {folder:folderName(state.folder)});
  const m = current();
  $('r-from').hidden = !m;
  if (m) $('r-from').textContent = isOut(m.f) ? t('New rule for mail to {name}', {name:nameOf(m.to)}) : t('New rule for mail from {name}', {name:m.from || m.email});
  $('r-list').innerHTML = RULES.length ? RULES.map((r, i) => `<div class="rrule${r.enabled ? '' : ' off'}" data-id="${r.id}">
<input type="checkbox" data-r="toggle"${r.enabled ? ' checked' : ''} title="${esc(t('On or off'))}" aria-label="${esc(t('Rule {name} on', {name:r.name}))}">
<div class="rtext"><div class="rname">${esc(r.name)}${r.enabled ? '' : ` <span class="tag">${esc(t('Off|rule'))}</span>`}</div><div class="rsum" title="${esc(ruleSummary(r))}">${esc(ruleSummary(r))}</div>${r.schedule || r.on_arrival === false ? `<div class="rsum" title="${esc(ruleWhen(r))}">${esc(ruleWhen(r))}</div>` : ''}</div>
<button type="button" class="btn icon" data-r="up" title="${esc(t('Run earlier'))}" aria-label="${esc(t('Run earlier'))}"${i ? '' : ' disabled'}>↑</button>
<button type="button" class="btn icon" data-r="down" title="${esc(t('Run later'))}" aria-label="${esc(t('Run later'))}"${i < RULES.length - 1 ? '' : ' disabled'}>↓</button>
<button type="button" class="btn" data-r="edit">${esc(t('Edit'))}</button><button type="button" class="btn danger" data-r="delete">${esc(t('Delete'))}</button></div>`).join('')
    : `<p class="dnote">${esc(t('No rules yet. A rule can file invoices, flag mail from a client, or answer while you’re away.'))}</p>`;
  $('r-new').focus();
}
// A rule started from a message: mail from its sender (or, in Sent, to its first recipient),
// filed where this one is when that is one of the user's folders, else flagged.
function ruleFrom(m){
  const out = isOut(m.f), to = String(m.to).match(/[^\s<>,;"]+@[^\s<>,;"]+/)?.[0] ?? '';
  return {id:0, name:out ? nameOf(m.to) : (m.from || m.email), enabled:true, account:isLocal(m.f) ? null : acctId(m.f), match_all:true,
    conditions:[out ? {field:'to', op:'is', value:to} : {field:'from', op:'is', value:m.email}],
    actions:[kindOf(m.f) === 'custom' ? {type:'move', folder:m.f} : {type:'flag'}], stop:false};
}
function showRuleForm(existing, start = null){
  $('r-preview-result').hidden = true;
  const firstAccount = servers()[0]?.id ?? null;
  rule = existing ? structuredClone(existing) : start ?? {id:0, name:'', enabled:true, account:firstAccount, match_all:true,
    conditions:[{field:'from', op:'contains', value:''}], actions:[{type:'flag'}], stop:false};
  rule.on_arrival ??= true;
  $('r-list-view').hidden = true; $('r-form').hidden = false; $('r-form-err').textContent = '';
  $('r-form-title').textContent = existing ? t('Edit rule') : t('New rule');
  $('r-name').value = rule.name;
  $('r-acct').innerHTML = `<option value="">${esc(t('All accounts'))}</option>` + servers().map(a => `<option value="${esc(a.id)}"${a.id === rule.account ? ' selected' : ''}>${esc(a.name)} · ${esc(a.email)}</option>`).join('');
  $('r-match').value = rule.match_all ? 'all' : 'any';
  $('r-stop').checked = rule.stop;
  renderConditions(); renderActions(); renderWhen();
  $('r-name').focus();
}
function conditionValue(c){
  if (['exists','not_exists'].includes(c.op)) return '';
  if (c.field === 'tag') return `<select data-k="value" aria-label="${esc(t('Tag'))}">${tagOptions(c.value)}</select>`;
  if (c.field === 'state') return `<select data-k="value" aria-label="${esc(t('Status|rule field'))}">${options(STATES, c.value)}</select>`;
  if (c.field === 'received') return `<input data-k="value" type="number" min="1" max="36500" step="1" value="${esc(c.value)}" aria-label="${esc(t('Days'))}"><span class="runit">${esc(t('days|rule unit'))}</span>`;
  if (c.field === 'size') return `<input data-k="value" value="${esc(c.value)}" placeholder="500k, 5m" autocomplete="off" spellcheck="false" aria-label="${esc(t('Size'))}">`;
  if (c.op === 'matches') return `<input data-k="value" class="rpattern" value="${esc(c.value)}" placeholder="faktur(a|y)" maxlength="1000" autocomplete="off" spellcheck="false" aria-label="${esc(t('Pattern'))}">`;
  return `<input data-k="value" value="${esc(c.value)}" placeholder="${esc(t('text to look for'))}" autocomplete="off" spellcheck="false" aria-label="${esc(t('Text to look for'))}">`;
}
function renderConditions(){
  $('r-conds').innerHTML = rule.conditions.map((c, i) => `<div class="rrow" data-i="${i}">
<select data-k="field" aria-label="${esc(t('Where'))}">${options(FIELDS, c.field)}</select><select data-k="op" aria-label="${esc(t('How'))}">${options(opsFor(c.field), c.op)}</select>
<div class="rvalue">${c.field === 'header' ? `<input data-k="header" value="${esc(c.header ?? '')}" placeholder="List-Id" aria-label="${esc(t('Header name'))}" autocomplete="off" spellcheck="false">` : ''}${conditionValue(c)}</div>
<button type="button" class="btn icon" data-x="${i}" title="${esc(t('Remove this condition'))}" aria-label="${esc(t('Remove condition'))}">×</button></div>`).join('');
  $('r-pattern-hint').hidden = !rule.conditions.some(c => c.op === 'matches');
}
// When the rule runs: on new mail, on a schedule over folders, or both.
function renderWhen(){
  const s = rule.schedule;
  $('r-arrival').checked = rule.on_arrival;
  $('r-sched').checked = !!s;
  $('r-every').innerHTML = options(RULE_EVERY, s?.every ?? 'day');
  $('r-every').disabled = !s;
  const targets = scheduleTargets(rule.account);
  $('r-sched-folders').hidden = $('r-sched-note').hidden = !s;
  $('r-sched-folders').innerHTML = [...new Set(targets.map(x => x[2]))].map(g => `<div class="rfgroup">${esc(g)}</div>`
    + targets.filter(x => x[2] === g).map(([k, l]) => `<label class="rfold"><input type="checkbox" value="${esc(k)}"${s?.folders.includes(k) ? ' checked' : ''}> <span>${esc(l)}</span></label>`).join('')).join('');
}
function readWhen(){
  rule.on_arrival = $('r-arrival').checked;
  rule.schedule = $('r-sched').checked ? {every:$('r-every').value || 'day', folders:[...$('r-sched-folders').querySelectorAll('input:checked')].map(x => x.value)} : null;
}
$('r-when').addEventListener('change', e => {
  const had = !!rule.schedule;
  readWhen();
  // Turning the schedule on starts it on the account's Inbox.
  if (!had && rule.schedule && !rule.schedule.folders.length){
    const inbox = scheduleTargets(rule.account).find(([k]) => folderOf(k)?.kind === 'inbox');
    if (inbox) rule.schedule.folders = [inbox[0]];
  }
  if (e.target.id !== 'r-every' && !e.target.closest('#r-sched-folders')) renderWhen();
});
function renderActions(){
  $('r-acts').innerHTML = rule.actions.map((a, i) => {
    let param = '<span class="none"></span>';
    if (a.type === 'move' || a.type === 'copy'){
      const targets = moveTargets(rule.account);
      param = targets.length ? `<select data-k="folder" aria-label="${esc(t('Folder'))}">${folderOptions(targets, a.folder)}</select>`
        : `<span class="none">${esc(t('Pick the rule’s account to choose a folder.'))}</span>`;
    } else if (a.type === 'auto_reply'){
      param = `<textarea data-k="body" aria-label="${esc(t('Reply text'))}" placeholder="${esc(t('Hello, I’m away until 20 October.'))}">${esc(a.body || '')}</textarea>`;
    } else if (a.type === 'add_tag' || a.type === 'remove_tag'){
      param = `<select data-k="tag" aria-label="${esc(t('Tag'))}">${tagOptions(a.tag)}</select>`;
    } else if (a.type === 'forward'){
      param = `<input data-k="to" value="${esc(a.to ?? '')}" aria-label="${esc(t('Forward to'))}" placeholder="${esc(t('Recipient addresses'))}" autocomplete="off" spellcheck="false">`;
    }
    return `<div class="rrow act" data-i="${i}"><select data-k="type" aria-label="${esc(t('Action'))}">${options(ACTIONS, a.type)}</select>${param}
<button type="button" class="btn icon" data-x="${i}" title="${esc(t('Remove this action'))}" aria-label="${esc(t('Remove action'))}">×</button></div>`;
  }).join('');
}
// A new or changed action gets sensible defaults: a folder to move to, empty reply text.
function fixAction(a){
  if (a.type === 'move' || a.type === 'copy'){
    const targets = moveTargets(rule.account);
    if (!targets.some(([k]) => k === a.folder)) a.folder = (targets.find(([k]) => folderOf(k)?.kind === 'custom') ?? targets[0])?.[0] ?? '';
  } else delete a.folder;
  if (a.type === 'auto_reply') a.body = a.body ?? ''; else delete a.body;
  if (a.type === 'forward') a.to = a.to ?? ''; else delete a.to;
  if (a.type === 'add_tag' || a.type === 'remove_tag') a.tag = tagOf(a.tag) ? a.tag : TAGS[0]?.keyword ?? ''; else delete a.tag;
  return a;
}
$('r-conds').addEventListener('input', e => {
  const row = e.target.closest('[data-i]'); if (!row) return;
  const before = rule.conditions[row.dataset.i].field;
  rule.conditions[row.dataset.i][e.target.dataset.k] = e.target.value;
  if (['field','op'].includes(e.target.dataset.k)){
    const c = rule.conditions[row.dataset.i];
    if (c.field === 'header') c.header ??= ''; else delete c.header;
    if (e.target.dataset.k === 'field') fitCondition(c, before);
    if (c.field === 'tag' && !['exists','not_exists'].includes(c.op) && !tagOf(c.value)) c.value = TAGS[0]?.keyword ?? '';
    renderConditions();
  }
});
$('r-acts').addEventListener('input', e => {
  const row = e.target.closest('[data-i]'); if (!row) return;
  const a = rule.actions[row.dataset.i];
  a[e.target.dataset.k] = e.target.value;
  if (e.target.dataset.k === 'type'){ fixAction(a); renderActions(); }
});
$('r-conds').addEventListener('click', e => {
  const x = e.target.closest('[data-x]'); if (!x) return;
  rule.conditions.splice(Number(x.dataset.x), 1); renderConditions();
});
$('r-acts').addEventListener('click', e => {
  const x = e.target.closest('[data-x]'); if (!x) return;
  rule.actions.splice(Number(x.dataset.x), 1); renderActions();
});
$('r-add-cond').addEventListener('click', () => {
  rule.conditions.push({field:'subject', op:'contains', value:''}); renderConditions();
  $('r-conds').querySelector('.rrow:last-child input')?.focus();
});
$('r-add-act').addEventListener('click', () => { rule.actions.push(fixAction({type:'mark_read'})); renderActions(); });
$('r-acct').addEventListener('change', () => {
  rule.account = $('r-acct').value || null; rule.actions.forEach(fixAction); renderActions();
  // A rule of one account runs on that account's folders only.
  if (rule.schedule){ const ok = new Set(scheduleTargets(rule.account).map(x => x[0])); rule.schedule.folders = rule.schedule.folders.filter(k => ok.has(k)); }
  renderWhen();
});
$('r-form').addEventListener('submit', async e => {
  e.preventDefault();
  readRuleForm();
  try { await invoke('save_rule', {rule}); } catch (err){ $('r-form-err').textContent = tr(err); return; }
  const name = rule.name.trim(), every = rule.schedule && labelOf(RULE_EVERY, rule.schedule.every).toLowerCase();
  say(!rule.schedule ? t('Rule “{name}” saved. It runs on new mail as it arrives.', {name})
    : rule.on_arrival ? t('Rule “{name}” saved. It runs on new mail and {every}, the first time within a minute.', {name, every})
    : t('Rule “{name}” saved. It runs {every}, the first time within a minute.', {name, every}));
  await showRuleList();
});
$('r-back').addEventListener('click', showRuleList);
const readRuleForm = () => { Object.assign(rule, {name:$('r-name').value,match_all:$('r-match').value === 'all',stop:$('r-stop').checked}); readWhen(); return rule; };
$('r-form').addEventListener('input', () => { $('r-preview-result').hidden = true; });
$('r-preview').addEventListener('click', async () => {
  readRuleForm();
  const snapshot = JSON.stringify(rule), button = $('r-preview'); button.disabled = true;
  let report;
  try { report = await invoke('preview_rule',{rule,view:{...viewArgs(),grouped:false,expanded:[]}}); }
  catch (error){ $('r-form-err').textContent = tr(error); button.disabled = false; return; }
  button.disabled = false; readRuleForm();
  if (snapshot !== JSON.stringify(rule)) return;
  $('r-form-err').textContent = '';
  $('r-preview-result').innerHTML = `<h3 class="rh">${esc(t('Preview: {matched} of {checked} messages match.',{matched:num(report.matched),checked:num(report.checked)}))}</h3><p class="dnote">${esc(t('No messages were changed or sent.'))} ${esc(ruleSummary(rule))}</p>`
    + (report.limited ? `<p class="dnote">${esc(t('Only the first {n} of the {total} messages in this list were checked.',{n:num(report.read),total:num(report.listed)}))}</p>` : '')
    + (report.new_mail_only ? `<p class="dnote">${esc(t('Automatic replies and forwards run only on new arrivals, never on Run now.'))}</p>` : '')
    + `<ul class="preview-matches">${report.samples.map(m => `<li><b>${esc(m.subject)}</b> · ${esc(m.from)}<span class="dnote">${esc(t('Condition matches: {matches}',{matches:m.conditions.map((hit,i) => (i+1) + (hit ? ' ✓' : ' ×')).join(' · ')}))}</span></li>`).join('')}</ul>`
    + (report.matched > report.samples.length ? `<p class="dnote">${esc(t('Showing the first {n} matches.',{n:report.samples.length}))}</p>` : '');
  $('r-preview-result').hidden = false;
});
$('r-new').addEventListener('click', () => showRuleForm(null));
$('r-from').addEventListener('click', () => { const m = current(); if (m) showRuleForm(null, ruleFrom(m)); });
$('r-close').addEventListener('click', () => rdlg.close());
rdlg.addEventListener('close', () => listEl.focus({preventScroll:true}));
$('r-list').addEventListener('click', async e => {
  const b = e.target.closest('[data-r]'), row = e.target.closest('[data-id]');
  if (!b || !row) return;
  const r = RULES.find(x => x.id === Number(row.dataset.id));
  if (!r) return;
  const act = b.dataset.r;
  try {
    if (act === 'edit'){ showRuleForm(r); return; }
    if (act === 'toggle') await invoke('save_rule', {rule:{...r, enabled:b.checked}});
    if (act === 'up' || act === 'down') await invoke('move_rule', {id:r.id, delta:act === 'up' ? -1 : 1});
    if (act === 'delete'){
      if (b.dataset.armed !== '1'){ b.dataset.armed = '1'; b.textContent = t('Delete it'); return; }
      await invoke('delete_rule', {id:r.id});
    }
  } catch (err){ $('r-err').textContent = tr(err); return; }
  await showRuleList();
});
$('r-run').addEventListener('click', async () => {
  const where = folderName(state.folder);
  let r;
  try { r = await call('run_rules', {folder:state.folder}); } catch { return; }
  rdlg.close();
  await refresh();
  const n = r.matched;
  const text = n ? tn(n, 'Rules changed {n} message in {where}.', 'Rules changed {n} messages in {where}.', {where}) : t('No rule changed anything in {where}.', {where});
  say([text, ...r.problems].join(' · '), r.problems.length ? 12000 : 6000);
});
$('b-rules').addEventListener('click', openRules);
// Rules on a schedule run while Telex runs: every minute the core runs those that are due.
async function runSchedules(){
  if (!ACCOUNTS.length) return;
  let runs;
  try { runs = await invoke('run_scheduled_rules'); } catch { return; }
  if (!runs.length) return;
  const said = runs.map(r => [r.moved ? tn(r.moved, 'Rule “{name}” moved {n} message.', 'Rule “{name}” moved {n} messages.', {name:r.name})
    : r.matched ? tn(r.matched, 'Rule “{name}” changed {n} message.', 'Rule “{name}” changed {n} messages.', {name:r.name}) : '', ...r.problems].filter(Boolean).join(' · ')).filter(Boolean);
  if (runs.some(r => r.matched)){ schedulePush(); await refresh(); }
  if (said.length) say(said.join(' · '), runs.some(r => r.problems.length) ? 12000 : 6000);
  if (rdlg.open && !$('r-list-view').hidden) showRuleList(); // their last runs
}

/* ---------- settings window ---------- */
// Every change is saved at once (changeSettings) and shows at once, theme cards included:
// each card carries its own data-theme, so it is drawn in its own colors. Accounts, Reading
// and Usage draw their lists when they open.
let setTab = 'look';
let move_ = {plan:null, choice:null, report:null, exported:null, error:'', counts:null}; // the Import and export tab
const radios = (name, list, current) => list.map(([value, label, extra = '']) =>
  `<label class="sopt"><input type="radio" name="${name}" value="${esc(value)}"${String(current) === String(value) ? ' checked' : ''}><span>${label}</span>${extra}</label>`).join('');
const segs = (name, list, current) => list.map(([value, label]) =>
  `<label class="seg2"><input type="radio" name="${name}" value="${esc(value)}"${String(current) === String(value) ? ' checked' : ''}><span>${label}</span></label>`).join('');
// A small window in a theme's colors: toolbar, folders, a few rows (unread, selected, flagged).
const mini = id => `<span class="mini" data-theme="${id}"><i class="m-bar"></i><i class="m-side"><b></b><b class="on"></b><b></b></i>`
  + '<i class="m-list"><b class="u"></b><b class="s"></b><b></b><b class="f"></b><b></b></i></span>';
function themeCards(){
  return THEMES.map(id => {
    const preview = id === 'system' ? `<span class="tprev both" aria-hidden="true">${mini('light')}${mini('dark')}</span>` : `<span class="tprev" aria-hidden="true">${mini(id)}</span>`;
    return `<label class="tcard"><input type="radio" name="theme" value="${id}"${pref().theme === id ? ' checked' : ''}>${preview}`
      + `<span class="tname">${esc(t(THEME_NAMES[id]))}</span><span class="tlvl"></span></label>`;
  }).join('');
}
// How accounts are told apart: each card draws two accounts that way in the tree, and rows of a
// list that mixes them (Unread, all mail), in their colors.
const MARKS = [['rail', N_('Line')], ['dot', N_('Dot')], ['initials', N_('Initials')], ['tint', N_('Tint')], ['none', N_('None|mark')]];
function markCards(){
  const group = (c, who) => `<span class="mgrp ${c}"><span class="mhead"><i class="adot ${c}" data-i="${who}"></i><b></b></span><b class="mf"></b><b class="mf on"></b></span>`;
  const row = (c, who) => `<span class="mrow ${c}"><b class="ml"></b><i class="adot ${c}" data-i="${who}"></i><b class="mw"></b></span>`;
  return MARKS.map(([id, name]) => `<label class="tcard mcard"><input type="radio" name="account_mark" value="${id}"${pref().account_mark === id ? ' checked' : ''}>`
    + `<span class="mprev" data-mark="${id}" aria-hidden="true"><span class="multi">${group('a0', 'AK')}${group('a1', 'JN')}`
    + `<span class="mlist">${row('a1', 'JN')}${row('a0', 'AK')}${row('a0', 'AK')}</span></span></span>`
    + `<span class="tname">${esc(t(name))}</span></label>`).join('');
}
// How unread mail stands out: each card draws a few rows that way, in the list's face.
const UNREAD_STYLES = [['quiet', N_('Read mail quieter')], ['bold', N_('Bold only')], ['color', N_('Unread in color')]];
function unreadCards(){
  const row = (u, who, what) => `<span class="urow${u ? ' u' : ''}"><i class="dot"></i><b>${esc(who)}</b><span>${esc(what)}</span></span>`;
  return UNREAD_STYLES.map(([id, name]) => `<label class="tcard ucard"><input type="radio" name="unread_style" value="${id}"${pref().unread_style === id ? ' checked' : ''}>`
    + `<span class="uprev" data-unread="${id}" aria-hidden="true">${row(true, 'Anna Kowalska', 'Faktura 10/2026')}${row(false, 'Jan Nowak', 'Re: Spotkanie')}`
    + `${row(true, 'GitHub', 'Release 0.9.1')}${row(false, 'Ewa Wiśniewska', 'Zdjęcia z wyjazdu')}</span>`
    + `<span class="tname">${esc(t(name))}</span></label>`).join('');
}
// The faces Telex can be written in: each card shows its three (styles.css: [data-font]).
// Plex, Inter, Atkinson and Source are bundled (ui/fonts); System and Classic are Windows' own.
const FONTS = [['plex', 'IBM Plex'], ['system', N_('System|font')], ['classic', N_('Classic|font')], ['inter', 'Inter'], ['atkinson', 'Atkinson Hyperlegible'], ['source', 'Source Sans Light']];
const fontName = id => { const f = FONTS.find(([x]) => x === id); return !f ? id : id === 'system' || id === 'classic' ? t(f[1]) : f[1]; };
function fontCards(){
  return FONTS.map(([id]) => `<label class="tcard fcard"><input type="radio" name="font" value="${id}"${pref().font === id ? ' checked' : ''}>`
    + `<span class="fprev" data-font="${id}" aria-hidden="true"><b>Ag Żółć</b><span class="fl">Anna Kowalska · Faktura 10/2026</span><span class="fd">09:30 · 14 KB</span></span>`
    + `<span class="tname">${esc(fontName(id))}</span></label>`).join('');
}
function toolbarCards(){
  const sample = (icon, label) => `<span class="navsample">${I[icon]}<span class="lbl">${esc(t(label))}</span></span>`;
  return TOOLBAR_STYLES.map(([id, name, description]) => `<label class="tcard" title="${esc(t(description))}"><input type="radio" name="toolbar_style" value="${id}"${(pref().toolbar_style || 'classic') === id ? ' checked' : ''}>`
    + `<span class="navprev" data-style="${id}" aria-hidden="true">${sample('neu', 'New|message')}${sample('reply', 'Reply')}</span>`
    + `<span class="tname">${esc(t(name))}</span></label>`).join('');
}
function renderSettings(keepFocus){
  const focused = keepFocus && document.activeElement?.closest?.('#set-dlg') ? document.activeElement : null;
  const again = focused && (focused.name ? `[name="${focused.name}"][value="${focused.value}"]` : focused.id ? '#' + focused.id : null);
  I18n.translatePage(sdlg);
  sdlg.querySelectorAll('[role="tab"]').forEach(b => {
    const on = b.dataset.tab === setTab;
    b.setAttribute('aria-selected', String(on)); b.tabIndex = on ? 0 : -1;
    $(b.getAttribute('aria-controls')).hidden = !on;
  });
  const s = pref();
  $('s-toolbar').innerHTML = toolbarCards();
  $('s-themes').innerHTML = themeCards();
  // The level comes from the theme itself (--wcag in themes.css), so it can't drift.
  $('s-themes').querySelectorAll('.tlvl').forEach(el => {
    const lvl = getComputedStyle(el.closest('.tcard').querySelector('.mini')).getPropertyValue('--wcag').trim() || 'AA';
    el.textContent = 'WCAG ' + lvl;
    el.title = t('Meets WCAG 2.2 {level} contrast', {level:lvl});
  });
  $('s-forced').hidden = !forcedColors.matches;
  $('s-density').innerHTML = segs('density', [['compact', esc(t('Compact'))], ['comfortable', esc(t('Comfortable'))]], s.density);
  $('s-marks').innerHTML = markCards();
  $('s-unread').innerHTML = unreadCards();
  renderColumnChoices();
  $('s-fonts').innerHTML = fontCards();
  $('s-size').innerHTML = segs('text_size', TEXT_SIZES.map(n => [n, n + '%']), s.text_size);
  const auto = LANG_NAMES[SETTINGS.settings.language === 'auto' ? SETTINGS.language : ''] ?? '';
  $('s-langs').innerHTML = radios('language', [
    ['auto', esc(t('Automatic, as in Windows')), auto ? `<small class="sdim">${esc(auto)}</small>` : ''],
    ['en', '<span lang="en">English</span>'], ['pl', '<span lang="pl">Polski</span>'],
  ], s.language);
  $('s-every').innerHTML = CHECK_EVERY.map(n => `<option value="${n}"${n === s.check_every ? ' selected' : ''}>${esc(
    n === 0 ? t('Only when I click Get mail') : n === 60 ? t('Every hour') : tn(n, 'Every {n} minute', 'Every {n} minutes'))}</option>`).join('');
  // "A moment" takes how long from the list next to it; choosing a time also chooses this.
  const delays = READ_DELAYS.map(ms => `<option value="${ms}"${ms === s.read_delay ? ' selected' : ''}>${esc(delayText(ms))}</option>`).join('');
  $('s-read').innerHTML = radios('mark_read', [
    ['at_once', esc(t('As soon as I select them'))],
    ['moment', esc(t('After they have been on screen for')), `<select id="s-delay" class="sselect sinline" aria-label="${esc(t('How long on screen'))}">${delays}</select>`],
    ['window', esc(t('When I open them in their own window'))],
    ['manual', esc(t('Only when I mark them myself'))],
  ], s.mark_read);
  $('s-view').innerHTML = radios('prefer_text', [['html', esc(t('Show the HTML'))], ['text', esc(t('Show the plain text'))]], s.prefer_text ? 'text' : 'html');
  $('s-paper').innerHTML = radios('mail_paper', [['theme', esc(t('On the theme’s dark paper'))], ['white', esc(t('On white, as sent'))]], s.mail_paper);
  $('s-format').innerHTML = radios('compose_format', [['html', esc(t('HTML'))], ['plain', esc(t('Plain text'))]], s.compose_format);
  $('s-reply').innerHTML = radios('reply_format', [['html', esc(t('Always HTML'))], ['same', esc(t('In the format of the message'))], ['plain', esc(t('Always plain text'))]], s.reply_format);
  $('s-signrep').checked = s.sign_replies !== false; $('s-risk').checked = s.risk_checks !== false;
  $('s-imgsize').innerHTML = radios('image_size', IMAGE_SIZES.map(([v, label]) => [v, esc(t(label))]), s.image_size ?? 'large');
  renderSignatures();
  $('s-subject').checked = s.ask_subject; $('s-spell').checked = s.spell_check;
  $('s-askfile').checked = s.ask_attachment !== false; $('s-askmany').checked = s.ask_many !== false;
  $('s-push').checked = s.push_mail; $('s-notify').checked = s.notify;
  $('s-spam').innerHTML = radios('spam_filter', SPAM_LEVELS.map(([v, label]) => [v, esc(t(label))]), s.spam_filter ?? 'careful');
  $('s-threaded').checked = !!s.threaded;
  $('s-undo').innerHTML = UNDO_SEND.map(n => `<option value="${n}"${n === s.undo_send ? ' selected' : ''}>${esc(undoText(n))}</option>`).join('');
  const can = !!upd.build?.updatable;
  $('s-updates').innerHTML = radios('updates', UPDATE_CHOICES.map(v => [v, esc(updatesText(v))]), s.updates === 'ask' ? '' : s.updates);
  $('s-updates').querySelectorAll('input').forEach(i => { i.disabled = !can; });
  $('s-upd-none').hidden = can; $('s-check').hidden = !can;
  $('s-count-group').hidden = !upd.build?.countable; $('s-count').checked = s.usage_count === 'on';
  paneOpened(setTab);
  if (again) sdlg.querySelector(again)?.focus();
}
// What a tab draws when it opens: the lists that change while Telex runs.
function paneOpened(tab){
  if (tab === 'accts'){
    if (accountView === 'home') renderAccountList();
    else if (accountView === 'ids') showIdentityList();
    else if (accountView === 'folders') showFolderChoices();
    else if (accountView === 'away') openAway(awayOf);
    else if (accountView === 'start') renderLangPick();
  }
  if (tab === 'read') renderTrusted();
  if (tab === 'mail') renderSpamStatus();
  if (tab === 'tags'){ tagEdit = {adding:false, armed:null, error:''}; renderTags(); }
  if (tab === 'write'){ spellInfo = null; renderSpellLanguages(); renderSignatures(); forgetTemplates(); renderFolderTemplates(); }
  if (tab === 'use') renderUsage();
  if (tab === 'conn') renderConnections();
  if (tab === 'backup'){ bk.state = null; bk.restore = null; bk.error = ''; renderBackups(); }
  if (tab === 'move') renderMove();
  if (tab === 'keys'){ keys_.list = null; keys_.paste = false; keys_.armed = ''; keys_.p12 = null; keys_.p12err = ''; renderKeys(); }
}
// After a change that keeps the words: only the ticks move, so focus stays where it was.
function syncSettings(){
  const s = pref();
  const want = {theme:s.theme, toolbar_style:s.toolbar_style || 'classic', density:s.density, account_mark:s.account_mark, unread_style:s.unread_style, font:s.font, text_size:String(s.text_size), language:s.language, mark_read:s.mark_read, prefer_text:s.prefer_text ? 'text' : 'html',
    mail_paper:s.mail_paper, spam_filter:s.spam_filter ?? 'careful', compose_format:s.compose_format, reply_format:s.reply_format, image_size:s.image_size ?? 'large', backup_every:s.backup_every, updates:s.updates};
  sdlg.querySelectorAll('input[type="radio"]').forEach(r => { if (r.name in want) r.checked = r.value === want[r.name]; });
  $('s-every').value = String(s.check_every);
  $('s-delay').value = String(s.read_delay);
  $('s-subject').checked = s.ask_subject; $('s-spell').checked = s.spell_check;
  $('s-askfile').checked = s.ask_attachment !== false; $('s-askmany').checked = s.ask_many !== false;
  $('s-push').checked = s.push_mail; $('s-notify').checked = s.notify;
  $('s-threaded').checked = !!s.threaded;
  $('s-signrep').checked = s.sign_replies !== false; $('s-risk').checked = s.risk_checks !== false;
  $('s-undo').value = String(s.undo_send);
  if ($('s-keep')) $('s-keep').value = String(s.backup_keep);
}
// How sure the spam filter must be before it moves new mail to Spam (settings' spam_filter).
const SPAM_LEVELS = [['careful', N_('Careful: only mail it is all but sure of')], ['normal', N_('Normal|spam filter')],
  ['strict', N_('Strict: catches more, and now and then a good message')], ['off', N_('Off: it learns, but moves nothing')]];
// What the spam filter has learned so far, under its levels.
async function renderSpamStatus(){
  const el = $('s-spam-learned');
  let s;
  try { s = await invoke('spam_status'); } catch { el.textContent = ''; return; }
  el.textContent = [t('Learned so far: spam {spam}, good mail {ham}.', {spam:num(s.spam), ham:num(s.ham)}),
    s.spam < s.needed || s.ham < s.needed ? t('It starts moving mail once it has {n} of each.', {n:num(s.needed)}) : ''].filter(Boolean).join(' ');
}
// When Telex looks for a newer version by itself.
const UPDATE_CHOICES = ['start', 'daily', 'off'];
const updatesText = v => ({start:t('When Telex starts, and once a day while it is open'), daily:t('Once a day'), off:t('Only when I click Check now or the version number')}[v] ?? v);
// How long a message is on screen before it counts as read.
const delayText = ms => ms === 600 ? t('a moment|on screen') : tn(ms / 1000, '{n} second|on screen', '{n} seconds|on screen');
// How long sent mail waits so it can be taken back.
const undoText = n => n === 0 ? t('Off: send at once') : tn(n, '{n} second', '{n} seconds');
function openSettings(tab){
  if (tab) setTab = tab;
  move_ = {plan:null, choice:null, report:null, exported:move_.exported, error:'', counts:null};
  if (!moveIn.running) moveIn = {...moveIn, source:null, report:null, error:'', passwords:{}};
  renderSettings();
  if (!sdlg.open) sdlg.showModal();
  sdlg.querySelector(`[role="tab"][data-tab="${setTab}"]`).focus();
}
function showTab(tab, focus){
  setTab = tab;
  sdlg.querySelectorAll('[role="tab"]').forEach(b => {
    const on = b.dataset.tab === tab;
    b.setAttribute('aria-selected', String(on)); b.tabIndex = on ? 0 : -1;
    $(b.getAttribute('aria-controls')).hidden = !on;
    if (on && focus) b.focus();
  });
  sdlg.querySelector('.setpanes').scrollTop = 0;
  paneOpened(tab);
}
$('s-tabs').addEventListener('click', e => { const b = e.target.closest('[role="tab"]'); if (b) showTab(b.dataset.tab); });
$('s-tabs').addEventListener('keydown', e => {
  const tabs = [...sdlg.querySelectorAll('[role="tab"]')], i = tabs.findIndex(b => b.dataset.tab === setTab);
  const to = {ArrowDown:i + 1, ArrowUp:i - 1, Home:0, End:tabs.length - 1}[e.key];
  if (to === undefined) return;
  e.preventDefault();
  showTab(tabs[(to + tabs.length) % tabs.length].dataset.tab, true);
});
sdlg.addEventListener('change', e => {
  const el = e.target, v = el.value;
  if (el.name === 'theme') changeSettings({theme:v});
  else if (el.name === 'toolbar_style') changeSettings({toolbar_style:v});
  else if (el.name === 'density') changeSettings({density:v});
  else if (el.name === 'account_mark') changeSettings({account_mark:v});
  else if (el.name === 'unread_style') changeSettings({unread_style:v});
  else if (el.name === 'list_dates') setDates(v);
  else if (el.id === 's-sizecol') setSizeColumn(el.checked);
  else if (el.name === 'font') changeSettings({font:v});
  else if (el.name === 'text_size') changeSettings({text_size:Number(v)});
  else if (el.name === 'language') changeSettings({language:v});
  else if (el.name === 'mark_read') changeSettings({mark_read:v});
  else if (el.id === 's-threaded') changeSettings({threaded:el.checked});
  else if (el.id === 's-delay') changeSettings({mark_read:'moment', read_delay:Number(v)});
  else if (el.name === 'prefer_text') changeSettings({prefer_text:v === 'text'});
  else if (el.name === 'mail_paper'){ state.white.clear(); changeSettings({mail_paper:v}).then(() => { if (!state.compose) renderPreview(); }); }
  else if (el.name === 'compose_format') changeSettings({compose_format:v});
  else if (el.name === 'reply_format') changeSettings({reply_format:v});
  else if (el.name === 'image_size') changeSettings({image_size:v});
  else if (el.id === 's-subject') changeSettings({ask_subject:el.checked});
  else if (el.id === 's-askfile') changeSettings({ask_attachment:el.checked});
  else if (el.id === 's-askmany') changeSettings({ask_many:el.checked});
  else if (el.id === 's-spell') changeSettings({spell_check:el.checked}).then(respell);
  else if (el.dataset.spell) toggleSpellLanguage(el);
  else if (el.id === 's-every') changeSettings({check_every:Number(v)});
  else if (el.id === 's-push') changeSettings({push_mail:el.checked});
  else if (el.id === 's-notify') changeSettings({notify:el.checked});
  else if (el.id === 's-undo') changeSettings({undo_send:Number(v)});
  else if (el.id === 's-signrep') changeSettings({sign_replies:el.checked});
  else if (el.name === 'spam_filter') changeSettings({spam_filter:v});
  else if (el.id === 's-risk') changeSettings({risk_checks:el.checked}).then(ok => ok && !state.compose && renderPreview());
  else if (el.id === 's-count') changeSettings({usage_count:el.checked ? 'on' : 'off'}).then(ok => ok && countCopy());
  else if (el.name === 'updates') changeSettings({updates:v}).then(ok => ok && autoCheck(true));
  else if (el.name === 'backup_every') changeSettings({backup_every:v}).then(ok => { if (ok){ renderTree(); autoBackup(); } });
  else if (el.dataset.mv) moveInTick(el);
  else if (el.dataset.part) move_.choice[el.dataset.part] = el.checked;
});
forcedColors.addEventListener('change', () => { if (sdlg.open) $('s-forced').hidden = !forcedColors.matches; });
$('s-check').addEventListener('click', () => { sdlg.close(); openUpdates(); });
// A login that works saves the account, so it isn't left halfway: no closing until it answers.
$('s-close').addEventListener('click', () => { if (wizardBusy !== 'login') sdlg.close(); });
sdlg.addEventListener('cancel', e => { if (wizardBusy === 'login') e.preventDefault(); });
sdlg.addEventListener('close', () => {
  ++wizardSeq; wizardBusy = null;
  paintSpellChip(); // its languages may have changed there
  if (accountView === 'work') accountView = 'start';
  listEl.focus({preventScroll:true});
});

// Import and export. The file holds settings, rules, identities and account servers, never
// passwords. Importing shows what would change first; nothing changes until Import.
// How large pictures put into a message are made (images::prepare).
const IMAGE_SIZES = [['large', N_('Large: at most 1600 pixels across')], ['medium', N_('Medium: at most 800 pixels across')], ['original', N_('As they are')]];
const SETTING_LABELS = {theme:N_('Theme'), language:N_('Language'), toolbar_style:N_('Toolbar'), density:N_('Rows'), account_mark:N_('Account colors'), unread_style:N_('Unread mail'), font:N_('Font'), text_size:N_('Text size'), prefer_text:N_('HTML mail'),
  threaded:N_('Threads'), mark_read:N_('Mark messages as read'), read_delay:N_('A moment on screen'), check_every:N_('Check for new mail'), mail_paper:N_('HTML mail in a dark theme'),
  risk_checks:N_('Scam warnings'), spam_filter:N_('Spam filter'), compose_format:N_('New messages'), reply_format:N_('Replies and forwards'), sign_replies:N_('Sign replies and forwards too'), image_size:N_('Pictures'), ask_subject:N_('Ask before sending a message without a subject'), ask_attachment:N_('Ask when the text mentions an attachment but none is attached'), ask_many:N_('Ask before a reply to all goes to 8 people or more'),
  spell_check:N_('Underline spelling mistakes'), spell_languages:N_('Spelling languages'), updates:N_('Updates'), push_mail:N_('Show new mail at once'),
  notify:N_('Notify me of new mail while Telex is in the background'), undo_send:N_('Undo sending'),
  backup_every:N_('Back up by itself'), backup_keep:N_('Backups to keep')};
function settingValue(key, v){
  switch (key){
    case 'theme': return THEME_NAMES[v] ? t(THEME_NAMES[v]) : v;
    case 'language': return v === 'auto' ? t('Automatic, as in Windows') : LANG_NAMES[v] ?? v;
    case 'density': return v === 'comfortable' ? t('Comfortable') : t('Compact');
    case 'toolbar_style': return TOOLBAR_STYLES.find(([id]) => id === v) ? t(TOOLBAR_STYLES.find(([id]) => id === v)[1]) : v;
    case 'account_mark': return MARKS.find(([id]) => id === v) ? t(MARKS.find(([id]) => id === v)[1]) : v;
    case 'unread_style': return UNREAD_STYLES.find(([id]) => id === v) ? t(UNREAD_STYLES.find(([id]) => id === v)[1]) : v;
    case 'font': return fontName(v);
    case 'text_size': return v + '%';
    case 'prefer_text': return v ? t('Show the plain text') : t('Show the HTML');
    case 'mark_read': return {moment:t('After a moment on screen'), at_once:t('As soon as I select them'), window:t('When I open them in their own window'), manual:t('Only when I mark them myself')}[v] ?? v;
    case 'read_delay': return delayText(v);
    case 'check_every': return v === 0 ? t('Only when I click Get mail') : v === 60 ? t('Every hour') : tn(v, 'Every {n} minute', 'Every {n} minutes');
    case 'updates': return v === 'ask' ? t('Not decided yet') : updatesText(v);
    case 'mail_paper': return {theme:t('On the theme’s dark paper'), white:t('On white, as sent')}[v] ?? v;
    case 'compose_format': return {plain:t('Plain text'), html:t('HTML')}[v] ?? v;
    case 'image_size': return IMAGE_SIZES.find(([id]) => id === v) ? t(IMAGE_SIZES.find(([id]) => id === v)[1]) : v;
    case 'spam_filter': return SPAM_LEVELS.find(([id]) => id === v) ? t(SPAM_LEVELS.find(([id]) => id === v)[1]) : v;
    case 'reply_format': return {same:t('In the format of the message'), plain:t('Always plain text'), html:t('Always HTML')}[v] ?? v;
    case 'threaded': case 'risk_checks': case 'sign_replies': case 'ask_subject': case 'ask_attachment': case 'ask_many': case 'spell_check': case 'push_mail': case 'notify': return v ? t('On') : t('Off');
    case 'undo_send': return undoText(v);
    case 'backup_every': return {off:t('Never'), daily:t('Every day'), weekly:t('Every week')}[v] ?? v;
    case 'backup_keep': return tn(v, 'the newest {n} backup', 'the newest {n} backups');
    case 'spell_languages': return v.length ? v.map(langName).join(', ') : t('Automatic|languages');
    default: return String(v);
  }
}
const STATUS = {add:N_('New|import'), update:N_('Updated|import'), same:N_('Unchanged|import'), skip:N_('Skipped|import')};
function skipText(s){
  if (!s) return '';
  if (s.code === 'no_account') return t('{email} isn’t set up here yet', {email:s.email});
  if (s.code === 'no_folder') return t('no folder “{folder}” here', {folder:s.folder});
  if (s.code === 'unreadable') return t('from a newer Telex, or edited by hand');
  if (s.code === 'unsupported') return t('Telex can’t do “{what}”', {what:s.what});
  return tr(s.detail ?? '');
}
function counted(items){
  const n = k => items.filter(i => i.status === k).length;
  return [[n('add'), N_('{n} new')], [n('update'), N_('{n} updated')], [n('same'), N_('{n} unchanged')], [n('skip'), N_('{n} skipped')]]
    .filter(([c]) => c).map(([c, key]) => tn(c, key, key)).join(', ');
}
const why = (i, accounts = true) => i.status === 'skip' ? skipText(i.skip) : accounts ? i.account ?? t('All accounts') : '';
// Items of a plan or report; tags belong to no account, so `accounts` false names none.
function planItems(items, accounts = true){
  return `<ul class="ilist">${items.map(i => `<li><span class="ist ${i.status}">${esc(t(STATUS[i.status]))}</span><span class="iname">${esc(i.name)}</span>`
    + `<span class="iwhy" title="${esc(why(i, accounts))}">${esc(why(i, accounts))}</span></li>`).join('')}</ul>`;
}
function planSection(part, title, items){
  if (!items.length) return '';
  const doable = items.some(i => i.status === 'add' || i.status === 'update');
  return `<section class="ipart"><label class="ihead"><input type="checkbox" data-part="${part}"${move_.choice[part] ? ' checked' : ''}${doable ? '' : ' disabled'}>`
    + `<b>${esc(title)}</b><span>${esc(counted(items))}</span></label>${planItems(items)}</section>`;
}
function renderMove(){
  const el = $('s-move'), p = move_.plan;
  if (moveIn.source){ renderSource(el); return; }
  if (!p){
    const c = move_.counts;
    const n = (k, one, other) => c ? ` <small class="sdim">${esc(tn(c[k], one, other))}</small>` : '';
    if (!moveIn.sources) findPrograms().then(() => { if (sdlg.open && setTab === 'move' && !moveIn.source && !move_.plan) renderMove(); });
    el.innerHTML = movingSection() + `<h3 class="rh">${esc(t('Export'))}</h3>
<p class="dnote">${esc(t('Save your settings to a file, to set up Telex on another PC or keep a copy. The file holds no passwords.'))}</p>
<div class="sopts">
  <label class="sopt"><input type="checkbox" data-ex="settings" checked><span>${esc(t('Settings: theme, language, rows, mail checks'))}</span></label>
  <label class="sopt"><input type="checkbox" data-ex="rules" checked><span>${esc(t('Rules and tags'))}</span>${n('rules', N_('{n} rule'), '{n} rules')}</label>
  <label class="sopt"><input type="checkbox" data-ex="identities" checked><span>${esc(t('Identities, signatures and templates'))}</span>${n('identities', N_('{n} identity'), '{n} identities')}</label>
  <label class="sopt"><input type="checkbox" data-ex="servers" checked><span>${esc(t('Account servers and user names'))}</span>${n('accounts', N_('{n} account'), '{n} accounts')}</label>
</div>
<div class="sbtns"><button class="btn primary" type="button" id="s-export">${esc(t('Export…'))}</button>
${move_.exported ? `<span class="sdone">${esc(t('Saved to {path}.', {path:move_.exported}))} <button type="button" class="linkbtn" id="s-show">${esc(t('Show in folder'))}</button></span>` : ''}</div>
<h3 class="rh">${esc(t('Import'))}</h3>
<p class="dnote">${esc(t('Bring in a file exported from Telex. You see what it would change before anything does.'))}</p>
<div class="sbtns"><button class="btn" type="button" id="s-import">${esc(t('Choose a file…'))}</button></div>
<p class="derr" role="alert">${esc(move_.error)}</p>`;
    if (!c) countThings();
    return;
  }
  const r = move_.report;
  const made = [p.app ? t('Telex {version}', {version:p.app}) : '', p.exported ? fmtFull(parseDate(p.exported)) : ''].filter(Boolean).join(', ');
  const settingsItems = p.settings.map(c => `<li><span class="iname">${esc(t(SETTING_LABELS[c.key] ?? c.key))}</span>`
    + `<span class="iwhy">${esc(settingValue(c.key, c.from))} → <b>${esc(settingValue(c.key, c.to))}</b></span></li>`).join('');
  const settings = p.has_settings ? `<section class="ipart"><label class="ihead"><input type="checkbox" data-part="settings"${move_.choice.settings ? ' checked' : ''}${p.settings.length ? '' : ' disabled'}>`
    + `<b>${esc(t('Settings'))}</b><span>${esc(p.settings.length ? tn(p.settings.length, '{n} change', '{n} changes') : t('the same as here'))}</span></label>`
    + (settingsItems ? `<ul class="ilist">${settingsItems}</ul>` : '')
    + (p.settings_ignored.length ? `<p class="dnote">${esc(t('Left out, unknown to this version: {keys}.', {keys:p.settings_ignored.join(', ')}))}</p>` : '') + '</section>' : '';
  const missing = p.accounts.filter(a => !a.present);
  const accounts = p.accounts.length ? `<section class="ipart"><div class="ihead"><b>${esc(t('Accounts'))}</b><span>${esc(missing.length
      ? tn(missing.length, '{n} to set up', '{n} to set up') : t('all set up here'))}</span></div><ul class="ilist">${p.accounts.map(a =>
      `<li><span class="iname">${esc(a.name || a.email)}</span><span class="iwhy">${esc(a.email)}</span>${a.present ? `<span class="ist same">${esc(t('Set up|account'))}</span>`
      : a.setup ? `<button type="button" class="btn" data-setup="${esc(a.email)}">${esc(t('Set up…'))}</button>` : ''}</li>`).join('')}</ul>`
    + (missing.length ? `<p class="dnote">${esc(t('Rules and identities of an account come along once it is set up here: set it up, then import the file again.'))}</p>` : '') + '</section>' : '';
  const doneLines = r ? [
    r.settings ? tn(r.settings, '{n} setting changed', '{n} settings changed') : '',
    r.rules_added ? tn(r.rules_added, '{n} rule added', '{n} rules added') : '',
    r.rules_updated ? tn(r.rules_updated, '{n} rule updated', '{n} rules updated') : '',
    r.tags_added ? tn(r.tags_added, '{n} tag added', '{n} tags added') : '',
    r.tags_updated ? tn(r.tags_updated, '{n} tag updated', '{n} tags updated') : '',
    r.identities_added ? tn(r.identities_added, '{n} identity added', '{n} identities added') : '',
    r.identities_updated ? tn(r.identities_updated, '{n} identity updated', '{n} identities updated') : '',
    r.templates ? tn(r.templates, 'templates set for {n} folder', 'templates set for {n} folders') : '',
  ].filter(Boolean) : [];
  // A folder's templates come with the identities.
  const ids = [...p.identities, ...(p.templates ?? []).map(i => ({...i, name:t('Templates of {folder}', {folder:i.name})}))];
  el.innerHTML = `<h3 class="rh">${esc(r ? t('Imported') : t('Import'))}</h3>
${made ? `<p class="dnote">${esc(t('From {made}.', {made}))}</p>` : ''}
${p.newer ? `<p class="whint warn">${esc(t('This file comes from a newer Telex. What this version doesn’t know is left out.'))}</p>` : ''}
${r ? `<p class="whint">${esc(doneLines.length ? doneLines.join(', ') + '.' : t('Nothing needed changing.'))}</p>` + (r.problems.length ? `<p class="derr">${esc(r.problems.map(tr).join(' · '))}</p>` : '') : ''}
${r ? accounts : settings + planSection('rules', t('Rules and tags'), [...p.rules, ...(p.tags ?? []).map(i => ({...i, account:t('Tag')}))]) + planSection('identities', t('Identities, signatures and templates'), ids) + accounts}
<p class="derr" role="alert">${esc(move_.error)}</p>
<div class="sbtns">${r ? `<button class="btn" type="button" id="s-back">${esc(t('Done'))}</button>`
  : `<button class="btn primary" type="button" id="s-apply">${esc(t('Import'))}</button><button class="btn" type="button" id="s-back">${esc(t('Cancel'))}</button>`}</div>`;
}
// How many rules, identities and accounts an export would hold, for the checkboxes.
async function countThings(){
  let rules = [];
  try { rules = await invoke('list_rules'); } catch {}
  const real = servers();
  move_.counts = {rules:rules.length, identities:real.reduce((n, a) => n + (a.identities?.length ?? 0), 0), accounts:real.length};
  if (sdlg.open && setTab === 'move' && !move_.plan) renderMove();
}
function importError(e){
  if (e?.code === 'not_json' || e?.code === 'not_telex') return t('This isn’t a Telex settings file. Choose a file that Telex exported.');
  if (e?.code === 'unreadable') return t('Telex couldn’t read the file: {detail}', {detail:e.detail});
  return tr(e?.detail ?? e);
}
$('s-move').addEventListener('input', e => {
  const key = e.target.dataset?.pw;
  if (key) moveIn.passwords[key] = e.target.value;
});
$('s-move').addEventListener('click', async e => {
  const b = e.target.closest('button');
  if (!b) return;
  if (b.dataset.source){ const src = moveIn.sources?.find(x => x.path === b.dataset.source); if (src) chooseSource(src); return; }
  if (b.id === 'mv-pst' || b.id === 'mv-dir'){
    let src;
    try {
      src = await invoke('open_source', {folder:b.id === 'mv-dir', words:b.id === 'mv-dir'
        ? {title:t('Choose a folder of mail: Maildir, MH, or a Thunderbird profile'), filter:''}
        : {title:t('Choose an Outlook data file'), filter:t('Outlook data files')}});
    } catch (err){ move_.error = tr(err); renderMove(); return; }
    if (src) chooseSource(src);
    return;
  }
  if (b.id === 'mv-go'){ bringIn(); return; }
  if (b.id === 'mv-back' || b.id === 'mv-done'){
    moveIn = {...moveIn, source:null, report:null, error:'', passwords:{}};
    renderMove();
    return;
  }
  move_.error = '';
  if (b.id === 's-export'){
    const choice = {};
    $('s-move').querySelectorAll('[data-ex]').forEach(c => { choice[c.dataset.ex] = c.checked; });
    if (!Object.values(choice).some(Boolean)){ move_.error = t('Tick at least one part to export.'); renderMove(); return; }
    let path;
    try {
      path = await invoke('export_settings', {choice, words:{title:t('Export Telex settings'), filter:t('Telex settings'),
        file_name:t('Telex settings {date}', {date:ymd(new Date())}) + '.json'}});
    } catch (err){ move_.error = tr(err); renderMove(); return; }
    if (!path) return; // cancelled
    move_.exported = path; renderMove();
    say(t('Saved to {path}.', {path}));
  } else if (b.id === 's-show'){
    invoke('show_export').catch(err => say(tr(err), 8000));
  } else if (b.id === 's-import'){
    let plan;
    try { plan = await invoke('open_import', {words:{title:t('Import Telex settings'), filter:t('Telex settings')}}); }
    catch (err){ move_.error = importError(err); renderMove(); return; }
    if (!plan) return; // cancelled
    const doable = items => items.some(i => i.status === 'add' || i.status === 'update');
    move_ = {...move_, plan, report:null, choice:{settings:plan.settings.length > 0, rules:doable(plan.rules) || doable(plan.tags ?? []), identities:doable(plan.identities)}};
    renderMove();
    ($('s-apply') ?? $('s-back'))?.focus();
  } else if (b.id === 's-apply'){
    let r;
    try { r = await invoke('apply_import', {choice:move_.choice}); }
    catch (err){ move_.error = tr(err); renderMove(); return; }
    move_.report = r.report;
    const before = SETTINGS.language;
    SETTINGS = r.settings;
    applySettings();
    await refresh();
    if (SETTINGS.language !== before) renderSettings(); else { syncSettings(); renderMove(); }
    $('s-back')?.focus();
    say(t('Settings imported.'));
  } else if (b.id === 's-back'){
    move_ = {plan:null, choice:null, report:null, exported:move_.exported, error:'', counts:null};
    renderMove();
  } else if (b.dataset.setup){
    const a = move_.plan?.accounts.find(x => x.email === b.dataset.setup);
    if (!a?.setup) return;
    const servers = a.setup.smtp_host && a.setup.smtp_host !== a.setup.host ? t('{a} and {b}', {a:a.setup.host, b:a.setup.smtp_host}) : a.setup.host;
    editing = null;
    openForm(a.setup, '', null, t('The servers come from the settings file: {servers}. Check them before you enter your password.', {servers}));
  }
});

// Moving in from another mail program (Thunderbird, Outlook, Maildir folders): what it has is
// found on this PC only; the user ticks accounts (typing their passwords), mail kept on the PC
// and filters; the core adds them and reads the mail into folders on this PC.
let moveIn = {sources:null, finding:null, source:null, choice:null, passwords:{}, running:null, report:null, error:''};
const PROGRAMS = {thunderbird:'Thunderbird', betterbird:'Betterbird', seamonkey:'SeaMonkey', postbox:'Postbox', outlook:'Outlook', evolution:'Evolution', kmail:'KMail', folder:N_('Mail folder')};
const programName = p => p === 'folder' ? t(PROGRAMS.folder) : PROGRAMS[p] ?? p;
const BLOCKED = {oauth:N_('signs in with Google or Microsoft, which Telex can’t do yet'), pop3:N_('POP3 in Outlook: Telex can’t tell which mail Outlook already downloaded; add the account by hand. Its mail on this PC can come along below'),
  exchange:N_('Exchange or Microsoft 365: Telex can’t sign in there yet')};
const canAdd = a => !a.present && !a.blocked;
function findPrograms(){
  moveIn.finding ??= invoke('find_programs').then(list => { moveIn.sources = list; }, () => { moveIn.sources = []; }).finally(() => { moveIn.finding = null; });
  return moveIn.finding;
}
// What a source holds, in a line: "40 accounts · 2.1 GB of mail · 5 filters".
function sourceSummary(s){
  const mail = s.folders.reduce((n, f) => n + f.bytes, 0);
  return [s.accounts.length ? tn(s.accounts.length, '{n} account', '{n} accounts') : '',
    s.folders.length ? t('{size} of mail', {size:fmtSize(Math.ceil(mail / 1024))}) : '',
    s.filters ? tn(s.filters, '{n} filter', '{n} filters') : '',
    s.tags?.length ? tn(s.tags.length, '{n} tag', '{n} tags') : '',
    s.books?.length ? tn(s.books.length, '{n} address book', '{n} address books') : ''].filter(Boolean).join(' · ');
}
function openMoveIn(path){
  openSettings('move');
  const s = moveIn.sources?.find(x => x.path === path);
  if (s) chooseSource(s);
}
function chooseSource(s){
  moveIn = {...moveIn, source:s, report:null, running:null, error:'', passwords:{}, choice:{
    accounts:new Set(s.accounts.filter(canAdd).map(a => a.key)),
    folders:new Set(s.folders.filter(f => f.tick).map(f => f.key)),
    filters:s.filters > 0 || s.tags?.length > 0,
    books:new Set((s.books ?? []).filter(b => b.tick).map(b => b.key))}};
  renderMove();
  sdlg.querySelector('#s-move input:not([disabled])')?.focus();
}
function movingSection(){
  const list = moveIn.sources;
  const found = list === null ? `<p class="dnote">${esc(t('Looking for mail programs on this PC…'))}</p>`
    : list.length ? `<ul class="ilist mvlist">${list.map(s => `<li><span class="iname">${esc(programName(s.program))}</span><span class="iwhy" title="${esc(s.path)}">${esc(s.name)} · ${esc(sourceSummary(s))}</span>`
      + `<button type="button" class="btn" data-source="${esc(s.path)}">${esc(t('Bring in…'))}</button></li>`).join('')}</ul>`
    : `<p class="dnote">${esc(t('No other mail program found on this PC.'))}</p>`;
  return `<h3 class="rh">${esc(t('From another mail program'))}</h3>
<p class="dnote">${esc(t('Thunderbird, Outlook and others: their accounts with identities and signatures, the mail they keep on this PC, and their filters. Telex looks only on this PC.'))}</p>
${found}
<div class="sbtns"><button class="btn" type="button" id="mv-pst">${esc(t('Outlook data file (.pst)…'))}</button><button class="btn" type="button" id="mv-dir">${esc(t('A folder of mail…'))}</button></div>`;
}
// Ticked or not, as the user left it.
const ticked = (on, attrs) => `<input type="checkbox" ${attrs}${on ? ' checked' : ''}>`;
function renderSource(el){
  const s = moveIn.source, c = moveIn.choice, r = moveIn.report, run = moveIn.running;
  const title = `<h3 class="rh">${esc(programName(s.program))} · ${esc(s.name)}</h3>`;
  if (run){
    const order = ['accounts', 'mail', 'rules', 'contacts'], at = order.indexOf(run.step);
    const step = (id, label, small) => `<li class="${run.step === id ? 'work' : order.indexOf(id) < at ? 'done' : ''}">${esc(label)}${small ? `<small>${esc(small)}</small>` : ''}</li>`;
    const size = n => fmtSize(Math.ceil(n / 1024));
    const accounts = c.accounts.size ? step('accounts', run.step === 'accounts' ? t('Adding accounts: {done} of {total}', {done:run.done, total:run.total}) : t('Adding accounts'),
      run.step === 'accounts' ? run.name : '') : '';
    const mail = c.folders.size ? step('mail', run.step === 'mail' ? t('Reading mail: {done} of {total}', {done:size(run.done), total:size(run.total)}) : t('Reading mail'),
      run.step === 'mail' ? run.name : '') : '';
    const rules = c.filters ? step('rules', t('Making rules of the filters'), '') : '';
    const books = c.books.size ? step('contacts', t('Bringing the address books'), '') : '';
    const bar = run.step === 'mail' && run.total ? `<div class="ubar mvbar"><i style="width:${Math.min(100, run.done / run.total * 100).toFixed(1)}%"></i></div>` : '';
    el.innerHTML = `${title}<ul class="wsteps" aria-live="polite">${accounts}${mail}${rules}${books}</ul>${bar}
<p class="dnote">${esc(t('Telex keeps working meanwhile. Mail already here isn’t brought twice.'))}</p>`;
    return;
  }
  if (r){
    const ok = r.accounts.filter(a => !a.error), bad = r.accounts.filter(a => a.error);
    const changed = items => (items ?? []).filter(i => i.status === 'add' || i.status === 'update').length;
    const made = changed(r.rules), tagged = changed(r.tags);
    const lines = [ok.length ? tn(ok.length, '{n} account added', '{n} accounts added') : '',
      r.mail.added ? tn(r.mail.added, '{n} message in folders on this PC', '{n} messages in folders on this PC', {n:num(r.mail.added)}) : '',
      r.mail.already ? tn(r.mail.already, '{n} was here already', '{n} were here already', {n:num(r.mail.already)}) : '',
      tagged ? tn(tagged, '{n} tag', '{n} tags') : '',
      made ? tn(made, '{n} rule', '{n} rules') : '',
      r.contacts?.added ? tn(r.contacts.added, '{n} new contact', '{n} new contacts', {n:num(r.contacts.added)}) : '',
      r.contacts?.updated ? tn(r.contacts.updated, '{n} contact filled in', '{n} contacts filled in', {n:num(r.contacts.updated)}) : '',
      r.contacts?.groups ? tn(r.contacts.groups, '{n} new group', '{n} new groups') : ''].filter(Boolean);
    const accounts = r.accounts.length ? `<section class="ipart"><div class="ihead"><b>${esc(t('Accounts'))}</b><span>${esc(bad.length ? tn(bad.length, '{n} needs attention', '{n} need attention') : t('all logged in'))}</span></div>`
      + `<ul class="ilist">${r.accounts.map(a => `<li><span class="ist ${a.error ? 'skip' : 'add'}">${esc(a.error ? t('Check|account') : t('Added|account'))}</span><span class="iname">${esc(a.email)}</span>`
      + `<span class="iwhy" title="${esc(a.error ? tr(a.error) : '')}">${esc(a.error ? tr(a.error) : '')}</span></li>`).join('')}</ul>`
      + (bad.length ? `<p class="dnote">${esc(t('Fix these in Settings › Accounts › Edit: the account is there and needs its password or servers.'))}</p>` : '') + '</section>' : '';
    const tags = r.tags?.length ? `<section class="ipart"><div class="ihead"><b>${esc(t('Tags'))}</b><span>${esc(counted(r.tags))}</span></div>${planItems(r.tags, false)}</section>` : '';
    const rules = r.rules.length ? `<section class="ipart"><div class="ihead"><b>${esc(t('Filters'))}</b><span>${esc(counted(r.rules))}</span></div>${planItems(r.rules)}</section>` : '';
    el.innerHTML = `${title}<p class="whint">${esc(lines.length ? lines.join(', ') + '.' : t('Nothing needed bringing.'))}</p>
${r.mail.problems.length ? `<p class="derr">${esc(r.mail.problems.map(tr).join(' · '))}</p>` : ''}${accounts}${tags}${rules}
<div class="sbtns"><button class="btn primary" type="button" id="mv-done">${esc(t('Done'))}</button></div>`;
    return;
  }
  const addable = s.accounts.filter(canAdd);
  const accounts = s.accounts.length ? `<section class="ipart"><label class="ihead">${ticked(addable.length && addable.every(a => c.accounts.has(a.key)), `data-mv="all-accounts"${addable.length ? '' : ' disabled'}`)}`
    + `<b>${esc(t('Accounts'))}</b><span>${esc(tn(c.accounts.size, '{n} to add', '{n} to add'))}</span></label><ul class="ilist mvaccts">${s.accounts.map(a => {
      const why = a.present ? t('already in Telex') : a.blocked ? t(BLOCKED[a.blocked] ?? a.blocked) : `${a.setup.email} · ${a.setup.host}`;
      const pw = canAdd(a) ? `<input type="password" class="mvpw" data-pw="${esc(a.key)}" value="${esc(moveIn.passwords[a.key] ?? '')}" placeholder="${esc(t('Password'))}" aria-label="${esc(t('Password for {email}', {email:a.setup.email}))}" autocomplete="off"${c.accounts.has(a.key) ? '' : ' disabled'}>` : '';
      return `<li><label class="mvpick">${ticked(c.accounts.has(a.key), `data-mv="account" data-key="${esc(a.key)}"${canAdd(a) ? '' : ' disabled'}`)}<span class="iname">${esc(a.setup.name || a.setup.email)}</span></label>`
        + `<span class="iwhy" title="${esc(why)}">${esc(why)}</span>${pw}</li>`;
    }).join('')}</ul><p class="dnote">${esc(t('Type each password once; Telex keeps it in the system’s password store. An account left without one is added anyway and asks for it in Settings › Accounts. Mail on the servers isn’t copied from here: Telex downloads it from them.'))}</p></section>` : '';
  const total = s.folders.filter(f => c.folders.has(f.key)).reduce((n, f) => n + f.bytes, 0);
  const folders = s.folders.length ? `<section class="ipart"><label class="ihead">${ticked(s.folders.every(f => c.folders.has(f.key)), 'data-mv="all-folders"')}`
    + `<b>${esc(t('Mail on this PC'))}</b><span>${esc(c.folders.size ? fmtSize(Math.ceil(total / 1024)) : t('none ticked'))}</span></label><ul class="ilist">${s.folders.map(f =>
      `<li><label class="mvpick">${ticked(c.folders.has(f.key), `data-mv="folder" data-key="${esc(f.key)}"`)}<span class="iname">${esc(f.name)}</span></label>`
      + `<span class="iwhy">${esc([f.messages != null ? tn(f.messages, '{n} message', '{n} messages', {n:num(f.messages)}) : '', fmtSize(Math.ceil(f.bytes / 1024))].filter(Boolean).join(' · '))}</span></li>`).join('')}</ul>`
    + `<p class="dnote">${esc(t('Each folder comes into a folder on this PC of the same name.'))}</p></section>` : '';
  // The tags come with the filters: filters use them, and so does the mail brought in.
  const stags = s.tags ?? [];
  const filters = s.filters || stags.length ? `<section class="ipart"><label class="ihead">${ticked(c.filters, 'data-mv="filters"')}<b>${esc(stags.length ? t('Filters and tags') : t('Filters'))}</b>`
    + `<span>${esc([s.filters ? tn(s.filters, '{n} filter', '{n} filters') : '', stags.length ? tn(stags.length, '{n} tag', '{n} tags') : ''].filter(Boolean).join(' · '))}</span></label>`
    + (stags.length ? `<p class="mvtags"><span class="tgs">${stags.map(x => `<span class="tg t-${esc(x.color)}" title="${esc(x.keyword)}">${esc(x.name)}</span>`).join('')}</span></p>` : '')
    + (s.filters ? `<p class="dnote">${esc(t('They become rules where Telex can do the same; the others are listed afterwards with what Telex can’t do.'))}</p>` : '')
    + (stags.length ? `<p class="dnote">${esc(t('Tags keep their keywords, so mail tagged there is tagged here too: on the servers and in the mail brought in.'))}</p>` : '') + '</section>' : '';
  const books = s.books?.length ? `<section class="ipart"><label class="ihead">${ticked(s.books.every(b => c.books.has(b.key)), 'data-mv="all-books"')}<b>${esc(t('Address books'))}</b><span>${esc(tn(c.books.size, '{n} ticked', '{n} ticked'))}</span></label><ul class="ilist">${s.books.map(b =>
      `<li><label class="mvpick">${ticked(c.books.has(b.key), `data-mv="book" data-key="${esc(b.key)}"`)}<span class="iname">${esc(b.name)}</span></label><span class="iwhy">${esc(tn(b.contacts, '{n} card', '{n} cards', {n:num(b.contacts)}))}</span></li>`).join('')}</ul>`
    + `<p class="dnote">${esc(t('Contacts and lists come into the address book (B); people already in it are only filled in. Thunderbird’s Collected Addresses are everyone it ever saw, so they start unticked.'))}</p></section>` : '';
  const nothing = !c.accounts.size && !c.folders.size && !c.filters && !c.books.size;
  el.innerHTML = `${title}${accounts}${folders}${filters}${books}<p class="derr" role="alert">${esc(moveIn.error)}</p>
<div class="sbtns"><button class="btn primary" type="button" id="mv-go"${nothing ? ' disabled' : ''}>${esc(t('Bring in'))}</button><button class="btn" type="button" id="mv-back">${esc(t('Back'))}</button></div>`;
}
// A tick in the source's lists: the counts and the buttons follow; typed passwords stay.
function moveInTick(el){
  const c = moveIn.choice, s = moveIn.source, on = el.checked, key = el.dataset.key;
  if (el.dataset.mv === 'account'){ on ? c.accounts.add(key) : c.accounts.delete(key); }
  else if (el.dataset.mv === 'folder'){ on ? c.folders.add(key) : c.folders.delete(key); }
  else if (el.dataset.mv === 'filters') c.filters = on;
  else if (el.dataset.mv === 'book'){ on ? c.books.add(key) : c.books.delete(key); }
  else if (el.dataset.mv === 'all-books') c.books = new Set(on ? s.books.map(b => b.key) : []);
  else if (el.dataset.mv === 'all-accounts') c.accounts = new Set(on ? s.accounts.filter(canAdd).map(a => a.key) : []);
  else if (el.dataset.mv === 'all-folders') c.folders = new Set(on ? s.folders.map(f => f.key) : []);
  const again = el.dataset.mv + (key ? `"][data-key="${key}` : '');
  renderMove();
  sdlg.querySelector(`#s-move [data-mv="${again}"]`)?.focus();
}
async function bringIn(){
  const s = moveIn.source, c = moveIn.choice;
  const passwords = {};
  c.accounts.forEach(k => { if (moveIn.passwords[k]) passwords[k] = moveIn.passwords[k]; });
  const choice = {accounts:[...c.accounts], passwords, folders:[...c.folders], filters:c.filters, books:[...c.books]};
  moveIn.passwords = {}; // typed once, sent once
  moveIn.running = {step:c.accounts.size ? 'accounts' : c.folders.size ? 'mail' : c.filters ? 'rules' : 'contacts', done:0, total:c.accounts.size, name:''};
  moveIn.error = '';
  renderMove();
  let r;
  try { r = await invoke('move_in', {path:s.path, choice}); }
  catch (e){ moveIn.running = null; moveIn.error = tr(e); renderMove(); return; }
  moveIn.running = null; moveIn.report = r;
  moveIn.sources = null; // what is in Telex now has changed
  if (sdlg.open && setTab === 'move') renderMove();
  await refresh();
  if (r.contacts?.added || r.contacts?.updated || r.contacts?.groups) bookChanged();
  if (r.accounts.some(a => !a.error)) getMail({quiet:true});
  say(t('Moved in from {program}.', {program:programName(s.program)}));
}
listenHere('move-progress', ({payload: p}) => {
  if (!moveIn.running) return;
  moveIn.running = p;
  if (sdlg.open && setTab === 'move') renderMove();
});

// Settings › Writing: the languages spelling is checked in, and dictionaries to fetch.
let spellInfo = null, spellFetching = '', spellError = '';
async function renderSpellLanguages(){
  const el = $('s-spell-langs');
  if (!spellInfo){
    el.innerHTML = `<p class="dnote">${esc(t('Looking for dictionaries…'))}</p>`;
    try { spellInfo = await invoke('spell_languages'); } catch (e){ el.innerHTML = `<p class="derr">${esc(tr(e))}</p>`; return; }
  }
  const info = spellInfo, chosen = new Set(info.chosen), ui = I18n.lang();
  const langs = info.languages.length ? `<div class="sopts">${info.languages.map(l =>
      `<label class="sopt"><input type="checkbox" data-spell="${esc(l.tag)}"${chosen.has(l.tag) ? ' checked' : ''}><span>${esc(langName(l.tag))}</span>`
      + (l.own ? `<small class="sdim">${esc(t('fetched by Telex'))} · <button type="button" class="linkbtn" data-unspell="${esc(l.tag)}">${esc(t('Remove'))}</button></small>` : '')
      + '</label>').join('')}</div>`
    : `<p class="dnote">${esc(t('This PC has no spelling dictionaries yet.'))}</p>`;
  const auto = info.auto && info.chosen.length ? `<p class="dnote">${esc(t('Ticked by themselves from Windows’ languages. Tick the ones you write in.'))}</p>` : '';
  // The languages the user writes in first: the ones most likely wanted.
  const first = d => (info.missing ?? []).includes(d.tag) || d.tag.split('-')[0] === ui;
  const offers = [...info.downloads].sort((a, b) => first(b) - first(a));
  const get = offers.length ? `<div class="sbtns"><select id="s-dict" class="sselect" aria-label="${esc(t('Dictionary to download'))}">${offers.map(d =>
      `<option value="${esc(d.tag)}">${esc(langName(d.tag))} · ${esc(fmtSize(Math.ceil(d.bytes / 1024)))}</option>`).join('')}</select>`
    + `<button type="button" class="btn" id="s-dict-get"${spellFetching ? ' disabled' : ''} title="${esc(t('LibreOffice’s dictionary, fetched from GitHub only when you click and checked before use. Nothing about you is sent.'))}">${esc(spellFetching ? t('Downloading…') : t('Add language'))}</button></div>` : '';
  el.innerHTML = langs + auto + get + `<p class="derr" role="alert">${esc(spellError)}</p>`;
}
async function toggleSpellLanguage(box){
  const list = new Set(spellInfo?.chosen ?? []);
  if (box.checked) list.add(box.dataset.spell); else list.delete(box.dataset.spell);
  await changeSettings({spell_languages:[...list]});
  spellInfo = null; await renderSpellLanguages(); respell();
  sdlg.querySelector(`[data-spell="${CSS.escape(box.dataset.spell)}"]`)?.focus();
}
$('s-spell-langs').addEventListener('click', async e => {
  const b = e.target.closest('button');
  if (!b) return;
  spellError = '';
  try {
    if (b.id === 's-dict-get'){
      const tag = $('s-dict').value;
      spellFetching = tag; renderSpellLanguages();
      try { spellInfo = await invoke('spell_download', {tag}); } finally { spellFetching = ''; }
      // What was fetched is checked from now on, besides what was checked before.
      if (!spellInfo.chosen.includes(tag)){ await changeSettings({spell_languages:[...spellInfo.chosen, tag]}); spellInfo = null; }
      say(t('{language} spelling is on this PC now.', {language:langName(tag)}));
    } else if (b.dataset.unspell){
      spellInfo = await invoke('spell_remove', {tag:b.dataset.unspell});
    } else return;
  } catch (err){ spellError = tr(err); }
  await renderSpellLanguages(); respell();
});

// Settings › Reading: senders whose images load without asking. Removing one asks again.
async function renderTrusted(){
  let senders = [];
  try { senders = await invoke('trusted_senders'); } catch {}
  $('s-trusted').innerHTML = senders.length ? senders.map(s =>
    `<div class="srow"><span class="em">${esc(s)}</span><button type="button" class="btn" data-untrust="${esc(s)}">${esc(t('Remove'))}</button></div>`).join('')
    : `<p class="dnote">${esc(t('Nobody yet. “Always load from …” above a message adds its sender here.'))}</p>`;
}
$('s-trusted').addEventListener('click', async e => {
  const b = e.target.closest('[data-untrust]'); if (!b) return;
  try { await invoke('trust_sender', {email:b.dataset.untrust, trusted:false}); } catch (err){ say(tr(err), 8000); return; }
  htmlViews.clear();
  await renderTrusted();
  $('s-trusted').querySelector('button')?.focus();
  if (!state.compose) renderPreview();
});

/* ---------- usage ---------- */
const USE_GROUPS = [['inbox', N_('Inbox'), 0], ['sent', N_('Sent'), 1], ['archive', N_('Archive'), 3], ['custom', N_('Your folders'), 2],
  ['drafts', N_('Drafts and Outbox'), 5], ['junk', N_('Spam and Trash'), 4]];
const groupOf = kind => ({outbox:'drafts', spam:'junk', trash:'junk'}[kind] ?? kind);
const bar = (pct, level) => `<div class="ubar" data-level="${level}"><i style="width:${Math.max(pct, 0.8)}%"></i></div>`;
function flowSvg(flow, since){
  const max = Math.max(1, ...flow.map(d => d.received + d.spam), ...flow.map(d => d.sent));
  const W = 300, H = 64, mid = 40, up = mid - 3, down = H - mid - 3, bw = W / flow.length;
  const day = i => new Date(since.getFullYear(), since.getMonth(), since.getDate() + i);
  const bars = flow.map((d, i) => {
    const x = (i * bw + 1).toFixed(1), w = (bw - 2).toFixed(1), date = day(i);
    const hr = d.received / max * up, hs = d.spam / max * up, ho = d.sent / max * down;
    const tip = `${t(DOW[date.getDay()])} ${dm(date)}: ` + [tn(d.received, '{n} received', '{n} received'), tn(d.sent, '{n} sent', '{n} sent'),
      d.spam ? tn(d.spam, '{n} in Spam', '{n} in Spam') : ''].filter(Boolean).join(', ');
    return `<g><title>${esc(tip)}</title><rect class="hit" x="${x}" y="0" width="${w}" height="${H}"/>`
      + (hr ? `<rect class="in" x="${x}" y="${(mid - hr).toFixed(1)}" width="${w}" height="${hr.toFixed(1)}" rx="1"/>` : '')
      + (hs ? `<rect class="junk" x="${x}" y="${(mid - hr - hs).toFixed(1)}" width="${w}" height="${hs.toFixed(1)}" rx="1"/>` : '')
      + (ho ? `<rect class="out" x="${x}" y="${(mid + 1).toFixed(1)}" width="${w}" height="${ho.toFixed(1)}" rx="1"/>` : '') + '</g>';
  }).join('');
  return `<svg class="flow" viewBox="0 0 ${W} ${H}" preserveAspectRatio="none" role="img" aria-label="${esc(t('Mail received and sent per day'))}">${bars}<line class="axis" x1="0" x2="${W}" y1="${mid + 0.5}" y2="${mid + 0.5}"/></svg>`;
}
// Settings › Usage: one line per account (the server's storage when it says, what this PC keeps,
// the last check), and below it the details of the account picked there: what each kind of
// folder takes, mail in and out over the last 30 days, who sends the most, the largest
// messages, and what still waits.
let usagePick = null;
function usageTotals(u){
  return {kb:u.folders.reduce((n, f) => n + f.size_kb, 0), count:u.folders.reduce((n, f) => n + f.count, 0)};
}
function usageLine(u){
  const a = ACCT[u.account]; if (!a) return '';
  const {kb, count} = usageTotals(u);
  let server = `<span class="sdim">${esc(a.id === LOCAL ? t('no server') : '–')}</span>`;
  if (u.quota){
    const pct = pctOf(u.quota);
    server = `${bar(pct, levelOf(pct))}<span class="uval">${esc(fmtSize(u.quota.used_kb))} / ${esc(fmtSize(u.quota.limit_kb))}</span>`;
  }
  const check = u.sync_error ? `<span class="ustat warn" title="${esc(u.sync_error)}">${esc(t('failed'))}</span>`
    : u.synced_at ? `<span class="ustat">${esc(fmtWhen(parseDate(u.synced_at)))}</span>` : '<span class="ustat">–</span>';
  return `<button type="button" class="uacct" data-pick="${esc(a.id)}" aria-pressed="${a.id === usagePick}" title="${esc(tn(count, '{n} message', '{n} messages', {n:num(count)}))}">`
    + `<span class="uname">${adot(a.id)}<span>${esc(acctLabel(a.id))}</span></span><span class="userver">${server}</span>`
    + `<span class="uval">${esc(fmtSize(kb))}</span>${check}</button>`;
}
function usageDetail(u, since){
  const a = ACCT[u.account]; if (!a) return '';
  const {kb, count} = usageTotals(u);
  let html = '';
  if (a.id === LOCAL) html += `<p class="unote">${esc(t('This mail is only on this PC. Back it up now and then: Settings › Backups.'))}</p>`;
  if (u.sync_error) html += `<p class="unote warn">${esc(u.sync_error)}</p>`;
  if (u.quota && pctOf(u.quota) >= 80) html += `<p class="unote ${levelOf(pctOf(u.quota))}">${esc(t('The mailbox is nearly full. Once it is, new mail bounces back to its senders: empty Trash and Spam, or move old mail out.'))}</p>`;
  // This PC, by kind of folder: one stacked bar and its legend.
  const groups = USE_GROUPS.map(([g, label, c]) => {
    const fs = u.folders.filter(f => groupOf(f.kind) === g);
    return {label, c, kb:fs.reduce((n, f) => n + f.size_kb, 0), n:fs.reduce((n, f) => n + f.count, 0)};
  }).filter(x => x.n);
  html += `<div class="ublock"><div class="uline"><span class="ulabel">${esc(t('On this PC'))}</span><span class="uval">${esc(fmtSize(kb))} · ${esc(tn(count, '{n} message', '{n} messages', {n:num(count)}))}</span></div>`
    + (groups.length ? `<div class="ustack" aria-hidden="true">${groups.map(x => `<i class="u${x.c}" style="flex-grow:${Math.max(x.kb, kb / 150, 1)}"></i>`).join('')}</div>`
    + `<ul class="ulegend">${groups.map(x => `<li><i class="u${x.c}" aria-hidden="true"></i>${esc(t(x.label))} <span>${esc(fmtSize(x.kb))}</span></li>`).join('')}</ul>` : '') + '</div>';
  // Mail in and out, day by day (the folders on this PC get none from a server).
  const sum = k => u.flow.reduce((n, d) => n + d[k], 0);
  if (a.id !== LOCAL){
    const today = new Date(since.getFullYear(), since.getMonth(), since.getDate() + u.flow.length - 1);
    html += `<div class="ublock"><div class="uline"><span class="ulabel">${esc(t('Last 30 days'))}</span><span class="uval"><i class="ukey in" aria-hidden="true"></i>${esc(tn(sum('received'), '{n} received', '{n} received'))}`
      + ` · <i class="ukey out" aria-hidden="true"></i>${esc(tn(sum('sent'), '{n} sent', '{n} sent'))}${sum('spam') ? ` · <i class="ukey junk" aria-hidden="true"></i>${esc(tn(sum('spam'), '{n} in Spam', '{n} in Spam'))}` : ''}</span></div>`
      + flowSvg(u.flow, since) + `<div class="uaxis"><span>${dm(since)}</span><span>${esc(t('today'))} ${dm(today)}</span></div></div>`;
  }
  // Who sends the most, and the largest messages, side by side: each a way to tidy up.
  const most = Math.max(1, ...u.senders.map(s => s.count));
  const senders = u.senders.length ? `<div class="ublock"><span class="ulabel">${esc(t('Most mail from'))}</span><ul class="ulist">${u.senders.map(s =>
    `<li><button type="button" class="ulink" data-sender="${esc(s.email)}" title="${esc(t('Show all mail from {email}', {email:s.email}))}"><span class="who">${esc(s.name || s.email)}</span>`
    + `<span class="spark" aria-hidden="true"><i style="width:${s.count / most * 100}%"></i></span><span class="n">${num(s.count)}</span></button></li>`).join('')}</ul></div>` : '';
  const largest = u.largest.length ? `<div class="ublock"><span class="ulabel">${esc(t('Largest messages'))}</span><ul class="ulist">${u.largest.map(m =>
    `<li><button type="button" class="ulink" data-open="${m.id}" data-folder="${esc(m.folder)}" title="${esc(t('Open it in {folder}', {folder:folderName(m.folder)}))} · ${esc(isOut(m.folder) ? nameOf(m.to) : m.from_name)}"><span class="who">${esc(m.subject)}</span>`
    + `<span class="n">${esc(fmtSize(m.size_kb))}</span></button></li>`).join('')}</ul></div>` : '';
  if (senders || largest) html += `<div class="ucols">${senders}${largest}</div>`;
  // What still waits.
  const outbox = u.folders.find(f => f.kind === 'outbox')?.count ?? 0, waits = [];
  if (u.queued) waits.push(tn(u.queued, '{n} change waits for the server', '{n} changes wait for the server'));
  if (outbox) waits.push(tn(outbox, '{n} message waits in the Outbox', '{n} messages wait in the Outbox'));
  if (waits.length) html += `<p class="unote">${esc(waits.join(' · '))}.</p>`;
  return html;
}
let usageSeq = 0;
async function renderUsage(){
  const box = $('u-usage'), seq = ++usageSeq;
  if (!USAGE) box.innerHTML = `<p class="dnote">${esc(t('Counting…'))}</p>`;
  try { USAGE = await invoke('usage'); } catch (e){ box.innerHTML = `<p class="derr">${esc(tr(e))}</p>`; return; }
  if (seq !== usageSeq) return;
  renderMeter(); renderTree();
  if (!ACCOUNTS.length){ box.innerHTML = `<p class="dnote">${esc(t('No accounts yet. Add one to start: Telex finds the servers itself.'))}</p>`; return; }
  paintUsage();
}
function paintUsage(){
  const box = $('u-usage'), list = USAGE.accounts.filter(u => ACCT[u.account]);
  // The account open in the window, until another is picked here.
  if (!list.some(u => u.account === usagePick)) usagePick = list.find(u => u.account === acctId(state.folder))?.account ?? list[0]?.account;
  const since = parseDate(USAGE.since + 'T00:00:00'), picked = list.find(u => u.account === usagePick);
  box.innerHTML = `<div class="ubase"><span>${esc(t('Mail base on this PC: {size}', {size:fmtBytes(USAGE.base_bytes)}))}</span>`
    + (USAGE.free_bytes > 1048576 ? `<span class="sdim" title="${esc(t('Space inside the file that new mail fills first. Settings › Backups › Compact gives it back to the disk.'))}">${esc(t('{size} unused inside', {size:fmtBytes(USAGE.free_bytes)}))}</span>` : '') + '</div>'
    + `<div class="uaccts" role="group" aria-label="${esc(t('Accounts'))}"><div class="uacct uhead" aria-hidden="true"><span>${esc(t('Account'))}</span><span>${esc(t('On the server'))}</span><span class="uval">${esc(t('On this PC'))}</span><span class="ustat">${esc(t('Last check'))}</span></div>`
    + list.map(usageLine).join('') + '</div>'
    + (picked ? `<section class="udetail" aria-label="${esc(acctLabel(picked.account))}">${usageDetail(picked, since)}</section>` : '')
    + `<p class="dnote">${esc(t('Counts cover the mail downloaded to this PC.'))}</p>`;
}
const fmtBytes = b => fmtSize(Math.ceil(b / 1024));
$('u-usage').addEventListener('click', e => {
  const p = e.target.closest('[data-pick]'), s = e.target.closest('[data-sender]'), o = e.target.closest('[data-open]');
  if (p){ usagePick = p.dataset.pick; paintUsage(); $('u-usage').querySelector(`[data-pick="${CSS.escape(usagePick)}"]`)?.focus(); }
  if (s){ sdlg.close(); filterAllMail(s.dataset.sender); }
  if (o){ sdlg.close(); openMessage(Number(o.dataset.open), o.dataset.folder); }
});

/* ---------- connections ---------- */
// Settings › Connections: every server Telex reached since it started (its places in use, a
// rest after it stopped answering) and the last connections, each step timed, where one failed
// and why (servers.rs, in memory only). Copy details puts what is shown as text, for whoever helps.
const conn = {data:null, error:'', seq:0, account:'', problems:false, timer:0, painted:''};
const CONN_ROWS = 300;
const purposeText = p => ({check:t('Check|connection'), push:t('Changes|connection'), older:t('Older mail|connection'), test:t('Settings test'), watch:t('Watching the Inbox'), send:t('Sending|connection')}[p] ?? p);
const stepText = (s, purpose) => s === 'work'
  ? ({check:t('checking'), push:t('sending changes'), older:t('downloading'), test:t('folder list'), watch:t('watching'), send:t('sending')}[purpose] ?? purpose)
  : ({lookup:t('address|step'), wait:t('waiting|step'), connect:t('connecting|step'), starttls:'STARTTLS', tls:'TLS', greeting:t('greeting|step'), login:t('login|step')}[s] ?? s);
const fmtMs = ms => ms < 1000 ? t('{n} ms', {n:num(ms)}) : t('{n} s|duration', {n:(ms / 1000).toLocaleString(I18n.lang() === 'pl' ? 'pl-PL' : 'en', {maximumFractionDigits:1})});
const hms = d => hm(d) + ':' + pad(d.getSeconds());
const connWhen = text => { const d = parseDate(text); return d.toDateString() === new Date().toDateString() ? hms(d) : dmy(d) + ' ' + hms(d); };
// The account a connection was for: the one that logs in with that user name, on that server
// when two do.
function connAccount(a){
  const user = a.user.toLowerCase(), host = a.host.toLowerCase();
  const mine = servers().filter(x => x.imap?.username.toLowerCase() === user);
  return (mine.find(x => [x.imap.host, x.smtp?.host].some(h => h?.toLowerCase() === host)) ?? mine[0])?.id ?? '';
}
// The names of an account's servers, to show only those with it.
const connNames = id => { const a = ACCT[id]; return a ? [a.imap?.host, a.smtp?.host].filter(Boolean).map(h => h.toLowerCase()) : []; };
function showConnections(account, problems){
  conn.account = account || ''; conn.problems = !!problems;
  if (sdlg.open) showTab('conn', true); else openSettings('conn');
}
function renderConnections(){
  const el = $('s-conn'), real = servers();
  if (conn.account && !ACCT[conn.account]) conn.account = '';
  el.innerHTML = `<div class="cnbar">`
    + (real.length > 1 ? `<label class="cnlabel"><span>${esc(t('Account'))}</span><select class="sselect" id="c-acct"><option value="">${esc(t('All accounts'))}</option>${real.map(a => `<option value="${esc(a.id)}"${a.id === conn.account ? ' selected' : ''}>${esc(a.email)}</option>`).join('')}</select></label>` : '')
    + `<label class="sopt"><input type="checkbox" id="c-bad"${conn.problems ? ' checked' : ''}><span>${esc(t('Only problems'))}</span></label>`
    + `<span class="sgap"></span><button type="button" class="btn" id="c-copy">${esc(t('Copy details'))}</button></div>`
    + `<div id="c-body"></div>`
    + `<p class="dnote">${esc(t('Kept in memory since Telex started, the last {n} connections; nothing of it is saved or sent. Copy details gives it as text for whoever helps you.', {n:num(1000)}))}</p>`;
  conn.painted = '';
  paintConnections();
  loadConnections();
  // Fresh while the tab is open.
  clearInterval(conn.timer);
  conn.timer = setInterval(() => {
    if (sdlg.open && setTab === 'conn') loadConnections();
    else { clearInterval(conn.timer); conn.timer = 0; }
  }, 3000);
}
async function loadConnections(){
  const seq = ++conn.seq;
  try { conn.data = await invoke('connections'); conn.error = ''; } catch (e){ conn.error = tr(e); }
  if (seq === conn.seq && sdlg.open && setTab === 'conn') paintConnections();
}
// What the tab shows with its filters: servers and connections.
function connShown(){
  const d = conn.data, names = conn.account ? connNames(conn.account) : null;
  return {
    servers: d.servers.filter(s => !names || s.names.some(n => names.includes(n))),
    log: d.log.filter(a => (!conn.account || connAccount(a) === conn.account) && (!conn.problems || a.error)),
  };
}
function paintConnections(){
  const body = $('c-body');
  if (!body) return;
  if (!conn.data){
    body.innerHTML = conn.error ? `<p class="derr">${esc(conn.error)}</p>` : `<p class="dnote">${esc(t('Counting…'))}</p>`;
    return;
  }
  // Every three seconds: drawn again only when something changed, so the focus stays.
  const key = JSON.stringify([conn.data, conn.account, conn.problems, I18n.lang()]);
  if (key === conn.painted) return;
  conn.painted = key;
  const {servers:srv, log} = connShown(), more = log.length - CONN_ROWS;
  body.innerHTML = `<div class="slabel">${esc(t('Servers'))}</div>`
    + (srv.length ? `<ul class="cnlist">${srv.map(connServer).join('')}</ul>` : `<p class="dnote">${esc(t('No connection yet since Telex started.'))}</p>`)
    + `<div class="slabel">${esc(t('Last connections'))}</div>`
    + (log.length ? `<ul class="cnlist">${log.slice(0, CONN_ROWS).map(connRow).join('')}</ul>`
      : `<p class="dnote">${esc(conn.problems ? t('No problems among the last connections.') : t('No connection yet since Telex started.'))}</p>`)
    + (more > 0 ? `<p class="dnote">${esc(tn(more, '{n} older connection not shown; Copy details has them all.', '{n} older connections not shown; Copy details has them all.', {n:num(more)}))}</p>` : '');
}
const connServerText = s => t('Checks: {n} of {max}', {n:num(s.sessions[0]), max:num(s.sessions[1])}) + ' · ' + t('Watching: {n} of {max}', {n:num(s.watches[0]), max:num(s.watches[1])});
const connRestText = s => s.resting_until ? tn(s.silences, 'Resting until {time}: it stopped answering {n} time in a row', 'Resting until {time}: it stopped answering {n} times in a row', {time:hm(parseDate(s.resting_until)), n:num(s.silences)}) : '';
function connServer(s){
  const rest = connRestText(s);
  return `<li class="cnrow${rest ? ' bad' : ''}"><div class="cnline"><b>${esc(s.names.join(' · ') || s.address)}</b><span class="cnmono">${esc(s.address)}</span></div>`
    + `<div class="cnsub">${esc(connServerText(s))}</div>`
    + (rest ? `<div class="cnerr">${esc(rest)} <button type="button" class="linkbtn" data-wake="${esc(s.address)}" title="${esc(t('The next check connects at once. If the server still doesn’t answer, it rests again, longer.'))}">${esc(t('Try now'))}</button></div>` : '')
    + '</li>';
}
const connSteps = a => a.steps.map(s => `${stepText(s.step, a.purpose)} ${fmtMs(s.ms)}${s.step === a.failed ? ' ✕' : ''}`);
const connPlace = a => `${a.protocol.toUpperCase()} ${a.host}:${a.port}${a.address && a.address !== a.host.toLowerCase() ? ' (' + a.address + ')' : ''}`;
const connState = a => a.error ? t('failed|connection') : a.ended ? t('done|connection') : t('open|connection');
function connRow(a){
  const id = connAccount(a);
  return `<li class="cnrow${a.error ? ' bad' : ''}"><div class="cnline"><span class="cnmono">${esc(connWhen(a.started))}</span><span class="cnpurp">${esc(purposeText(a.purpose))}</span>`
    + `<span class="cnwho">${id ? adot(id) : ''}${esc(id ? acctLabel(id) : a.user)}</span><span class="cnmono cnplace">${esc(connPlace(a))}</span><span class="cnstate">${esc(connState(a))}</span></div>`
    + `<div class="cnsub cnmono">${connSteps(a).map((s, i) => `<span${a.steps[i].step === a.failed ? ' class="cnfail"' : ''}>${esc(s)}</span>`).join(' · ')}</div>`
    + (a.error ? `<div class="cnerr">${esc(a.error)}</div>` : '') + '</li>';
}
// What is shown, as plain text: every connection that matches, not only the first rows.
function connText(){
  const {servers:srv, log} = connShown(), now = new Date();
  const lines = [`Telex ${upd.build?.version ?? ''} · ${dmy(now)} ${hms(now)}`];
  if (conn.account) lines.push(t('Account') + ': ' + acctLabel(conn.account));
  lines.push('', t('Servers'));
  for (const s of srv){
    lines.push(`  ${s.names.join(' · ') || s.address} (${s.address})`, `    ${connServerText(s)}`);
    if (s.resting_until) lines.push(`    ${connRestText(s)}`);
  }
  lines.push('', t('Last connections'));
  for (const a of log){
    const id = connAccount(a);
    lines.push(`  ${connWhen(a.started)}  ${purposeText(a.purpose)} · ${id ? acctLabel(id) : a.user} · ${connPlace(a)} · ${connState(a)}`, `    ${connSteps(a).join(' · ')}`);
    if (a.error) lines.push(`    ${a.error}`);
  }
  return lines.join('\n');
}
$('s-conn').addEventListener('change', e => {
  if (e.target.id === 'c-acct') conn.account = e.target.value;
  else if (e.target.id === 'c-bad') conn.problems = e.target.checked;
  else return;
  paintConnections();
});
$('s-conn').addEventListener('click', async e => {
  const wake = e.target.closest('[data-wake]');
  if (wake){
    try { await invoke('wake_server', {address:wake.dataset.wake}); } catch (err){ say(tr(err), 4000); return; }
    say(t('The next check connects at once.'), 3000);
    loadConnections();
    return;
  }
  if (e.target.closest('#c-copy')){
    if (!conn.data) return;
    if (await copyText(connText())) say(t('Connection details copied.'), 2500);
    else say(t('Couldn’t copy the connection details.'), 4000);
  }
});

/* ---------- backups ---------- */
// Settings › Backups: the mail base (where, how large, compacting it), backing it up now or by
// itself into a folder that keeps the newest few, and putting a backup back. Restoring keeps
// the mail base as it was next to it, then the window starts again on the restored mail.
const BACKUP_KEEP = [3, 5, 10, 20];
const bk = {state:null, work:'', restore:null, error:''}; // work: 'backup', 'restore' or 'compact' while it runs
const fmtWhenFull = d => d.toDateString() === new Date().toDateString() ? t('today {time}', {time:hm(d)}) : dmy(d) + ' ' + hm(d);
async function loadBackups(){
  try { bk.state = await invoke('backup_state'); bk.error = bk.work ? bk.error : ''; } catch (e){ bk.error = tr(e); }
  if (sdlg.open && setTab === 'backup') renderBackups();
  renderTree(); // the folders on this PC say when they aren't backed up
}
function renderBackups(){
  const el = $('s-backup'), s = bk.state, p = pref();
  if (!s){
    el.innerHTML = bk.error ? `<p class="derr">${esc(bk.error)}</p>` : `<p class="dnote">${esc(t('Counting…'))}</p>`;
    if (!bk.error && !bk.loading){ bk.loading = true; loadBackups().finally(() => { bk.loading = false; }); }
    return;
  }
  const newest = s.backups[0], when = b => fmtWhenFull(parseDate(b.made));
  const work = bk.work, off = work ? ' disabled' : '';
  let html = `<h3 class="rh">${esc(t('Mail base'))}</h3>`;
  html += s.base ? `<p class="bkpath"><code title="${esc(s.base)}">${esc(s.base)}</code><button type="button" class="linkbtn" id="bk-show-base">${esc(t('Show in folder'))}</button></p>`
    + `<p class="dnote">${esc(fmtBytes(s.base_bytes))}${s.free_bytes > 1048576 ? ' · ' + esc(t('{size} of it is free space left by removed mail', {size:fmtBytes(s.free_bytes)})) : ''}</p>`
    + (s.free_bytes > 1048576 ? `<div class="sbtns"><button type="button" class="btn" id="bk-compact"${off}>${esc(work === 'compact' ? t('Compacting…') : t('Compact|base'))}</button><span class="sdim">${esc(t('Gives that space back to the disk. Telex pauses while it works.'))}</span></div>` : '')
    : `<p class="dnote">${esc(t('There is no mail base yet: it is made when you add your first account.'))}</p>`;
  html += `<h3 class="rh">${esc(t('Back up'))}</h3>
<p class="dnote">${esc(t('One file with your mail, the folders on this PC, rules and identities. Passwords stay in the system’s password store.'))}</p>
<label class="sopt"><input type="checkbox" id="bk-secret"${p.backup_secret_keys !== false ? ' checked' : ''}><span>${esc(t('Include the secret parts of my OpenPGP keys (locked with their passphrases)'))}</span></label>
<p class="dnote">${esc(t('Without them a backup can still check signatures, but mail encrypted to you can be read only with your keys backed up some other way (Settings › Keys › Back up secret key).'))}</p>
<div class="sbtns"><button type="button" class="btn primary" id="bk-now"${off || !s.base ? ' disabled' : ''}>${esc(work === 'backup' || s.running ? t('Backing up…') : t('Back up now'))}</button>
<span class="sdone">${newest ? esc(t('Last backup {when}, {size}.', {when:when(newest), size:fmtBytes(newest.bytes)})) : esc(t('No backups yet.'))}</span></div>
<div class="sgrid"><span class="lab" id="bk-every">${esc(t('Automatically'))}</span><div class="segs" role="radiogroup" aria-labelledby="bk-every">${segs('backup_every', [['off', esc(t('Never'))], ['daily', esc(t('Every day'))], ['weekly', esc(t('Every week'))]], p.backup_every)}</div>
<label class="lab" for="s-keep">${esc(t('Keep'))}</label><div><select id="s-keep" class="sselect">${BACKUP_KEEP.map(n => `<option value="${n}"${n === p.backup_keep ? ' selected' : ''}>${esc(tn(n, 'the newest {n} backup', 'the newest {n} backups'))}</option>`).join('')}</select></div>
<span class="lab">${esc(t('Folder'))}</span><div class="bkdir"><code title="${esc(s.dir)}">${esc(s.dir)}</code>${s.suggested ? `<small class="sdim">${esc(t('suggested'))}</small>` : ''}
<button type="button" class="btn" id="bk-pick">${esc(t('Choose…'))}</button>${newest ? `<button type="button" class="linkbtn" id="bk-show">${esc(t('Show'))}</button>` : ''}</div></div>
<p class="dnote">${esc(t('A folder on another disk, a USB stick or OneDrive keeps the mail safe if this disk fails. Telex removes only its own older backups there.'))}</p>`;
  html += `<h3 class="rh">${esc(t('Restore'))}</h3>
<p class="dnote">${esc(t('Restoring puts a backup in place of the mail base. The mail base as it is now is kept next to it, so nothing is lost.'))}</p>`;
  if (s.backups.length) html += `<ul class="bklist">${s.backups.map(b => `<li><span class="bkwhen">${esc(when(b))}</span><span class="bksize">${esc(fmtBytes(b.bytes))}</span><button type="button" class="btn" data-restore="${esc(b.path)}"${off}>${esc(t('Restore…'))}</button></li>`).join('')}</ul>`;
  html += `<div class="sbtns"><button type="button" class="btn" id="bk-file"${off}>${esc(t('Restore from a file…'))}</button></div>`;
  const r = bk.restore;
  if (r){
    const what = tn(r.messages, '{n} message', '{n} messages', {n:num(r.messages)}) + (r.on_this_pc ? ` (${tn(r.on_this_pc, '{n} on this PC', '{n} on this PC', {n:num(r.on_this_pc)})})` : '');
    html += `<div class="bkconfirm" role="group" aria-labelledby="bk-q"><b id="bk-q">${esc(r.made ? t('Restore the backup from {when}?', {when:fmtWhenFull(parseDate(r.made))}) : t('Restore this backup?'))}</b>
<p>${esc(t('It holds {what}.', {what}))}${r.accounts.length ? ' ' + esc(t('Accounts: {accounts}.', {accounts:r.accounts.join(', ')})) : ''}</p>
<ul><li>${esc(t('The mail base as it is now is kept next to it, as telex-before-restore-….db.'))}</li>
<li>${esc(t('Changes not yet sent to the servers are dropped: at the next check, what the servers have wins, and newer mail comes down again.'))}</li>
<li>${esc(t('Mail waiting in an Outbox moves to Drafts, since it may have gone out after the backup.'))}</li>
<li>${esc(t('Mail kept only on this PC after the backup was made isn’t in it.'))}</li></ul>
<div class="sbtns"><button type="button" class="btn primary" id="bk-go"${off}>${esc(work === 'restore' ? t('Restoring…') : t('Restore and start again'))}</button><button type="button" class="btn" id="bk-cancel"${off}>${esc(t('Cancel'))}</button></div></div>`;
  }
  html += `<p class="derr" role="alert">${esc(bk.error)}</p>`;
  el.innerHTML = html;
}
async function backupWork(kind, run){
  bk.work = kind; bk.error = ''; renderBackups();
  try { await run(); } catch (e){ bk.error = tr(e); }
  bk.work = '';
  await loadBackups();
}
$('s-backup').addEventListener('click', async e => {
  const b = e.target.closest('button');
  if (!b || b.disabled) return;
  if (b.id === 'bk-now') backupWork('backup', async () => {
    const made = await invoke('backup_now');
    say(t('Backed up the mail base to {path}.', {path:made.path}), 8000);
  });
  else if (b.id === 'bk-pick'){
    let dir;
    try { dir = await invoke('pick_backup_dir', {words:{title:t('Folder for backups')}}); } catch (err){ bk.error = tr(err); renderBackups(); return; }
    if (dir && await changeSettings({backup_dir:dir})) loadBackups();
  }
  else if (b.id === 'bk-show' || b.id === 'bk-show-base') invoke('show_backup_place', {base:b.id === 'bk-show-base'}).catch(err => say(tr(err), 8000));
  else if (b.id === 'bk-compact') backupWork('compact', async () => {
    const [before, after] = await invoke('compact_base');
    say(t('Compacted the mail base: {before} to {after}.', {before:fmtBytes(before), after:fmtBytes(after)}), 8000);
  });
  else if (b.dataset.restore || b.id === 'bk-file'){
    bk.error = '';
    try { bk.restore = await invoke('inspect_backup', {path:b.dataset.restore ?? null, words:{title:t('Restore from a backup'), filter:t('Telex backups')}}) ?? bk.restore; }
    catch (err){ bk.error = tr(err); bk.restore = null; }
    renderBackups();
    $('bk-go')?.focus();
  }
  else if (b.id === 'bk-cancel'){ bk.restore = null; renderBackups(); }
  else if (b.id === 'bk-go'){
    const r = bk.restore;
    backupWork('restore', async () => {
      const aside = await invoke('restore_backup', {path:r.path});
      // The window starts again on the restored mail, and says so then.
      try { sessionStorage.setItem('telex-restored', JSON.stringify({made:r.made, aside})); } catch {}
      location.reload();
    });
  }
});
$('s-backup').addEventListener('change', e => {
  if (e.target.id === 's-keep') changeSettings({backup_keep:Number(e.target.value)});
  if (e.target.id === 'bk-secret') changeSettings({backup_secret_keys:e.target.checked});
});
// Backups by themselves: a minute after start, then every hour, when the settings say one is due.
let backupWarned = false;
async function autoBackup(){
  if (pref().backup_every === 'off' || !ACCOUNTS.length) return;
  let made;
  try { made = await invoke('auto_backup'); }
  catch (e){ if (!backupWarned){ backupWarned = true; say(t('The automatic backup didn’t work: {why}', {why:tr(e)}), 15000); } return; }
  if (!made) return;
  backupWarned = false;
  say(t('Backed up the mail base ({size}).', {size:fmtBytes(made.bytes)}), 5000);
  bk.state = null;
  if (sdlg.open && setTab === 'backup') loadBackups();
}
// After a restore the window starts again: say what happened.
function sayRestored(){
  let r = null;
  try { r = JSON.parse(sessionStorage.getItem('telex-restored') || 'null'); sessionStorage.removeItem('telex-restored'); } catch {}
  if (!r) return;
  const file = String(r.aside).split(/[\\/]/).pop();
  say(t('Restored the backup{when}. The mail base as it was is kept as {file}.', {when:r.made ? ' ' + t('from {when}', {when:fmtWhenFull(parseDate(r.made))}) : '', file}), 15000);
}

/* ---------- OpenPGP: signed and encrypted mail, keys ---------- */
// The core reads, decrypts and checks (openpgp.rs); the window shows what it found above the
// message, asks for passphrases, offers Sign and Encrypt in compose and lists the keys.
const PGP_I = {
  seal: sv('<path d="M8 1.6l1.5 1.1 1.9-.1.6 1.8 1.5 1.1-.6 1.8.6 1.8-1.5 1.1-.6 1.8-1.9-.1L8 14.4l-1.5-1.1-1.9.1-.6-1.8-1.5-1.1.6-1.8-.6-1.8 1.5-1.1.6-1.8 1.9.1z"/><path d="M5.9 8l1.4 1.4 2.8-2.8"/>'),
  lock: sv('<rect x="3" y="7" width="10" height="7" rx="1.5"/><path d="M5.5 7V5a2.5 2.5 0 015 0v2"/>'),
  key: sv('<circle cx="5" cy="10.5" r="2.5"/><path d="M6.8 8.7L13 2.5M11 4.5l1.6 1.6M9.4 6.1L11 7.7"/>'),
};
// "ABCD 1234 …": a fingerprint in groups of four, as people read them out to each other.
const fpText = fp => String(fp || '').replace(/(.{4})(?=.)/g, '$1 ');
const keyName = k => k?.name || fpText(k?.fingerprint).slice(-19);
// Whatever was shown may read differently after a key or passphrase changed.
function pgpChanged(){ bodies.clear(); htmlViews.clear(); }
function pgpHtml(m){
  const p = m.pgp;
  if (!p) return '';
  // S/MIME says "certificate" and "password" where OpenPGP says "key" and "passphrase".
  const smime = !!p.smime, scheme = smime ? ' data-scheme="smime"' : '';
  const lines = [];
  if (p.encrypted){
    if (p.decrypted) lines.push(['good', PGP_I.lock, esc(t('Encrypted. Decrypted on this PC only: its text isn’t kept or searched.'))]);
    else if (p.problem === 'locked'){
      const k = p.locked[0];
      const intro = smime ? t('Encrypted to your certificate {name}.', {name:keyName(k)}) : t('Encrypted to your key {name}.', {name:keyName(k)});
      const word = smime ? t('Password') : t('Passphrase');
      const label = smime ? t('Password for {name}', {name:keyName(k)}) : t('Passphrase for {name}', {name:keyName(k)});
      lines.push(['warn', PGP_I.lock, `${esc(intro)}
<input type="password" class="pgppass" data-fp="${esc(k.fingerprint)}"${scheme} autocomplete="off" spellcheck="false" placeholder="${esc(word)}" aria-label="${esc(label)}">
<label class="pgprem"><input type="checkbox" class="pgpremember"> ${esc(t('Remember it on this PC'))}</label>
<button type="button" class="btn" data-pgp="unlock">${esc(t('Unlock'))}</button><span class="pgperr" role="alert"></span>`]);
    } else {
      const why = {
        not_for_you: smime ? t('Encrypted to someone else’s certificate: this PC has no certificate that can read it.') : t('Encrypted to someone else’s key: this PC has no key that can read it.'),
        failed: t('Encrypted, but it can’t be decrypted: it is damaged or was changed on the way.'),
        too_large: t('Encrypted, and too large to decrypt here.'),
        unsupported: t('Encrypted in an old or rare format Telex doesn’t read.'),
      }[p.problem] ?? t('Encrypted.');
      lines.push(['bad', PGP_I.lock, esc(why)]);
    }
  }
  const s = p.signature;
  if (s){
    const name = keyName(s.key), fp = s.key ? ` title="${esc(fpText(s.key.fingerprint))}"` : '', issuer = s.issuer || '?';
    const part = p.partial ? ' ' + esc(t('Only part of the message is signed: what is outside it may have been added.')) : '';
    const confirm = s.key ? `<code class="pgpfp">${esc(fpText(s.key.fingerprint))}</code><button type="button" class="btn" data-pgp="confirm" data-fp="${esc(s.key.fingerprint)}"${scheme}>${esc(smime ? t('Confirm certificate') : t('Confirm key'))}</button>` : '';
    const line = smime ? {
      good: ['good', `<span${fp}>${esc(t('Signed by {name}, certified by {issuer}.', {name, issuer}))}</span>${part}`],
      unconfirmed: ['info', `${esc(t('Signed by {name}, with a certificate this PC can’t confirm (from {issuer}). Compare its fingerprint with them, then confirm it:', {name, issuer}))} ${confirm}${part}`],
      other_sender: ['bad', `<span${fp}>${esc(t('Signed by {name}, not by the sender: the certificate belongs to another address.', {name}))}</span>`],
      no_key: ['info', esc(t('Signed with a certificate the message doesn’t carry (serial {id}).', {id:s.key_id}))],
      bad: ['bad', esc(t('The signature doesn’t match: the message was changed after it was signed, or the signature is broken.'))],
      revoked: ['bad', `<span${fp}>${esc(t('Signed with a certificate that was revoked ({name}).', {name}))}</span>`],
      expired: ['warn', `<span${fp}>${esc(t('Signed with a certificate that wasn’t valid at the time ({name}).', {name}))}</span>`],
      unsupported: ['info', esc(t('Signed in a way Telex can’t check.'))],
    }[s.status] : {
      good: ['good', `<span${fp}>${esc(t('Signed by {name}.', {name}))}</span>${part}`],
      unconfirmed: ['info', `${esc(t('Signed by {name}, with a key that came with their mail. Compare its fingerprint with them, then confirm it:', {name}))} ${confirm}${part}`],
      other_sender: ['bad', `<span${fp}>${esc(t('Signed by {name}, not by the sender: the key belongs to another address.', {name}))}</span>`],
      no_key: ['info', `${esc(t('Signed with a key Telex doesn’t have ({id}).', {id:fpText(s.key_id)}))} <button type="button" class="linkbtn" data-pgp="keys">${esc(t('Keys…'))}</button>`],
      bad: ['bad', esc(t('The signature doesn’t match: the message was changed after it was signed, or the signature is broken.'))],
      revoked: ['bad', `<span${fp}>${esc(t('Signed with a key its owner has revoked ({name}).', {name}))}</span>`],
      expired: ['warn', `<span${fp}>${esc(t('Signed with a key that had expired ({name}).', {name}))}</span>`],
      unsupported: ['info', esc(t('Signed in a way Telex can’t check.'))],
    }[s.status];
    if (line) lines.push([line[0], PGP_I.seal, line[1]]);
  }
  if (p.offered?.length){
    const names = p.offered.map(keyName).join(', ');
    lines.push(['quiet', PGP_I.key, `${esc(tn(p.offered.length, 'This message carries a key: {names}.', 'This message carries {n} keys: {names}.', {names}))} <button type="button" class="linkbtn" data-pgp="import">${esc(t('Import|key'))}</button>`]);
  }
  return lines.map(([kind, icon, html]) => `<div class="bar pgp ${kind}">${icon}<span class="pgpline">${html}</span></div>`).join('');
}
// A passphrase (OpenPGP) or a certificate's password (S/MIME): true when it opened the key.
async function pgpUnlock(fp, passphrase, remember, smime = false){
  try {
    return smime ? await invoke('smime_unlock', {fingerprint:fp, password:passphrase, remember})
      : await invoke('pgp_unlock', {fingerprint:fp, passphrase, remember});
  } catch (e){ say(tr(e), 8000); return false; }
}
previewEl.addEventListener('click', async e => {
  const b = e.target.closest('[data-pgp]');
  if (!b) return;
  if (state.compose){ pgpToggle(b.dataset.pgp); return; }
  const m = current();
  if (!m) return;
  const kind = b.dataset.pgp;
  if (kind === 'unlock'){
    const field = previewEl.querySelector('.pgppass'), err = previewEl.querySelector('.pgperr');
    if (!field?.value) { field?.focus(); return; }
    const smime = field.dataset.scheme === 'smime';
    b.disabled = true;
    const ok = await pgpUnlock(field.dataset.fp, field.value, !!previewEl.querySelector('.pgpremember')?.checked, smime);
    b.disabled = false;
    if (!ok){ if (err) err.textContent = smime ? t('That password doesn’t open the certificate.') : t('That passphrase doesn’t open the key.'); field.select(); return; }
    pgpChanged(); renderPreview();
  } else if (kind === 'confirm'){
    const smime = b.dataset.scheme === 'smime';
    try { await invoke(smime ? 'smime_verify' : 'pgp_verify', {fingerprint:b.dataset.fp, verified:true}); } catch (err){ say(tr(err), 8000); return; }
    pgpChanged(); renderPreview();
    say(smime ? t('Certificate confirmed. Its signatures count as the sender’s from now on.') : t('Key confirmed. Its signatures count as the sender’s from now on.'), 5000);
  } else if (kind === 'import'){
    let r;
    try { r = await invoke('pgp_import_offered', {id:m.id}); } catch (err){ say(tr(err), 8000); return; }
    pgpChanged(); renderPreview(); say(importedText(r), 6000);
  } else if (kind === 'keys') openSettings('keys');
});
previewEl.addEventListener('keydown', e => {
  if (e.key === 'Enter' && e.target.classList?.contains('pgppass')){ e.preventDefault(); previewEl.querySelector('[data-pgp="unlock"]')?.click(); }
});
const importedText = r => {
  const n = r.added.length + r.updated.length;
  if (!n) return r.unchanged.length ? t('Telex had that key already.') : t('Nothing was imported.');
  return tn(n, 'Imported {names}.', 'Imported {n} keys: {names}.', {names:[...r.added, ...r.updated].map(keyName).join(', ')});
};

/* compose: Sign, Encrypt, My key, and OpenPGP or S/MIME */
// What the core says about the message as typed (pgp_compose), asked again as recipients
// change. Until the user touches a switch, it starts where the core suggests: a reply to
// encrypted mail encrypted, otherwise as last chosen for these people; S/MIME when the sender
// has only a certificate, answers S/MIME mail, or only certificates reach everyone.
let pgpTimer = 0;
async function pgpComposeState(c, first = false){
  let st;
  try {
    st = await invoke('pgp_compose', {draft:draftOf($('cform') ? readCompose() : c)});
  } catch { return; }
  if (state.compose !== c) return;
  c.pgp = st;
  if (!c.pgpTouched){
    c.smime = !!st.smime && !!st.smime_own;
    c.sign = st.sign;
    c.encrypt = st.encrypt || (first && !!c.pgpWas?.encrypted && !!(c.smime ? st.smime_own : st.own));
  }
  if (c.smime && !st.smime_own) c.smime = false;
  if (!c.smime && !st.own && st.smime_own) c.smime = true;
  paintPgp();
}
// The sender's key or certificate for the scheme compose is on, and what it is missing.
function pgpNow(c){
  const st = c?.pgp;
  if (!st) return {own:null, unlocked:false, missing:[]};
  return c.smime ? {own:st.smime_own, unlocked:st.smime_unlocked, missing:st.smime_missing ?? []}
    : {own:st.own, unlocked:st.unlocked, missing:st.missing ?? []};
}
function wirePgp(){
  const c = state.compose;
  if (!c) return;
  const again = () => { clearTimeout(pgpTimer); pgpTimer = setTimeout(() => pgpComposeState(c), 450); };
  ['c-to', 'c-cc', 'c-bcc'].forEach(id => $(id)?.addEventListener('input', again));
  $('c-from')?.addEventListener('change', () => setTimeout(() => pgpComposeState(c), 0));
  pgpComposeState(c, !c.pgp);
}
function paintPgp(){
  const c = state.compose, set = previewEl.querySelector('.pgpset'), hint = $('c-pgp');
  if (!c || !set) return;
  const st = c.pgp, now = pgpNow(c);
  set.hidden = !now.own;
  const button = kind => set.querySelector(`[data-pgp="${kind}"]`);
  const press = (kind, on) => button(kind)?.setAttribute('aria-pressed', String(!!on));
  press('sign', c.sign); press('encrypt', c.encrypt); press('key', c.attachKey);
  // Both a key and a certificate: a switch between them.
  const scheme = button('scheme');
  if (scheme){
    scheme.hidden = !(st?.own && st?.smime_own);
    scheme.querySelector('span').textContent = c.smime ? 'S/MIME' : 'OpenPGP';
  }
  const key = button('key');
  if (key) key.hidden = !!c.smime;
  set.setAttribute('aria-label', c.smime ? 'S/MIME' : t('OpenPGP'));
  button('sign')?.setAttribute('title', c.smime ? t('Sign with your certificate: the recipients can check it is from you and unchanged') : t('Sign with your OpenPGP key: the recipients can check it is from you and unchanged'));
  button('encrypt')?.setAttribute('title', c.smime ? t('Encrypt to the recipients’ certificates and yours: only you and they can read it') : t('Encrypt to the recipients’ keys and yours: only you and they can read it'));
  if (!hint) return;
  const missing = c.encrypt && now.missing.length ? now.missing : [];
  hint.hidden = !missing.length;
  if (missing.length){
    const list = missing.join(', ');
    const text = c.smime
      ? tn(missing.length, 'No certificate for {list}: they can’t read encrypted mail yet. A signed message from them brings it, or import it in Settings › Keys. Or turn Encrypt off.', 'No certificates for {list}: they can’t read encrypted mail yet. A signed message from each brings it, or import them in Settings › Keys. Or turn Encrypt off.', {list})
      : tn(missing.length, 'No key for {list}: they can’t read encrypted mail yet. Import their key in Settings › Keys, or turn Encrypt off.', 'No keys for {list}: they can’t read encrypted mail yet. Import their keys in Settings › Keys, or turn Encrypt off.', {list});
    hint.innerHTML = `${PGP_I.lock}<span>${esc(text)}</span>`;
  }
}
function pgpToggle(kind){
  const c = state.compose;
  if (!c || !pgpNow(c).own) return;
  c.pgpTouched = true;
  if (kind === 'sign') c.sign = !c.sign;
  else if (kind === 'encrypt'){ c.encrypt = !c.encrypt; if (c.encrypt) c.sign = true; }
  else if (kind === 'key') c.attachKey = !c.attachKey;
  else if (kind === 'scheme' && c.pgp?.own && c.pgp?.smime_own){ c.smime = !c.smime; hideComposeAsk(); }
  paintPgp();
}
const pgpSetHtml = () => `<span class="pgpset" hidden role="group" aria-label="${esc(t('OpenPGP'))}">
<button class="btn pgptog pgpscheme" type="button" data-pgp="scheme" hidden title="${esc(t('Sign and encrypt with OpenPGP or with your S/MIME certificate'))}">${PGP_I.key}<span>OpenPGP</span></button>
<button class="btn pgptog" type="button" data-pgp="sign" aria-pressed="false" title="${esc(t('Sign with your OpenPGP key: the recipients can check it is from you and unchanged'))}">${PGP_I.seal}<span>${esc(t('Sign'))}</span></button>
<button class="btn pgptog" type="button" data-pgp="encrypt" aria-pressed="false" title="${esc(t('Encrypt to the recipients’ keys and yours: only you and they can read it'))}">${PGP_I.lock}<span>${esc(t('Encrypt'))}</span></button>
<button class="btn pgptog" type="button" data-pgp="key" aria-pressed="false" title="${esc(t('Attach your public key, so they can encrypt their replies'))}">${PGP_I.key}<span>${esc(t('My key'))}</span></button></span>`;
// What compose asks of OpenPGP or S/MIME for this message; null when the sender has neither.
const pgpChoice = c => pgpNow(c).own ? {sign:!!c.sign, encrypt:!!c.encrypt, attach_key:!c.smime && !!c.attachKey, smime:!!c.smime} : null;
// Before Send: a signature needs the key's passphrase (or the certificate's password), and
// encryption a key or certificate for everyone.
async function pgpReady(){
  const c = state.compose, box = $('c-ask'), now = pgpNow(c);
  if (!now.own) return true;
  if (c.encrypt && now.missing.length){ paintPgp(); say(c.smime ? t('Encryption needs a certificate for every recipient.') : t('Encryption needs a key for every recipient.'), 6000); return false; }
  if (!c.sign || now.unlocked) return true;
  if (box && !box.hidden && box.dataset.kind === 'pass'){
    const field = $('c-pass');
    if (!field?.value){ field?.focus(); return false; }
    const ok = await pgpUnlock(now.own.fingerprint, field.value, !!$('c-pass-rem')?.checked, !!c.smime);
    if (state.compose !== c) return false;
    if (!ok){ box.querySelector('.pgperr').textContent = c.smime ? t('That password doesn’t open the certificate.') : t('That passphrase doesn’t open the key.'); field.select(); return false; }
    if (c.smime) c.pgp.smime_unlocked = true; else c.pgp.unlocked = true;
    pgpChanged(); hideComposeAsk();
    return true;
  }
  if (!box) return false;
  box.dataset.kind = 'pass';
  const word = c.smime ? t('Password') : t('Passphrase');
  box.innerHTML = `<span>${esc(c.smime ? t('Signing needs the password of the certificate {name}.', {name:keyName(now.own)}) : t('Signing needs the passphrase of {name}.', {name:keyName(now.own)}))}</span>
<input type="password" id="c-pass" autocomplete="off" spellcheck="false" aria-label="${esc(word)}" placeholder="${esc(word)}">
<label class="pgprem"><input type="checkbox" id="c-pass-rem"> ${esc(t('Remember it on this PC'))}</label>
<button type="button" class="btn" data-act="ask-send">${esc(t('Unlock and send'))}</button><span class="pgperr"></span>`;
  box.hidden = false;
  $('c-pass').focus();
  $('c-pass').addEventListener('keydown', e => { if (e.key === 'Enter'){ e.preventDefault(); send(); } });
  return false;
}

/* settings: keys */
// certs: S/MIME certificates; p12: the name of a picked .p12 waiting for its password.
const keys_ = {list:null, certs:[], error:'', paste:false, making:false, armed:'', loading:false, p12:null, p12err:''};
async function loadKeys(){
  try { [keys_.list, keys_.certs] = await Promise.all([invoke('pgp_keys'), invoke('smime_certs')]); keys_.error = ''; }
  catch (e){ keys_.error = tr(e); }
  if (sdlg.open && setTab === 'keys') renderKeys();
}
const sourceText = k => k.secret ? (k.source === 'own' ? t('made in Telex') : t('imported with its secret part'))
  : {file:t('from a file'), autocrypt:t('from their mail (Autocrypt)'), message:t('attached to a message')}[k.source] ?? k.source;
function keyRow(k){
  const dates = [t('made {date}', {date:dmy(parseDate(k.created))}), k.expires ? t('expires {date}', {date:dmy(parseDate(k.expires))}) : ''].filter(Boolean).join(' · ');
  const chips = [
    k.revoked ? `<span class="kchip bad">${esc(t('revoked'))}</span>` : '',
    !k.secret && k.verified ? `<span class="kchip">${esc(t('confirmed'))}</span>` : '',
    !k.secret && !k.verified ? `<span class="kchip warn">${esc(t('not confirmed'))}</span>` : '',
    !k.can_encrypt && !k.revoked ? `<span class="kchip warn">${esc(t('can’t encrypt'))}</span>` : '',
    k.secret && k.protected ? `<span class="kchip">${esc(k.remembered ? t('passphrase remembered') : k.unlocked ? t('unlocked until Telex closes') : t('locked'))}</span>` : '',
    k.secret && !k.protected ? `<span class="kchip warn">${esc(t('no passphrase'))}</span>` : '',
  ].join('');
  const fp = esc(k.fingerprint), armed = keys_.armed === k.fingerprint;
  const btns = [
    `<button type="button" class="linkbtn" data-key="export" data-fp="${fp}">${esc(t('Export public key…'))}</button>`,
    `<button type="button" class="linkbtn" data-key="copy" data-fp="${fp}">${esc(t('Copy public key'))}</button>`,
    k.secret ? `<button type="button" class="linkbtn" data-key="secret" data-fp="${fp}">${esc(t('Back up secret key…'))}</button>` : '',
    k.secret && (k.remembered || (k.protected && k.unlocked)) ? `<button type="button" class="linkbtn" data-key="forget" data-fp="${fp}">${esc(t('Forget passphrase'))}</button>` : '',
    !k.secret ? `<button type="button" class="linkbtn" data-key="verify" data-fp="${fp}" data-on="${!k.verified}">${esc(k.verified ? t('Take back confirmation') : t('Confirm key'))}</button>` : '',
    `<button type="button" class="linkbtn${armed ? ' danger' : ''}" data-key="delete" data-fp="${fp}">${esc(armed ? (k.secret ? t('Delete for good? Mail encrypted to it can’t be read then') : t('Delete for good?')) : t('Delete…'))}</button>`,
  ].join('');
  return `<li class="krow"><div class="khead"><b>${esc(k.user_ids[0] || k.emails[0] || '')}</b>${chips}</div>
${k.user_ids.length > 1 ? `<div class="kuids">${k.user_ids.slice(1).map(esc).join('<br>')}</div>` : ''}
<code class="kfp" title="${esc(t('Fingerprint'))}">${esc(fpText(k.fingerprint))}</code>
<div class="kmeta">${esc([k.algorithm, dates, sourceText(k)].filter(Boolean).join(' · '))}</div>
<div class="kbtns">${btns}</div></li>`;
}
// An S/MIME certificate on the keys page.
function certRow(c){
  const chips = [
    c.expired ? `<span class="kchip bad">${esc(t('expired'))}</span>` : '',
    !c.secret && c.trusted ? `<span class="kchip">${esc(t('from a trusted authority'))}</span>` : '',
    !c.secret && !c.trusted && c.verified ? `<span class="kchip">${esc(t('confirmed'))}</span>` : '',
    !c.secret && !c.trusted && !c.verified ? `<span class="kchip warn">${esc(t('not confirmed'))}</span>` : '',
    !c.can_encrypt && !c.expired ? `<span class="kchip warn">${esc(t('can’t encrypt'))}</span>` : '',
    c.secret && c.protected ? `<span class="kchip">${esc(c.remembered ? t('password remembered') : c.unlocked ? t('unlocked until Telex closes') : t('locked'))}</span>` : '',
    c.secret && !c.protected ? `<span class="kchip warn">${esc(t('no password'))}</span>` : '',
  ].join('');
  const fp = esc(c.fingerprint), armed = keys_.armed === c.fingerprint;
  const btns = [
    `<button type="button" class="linkbtn" data-cert="export" data-fp="${fp}">${esc(t('Export certificate…'))}</button>`,
    c.secret && (c.remembered || (c.protected && c.unlocked)) ? `<button type="button" class="linkbtn" data-cert="forget" data-fp="${fp}">${esc(t('Forget password'))}</button>` : '',
    !c.secret && !c.trusted ? `<button type="button" class="linkbtn" data-cert="verify" data-fp="${fp}" data-on="${!c.verified}">${esc(c.verified ? t('Take back confirmation') : t('Confirm certificate'))}</button>` : '',
    `<button type="button" class="linkbtn${armed ? ' danger' : ''}" data-cert="delete" data-fp="${fp}">${esc(armed ? (c.secret ? t('Delete for good? Mail encrypted to it can’t be read then') : t('Delete for good?')) : t('Delete…'))}</button>`,
  ].join('');
  const source = c.secret ? t('imported with its private key') : {file:t('from a file'), message:t('from their signed mail')}[c.source] ?? c.source;
  const meta = [c.algorithm, t('issued by {issuer}', {issuer:c.issuer}), t('valid {from} to {to}', {from:dmy(parseDate(c.not_before)), to:dmy(parseDate(c.not_after))}), source];
  return `<li class="krow"><div class="khead"><b>${esc(c.name)}</b>${chips}</div>
<code class="kfp" title="${esc(t('Fingerprint (SHA-256)'))}">${esc(fpText(c.fingerprint))}</code>
<div class="kmeta">${esc(meta.filter(Boolean).join(' · '))}</div>
<div class="kbtns">${btns}</div></li>`;
}
function certsHtml(){
  const own = keys_.certs.filter(c => c.secret), others = keys_.certs.filter(c => !c.secret);
  let html = `<h3 class="rh">${esc(t('Certificates (S/MIME)'))}</h3>`;
  html += own.length ? `<ul class="klist">${own.map(certRow).join('')}</ul>`
    : `<p class="dnote">${esc(t('With a certificate of your own (a .p12 or .pfx file from your company or a certificate authority) you sign mail and read mail encrypted to you, as Outlook and Apple Mail do.'))}</p>`;
  if (others.length) html += `<ul class="klist">${others.map(certRow).join('')}</ul>`;
  html += `<p class="dnote">${esc(t('Other people’s certificates come with their signed mail. Windows says which authorities it trusts; others you confirm by comparing the fingerprint.'))}</p>`;
  html += `<div class="sbtns"><button type="button" class="btn" data-cert="pick">${esc(t('Import certificate…'))}</button></div>`;
  if (keys_.p12) html += `<div class="kpaste"><div class="sgrid"><label class="lab" for="k-p12">${esc(t('Password'))}</label><div><input type="password" id="k-p12" class="sfield" autocomplete="off" spellcheck="false" aria-describedby="k-p12-note"></div></div>
<p class="dnote" id="k-p12-note">${esc(t('The password of {file}. Telex keeps the key locked with it.', {file:keys_.p12}))}</p>
<label class="sopt"><input type="checkbox" id="k-p12-rem" checked><span>${esc(t('Remember the password on this PC (the system’s password store)'))}</span></label>
<div class="sbtns"><button type="button" class="btn primary" data-cert="p12-go">${esc(t('Import'))}</button><button type="button" class="btn" data-cert="p12-cancel">${esc(t('Cancel'))}</button><span class="pgperr" role="alert">${esc(keys_.p12err)}</span></div></div>`;
  return html;
}
function renderKeys(){
  const el = $('s-keys'), p = pref();
  if (!keys_.list){
    el.innerHTML = keys_.error ? `<p class="derr">${esc(keys_.error)}</p>` : `<p class="dnote">${esc(t('Counting…'))}</p>`;
    if (!keys_.error && !keys_.loading){ keys_.loading = true; loadKeys().finally(() => { keys_.loading = false; }); }
    return;
  }
  const own = keys_.list.filter(k => k.secret), others = keys_.list.filter(k => !k.secret);
  const ids = allIdentities(), keyed = new Set(own.flatMap(k => k.emails));
  const free = ids.filter(i => !keyed.has(i.email.toLowerCase()));
  const pick = free.length ? free : ids;
  let html = `<h3 class="rh">${esc(t('Your keys'))}</h3>`;
  html += own.length ? `<ul class="klist">${own.map(keyRow).join('')}</ul>`
    : `<p class="dnote">${esc(t('With a key of your own you can sign your mail and read mail encrypted to you. Make one, or import the one you have (with its secret part).'))}</p>`;
  html += `<details class="kmake"${own.length ? '' : ' open'}><summary>${esc(t('Make a key'))}</summary>
<div class="sgrid">
<label class="lab" for="k-who">${esc(t('For'))}</label><div><select id="k-who" class="sselect">${pick.map(i => `<option value="${i.id}">${esc(i.name ? `${i.name} <${i.email}>` : i.email)}</option>`).join('')}</select></div>
<label class="lab" for="k-kind">${esc(t('Kind'))}</label><div><select id="k-kind" class="sselect"><option value="ed25519">${esc(t('Ed25519 (small and fast; recommended)'))}</option><option value="rsa3072">${esc(t('RSA 3072 (for old programs)'))}</option></select></div>
<label class="lab" for="k-pass">${esc(t('Passphrase'))}</label><div><input type="password" id="k-pass" class="sfield" autocomplete="new-password" spellcheck="false"></div>
<label class="lab" for="k-pass2">${esc(t('Once more'))}</label><div><input type="password" id="k-pass2" class="sfield" autocomplete="new-password" spellcheck="false"></div>
</div>
<label class="sopt"><input type="checkbox" id="k-rem" checked><span>${esc(t('Remember the passphrase on this PC (the system’s password store)'))}</span></label>
<p class="dnote">${esc(t('The key is kept in the mail base, locked with the passphrase. Without the passphrase, mail encrypted to the key can’t be read, so keep it somewhere safe.'))}</p>
<div class="sbtns"><button type="button" class="btn primary" data-key="make"${keys_.making || !ids.length ? ' disabled' : ''}>${esc(keys_.making ? t('Making the key…') : t('Make key'))}</button></div></details>`;
  html += `<h3 class="rh">${esc(t('Other people’s keys'))}</h3>`;
  html += others.length ? `<ul class="klist">${others.map(keyRow).join('')}</ul>`
    : `<p class="dnote">${esc(t('Keys come from files you import, and from the mail of people whose programs send theirs (Autocrypt).'))}</p>`;
  html += `<div class="sbtns"><button type="button" class="btn" data-key="import">${esc(t('Import key file…'))}</button><button type="button" class="btn" data-key="paste">${esc(t('Paste a key…'))}</button></div>`;
  if (keys_.paste) html += `<div class="kpaste"><textarea id="k-text" class="sfield" rows="6" spellcheck="false" aria-label="${esc(t('Key'))}" placeholder="-----BEGIN PGP PUBLIC KEY BLOCK-----"></textarea>
<div class="sbtns"><button type="button" class="btn primary" data-key="paste-go">${esc(t('Import'))}</button><button type="button" class="btn" data-key="paste-cancel">${esc(t('Cancel'))}</button></div></div>`;
  html += certsHtml();
  html += `<h3 class="rh">${esc(t('Sending'))}</h3><div class="sopts">
<label class="sopt"><input type="checkbox" id="k-sign"${p.pgp_sign ? ' checked' : ''}><span>${esc(t('Sign new messages'))}</span></label>
<label class="sopt"><input type="checkbox" id="k-autocrypt"${p.pgp_autocrypt !== false ? ' checked' : ''}><span>${esc(t('Send my public key along in each message’s header (Autocrypt)'))}</span></label></div>
<p class="dnote">${esc(t('Compose has Sign and Encrypt for each message. A reply to encrypted mail starts encrypted, and Telex remembers what you chose for each person. Encrypted mail is decrypted only while it is open; its text is never kept or searched.'))}</p>`;
  html += `<p class="derr" role="alert">${esc(keys_.error)}</p>`;
  el.innerHTML = html;
}
async function keyWork(run){
  keys_.error = '';
  try { await run(); } catch (e){ keys_.error = tr(e); }
  pgpChanged();
  await loadKeys();
}
$('s-keys').addEventListener('click', async e => {
  const b = e.target.closest('[data-key]');
  if (!b || b.disabled) return;
  const fp = b.dataset.fp, act = b.dataset.key;
  if (act !== 'delete') keys_.armed = '';
  if (act === 'export' || act === 'secret'){
    const secret = act === 'secret';
    keyWork(async () => {
      const path = await invoke('pgp_export', {fingerprint:fp, secret, words:{title:secret ? t('Back up secret key') : t('Export public key'), filter:t('OpenPGP keys'), file_name:`${secret ? 'secret' : 'public'}-${fp.slice(-16)}.asc`}});
      if (path) say(secret ? t('Saved to {path}. It is locked with the key’s passphrase.', {path}) : t('Saved to {path}.', {path}), 8000);
    });
  } else if (act === 'copy'){
    try { if (await copyText(await invoke('pgp_public_text', {fingerprint:fp}))) say(t('Public key copied.'), 2500); } catch (err){ say(tr(err), 8000); }
  } else if (act === 'forget') keyWork(async () => { await invoke('pgp_forget', {fingerprint:fp}); say(t('Telex will ask for the passphrase again.'), 5000); });
  else if (act === 'verify') keyWork(() => invoke('pgp_verify', {fingerprint:fp, verified:b.dataset.on === 'true'}));
  else if (act === 'delete'){
    if (keys_.armed !== fp){ keys_.armed = fp; renderKeys(); return; }
    keys_.armed = '';
    keyWork(async () => { await invoke('pgp_delete', {fingerprint:fp}); say(t('Key deleted.'), 4000); });
  } else if (act === 'import') keyWork(async () => {
    const r = await invoke('pgp_import', {words:{title:t('Import key file'), filter:t('OpenPGP keys')}});
    if (r) say(importedText(r), 6000);
  });
  else if (act === 'paste'){ keys_.paste = true; renderKeys(); $('k-text')?.focus(); }
  else if (act === 'paste-cancel'){ keys_.paste = false; renderKeys(); }
  else if (act === 'paste-go'){
    const text = $('k-text')?.value ?? '';
    keyWork(async () => { const r = await invoke('pgp_import_text', {text}); keys_.paste = false; say(importedText(r), 6000); });
  } else if (act === 'make'){
    const pass = $('k-pass').value, again = $('k-pass2').value, who = allIdentities().find(i => i.id === Number($('k-who').value));
    if (!who) return;
    if (!pass){ keys_.error = t('Choose a passphrase for the key.'); renderKeys(); return; }
    if (pass !== again){ keys_.error = t('The two passphrases differ.'); renderKeys(); $('k-pass2')?.focus(); return; }
    const kind = $('k-kind').value === 'rsa3072' ? 'rsa3072' : 'ed25519', remember = $('k-rem').checked;
    keys_.making = true; renderKeys();
    await keyWork(async () => {
      const made = await invoke('pgp_generate', {name:who.name, email:who.email, passphrase:pass, kind, remember});
      say(t('Made a key for {email}.', {email:who.email}), 6000);
      return made;
    });
    keys_.making = false; renderKeys();
  }
});
const certImported = r => {
  const n = r.added.length + r.updated.length;
  if (!n) return r.unchanged.length ? t('Telex had that certificate already.') : t('Nothing was imported.');
  return tn(n, 'Imported the certificate of {names}.', 'Imported {n} certificates: {names}.', {names:[...r.added, ...r.updated].map(keyName).join(', ')});
};
async function importP12(){
  const field = $('k-p12');
  keys_.p12err = '';
  let r;
  try { r = await invoke('smime_import_p12', {password:field?.value ?? '', remember:!!$('k-p12-rem')?.checked}); }
  catch (e){ keys_.p12err = tr(e); renderKeys(); $('k-p12')?.focus(); return; }
  if (!r){ keys_.p12err = t('That password doesn’t open the file.'); renderKeys(); $('k-p12')?.select(); return; }
  keys_.p12 = null;
  say(certImported(r), 6000);
  pgpChanged();
  await loadKeys();
}
$('s-keys').addEventListener('click', async e => {
  const b = e.target.closest('[data-cert]');
  if (!b || b.disabled) return;
  const fp = b.dataset.fp, act = b.dataset.cert;
  if (act !== 'delete') keys_.armed = '';
  if (act === 'pick') keyWork(async () => {
    const r = await invoke('smime_pick', {words:{title:t('Import certificate'), filter:t('Certificates (.p12, .pfx, .cer, .pem)')}});
    if (!r) return;
    if (r.kind === 'password'){ keys_.p12 = r.name; keys_.p12err = ''; setTimeout(() => $('k-p12')?.focus(), 0); }
    else say(certImported(r.report), 6000);
  });
  else if (act === 'p12-go') importP12();
  else if (act === 'p12-cancel'){ keys_.p12 = null; keys_.p12err = ''; invoke('smime_cancel_p12').catch(() => {}); renderKeys(); }
  else if (act === 'export') keyWork(async () => {
    const path = await invoke('smime_export', {fingerprint:fp, words:{title:t('Export certificate'), filter:t('Certificates'), file_name:`certificate-${fp.slice(0, 16)}.cer`}});
    if (path) say(t('Saved to {path}.', {path}), 8000);
  });
  else if (act === 'forget') keyWork(async () => { await invoke('smime_forget', {fingerprint:fp}); say(t('Telex will ask for the password again.'), 5000); });
  else if (act === 'verify') keyWork(() => invoke('smime_verify', {fingerprint:fp, verified:b.dataset.on === 'true'}));
  else if (act === 'delete'){
    if (keys_.armed !== fp){ keys_.armed = fp; renderKeys(); return; }
    keys_.armed = '';
    keyWork(async () => { await invoke('smime_delete', {fingerprint:fp}); say(t('Certificate deleted.'), 4000); });
  }
});
$('s-keys').addEventListener('keydown', e => {
  if (e.key === 'Enter' && e.target.id === 'k-p12'){ e.preventDefault(); importP12(); }
});
$('s-keys').addEventListener('change', e => {
  if (e.target.id === 'k-sign') changeSettings({pgp_sign:e.target.checked});
  else if (e.target.id === 'k-autocrypt') changeSettings({pgp_autocrypt:e.target.checked});
});

/* ---------- updates ---------- */
// Only release builds can update themselves. Telex checks once a day if the user said yes
// to that, and whenever they click the version number. The core asks GitHub and installs;
// the window never goes online.
const udlg = $('upd-dlg');
// The license and the third-party notices, on the site: in the update dialog and Settings › Updates.
for (const d of [udlg, sdlg]) d.addEventListener('click', e => {
  const l = e.target.closest('[data-weblink]');
  if (l) call('open_link', {url:l.dataset.weblink}).catch(() => {});
});
const upd = {build:null, found:null, checking:false, installing:false, downloading:false, progress:null, text:'', error:''};
function renderUpdates(){
  const {build, found} = upd, can = !!build?.updatable;
  $('st-ver').hidden = !build;
  if (!build) return;
  $('st-ver').textContent = build.version;
  $('st-ver').title = t('Telex {version}.', {version:build.version}) + ' ' + (can ? t('Click to check for updates.') : t('This build can’t update itself.'));
  $('st-ask').hidden = !can || pref().updates !== 'ask';
  // One question at a time: counting is asked once updates are answered.
  $('st-count').hidden = !build.countable || pref().usage_count !== 'ask' || !$('st-ask').hidden;
  $('st-update').hidden = !found;
  if (found) $('st-update').textContent = t('Update to {version}', {version:found.version});
  $('upd-title').textContent = 'Telex ' + build.version;
  $('u-state').textContent = can ? upd.text : t('This build of Telex can’t update itself. Release builds from telexmail.app can.');
  $('u-err').textContent = upd.error;
  $('u-progress-row').hidden = !upd.downloading;
  if (upd.progress === null) $('u-progress').removeAttribute('value');
  else $('u-progress').value = upd.progress;
  $('u-notes').hidden = !found?.notes;
  $('u-notes').textContent = found?.notes ?? '';
  $('u-install').hidden = !found;
  if (found) $('u-install').textContent = t('Install {version} and restart', {version:found.version});
  $('u-install').disabled = $('u-check').disabled = upd.checking || upd.installing;
  $('u-auto').checked = pref().updates === 'start' || pref().updates === 'daily';
  ['u-auto-row', 'u-note', 'u-check'].forEach(id => { $(id).hidden = !can; });
}
// quiet: the daily check, which speaks up only when there is an update.
async function checkUpdate({quiet = false} = {}){
  if (!upd.build?.updatable || upd.checking || upd.installing) return;
  Object.assign(upd, {checking:true, text:t('Checking for updates…'), error:''});
  renderUpdates();
  try {
    const found = upd.found = await invoke('check_update');
    state.updateChecked = Date.now(); persist();
    upd.text = found ? (found.date ? t('Telex {version} is out, from {date}. You have {mine}.', {version:found.version, date:dmy(parseDate(found.date)), mine:upd.build.version})
        : t('Telex {version} is out. You have {mine}.', {version:found.version, mine:upd.build.version}))
      : t('You have the newest version. Checked at {time}.', {time:hm(new Date())});
    if (found && quiet) say(t('Telex {version} is out. Click Update in the status bar to see what’s new.', {version:found.version}), 10000);
  } catch (e){
    Object.assign(upd, {text:'', error:tr(e)});
    if (!quiet && !udlg.open) say(tr(e), 8000);
  } finally {
    upd.checking = false;
    renderUpdates();
  }
}
// The installer closes Telex, updates it and opens it again. A message being written is
// saved to Drafts first.
async function installUpdate(){
  if (!upd.found || upd.installing) return;
  if (state.compose && !await closeCompose()) return;
  Object.assign(upd, {installing:true, downloading:true, progress:null, text:t('Downloading Telex {version}…', {version:upd.found.version}), error:''});
  renderUpdates();
  let r;
  try { r = await invoke('install_update'); }
  catch (e){
    Object.assign(upd, {installing:false, downloading:false, progress:null, text:'', error:tr(e)}); renderUpdates();
    if (!udlg.open) say(tr(e), 10000); // replaces the download progress
    return;
  }
  if (r.busy){ // a check or a send is running: start when it's done
    upd.downloading = false;
    upd.text = t('Waiting for Telex to finish with the mail server…'); renderUpdates();
    setTimeout(() => { upd.installing = false; installUpdate(); }, 2000);
  }
}
listenMain('update-progress', ({payload: p}) => {
  const mb = n => I18n.decimal(n / 1048576);
  upd.progress = p.total ? Math.max(0, Math.min(100, p.done / p.total * 100)) : null;
  upd.text = p.total && p.done >= p.total ? t('Checking the signature. Telex will close, update and open again.')
    : p.total ? t('Downloading Telex {version}: {done} of {total} MB', {version:upd.found?.version ?? '', done:mb(p.done), total:mb(p.total)})
    : t('Downloading Telex {version}: {done} MB', {version:upd.found?.version ?? '', done:mb(p.done)});
  renderUpdates();
  if (!udlg.open) say(upd.text, 0);
});
function openUpdates(){
  renderUpdates();
  udlg.showModal();
  // Clicking the version number checks right away, unless an update is already known.
  if (!upd.found && !upd.installing) checkUpdate();
  $(upd.found ? 'u-install' : 'u-close').focus();
}
// "When Telex starts": once at start (unless a check answered in the last few minutes, as after
// an update's restart), then like "once a day" while it stays open. Each check asks only for
// the newest version's file; the answer shows as the chip in the status bar.
const UPDATE_DAY = 864e5, UPDATE_AGAIN = 10 * 60e3;
function autoCheck(starting = false){
  const how = pref().updates, since = Date.now() - state.updateChecked;
  if (how !== 'start' && how !== 'daily') return;
  if (since >= UPDATE_DAY || (starting && how === 'start' && since >= UPDATE_AGAIN)) checkUpdate({quiet:true});
}
// Counting this copy (count.rs): only release builds, only after a yes; the core sends at most
// one count a day and says nothing when it can't.
function countCopy(){
  if (upd.build?.countable && pref().usage_count === 'on') invoke('count_copy').catch(() => {});
}
async function bootUpdates(){
  try { upd.build = await invoke('update_info'); } catch { return; }
  renderUpdates();
  if (upd.build?.countable){ setTimeout(countCopy, 15e3); setInterval(countCopy, 60 * 60e3); }
  if (!upd.build?.updatable) return;
  setTimeout(() => autoCheck(true), 8e3); // after the first mail check has started
  setInterval(autoCheck, 60 * 60e3);
}
$('st-ask').addEventListener('click', async e => {
  const b = e.target.closest('[data-ask]'); if (!b) return;
  if (!await changeSettings({updates:b.dataset.ask})) return;
  if (pref().updates === 'start'){ say(t('Telex will check for updates when it starts and once a day.')); autoCheck(true); }
  else say(t('Telex won’t check for updates. Click the version number to check yourself.'), 7000);
});
$('st-count').addEventListener('click', async e => {
  const b = e.target.closest('[data-count]'); if (!b) return;
  if (!await changeSettings({usage_count:b.dataset.count})) return;
  if (pref().usage_count === 'on'){ say(t('Thank you. Telex will be counted once a day. Settings › Updates turns it off.'), 7000); countCopy(); }
  else say(t('Telex won’t be counted. Settings › Updates can change that.'), 7000);
});
$('u-auto').addEventListener('change', e => { changeSettings({updates:e.target.checked ? 'start' : 'off'}); });
$('st-ver').addEventListener('click', openUpdates);
$('st-update').addEventListener('click', openUpdates);
$('u-check').addEventListener('click', () => checkUpdate());
$('u-install').addEventListener('click', installUpdate);
$('u-close').addEventListener('click', () => udlg.close());
udlg.addEventListener('close', () => listEl.focus({preventScroll:true}));

/* ---------- keys and tips ---------- */
// Every key, in one list: the help draws it, and scripts/check-keys.mjs checks it against the
// keyboard table in CLAUDE.md (every row of the table has its keys here, every `doc` names a
// row, and the keys written here are in that row). To add a key: a line here, its Polish in
// i18n.js, its row in CLAUDE.md's table (and the status-bar hints when it is a main one).
//   g    the group (KEY_GROUPS)
//   k    the keys, written "Shift+R"; a space separates keys that do the same or go together,
//        and a word in lower case is the mouse (click, drag)
//   d    what it does, short, in lower case
//   doc  the row of CLAUDE.md's keyboard table it belongs to: that row's first column, as written
//   run  Enter (or a click) in the help presses it: only one key, safe and obvious
// keys:start
const KEY_GROUPS = [['move', N_('Moving around')], ['read', N_('Reading')], ['select', N_('Selecting')], ['act', N_('Acting on mail')],
  ['write', N_('Writing')], ['find', N_('Search|help')], ['tree', N_('Folders and accounts')], ['window', N_('The window')]];
const KEYS = [
  {g:'move', k:'↑ ↓', d:N_('previous or next message'), doc:'↑ / ↓'},
  {g:'move', k:'Home End', d:N_('first or last message'), doc:'Home / End'},
  {g:'move', k:'Space', d:N_('page down the message; at its end, the next unread one'), doc:'Space / Shift+Space'},
  {g:'move', k:'Shift+Space', d:N_('page up; at the top, the previous unread one'), doc:'Space / Shift+Space'},
  {g:'move', k:'G', d:N_('go to a folder: type part of its name'), doc:'G', run:true},
  {g:'move', k:'T', d:N_('the whole conversation; T or Esc goes back'), doc:'T', run:true},
  {g:'move', k:'O', d:N_('only the unread messages; O again shows them all'), doc:'O', run:true},
  {g:'move', k:'Enter', d:N_('in all mail and conversations: the message’s folder'), doc:'Enter, double-click'},
  {g:'read', k:'Enter double-click', d:N_('the message in a window of its own'), doc:'Enter, double-click'},
  {g:'read', k:'Esc', d:N_('in a message’s own window: close it'), doc:'Enter, double-click'},
  {g:'read', k:'H', d:N_('HTML or plain text'), doc:'H', run:true},
  {g:'read', k:'Q', d:N_('open or fold the quoted mail'), doc:'Q', run:true},
  {g:'read', k:'→ ←', d:N_('with threads on: expand or collapse a thread'), doc:'→ / ←'},
  {g:'read', k:'Ctrl+C', d:N_('the selected messages, or the open one, as Markdown for notes or an AI chat'), doc:'Ctrl+C'},
  {g:'read', k:'Ctrl+S', d:N_('save as .eml files, as they came'), doc:'Ctrl+S'},
  {g:'read', k:'Ctrl+P', d:N_('print, or save as PDF from the … above the message'), doc:'Ctrl+P'},
  {g:'read', k:'Ctrl+U', d:N_('the message’s source and all its headers'), doc:'Ctrl+U'},
  {g:'select', k:'Shift+↑ ↓', d:N_('one more or one fewer'), doc:'Shift+↑ / ↓'},
  {g:'select', k:'Shift+Home End', d:N_('everything to the top or bottom'), doc:'Shift+↑ / ↓'},
  {g:'select', k:'Ctrl+click', d:N_('one more message'), doc:'Ctrl+click, Shift+click'},
  {g:'select', k:'Shift+click', d:N_('a range of messages'), doc:'Ctrl+click, Shift+click'},
  {g:'select', k:'Ctrl+A', d:N_('the whole list'), doc:'Ctrl+A'},
  {g:'select', k:'Esc', d:N_('just the one message again'), doc:'Esc'},
  {g:'act', k:'F', d:N_('flag or unflag'), doc:'F', run:true},
  {g:'act', k:'1 2 3 4 5', d:N_('the first five tags on or off'), doc:'1 2 3 4 5'},
  {g:'act', k:'U', d:N_('read or unread'), doc:'U', run:true},
  {g:'act', k:'click', d:N_('the dot or the flag at the start of a row: read or unread, flagged or not'), doc:'click the dot / flag'},
  {g:'act', k:'Shift+U', d:N_('everything in the folder read'), doc:'Shift+U'},
  {g:'act', k:'Del', d:N_('to Trash; in Trash, twice to delete for good'), doc:'Del'},
  {g:'act', k:'M', d:N_('move to a folder: type part of its name'), doc:'M', run:true},
  {g:'act', k:'drag', d:N_('messages onto a folder: rest there until it fills, then let go'), doc:'drag'},
  {g:'act', k:'Z', d:N_('undo a delete, a move or Discard; take back mail just sent'), doc:'Z', run:true},
  {g:'act', k:'Shift+F10 Menu right-click', d:N_('the selected message’s menu, every action with its key'), doc:'Shift+F10, menu key'},
  {g:'write', k:'N', d:N_('new message'), doc:'N', run:true},
  {g:'write', k:'R', d:N_('reply'), doc:'R', run:true},
  {g:'write', k:'Shift+R', d:N_('reply to all'), doc:'Shift+R', run:true},
  {g:'write', k:'Shift+F', d:N_('forward'), doc:'Shift+F', run:true},
  {g:'write', k:'E', d:N_('edit a draft or a message in the Outbox'), doc:'E', run:true},
  {g:'write', k:'Ctrl+Enter', d:N_('send'), doc:'Ctrl+Enter'},
  {g:'write', k:'Ctrl+Shift+Enter', d:N_('send later: in an hour, tomorrow, Monday or when you pick'), doc:'Ctrl+Shift+Enter'},
  {g:'write', k:'Esc', d:N_('save as a draft and close'), doc:'Esc'},
  {g:'write', k:'Ctrl+B I U K', d:N_('bold, italic, underline, link in HTML'), doc:'Ctrl+B / I / U / K'},
  {g:'write', k:'F7', d:N_('the next spelling mistake, with suggestions; or click it'), doc:'F7'},
  {g:'write', k:'↓ ↑ Enter Tab', d:N_('in To, Cc and Bcc: choose a suggested address'), doc:'↓ ↑ Enter Tab Esc'},
  {g:'find', k:'/', d:N_('search: the list shows what matches as you type'), doc:'/', run:true},
  {g:'find', k:'↓ ↑ Enter', d:N_('choose a suggestion; with none chosen, to the results'), doc:'↓ ↑ Enter'},
  {g:'find', k:'Tab', d:N_('this folder or all mail'), doc:'Tab'},
  {g:'find', k:'Esc', d:N_('clear the filter'), doc:'Esc'},
  {g:'tree', k:'F2', d:N_('rename a folder on this PC'), doc:'F2, Shift+F10'},
  {g:'tree', k:'Shift+F10 right-click', d:N_('a folder’s or account’s menu'), doc:'F2, Shift+F10'},
  {g:'tree', k:'Alt+↑ ↓', d:N_('an account up or down'), doc:'Alt+↑ / ↓'},
  {g:'window', k:'Ctrl+,', d:N_('settings'), doc:'Ctrl+,', run:true},
  {g:'window', k:'B', d:N_('the address book: contacts and groups'), doc:'B', run:true},
  {g:'window', k:'Ctrl+= − 0', d:N_('larger, smaller or normal text'), doc:'Ctrl+= / Ctrl+− / Ctrl+0'},
  {g:'window', k:'? F1', d:N_('this help'), doc:'? / F1'},
];
// keys:end
const HELP_TIPS = [
  N_('Right-click a message for everything you can do with it, each with its key.'),
  N_('Unread, at the top of the folders, gathers the unread mail of every account.'),
  N_('Drag the edge before Subject to widen From; double-click it to fit the names.'),
  N_('Right-click the column headings for full dates and the Size column.'),
  N_('Right-click a folder › Keep in view, and it stays out of “more”.'),
  N_('Folders on this PC keep mail off the server; right-click one to save it as mbox.'),
  N_('Drag messages out of the window onto the desktop or into a folder: they land there as .eml files.'),
  N_('Drop .eml or mbox files on a folder on this PC to bring that mail in.'),
  N_('Click the sender above a message to see all their mail.'),
  N_('Drop files on the window to attach them, even with no message open.'),
  N_('The clock next to Send sends a message later; it waits in the Outbox, which says when it goes. Telex sends it then, or at its next start if it was closed.'),
  N_('Mail from a mailing list has Unsubscribe above it: one click where the list takes it, else a mail or the list’s page, after saying where. Never in Spam.'),
  N_('In an HTML message, paste a picture or drop one where it should go; it goes in the text, not as a file. Settings › Writing says how large.'),
  N_('A folder can have its own greeting and reply header: right-click it › Templates….'),
  N_('Spelling checks every ticked language at once; the end of the compose bar shows them.'),
  N_('Rules › “New rule for mail from …” starts a rule from the selected message.'),
  N_('The storage bar in the status bar shows how full the server is; click it for more.'),
  N_('In a dark theme, the sun button above HTML mail shows it on white, as it was sent.'),
  N_('Errors and notes from rules show above the message they are about.'),
  N_('The person button by a sender adds them to the address book. Type a group’s name in To and it goes in as its people.'),
  N_('Tags are colored labels, several to a message: Settings › Tags names and colors them. Search with tag:work or is:tagged.'),
];
const OPERATORS = 'from: to: subject: is:unread is:flagged tag:work is:tagged has:file after:2026-09 before:2026 larger:5m smaller:100k';
const MOUSE = {click:N_('click|mouse'), 'double-click':N_('double-click|mouse'), 'right-click':N_('right-click|mouse'), drag:N_('drag|mouse')};
// "Shift+R" as keycaps; a mouse word as a word.
const keysHtml = spec => spec.split(' ').map(c => c.split(/\+(?=.)/)
  .map(k => MOUSE[k] ? `<span class="mse">${esc(t(MOUSE[k]))}</span>` : `<kbd>${esc(k)}</kbd>`).join('<i>+</i>')).join(' ');
// Words to find a key or a tip by, in English, the language on screen and Polish; case and
// diacritics ignored (fold).
const both = key => [key.split('|')[0], t(key), I18n.dictionaries?.PL?.[key] ?? ''].join(' ');
const helpDlg = $('help-dlg'), helpQ = $('help-q'), helpBody = $('help-body');
const help = {from:null, last:null, done:true, at:-1, shown:[]};
// The help keeps where the focus was before it, not the button that opened it.
document.addEventListener('focusin', e => { if (!e.target.closest?.('[data-help-open], #help-dlg')) help.last = e.target; });
// Where the help was opened from puts its keys first: compose, the folder tree, the search box,
// several messages, the message itself; else moving around the list.
function helpContext(){
  const el = help.from;
  if (state.compose) return 'write';
  if (el?.closest?.('#tree')) return 'tree';
  if (el === qEl || el?.closest?.('#q-pop')) return 'find';
  if (state.picked.size > 1) return 'select';
  if (VIEWER || el?.closest?.('#preview')) return 'read';
  return 'move';
}
function renderHelp(){
  const words = fold(helpQ.value).split(/\s+/).filter(Boolean);
  const hit = text => words.every(w => fold(text).includes(w));
  const here = helpContext();
  const groups = [...KEY_GROUPS].sort((a, b) => (b[0] === here) - (a[0] === here));
  help.shown = [];
  const cols = groups.map(([g, title]) => {
    const rows = KEYS.filter(r => r.g === g && (!words.length || hit(`${r.k} ${both(r.d)} ${both(title)} ${r.k.split(/[ +]/).map(k => MOUSE[k] ? both(MOUSE[k]) : '').join(' ')}`)));
    if (!rows.length) return '';
    return `<section class="hgrp${g === here && !words.length ? ' here' : ''}" role="group" aria-labelledby="hg-${g}"><h3 id="hg-${g}">${esc(t(title))}${g === here && !words.length ? ` <span class="hhere">${esc(t('here|help'))}</span>` : ''}</h3>`
      + rows.map(r => {
        const i = help.shown.push(r) - 1;
        return `<div class="hrow${r.run ? ' run' : ''}" role="option" id="hk-${i}" data-i="${i}" aria-selected="false"${r.run ? ` title="${esc(t('Click or press Enter to use it now'))}"` : ''}>`
          + `<span class="k">${keysHtml(r.k)}</span><span class="d">${esc(t(r.d))}</span></div>`;
      }).join('')
      + (g === 'find' && (!words.length || words.every(w => fold(OPERATORS).includes(w))) ? `<p class="hops">${OPERATORS.split(' ').map(o => `<code>${esc(o)}</code>`).join(' ')}</p>` : '')
      + '</section>';
  }).join('');
  const tips = HELP_TIPS.filter(tip => !words.length || hit(both(tip)));
  helpBody.innerHTML = (cols ? `<div class="hcols" role="listbox" aria-label="${esc(t('Keys'))}">${cols}</div>` : '')
    + (tips.length ? `<section class="htips"><h3>${esc(t('Good to know'))}</h3><ul>${tips.map(tip => `<li>${esc(t(tip))}</li>`).join('')}</ul></section>` : '')
    + (!cols && !tips.length ? `<p class="hnone">${esc(t('No key or tip matches “{q}”.', {q:helpQ.value.trim()}))}</p>` : '');
  helpAt(words.length ? help.shown.findIndex(r => r.run) : -1);
}
// The row Enter presses: one that can run, or none.
function helpAt(i){
  helpBody.querySelector('.hrow.on')?.classList.remove('on');
  helpBody.querySelector('.hrow[aria-selected="true"]')?.setAttribute('aria-selected', 'false');
  help.at = i;
  const row = i >= 0 ? $('hk-' + i) : null;
  if (row){ row.classList.add('on'); row.setAttribute('aria-selected', 'true'); row.scrollIntoView({block:'nearest'}); helpQ.setAttribute('aria-activedescendant', row.id); }
  else helpQ.removeAttribute('aria-activedescendant');
}
function openHelp(){
  if (helpDlg.open) return;
  const el = document.activeElement;
  help.from = !el || el === document.body || el.closest?.('[data-help-open]') ? help.last : el;
  helpQ.value = '';
  renderHelp();
  helpDlg.showModal();
  help.done = false;
  helpBody.scrollTop = 0;
  helpQ.focus();
  $('st-help').setAttribute('aria-expanded', 'true');
}
// The focus goes back where it was, once: right away when the help closes itself, else (Esc) on
// the dialog's close event, which comes later and may come after it was opened again.
function helpClosed(){
  if (helpDlg.open || help.done) return;
  help.done = true;
  $('st-help').setAttribute('aria-expanded', 'false');
  const back = help.from;
  help.from = null;
  if (back !== false) (back?.isConnected && back.focus ? back : listEl).focus({preventScroll:true});
}
function closeHelp(){ if (helpDlg.open){ helpDlg.close(); helpClosed(); } }
const toggleHelp = () => helpDlg.open ? closeHelp() : openHelp();
helpDlg.addEventListener('close', helpClosed);
// Esc clears the filter first, then closes.
helpDlg.addEventListener('cancel', e => {
  e.preventDefault();
  if (helpQ.value){ helpQ.value = ''; renderHelp(); } else closeHelp();
});
// A click outside the panel (on the dimmed window) closes it.
helpDlg.addEventListener('click', e => {
  const r = helpDlg.getBoundingClientRect();
  if (e.clientX < r.left || e.clientX > r.right || e.clientY < r.top || e.clientY > r.bottom) closeHelp();
});
helpQ.addEventListener('input', renderHelp);
// ↓ ↑ go through the keys that can run; Enter presses the chosen one.
helpQ.addEventListener('keydown', e => {
  if (e.key === 'ArrowDown' || e.key === 'ArrowUp'){
    e.preventDefault();
    const runs = help.shown.map((r, i) => r.run ? i : -1).filter(i => i >= 0);
    if (!runs.length) return;
    const now = runs.indexOf(help.at);
    helpAt(runs[now < 0 ? (e.key === 'ArrowDown' ? 0 : runs.length - 1) : (now + (e.key === 'ArrowDown' ? 1 : runs.length - 1)) % runs.length]);
  } else if (e.key === 'Enter' && help.at >= 0){ e.preventDefault(); runKey(help.shown[help.at]); }
});
helpBody.addEventListener('click', e => {
  const row = e.target.closest('.hrow.run');
  if (row) runKey(help.shown[row.dataset.i]);
});
// Closes the help and presses the key in the list, as if typed there.
function runKey(r){
  if (!r?.run) return;
  const parts = r.k.split(/\+(?=.)/), key = parts.pop(), shift = parts.includes('Shift'), ctrl = parts.includes('Ctrl');
  help.from = false; // the key moves the focus itself
  closeHelp();
  listEl.focus({preventScroll:true});
  listEl.dispatchEvent(new KeyboardEvent('keydown', {key:shift ? key.toUpperCase() : key.toLowerCase(), shiftKey:shift, ctrlKey:ctrl, bubbles:true, cancelable:true}));
}
$('help-close').addEventListener('click', closeHelp);
$('st-help').addEventListener('click', toggleHelp);
$('b-help').addEventListener('click', openHelp);
// Once, a few seconds after the first start with mail: where the keys and tips are.
// Waits for an account, a quiet status bar and no dialog.
function tellHelp(){
  if (state.helpTold || VIEWER) return;
  setTimeout(() => {
    if (!ACCOUNTS.length || $('st-msg').textContent || document.querySelector('dialog[open]')) return tellHelp();
    say(t('Press ? or F1 for every key and tip.'), 9000);
    state.helpTold = true; persist();
  }, 4000);
}

/* ---------- keyboard ---------- */
// Also attached to the HTML message frame, so keys work after clicking into it.
function onKey(e){
  const mod = (e.ctrlKey || e.metaKey) && !e.altKey;
  // Text size works everywhere, in dialogs too.
  if (mod && ['=', '+', '-', '0'].includes(e.key)){ e.preventDefault(); stepTextSize(e.key === '0' ? 0 : e.key === '-' ? -1 : 1); return; }
  // F1 everywhere, over a dialog too; in the help it closes it.
  if (e.key === 'F1'){ e.preventDefault(); toggleHelp(); return; }
  if (document.querySelector('dialog[open]')) return; // dialogs handle their own Enter and Esc
  if (mod && e.key === ','){ e.preventDefault(); openSettings(); return; }
  // Ctrl+S, Ctrl+P, Ctrl+U: save as .eml, print, the source; in the search box too, never the
  // webview's own. Compose keeps them (Ctrl+U underlines).
  if (mod && !e.shiftKey && !state.compose && ['s', 'p', 'u'].includes(e.key.toLowerCase())){
    e.preventDefault();
    const k = e.key.toLowerCase();
    if (k === 's') saveMessages(); else if (k === 'p') printMessage(); else showSource();
    return;
  }
  const tg = e.target;
  if (state.compose && e.key === 'Enter' && (e.ctrlKey || e.metaKey) && e.shiftKey){ e.preventDefault(); laterMenu(); return; }
  if (state.compose && e.key === 'Enter' && (e.ctrlKey || e.metaKey)){ e.preventDefault(); if (e.target.id === 'c-when') sendAtPicked(); else send(); return; }
  if (VIEWER && (mod ? e.key === ',' : !e.altKey && ['g', 'G', 't', 'T', 'o', 'O', 'b', 'B', '/', 'Enter'].includes(e.key) && !tg.closest?.('input, textarea, [contenteditable="true"]'))) return;
  if (e.key === 'Escape'){
    if (!linkTip.hidden){ e.preventDefault(); hideLink(); return; }
    if (state.compose){ e.preventDefault(); closeCompose().then(ok => ok && say(t('Saved to Drafts.'))); return; }
    if (tg === qEl){ qEl.value = ''; setFilter(''); listEl.focus(); return; }
    if (unpick()){ e.preventDefault(); afterPick(); return; }
    if (state.thread){ e.preventDefault(); leaveThread(); return; }
    if (state.unread){ e.preventDefault(); toggleUnreadOnly(); return; }
    if (VIEWER && menuEl.hidden){ e.preventDefault(); closeViewer(); return; }
  }
  // Tab in the filter box: this folder or all mail, while there is something to look for.
  if (tg === qEl && e.key === 'Tab' && !e.shiftKey && !e.ctrlKey && !e.altKey && qEl.value.trim()){ e.preventDefault(); toggleScope(); return; }
  if (tg.closest && (tg.closest('input, textarea, select, [contenteditable="true"]') || tg.closest('.split'))) return;
  // Ctrl+A in the list takes every message; elsewhere it selects text as always.
  if (e.key === 'Enter' && tg.closest?.('[data-thread]')) return;
  if (mod && !e.shiftKey && e.key.toLowerCase() === 'a' && listEl.contains(tg)){ e.preventDefault(); pickAll(); return; }
  // Ctrl+C with no text selected: the selected messages as Markdown. Selected text copies as always.
  if (mod && !e.shiftKey && e.key.toLowerCase() === 'c' && !state.compose && !(tg.ownerDocument ?? document).getSelection?.()?.toString()){
    e.preventDefault(); copyMarkdown(); return;
  }
  // The menu key (or Shift+F10) in the list: the selected message's menu, at its row.
  if ((e.key === 'ContextMenu' || (e.key === 'F10' && e.shiftKey)) && listEl.contains(tg)){
    e.preventDefault();
    const row = rowsEl.querySelector('tr.sel') ?? rowsEl.querySelector('tr[data-id]'), r = (row ?? listEl).getBoundingClientRect();
    openMessageMenu(r.left + 60, row ? r.bottom : r.top + 30);
    return;
  }
  if (e.ctrlKey || e.metaKey || e.altKey) return;
  // Space presses a focused button or link, as always.
  if (e.key === ' ' && tg.closest?.('button, a[href], summary, label, [role="tab"], [role="option"], [data-i]')) return;
  // A letter is the same key with Caps Lock on: only Shift changes what it does.
  const shift = e.shiftKey, k = e.key.length === 1 ? e.key.toLowerCase() : e.key;
  if (k === 'ArrowDown') shift ? pickMove(1) : move(1);
  else if (k === 'ArrowUp') shift ? pickMove(-1) : move(-1);
  else if ((k === 'ArrowRight' || k === 'ArrowLeft') && !shift && listEl.contains(tg) && grouped()) toggleGroup(current(), k === 'ArrowRight');
  else if (k === 'Home') shift ? pickEnd(false) : move(-Infinity);
  else if (k === 'End') shift ? pickEnd(true) : move(Infinity);
  else if (k === ' ') space(shift);
  else if (k === 'Delete'){ if (!e.repeat) del(); } // holding Del deletes one, not a run of them
  else if (k === 'Enter' && allMail()) openInFolder();
  else if (k === 'Enter' && listEl.contains(tg) && current()) openWindow(current().id);
  else if (k === 'r') openCompose(shift ? 'replyall' : 'reply');
  else if (k === 'f') shift ? openCompose('forward') : toggleFlag();
  else if (k === 'u') shift ? markAllRead() : toggleUnread();
  else if (k === 'n') openCompose('new');
  else if (k === 'e') openCompose('edit');
  else if (k === 'h') setView(!pref().prefer_text);
  else if (k === 'q') toggleQuotes();
  else if (k === 'm') openPicker('move');
  else if (k === 'g') openPicker('go');
  else if (k === 'b') openAddressBook();
  else if (k === 't') toggleThread();
  else if (k === 'o') toggleUnreadOnly();
  else if (/^[1-5]$/.test(k) && !shift && TAGS[Number(k) - 1]) toggleTag(TAGS[Number(k) - 1].keyword);
  else if (k === 'z') undo();
  else if (k === '/') qEl.focus();
  else if (k === '?') toggleHelp();
  else return;
  e.preventDefault();
}
document.addEventListener('keydown', onKey);

/* ---------- splitters ---------- */
function splitter(el, onMove, onKey){
  el.addEventListener('pointerdown', e => {
    e.preventDefault(); el.setPointerCapture(e.pointerId); el.classList.add('drag');
    const mv = ev => onMove(ev);
    const up = () => { el.classList.remove('drag'); el.removeEventListener('pointermove', mv); el.removeEventListener('pointerup', up); el.removeEventListener('pointercancel', up); persist(); };
    el.addEventListener('pointermove', mv); el.addEventListener('pointerup', up); el.addEventListener('pointercancel', up);
  });
  el.addEventListener('keydown', e => { if (onKey(e.key)){ e.preventDefault(); persist(); } });
}
const clamp = (v, lo, hi) => Math.max(lo, Math.min(hi, v));
splitter($('split-v'),
  ev => { const r = document.querySelector('.main').getBoundingClientRect(); win.style.setProperty('--tree-w', clamp(Math.round(ev.clientX - r.left), 150, 380) + 'px'); },
  k => {
    const d = k === 'ArrowLeft' ? -12 : k === 'ArrowRight' ? 12 : 0; if (!d) return false;
    win.style.setProperty('--tree-w', clamp(Math.round(treeEl.getBoundingClientRect().width + d), 150, 380) + 'px'); return true;
  });
splitter($('split-h'),
  ev => { const r = $('right').getBoundingClientRect(); win.style.setProperty('--list-h', clamp((ev.clientY - r.top) / r.height * 100, 18, 82).toFixed(1) + '%'); },
  k => {
    const d = k === 'ArrowUp' ? -4 : k === 'ArrowDown' ? 4 : 0; if (!d) return false;
    const r = $('right').getBoundingClientRect(), cur = listEl.getBoundingClientRect().height / r.height * 100;
    win.style.setProperty('--list-h', clamp(cur + d, 18, 82).toFixed(1) + '%'); return true;
  });

/* ---------- boot ---------- */
/* ---------- a message in its own window ---------- */
// Just the message, its header and the toolbar's buttons for it. Read when opened, as the
// settings say; Esc closes it; deleting or sending closes it too.
const closeViewer = () => { const w = window.__TAURI__?.window?.getCurrentWindow?.(); if (w?.close) w.close(); else window.close(); };
// Show the message (again, after a change). When it is gone, the window closes.
async function viewerShow(){
  bodies.delete(VIEWER);
  let m;
  try { m = await full(VIEWER); } catch { closeViewer(); return false; }
  state.folder = m.folder; state.sel[m.folder] = m.id;
  selRow = {id:m.id, f:m.folder, from:m.from_name, email:m.from_email, to:m.to, subj:m.subject, date:parseDate(m.date),
    kb:m.size_kb, unread:m.unread, flagged:m.flagged, hasAtt:m.attachments.length > 0, tags:m.tags ?? []};
  renderStatus();
  if (!state.compose) await renderPreview();
  return true;
}
async function bootViewer(){
  win.classList.add('viewer');
  await bootSettings();
  try { await loadTree(); } catch {}
  if (!await viewerShow()) return;
  if (selRow.unread && pref().mark_read !== 'manual'){
    call('set_unread', {id:selRow.id, unread:false}).then(() => { selRow.unread = false; bodies.delete(VIEWER); }).catch(() => {});
  }
}
// Another window changed a setting (the look, H): this one follows.
window.__TAURI__?.event?.listen('settings-changed', async ({payload: from}) => {
  if (from === ownLabel()) return; // this window's own change
  try { const got = await invoke('get_settings'); if (got?.settings){ SETTINGS = got; applySettings(); } } catch {}
});
// Closing a window with a message being written keeps it as a draft.
window.__TAURI__?.window?.getCurrentWindow?.().onCloseRequested?.(async () => { if (state.compose) await closeCompose(); });

async function boot(){
  I18n.translatePage();
  paintToolbar(); paintDensity();
  if (VIEWER && invoke){ await bootViewer(); return; }
  if (!invoke){
    previewEl.innerHTML = `<div class="pempty">${I.mail}<div>${t('This page needs the Rust core. Start Telex with {command}.', {command:'<code>npm run dev</code>'})}</div></div>`;
    return;
  }
  await bootSettings(); // first, so the first screen is in the right language and colors
  bootUpdates(); // then, so a version that can't load the mail can still be updated
  try { await loadTree(); } catch (e){ previewEl.innerHTML = `<div class="pempty">${esc(t('Couldn’t load mail: {error}', {error:tr(e)}))}</div>`; return; }
  paintScope();
  await setFolder(homeOf(ACCOUNTS[0]?.id));
  listEl.focus({preventScroll:true});
  sayRestored();
  loadBackups(); // whether the folders on this PC are backed up
  setTimeout(autoBackup, 60e3); setInterval(autoBackup, 60 * 60e3);
  setTimeout(runSchedules, 30e3); setInterval(runSchedules, 60e3); // overdue ones catch up once, after the first check
  // Real accounts: check now, then as often as the settings say, quietly.
  if (ACCOUNTS.length) getMail({quiet:true});
  else openAccountDialog(); // first start: straight to adding an account
  tellHelp();
}
boot();
})();
