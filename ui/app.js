// Telex UI. Plain JS, no build step: Tauri serves this folder as-is.
// All mail data comes from the Rust core (crates/telex-core) through `invoke`.
// Every word on screen goes through t() or tn() from i18n.js, in English here; i18n.js has Polish.
(() => {
'use strict';

const invoke = window.__TAURI__?.core?.invoke;
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
  flagFill: '<svg class="flag" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true"><path d="M3 15V2h10l-2.2 3.5L13 9H4.5v6z"/></svg>',
  clip: sv('<path d="M10.5 5L6 9.5a1.4 1.4 0 002 2L13 6.5a2.8 2.8 0 00-4-4L4 7.5a4.2 4.2 0 006 6l3.5-3.5"/>', 'class="clip"'),
  rows: sv('<path d="M2.5 4h11M2.5 8h11M2.5 12h11"/>'),
  sliders: sv('<path d="M2.5 4.5h6M11.5 4.5h2M2.5 11.5h2M7.5 11.5h6"/><circle cx="10" cy="4.5" r="1.5"/><circle cx="6" cy="11.5" r="1.5"/>'),
  search: sv('<circle cx="7" cy="7" r="4.2"/><path d="M10.2 10.2L14 14"/>'),
  caret: sv('<path d="M6 3.5L10.5 8 6 12.5"/>'),
  folder: sv('<path d="M2 4.5v8h12V5.8H8L6.5 3.5H2z"/>'),
  inbox: sv('<path d="M2 9h3.5l1 1.5h3l1-1.5H14"/><path d="M4 3.5h8L14 9v3.5H2V9z"/>'),
  drafts: sv('<path d="M4 2h5.5L12 4.5V14H4z"/><path d="M6 8h4M6 10.5h4"/>'),
  sent: sv('<path d="M14 2L2 7l5 2 2 5z"/><path d="M7 9l3-3"/>'),
  archive: sv('<rect x="2" y="3" width="12" height="3" rx=".6"/><path d="M3 6v7h10V6M6.5 8.5h3"/>'),
  spam: sv('<circle cx="8" cy="8" r="5.5"/><path d="M4.2 11.8l7.6-7.6"/>'),
  outbox: sv('<path d="M2 9h3.5l1 1.5h3l1-1.5H14"/><path d="M2 9v4.5h12V9"/><path d="M8 7.5V1.8M5.6 4.2L8 1.8l2.4 2.4"/>'),
  mail: sv('<rect x="1.5" y="3" width="13" height="10" rx="1.2"/><path d="M2 4l6 5 6-5"/>'),
  plus: sv('<path d="M8 3v10M3 8h10"/>'),
  rules: sv('<path d="M2 3h12l-4.5 5.5V13l-3 1.5V8.5z"/>'),
  sun: sv('<circle cx="8" cy="8" r="2.7"/><path d="M8 1.6v1.5M8 12.9v1.5M1.6 8h1.5M12.9 8h1.5M3.5 3.5l1.05 1.05M11.45 11.45l1.05 1.05M3.5 12.5l1.05-1.05M11.45 4.55l1.05-1.05"/>'),
  moon: sv('<path d="M13.2 9.6A5.6 5.6 0 016.4 2.8a5.6 5.6 0 106.8 6.8z"/>'),
  ul: sv('<path d="M6 4h8M6 8h8M6 12h8"/><circle cx="2.8" cy="4" r=".9" fill="currentColor" stroke="none"/><circle cx="2.8" cy="8" r=".9" fill="currentColor" stroke="none"/><circle cx="2.8" cy="12" r=".9" fill="currentColor" stroke="none"/>'),
  ol: sv('<path d="M6.5 4h7.5M6.5 8h7.5M6.5 12h7.5"/><path d="M2.4 2.9l1-.5v3.2M2.2 9.3c.2-.6 1.7-.7 1.7.3 0 .8-1.6 1.2-1.7 2h1.8" stroke-width="1.1"/>'),
  quote: sv('<path d="M3 3.5v9"/><path d="M6 5h7M6 8h7M6 11h5"/>'),
  link: sv('<path d="M6.8 9.2a2.6 2.6 0 003.7 0l2-2a2.6 2.6 0 00-3.7-3.7l-.8.8"/><path d="M9.2 6.8a2.6 2.6 0 00-3.7 0l-2 2a2.6 2.6 0 003.7 3.7l.8-.8"/>'),
  clear: sv('<path d="M5 3.5h7M8.5 3.5L7 12.5"/><path d="M3 3l10 10"/>'),
};
const KIND_ICON = {inbox:'inbox', outbox:'outbox', drafts:'drafts', sent:'sent', archive:'archive', spam:'spam', trash:'trash', custom:'folder'};
// Special folders have a name in the interface's language, whatever the server calls them.
const KIND_NAME = {inbox:N_('Inbox'), outbox:N_('Outbox'), drafts:N_('Drafts'), sent:N_('Sent'), archive:N_('Archive'), spam:N_('Spam'), trash:N_('Trash')};

/* ---------- data (filled from Rust) ---------- */
// The folder tree and its counts; the message list asks the core for pages (see "list").
let ACCOUNTS = [], ACCT = {}, COUNTS = {};
let USAGE = null; // the last answer of `usage`: quotas for the meter and the Usage tab
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
});
// Accounts, folders and how many messages each folder holds (the core counts them).
async function loadTree(){
  const t = await invoke('folder_tree');
  ACCOUNTS = t.accounts;
  ACCT = Object.fromEntries(ACCOUNTS.map(a => [a.id, a]));
  COUNTS = t.counts;
}
// The core's errors and notes come in the interface's language already (its tr! macro).
const tr = e => String(e);
// Commands that change mail; the core queues them for the server and we push shortly after.
// send_message counts: a sent message's copy for the server's Sent folder is queued too.
const CHANGES = new Set(['set_unread', 'set_flagged', 'move_message', 'delete_message', 'send_message', 'run_rules',
  'set_unread_many', 'set_flagged_many', 'move_messages', 'delete_messages', 'mark_folder_read']);
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
  if (!bodies.has(id)) bodies.set(id, await invoke('get_message', {id}));
  return bodies.get(id);
}

/* ---------- helpers ---------- */
const $ = id => document.getElementById(id);
const esc = s => String(s).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const pad = n => String(n).padStart(2, '0');
const DOW = [N_('Sun'),N_('Mon'),N_('Tue'),N_('Wed'),N_('Thu'),N_('Fri'),N_('Sat')];
const hm = d => pad(d.getHours()) + ':' + pad(d.getMinutes());
const dmy = d => pad(d.getDate()) + '.' + pad(d.getMonth() + 1) + '.' + d.getFullYear();
function fmtList(d){
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
const folderOf = key => { const [a, f] = key.split('/'); return ACCT[a]?.folders.find(x => x.id === f); };
const folderName = key => { const f = folderOf(key); return !f ? key : KIND_NAME[f.kind] ? t(KIND_NAME[f.kind]) : f.name; };
const kindOf = key => folderOf(key)?.kind;
const isOut = key => ['sent', 'drafts', 'outbox'].includes(kindOf(key));
const keyOfKind = (acct, kind) => { const f = ACCT[acct]?.folders.find(x => x.kind === kind); return f ? acct + '/' + f.id : null; };
const homeOf = acct => keyOfKind(acct, 'inbox') || (ACCT[acct]?.folders[0] ? acct + '/' + ACCT[acct].folders[0].id : '');
const countOf = key => COUNTS[key] ?? {total:0, unread:0};
const num = n => n.toLocaleString(I18n.lang() === 'pl' ? 'pl-PL' : 'en');
// Each account has a color, shown as a dot once there is more than one account.
const adot = acct => { const i = ACCOUNTS.findIndex(a => a.id === acct); return i < 0 ? '' : `<i class="adot a${i % 6}" aria-hidden="true"></i>`; };
// The window's own state: pane sizes, the filter's scope, when updates were last checked. Settings are elsewhere.
const store = {
  get(){ try { return JSON.parse(localStorage.getItem('telex-ui') || '{}'); } catch { return {}; } },
  set(v){ try { localStorage.setItem('telex-ui', JSON.stringify(v)); } catch {} },
};

/* ---------- state ---------- */
const saved = store.get();
const state = {
  folder:'', sel:{}, sort:{key:'date', dir:-1}, q:'', inText:false,
  scope: saved.scope === 'all' ? 'all' : 'folder', // the quick filter looks in this folder or in all mail
  more: saved.more && typeof saved.more === 'object' ? saved.more : {}, // accounts showing all their folders
  collapsed:{}, compose:null, images:new Set(), white:new Set(), // white: HTML shown on white
  undo:null, // what Z takes back: {kind:'move', items:[{id, from}]} after a delete or move; {kind:'send', id, until}
  picked:new Set(), anchor:null, span:0, // more than one selected message; the Shift selection's start and length
  thread:null, // {id, subject}: the list shows this message's conversation
  recent: Array.isArray(saved.recent) ? saved.recent.slice(0, 8) : [], // folders last moved to or opened with M and G
  updateChecked: Number(saved.updateChecked) || 0, // when a check last got an answer (ms)
  offered: saved.offered || null, // the language offered on the first start, once
};
const sdlg = $('set-dlg'); // the settings window, which also holds the accounts
const win = $('win'), rowsEl = $('rows'), listEl = $('list'), previewEl = $('preview'), treeEl = $('tree'), qEl = $('q');
if (saved.treeW) win.style.setProperty('--tree-w', saved.treeW + 'px');
if (saved.listH) win.style.setProperty('--list-h', saved.listH + '%');
const persist = () => store.set({
  treeW: parseInt(win.style.getPropertyValue('--tree-w')) || undefined,
  listH: parseFloat(win.style.getPropertyValue('--list-h')) || undefined,
  updateChecked: state.updateChecked || undefined,
  offered: state.offered || undefined,
  scope: state.scope === 'all' ? 'all' : undefined,
  more: Object.keys(state.more).length ? state.more : undefined,
  recent: state.recent.length ? state.recent : undefined,
});

/* ---------- settings ---------- */
// Theme, language, density, text size and the rest live in settings.json, through the core
// (get_settings, update_settings). The look is also kept for boot.js, which puts it on the
// page before the first paint.
const THEMES = ['system', 'light', 'dark', 'paper', 'heather', 'fjord', 'phosphor', 'contrast-light', 'contrast-dark'];
const THEME_NAMES = {system:N_('Match Windows'), light:N_('Light'), dark:N_('Dark'), paper:N_('Paper'), heather:N_('Heather'), fjord:N_('Fjord'),
  phosphor:N_('Phosphor'), 'contrast-light':N_('High contrast light'), 'contrast-dark':N_('High contrast dark')};
const LANG_NAMES = {en:'English', pl:'Polski'}; // each in itself, never translated
const TEXT_SIZES = [90, 100, 110, 125, 150, 175, 200];
const CHECK_EVERY = [5, 10, 15, 30, 60, 0];
const UNDO_SEND = [0, 5, 10, 20, 30];
const darkMode = matchMedia('(prefers-color-scheme: dark)'), forcedColors = matchMedia('(forced-colors: active)');
const shownTheme = id => id === 'system' ? (darkMode.matches ? 'dark' : 'light') : id;
// Themes on dark paper. HTML mail goes on the same paper there, unless the settings say white.
const DARK_THEMES = new Set(['dark', 'fjord', 'phosphor', 'contrast-dark']);
let SETTINGS = {
  settings:{theme:'system', language:'auto', density:'compact', text_size:100, prefer_text:false, mark_read:'moment', check_every:10,
    push_mail:true, notify:true, mail_paper:'theme', compose_format:'plain', reply_format:'same', undo_send:5, ask_subject:true,
    spell_check:true, updates:'ask'},
  language:I18n.lang(), offer:null, fresh:false,
};
const pref = () => SETTINGS.settings;
let zoomed = 100;
// The colors HTML mail gets on dark paper: the theme's own (themes.css), or null for white.
function darkPaper(id){
  const theme = document.documentElement.dataset.theme;
  if (!DARK_THEMES.has(theme) || pref().mail_paper !== 'theme' || state.white.has(id)) return null;
  const css = getComputedStyle(document.documentElement), v = name => css.getPropertyValue(name).trim();
  return {paper:v('--surface'), ink:v('--fg'), link:v('--accent'), line:v('--line'), muted:v('--muted')};
}
function applySettings(){
  const s = pref(), theme = document.documentElement.dataset.theme;
  document.documentElement.dataset.theme = shownTheme(s.theme);
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
  try { localStorage.setItem('telex-look', JSON.stringify({theme:s.theme, lang:SETTINGS.language, zoom:s.text_size})); } catch {}
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
  return true;
}
// After a language change: everything on screen again, in the new words.
function relabel(){
  I18n.translatePage();
  paintToolbar(); paintDensity(); paintScope();
  renderTree(); renderHead(); drawRows(); renderStatus();
  if (!helpPop.hidden) renderHelp();
  if (!state.compose) renderPreview(); else { readCompose(); renderCompose(); }
  if (rdlg.open){ if (!$('r-list-view').hidden) showRuleList(); else { renderConditions(); renderActions(); } }
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
  ['b-forward','forward',N_('Forward')],['b-flag','flag',N_('Flag')],['b-delete','trash',N_('Delete')],['b-rules','rules',N_('Rules')],['b-settings','sliders',N_('Settings')]];
function paintToolbar(){
  TOOLBAR.forEach(([id, ic, label]) => { $(id).innerHTML = I[ic] + `<span class="lbl">${esc(t(label))}</span>`; });
  $('search-ic').innerHTML = I.search;
}
function paintDensity(){
  win.dataset.density = pref().density;
  $('b-density').innerHTML = I.rows + `<span class="lbl">${esc(pref().density === 'compact' ? t('Compact') : t('Comfortable'))}</span>`;
}

/* ---------- status ---------- */
let stTimer, lastCheck = null, lastTitle = '';
function say(text, ms = 4500){
  $('st-msg').textContent = text;
  clearTimeout(stTimer);
  if (ms) stTimer = setTimeout(() => { $('st-msg').textContent = ''; }, ms);
}
// How many older messages the "download" button in the status bar asks for at a time.
const OLDER_BATCH = 1000;
function renderStatus(){
  const {total, unread, older = 0} = countOf(state.folder), found = list.total;
  const picked = state.picked.size > 1 ? ' · ' + esc(tn(state.picked.size, '{n} selected', '{n} selected', {n:num(state.picked.size)})) : '';
  const more = older && !allMail() && !state.q.trim()
    ? ` · ${esc(tn(older, '{n} older on the server', '{n} older on the server', {n:num(older)}))} <button type="button" class="linkbtn" id="st-older" title="${esc(t('The first check downloads the newest 300 messages of each folder.'))}">${esc(older > OLDER_BATCH ? t('Download {n}', {n:num(OLDER_BATCH)}) : t('Download'))}</button>` : '';
  $('st-folder').innerHTML = !ACCOUNTS.length ? esc(t('No account yet'))
    : state.thread ? esc(t('Conversation · {n}', {n:tn(found, '{n} message', '{n} messages', {n:num(found)})})) + picked
    : allMail() ? esc(t('All mail') + ' · ' + tn(found, '{n} match', '{n} matches', {n:num(found)})) + picked
    : `${adot(acctId(state.folder))}${esc(folderName(state.folder))} · ${esc(tn(total, '{n} message', '{n} messages', {n:num(total)}))}${unread ? ', ' + esc(tn(unread, '{n} unread', '{n} unread', {n:num(unread)})) : ''}${picked}${more}`;
  $('st-check').textContent = lastCheck ? t('Last check {time}', {time:hm(lastCheck)}) : '';
  renderMeter();
  const title = ACCOUNTS.length ? `${folderName(state.folder)} · ${ACCT[acctId(state.folder)]?.email ?? ''} – Telex` : 'Telex';
  if (title !== lastTitle){
    lastTitle = title;
    document.title = title;
    try { window.__TAURI__?.window?.getCurrentWindow().setTitle(title); } catch {}
  }
}

// Older mail the first download left on the server, a batch at a time.
$('st-folder').addEventListener('click', e => { if (e.target.closest('#st-older')) downloadOlder(); });
async function downloadOlder(){
  const folder = state.folder, b = $('st-older');
  if (!folder || b?.disabled) return;
  if (b) b.disabled = true;
  say(t('Downloading older messages…'), 0);
  let r;
  try { r = await invoke('download_older', {folder, limit:OLDER_BATCH}); }
  catch (e){ say(tr(e), 10000); if (b) b.disabled = false; return; }
  if (r.busy){ setTimeout(downloadOlder, 2000); return; } // a check is running
  await refresh();
  say(tn(r.new, '{n} older message downloaded.', '{n} older messages downloaded.', {n:num(r.new)}));
}

/* ---------- usage meter ---------- */
// The server's storage for the open folder's account, as a small bar in the status bar,
// when the server reports a quota. A nearly full account also says so in the folder tree.
const quotaOf = acct => USAGE?.accounts.find(u => u.account === acct)?.quota ?? null;
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
async function loadUsage(){
  try { USAGE = await invoke('usage'); } catch { return; }
  renderMeter(); renderTree();
}
$('st-usage').addEventListener('click', () => openSettings('use'));

/* ---------- tree ---------- */
// Each account shows its Inbox and Sent, Drafts and the Outbox when they hold something,
// and the open folder. The rest wait behind a quiet "more" line, remembered per account.
const MAIN_KINDS = new Set(['inbox', 'sent']);
const COUNT_ALL = new Set(['drafts', 'outbox']); // these count everything, not unread
function folderButton(a, f){
  const key = a.id + '/' + f.id, c = countOf(key), all = COUNT_ALL.has(f.kind);
  const n = all ? c.total : c.unread;
  const cnt = n ? `<span class="cnt${all ? ' dim' : ''}">${num(n)}</span>` : '';
  const on = key === state.folder;
  return `<li><button type="button" class="fold${on ? ' active' : ''}" data-folder="${key}"${on ? ' aria-current="true"' : ''}>${I[KIND_ICON[f.kind] || 'folder']}<span>${esc(folderName(key))}</span>${cnt}</button></li>`;
}
function renderTree(){
  win.classList.toggle('noacct', !ACCOUNTS.length); // the welcome takes the whole right side
  win.classList.toggle('multi', ACCOUNTS.length > 1);
  treeEl.innerHTML = ACCOUNTS.map(a => {
    const open = !state.collapsed[a.id];
    let items = '';
    if (open){
      const shown = f => MAIN_KINDS.has(f.kind) || a.id + '/' + f.id === state.folder || (COUNT_ALL.has(f.kind) && countOf(a.id + '/' + f.id).total > 0);
      const main = a.folders.filter(shown), rest = a.folders.filter(f => !shown(f)), more = !!state.more[a.id];
      items = '<ul>' + (more ? a.folders : main).map(f => folderButton(a, f)).join('');
      if (rest.length){
        const unread = more ? 0 : rest.reduce((n, f) => n + (COUNT_ALL.has(f.kind) ? 0 : countOf(a.id + '/' + f.id).unread), 0);
        items += `<li><button type="button" class="fold more" data-more="${esc(a.id)}" aria-expanded="${more}">${I.caret}<span>${esc(more ? t('Fewer folders') : tn(rest.length, '{n} more', '{n} more'))}</span>${unread ? `<span class="cnt">${num(unread)}</span>` : ''}</button></li>`;
      }
      items += '</ul>';
    }
    // A nearly full mailbox on the server says so next to its address.
    const q = quotaOf(a.id), pct = q ? pctOf(q) : 0;
    const full = pct >= 90 ? `<span class="qchip" data-level="${levelOf(pct)}" title="${esc(quotaText(q))}">${pct}%</span>` : '';
    return `<button type="button" class="acct" data-acct="${esc(a.id)}" aria-expanded="${open}" title="${esc(a.name)} · ${esc(a.email)}">${I.caret}${adot(a.id)}<span class="acct-mail">${esc(a.email)}</span>${full}</button>${items}`;
  }).join('') + `<button type="button" class="add-acct" data-act="add-account">${I.plus}<span>${esc(ACCOUNTS.length ? t('Add account') : t('Add your account'))}</span></button>`;
  $('folder-select').innerHTML = ACCOUNTS.map(a => `<optgroup label="${esc(a.email)}">` + a.folders.map(f => {
    const key = a.id + '/' + f.id, n = countOf(key).unread;
    return `<option value="${key}"${key === state.folder ? ' selected' : ''}>${esc(folderName(key))}${n ? ` (${n})` : ''}</option>`;
  }).join('') + '</optgroup>').join('');
}
treeEl.addEventListener('click', e => {
  const f = e.target.closest('[data-folder]'), a = e.target.closest('[data-acct]'), m = e.target.closest('[data-more]');
  if (e.target.closest('[data-act="add-account"]')) openAccountDialog();
  else if (f) setFolder(f.dataset.folder);
  else if (m){ state.more[m.dataset.more] = !state.more[m.dataset.more]; if (!state.more[m.dataset.more]) delete state.more[m.dataset.more]; persist(); renderTree(); }
  else if (a){ state.collapsed[a.dataset.acct] = !state.collapsed[a.dataset.acct]; renderTree(); }
});
$('folder-select').addEventListener('change', e => setFolder(e.target.value));

function setFolder(key){
  state.folder = key; state.q = ''; qEl.value = ''; state.inText = false; state.thread = null;
  unpick();
  paintScope(); renderTree(); renderStatus();
  return loadView({top:true});
}

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
const selKey = () => state.thread ? '#' : allMail() ? '*' : state.folder;
const viewArgs = () => ({folder:allMail() ? null : state.folder, sort:state.sort.key, ascending:state.sort.dir > 0,
  filter:state.q, bodies:state.inText, thread:state.thread?.id ?? null});
const current = () => selRow && selRow.id === state.sel[selKey()] && (allMail() || selRow.f === state.folder) ? selRow : null;
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
  return {total:p.total, start:p.start, rows:p.rows.map(fromSummary)};
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

function rowHtml(m, sel){
  // In all mail, Sent and Drafts say who they went to.
  const out = isOut(m.f), who = out ? (allMail() ? t('To: {who}', {who:nameOf(m.to)}) : nameOf(m.to)) : m.from, tip = out ? m.to : `${m.from} <${m.email}>`;
  const on = m.id === sel || state.picked.has(m.id);
  const cls = (m.unread ? 'unread ' : '') + (m.id === sel ? 'sel ' : '') + (state.picked.has(m.id) ? 'picked' : '');
  return `<tr data-id="${m.id}" class="${cls}" aria-selected="${on}">
<td class="c-ic"><span class="ic"><i class="dot"></i>${m.flagged ? I.flagFill.replace('aria-hidden="true"', `role="img" aria-label="${esc(t('Flagged'))}"`) : '<i class="sp"></i>'}${m.hasAtt ? I.clip : '<i class="sp"></i>'}</span></td>
<td class="c-from" title="${esc(tip)}">${esc(who)}</td>
<td class="c-subj" title="${esc(m.subj)}">${esc(m.subj)}</td>
<td class="c-where" title="${esc(ACCT[acctId(m.f)]?.email ?? '')} › ${esc(folderName(m.f))}">${adot(acctId(m.f))}${esc(folderName(m.f))}</td>
<td class="c-date">${fmtList(m.date)}</td>
<td class="c-size">${fmtSize(m.kb)}</td></tr>`;
}
// Rows that span the visible columns: more would add an empty column to the table.
const cols = () => [...listEl.querySelectorAll('thead th')].filter(th => th.offsetWidth > 0).length || 5;
const spacer = (px, n) => px > 0 ? `<tr class="pad" aria-hidden="true"><td colspan="${n}" style="height:${px}px"></td></tr>` : '';
// Draw the rows on screen and a margin; ask for the pages that are missing.
function drawRows(){
  if (!list.total){
    rowsEl.innerHTML = !ACCOUNTS.length || list.seq === 0 ? '' : `<tr class="empty"><td colspan="${cols()}">${state.q ? (allMail() ? t('No messages match “{q}” in any account. Press Esc to clear the filter.', {q:esc(state.q)}) : t('No messages match “{q}”. Press Esc to clear the filter.', {q:esc(state.q)})) : esc(t('This folder is empty.'))}</td></tr>`;
    return;
  }
  const [va, vb] = visibleRange(), a = Math.max(0, va - MARGIN), b = Math.min(list.total, vb + MARGIN);
  const s = list.start, e = s + list.rows.length, sel = state.sel[selKey()], n = cols();
  let html = spacer(a * rowH, n);
  for (let i = a; i < b; i++) html += i >= s && i < e ? rowHtml(list.rows[i - s], sel) : `<tr class="ph" aria-hidden="true"><td colspan="${n}"></td></tr>`;
  rowsEl.innerHTML = html + spacer((list.total - b) * rowH, n);
  const r = rowsEl.querySelector('tr[data-id]'), h = r?.getBoundingClientRect().height;
  if (h && Math.abs(h - rowH) > 0.01){ // the density changed: keep the same rows on screen
    listEl.scrollTop = listEl.scrollTop * h / rowH; rowH = h; drawRows(); return;
  }
  fill(a, b);
}
let drawQueued = false;
const drawSoon = () => { if (!drawQueued){ drawQueued = true; requestAnimationFrame(() => { drawQueued = false; drawRows(); }); } };
listEl.addEventListener('scroll', drawSoon);
new ResizeObserver(drawSoon).observe(listEl);

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
  const seq = ++list.seq, key = selKey();
  let id = state.sel[key], p;
  try { p = await fetchPage(id != null ? {around:id} : 'first'); }
  catch { try { p = await fetchPage('first'); } catch (e){ say(tr(e), 8000); return; } }
  if (!p || seq !== list.seq) return;
  merge(p);
  // Like before: a folder opens on its first message; a filter leaves the selection alone.
  if (!list.rows.some(r => r.id === id) && (!state.q.trim() || key === '*')) id = state.sel[key] = list.rows[0]?.id;
  selRow = list.rows.find(r => r.id === id) ?? (state.q.trim() ? selRow : null);
  renderHead();
  if (top) listEl.scrollTop = 0;
  drawRows();
  const pos = posOf(id);
  if (pos >= 0 && reveal(pos)) drawRows();
  renderStatus();
  if (!state.compose) renderPreview();
}
// After mail changed (a check, rules, sending, a push): new counts, and the open page again,
// keeping the first row on screen where it was. `skip`: a message that just went away.
async function refresh({skip} = {}){
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
  let p = null;
  try { p = await fetchPage(anchor ? {around:anchor.id} : 'first'); } catch {}
  if (!p || seq !== list.seq) return;
  const was = anchor ? posOf(anchor.id) : -1;
  merge(p);
  const now = anchor ? posOf(anchor.id) : -1;
  if (was >= 0 && now >= 0) listEl.scrollTop += (now - was) * rowH;
  if (!current()){ // the selection is gone: the first row, as when mail arrives in an empty folder
    selRow = list.rows.find(r => r.id === state.sel[selKey()]) ?? null;
    if (!selRow && !state.q.trim()){ selRow = list.rows[0] ?? null; state.sel[selKey()] = selRow?.id; }
  }
  drawRows(); renderStatus();
  if (selRow?.id !== before && !state.compose) renderPreview();
}

function renderHead(){
  const out = !allMail() && isOut(state.folder);
  $('from-label').textContent = out ? t('To') : t('From');
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
  for (const r of list.rows) if (r.id === id) Object.assign(r, fields);
  if (selRow?.id === id) Object.assign(selRow, fields);
}
rowsEl.addEventListener('click', e => {
  const tr = e.target.closest('tr[data-id]');
  if (!tr) return;
  const id = Number(tr.dataset.id);
  if (e.ctrlKey || e.metaKey) togglePick(id);
  else if (e.shiftKey) pickTo(id);
  else { unpick(); select(id); }
  listEl.focus({preventScroll:true});
});
rowsEl.addEventListener('dblclick', e => { if (e.target.closest('tr[data-id]') && allMail()) openInFolder(); });
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

// Mark as read after the message has been on screen for a moment, so arrowing past doesn't;
// or at once, or only with U, as the settings say.
let readTimer;
function select(id, silent){
  state.sel[selKey()] = id;
  selRow = list.rows.find(r => r.id === id) ?? (selRow?.id === id ? selRow : null);
  clearTimeout(readTimer);
  const m = current(), how = pref().mark_read;
  if (m && m.unread && !silent && how !== 'manual'){
    readTimer = setTimeout(() => {
      call('set_unread', {id:m.id, unread:false}).then(() => { changed(m, {unread:false}); reorder(); }).catch(() => {});
    }, how === 'at_once' ? 0 : 600);
  }
  paintSelection();
  if (!state.compose) renderPreview();
}
// A message's read or flag state changed here: its row and the counts.
function changed(m, fields){
  if ('unread' in fields && fields.unread !== m.unread){
    const c = COUNTS[m.f] ??= {total:0, unread:0};
    c.unread = Math.max(0, c.unread + (fields.unread ? 1 : -1));
  }
  patch(m.id, fields);
  renderTree(); renderStatus(); drawRows();
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
function bodyHtml(m){
  const loaded = state.images.has(m.id);
  return m.body.split('\n').map(line => {
    const img = line.match(/^\[img:(.+)\]$/);
    if (img) return `<div class="imgph${loaded ? ' loaded' : ''}">${loaded ? esc(img[1]) : t('Blocked image: {name}', {name:esc(img[1])})}</div>`;
    const q = line.startsWith('>');
    let h = esc(q ? line.replace(/^> ?/, '') : line);
    if (kindOf(m.folder) !== 'spam') h = h.replace(/(https?:\/\/[^\s<]+)/g, '<a href="$1">$1</a>');
    return `<div class="ln${q ? ' q' : ''}">${h}</div>`;
  }).join('');
}
// The HTML view of a message, cached per images setting and paper. load: true loads remote
// images, false never does (only asks whether there is HTML), null lets the core decide
// (trusted sender, not spam). dark: the theme's colors for dark paper, or null for white.
// The core does any downloading; the window never talks to the network.
const htmlViews = new Map();
async function htmlView(id, load, dark = null){
  const key = id + ':' + load + ':' + (dark ? Object.values(dark).join() : 'white');
  if (!htmlViews.has(key)) htmlViews.set(key, await invoke('get_html', {id, loadImages:load, dark}));
  return htmlViews.get(key);
}
// Links never navigate inside Telex: web links open in the browser, mailto opens compose.
function followLink(href, spam){
  const h = String(href || '').trim();
  if (spam){ say(t('Links are off in Spam. Move the message out of Spam to use them.')); return; }
  if (/^mailto:/i.test(h)){
    const to = decodeURIComponent(h.slice(7).split('?')[0]);
    openCompose('new', {to});
  } else if (/^https?:\/\//i.test(h)){
    call('open_link', {url:h}).then(() => say(t('Opened in your browser.'), 2500)).catch(() => {});
  }
}
// The frame shows the core's inert document: no scripts may run in it (sandbox without
// allow-scripts). allow-same-origin only lets this page size it and catch link clicks.
function mountFrame(frame, documentHtml, spam){
  frame.addEventListener('load', () => {
    const d = frame.contentDocument;
    if (!d) return;
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
  return `<div class="atts"${writing ? ' id="c-atts"' : ''}>${chips}${tail}</div>`;
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
const people = line => addresses(line).map(p => `<span class="pp">${person(p.name, p.email)}</span>`).join(', ');
// The message's header: the subject with its buttons (`controls`), who it is from (a click shows all
// their mail), when and how large, who it went to, and its attachments. One face throughout.
function headerHtml(m, controls){
  const sender = m.from_email
    ? `<button type="button" class="ph-who" data-act="sender" title="${esc(t('Show all mail from {email}', {email:m.from_email}))}">${person(m.from_name, m.from_email)}</button>`
    : `<span class="ph-who">${person(m.from_name, '')}</span>`;
  return `<header class="ph">
  <div class="ph-top"><h2 class="ph-subj">${esc(m.subject)}</h2>${controls}</div>
  <dl class="ph-grid">
    <dt>${esc(t('From'))}</dt><dd class="ph-from">${sender}<span class="ph-when" title="${esc(t('Date'))}">${fmtFull(parseDate(m.date))} · ${fmtSize(m.size_kb)}</span></dd>
    <dt>${esc(t('To'))}</dt><dd>${m.to ? people(m.to) : esc(t('(no recipient)'))}</dd>
    ${m.cc ? `<dt>${esc(t('Cc'))}</dt><dd>${people(m.cc)}</dd>` : ''}
    ${m.bcc ? `<dt>${esc(t('Bcc'))}</dt><dd>${people(m.bcc)}</dd>` : ''}
  </dl>
  ${attsHtml(m.attachments)}
</header>`;
}
let pvSeq = 0;
async function renderPreview(){
  const seq = ++pvSeq, s = current();
  if (!ACCOUNTS.length){
    previewEl.innerHTML = `<div class="welcome">${I.mail}<h2>${esc(t('Welcome to Telex'))}</h2>
<p>${esc(t('Add your mail account to start. Telex keeps your mail on this PC and talks only to your mail servers.'))}</p>
<button type="button" class="btn primary" data-act="add-account">${esc(t('Add your mail account'))}</button></div>`;
    return;
  }
  if (state.picked.size > 1){
    const keys = [['Del', N_('to Trash')], ['M', N_('move')], ['F', N_('flag|several')], ['U', N_('read or unread')], ['Esc', N_('clear the selection')]];
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
  previewEl.innerHTML = `<article>
${headerHtml(m, `${paper}${views}${s.flagged ? `<span class="pill">${I.flag}${esc(t('Flagged'))}</span>` : ''}`)}
${m.note ? `<div class="bar note${spam || kindOf(s.f) === 'outbox' ? ' warn' : ''}">${esc(m.note)}</div>` : ''}
${images}
${asHtml ? `<div class="pbody html${dark ? ' dark' : ''}"><iframe class="hframe" sandbox="allow-same-origin" referrerpolicy="no-referrer" title="${esc(t('Message'))}"></iframe></div>` : `<div class="pbody" lang="">${bodyHtml(m)}</div>`}
</article>`;
  if (asHtml) mountFrame(previewEl.querySelector('.hframe'), hv.document, spam);
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
  const att = e.target.closest('[data-att]'), save = e.target.closest('[data-save]'), act = e.target.closest('[data-act]');
  if (!state.compose && (att || save || e.target.closest('[data-saveall]'))){
    const m = current(); if (!m) return;
    if (att) openAttachment(m.id, Number(att.dataset.att));
    else if (save) saveAttachment(m.id, Number(save.dataset.save));
    else saveAttachments(m.id);
    return;
  }
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
  if (act.dataset.act === 'sender' && m){ filterAllMail((await full(m.id)).from_email); return; }
  if (act.dataset.act === 'paper' && m){
    if (state.white.has(m.id)) state.white.delete(m.id); else state.white.add(m.id);
    renderPreview();
  }
  if (act.dataset.act === 'bcc' && state.compose){
    readCompose(); state.compose.showBcc = true; renderCompose(); $('c-bcc')?.focus();
  }
  if (act.dataset.act === 'attach' && state.compose) pickFiles();
  if (act.dataset.act === 'discard'){ state.compose = null; closeSuggestions(); renderPreview(); say(t('Message discarded.')); listEl.focus(); }
  if (act.dataset.act === 'draft') closeCompose().then(ok => ok && say(t('Saved to Drafts.')));
  if (act.dataset.act === 'add-account') openAccountDialog();
});

/* ---------- compose ---------- */
// kind: new, reply, replyall or forward, started by the core with the identity's signature
// and templates; or edit, which reopens a draft or an unsent message in the Outbox.
// A message is plain text (`body`) or HTML (`html`, edited in place); Settings › Writing
// says which a new message, a reply and a forward start in, and Text | HTML switches.
// `of`: the message to edit when it isn't the selected one (one just taken back from sending).
async function openCompose(kind, preset = {}, of = null){
  if (!ACCOUNTS.length){ say(t('Add a mail account first.')); openAccountDialog(); return; }
  const s = of != null ? {id:of, f:(await full(of).catch(() => null))?.folder} : current();
  if (kind !== 'new' && !s?.f){ say(t('Select a message first.')); return; }
  if (kind === 'edit' && !['drafts', 'outbox'].includes(kindOf(s.f))){ say(t('Only drafts and unsent messages in the Outbox can be edited.')); return; }
  if (kind !== 'new' && state.picked.size > 1){ unpick(); paintSelection(); }
  loadRecipients();
  let c;
  try {
    if (kind === 'edit'){
      bodies.delete(s.id); // its note may have changed (taken back, refused)
      const m = await full(s.id), account = acctId(s.f), ids = ACCT[account]?.identities ?? [];
      const me = ids.find(i => i.email.toLowerCase() === m.from_email.toLowerCase()) ?? ids.find(i => i.is_default) ?? ids[0];
      // Attachments come from the old message itself, which Send replaces.
      c = {kind, account, identity:me?.id ?? null, to:m.to, cc:m.cc, bcc:m.bcc ?? '', subject:m.subject, body:m.body,
        html:await invoke('draft_html', {id:s.id}), attachments:m.attachments.slice(), replyTo:null, forwardOf:m.id, replaces:m.id};
    } else {
      const d = await invoke('start_draft', {kind:kind === 'replyall' ? 'reply_all' : kind, of:kind === 'new' ? null : s.id, account:acctId(state.folder)});
      c = {kind, account:d.account, identity:d.identity, to:d.to, cc:d.cc, bcc:'', subject:d.subject, body:d.body, html:null,
        attachments:d.attachments, replyTo:d.reply_to, forwardOf:d.forward_of, replaces:null};
      const p = pref();
      const html = kind === 'new' ? p.compose_format === 'html'
        : p.reply_format === 'same' ? !!(await htmlView(s.id, false)) : p.reply_format === 'html';
      if (html) c.html = await invoke('text_to_html', {text:c.body});
    }
  } catch (e){ say(tr(e), 8000); return; }
  Object.assign(c, preset); // e.g. {to} from a mailto: link
  state.compose = c;
  renderCompose();
}
const allIdentities = () => ACCOUNTS.flatMap(a => (a.identities || []).map(i => ({...i, account:a.id})));
const composeNote = c => !ACCT[c.account]?.smtp ? t('This account has no SMTP server yet: add one in Settings › Accounts to send.') : '';
function fromOptions(c){
  return ACCOUNTS.map(a => `<optgroup label="${esc(a.name)}">` + (a.identities || []).map(i =>
    `<option value="${i.id}"${i.id === c.identity ? ' selected' : ''}>${esc(i.name)} &lt;${esc(i.email)}&gt;</option>`).join('') + '</optgroup>').join('');
}
// Another identity: the account follows it, and its signature replaces the old one.
async function changeIdentity(id){
  const c = readCompose(), next = allIdentities().find(i => i.id === id);
  if (!next) return;
  const before = (allIdentities().find(i => i.id === c.identity)?.signature || '').trim(), after = (next.signature || '').trim();
  Object.assign(c, {identity:next.id, account:next.account});
  previewEl.querySelector('.cnote').textContent = composeNote(c);
  if (!before || before === after) return;
  if (c.html == null){
    if (c.body.includes(before)){ c.body = c.body.replace(before, after); $('c-body').value = c.body; }
    return;
  }
  // The signature went into the HTML line by line, the same way: swap those lines.
  const [b, a] = await Promise.all([invoke('text_to_html', {text:before}), after ? invoke('text_to_html', {text:after}) : '']);
  const ed = $('c-html');
  if (state.compose !== c || !ed || !ed.innerHTML.includes(b)) return;
  ed.innerHTML = ed.innerHTML.replace(b, a);
}
// The HTML editor's formatting bar: the browser's own editing commands.
const FORMATS = [['bold', '<b>B</b>', N_('Bold (Ctrl+B)')], ['italic', '<i>I</i>', N_('Italic (Ctrl+I)')], ['underline', '<u>U</u>', N_('Underline (Ctrl+U)')],
  ['strikeThrough', '<s>S</s>', N_('Strikethrough')], ['insertUnorderedList', I.ul, N_('Bulleted list')], ['insertOrderedList', I.ol, N_('Numbered list')],
  ['quote', I.quote, N_('Quote')], ['link', I.link, N_('Link (Ctrl+K)')], ['removeFormat', I.clear, N_('Clear formatting')]];
function renderCompose(){
  const c = state.compose, html = c.html != null, spell = String(pref().spell_check);
  const title = {new:t('New message'), reply:t('Reply|compose title'), replyall:t('Reply to all|compose title'), forward:t('Forward|compose title'), edit:t('Edit|compose title')}[c.kind];
  const note = composeNote(c);
  const bcc = !!c.bcc || c.showBcc;
  previewEl.innerHTML = `<form class="compose" id="cform" novalidate>
<div class="ctitle">${esc(title)}<label class="cfrom">${esc(t('from|compose'))} <select id="c-from" aria-label="${esc(t('From'))}">${fromOptions(c)}</select></label>
<span class="seg cfmt" role="group" aria-label="${esc(t('Format'))}"><button type="button" data-fmt="plain" aria-pressed="${!html}" title="${esc(t('Plain text'))}">${esc(t('Text|view'))}</button><button type="button" data-fmt="html" aria-pressed="${html}" title="${esc(t('HTML: bold, lists, links'))}">HTML</button></span></div>
<div class="cf">
  <label for="c-to">${esc(t('To'))}</label><input id="c-to" value="${esc(c.to)}" autocomplete="off" spellcheck="false" aria-autocomplete="list">
  <label for="c-cc">${esc(t('Cc'))}</label><div class="crow"><input id="c-cc" value="${esc(c.cc)}" autocomplete="off" spellcheck="false" aria-autocomplete="list">${bcc ? '' : `<button type="button" class="linkbtn cbcc" data-act="bcc" title="${esc(t('Blind copy: they get it, the others don’t see them'))}">${esc(t('Bcc'))}</button>`}</div>
  ${bcc ? `<label for="c-bcc">${esc(t('Bcc'))}</label><input id="c-bcc" value="${esc(c.bcc)}" autocomplete="off" spellcheck="false" aria-autocomplete="list">` : ''}
  <label for="c-subj">${esc(t('Subject'))}</label><input id="c-subj" value="${esc(c.subject)}" autocomplete="off" spellcheck="${spell}">
</div>
<div id="c-attbox">${attsHtml(c.attachments, true)}</div>
${html ? `<div class="fbar" role="toolbar" aria-label="${esc(t('Formatting'))}">${FORMATS.map(([cmd, label, tip]) =>
    `<button type="button" class="fb" data-cmd="${cmd}" title="${esc(t(tip))}" aria-label="${esc(t(tip))}">${label}</button>`).join('')}<span class="flink" hidden><input id="c-url" type="url" placeholder="https://" spellcheck="false" aria-label="${esc(t('Link address'))}"><small>${esc(t('Enter sets the link, Esc cancels'))}</small></span></div>
<div id="c-html" class="hedit" contenteditable="true" role="textbox" aria-multiline="true" aria-label="${esc(t('Message text'))}" spellcheck="${spell}"></div>`
  : `<textarea id="c-body" aria-label="${esc(t('Message text'))}" spellcheck="${spell}">
${esc(c.body)}</textarea>`}
<div class="cbar">
  <button class="btn primary" type="submit"><span>${esc(t('Send'))}</span> <kbd>Ctrl+Enter</kbd></button>
  <button class="btn" type="button" data-act="draft">${esc(t('Save draft'))} <kbd>Esc</kbd></button>
  <button class="btn" type="button" data-act="attach" title="${esc(t('Attach files from this PC. You can also drop them on the window.'))}">${I.clip}<span>${esc(t('Attach…'))}</span></button>
  <button class="btn" type="button" data-act="discard">${esc(t('Discard'))}</button>
  <span class="cnote">${esc(note)}</span>
</div></form>`;
  $('cform').addEventListener('submit', e => { e.preventDefault(); send(); });
  // Enter in a header field doesn't send: Ctrl+Enter does.
  $('cform').addEventListener('keydown', e => { if (e.key === 'Enter' && !e.ctrlKey && !e.metaKey && e.target.tagName === 'INPUT') e.preventDefault(); });
  $('c-from').addEventListener('change', e => changeIdentity(Number(e.target.value)));
  previewEl.querySelector('.cfmt').addEventListener('click', e => { const b = e.target.closest('[data-fmt]'); if (b) setFormat(b.dataset.fmt); });
  ['c-to', 'c-cc', 'c-bcc'].forEach(id => { if ($(id)) wireSuggestions($(id)); });
  const body = html ? $('c-html') : $('c-body');
  if (html){ body.innerHTML = c.html; wireEditor(body); }
  if (c.kind === 'new' || c.kind === 'forward' || (c.kind === 'edit' && !c.to)) $('c-to').focus();
  else focusStart(body);
}
/* ---------- recipient suggestions ---------- */
// People come from the mail base (recipients): those written to first, then those who write
// most. Typing in To, Cc or Bcc shows the ones whose name or address has a word starting with
// what was typed since the last comma; ↓ ↑ choose, Enter or Tab takes one, Esc closes.
let RECIPS = null, recipsAsked = null;
function loadRecipients(){
  if (!recipsAsked) recipsAsked = invoke('recipients').then(r => { RECIPS = r; }).catch(() => { recipsAsked = null; });
  return recipsAsked;
}
const forgetRecipients = () => { RECIPS = null; recipsAsked = null; };
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
function matchesTyped(p, typed){
  const words = fold(p.name + ' ' + p.email.replace(/[@.]/g, ' ')).split(/\s+/);
  return words.some(w => w.startsWith(typed)) || fold(p.email).startsWith(typed);
}
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
  const found = [];
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
    + `<span class="nm">${esc(p.name || p.email)}</span>${p.name ? `<span class="em">${esc(p.email)}</span>` : ''}</li>`).join('');
  input.setAttribute('aria-activedescendant', 'sugg-' + sugg.at);
}
function takeSuggestion(i){
  const input = sugg.input, p = sugg.list[i];
  if (!input || !p) return;
  const caret = input.selectionStart ?? input.value.length;
  const before = input.value.slice(0, caret), after = input.value.slice(caret).replace(/^[^,;]*[,;]?\s*/, '');
  const start = tokenStart(before);
  const head = before.slice(0, start).trimEnd();
  const value = (head ? head + ' ' : '') + addressOf(p) + ', ';
  input.value = value + after;
  input.setSelectionRange(value.length, value.length);
  closeSuggestions();
}
function wireSuggestions(input){
  input.addEventListener('input', () => { sugg.at = 0; loadRecipients().then(() => { if (document.activeElement === input) showSuggestions(input); }); });
  input.addEventListener('blur', () => setTimeout(() => { if (sugg.input === input) closeSuggestions(); }, 120));
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
  if (box && state.compose) box.innerHTML = attsHtml(state.compose.attachments, true);
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
  say(fresh.length === 1 ? t('Attached {name}.', {name:fresh[0].name}) : tn(fresh.length, 'Attached {n} file.', 'Attached {n} files.'), 3000);
}
async function pickFiles(){
  let files;
  try { files = await invoke('pick_files', {words:{title:t('Attach files')}}); } catch (e){ say(tr(e), 8000); return; }
  await addFiles(files);
}
window.__TAURI__?.event?.listen('files-dropped', ({payload}) => {
  if (document.querySelector('dialog[open]')) return; // not while a dialog is open
  addFiles(payload);
});
// The caret at the very start, above the quote, as in the plain-text editor.
function focusStart(body){
  body.focus();
  if (body.setSelectionRange) body.setSelectionRange(0, 0);
  else { const r = document.createRange(); r.setStart(body, 0); r.collapse(true); const sel = getSelection(); sel.removeAllRanges(); sel.addRange(r); }
  body.scrollTop = 0;
}
// The HTML editor: formatting buttons, Ctrl+K for a link, pasted HTML cleaned by the core.
function wireEditor(ed){
  document.execCommand('defaultParagraphSeparator', false, 'div');
  document.execCommand('styleWithCSS', false, false);
  const bar = previewEl.querySelector('.fbar'), box = bar.querySelector('.flink'), url = $('c-url');
  let kept = null; // the selection while the link box has the focus
  bar.addEventListener('mousedown', e => { if (e.target.closest('.fb')) e.preventDefault(); }); // keeps the selection
  bar.addEventListener('click', e => {
    const b = e.target.closest('[data-cmd]'); if (!b) return;
    ed.focus();
    if (b.dataset.cmd === 'link') openLinkBox();
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
  ed.addEventListener('keydown', e => {
    if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 'k'){ e.preventDefault(); openLinkBox(); }
  });
  // A pasted web page brings styles, scripts and images: only what Telex sends goes in.
  ed.addEventListener('paste', async e => {
    const pasted = e.clipboardData?.getData('text/html'), plain = e.clipboardData?.getData('text/plain') ?? '';
    if (!pasted) return;
    e.preventDefault();
    try { document.execCommand('insertHTML', false, await invoke('clean_html', {html:pasted})); }
    catch { document.execCommand('insertText', false, plain); }
  });
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
  attachments:c.attachments, reply_to:c.replyTo, forward_of:c.forwardOf, replaces:c.replaces, identity:c.identity});
const trimList = s => s.trim().replace(/[,;]\s*$/, '');
function readCompose(){
  const c = state.compose;
  c.to = trimList($('c-to').value); c.cc = trimList($('c-cc').value); c.subject = $('c-subj').value.trim();
  if ($('c-bcc')) c.bcc = trimList($('c-bcc').value);
  if (c.html != null) c.html = $('c-html').innerHTML; else c.body = $('c-body').value;
  return c;
}
// Saves the compose content as a draft and closes it. Resolves to true when it worked.
async function closeCompose(){
  const draft = draftOf(readCompose());
  try { await call('save_outgoing', {draft}); } catch { return false; }
  state.compose = null;
  closeSuggestions();
  renderPreview();
  await refresh();
  listEl.focus();
  return true;
}
const sentText = sent => sent.length === 1 ? t('Sent {what}.', {what:sentLine(sent[0])}) : sent.length ? tn(sent.length, 'Sent {n} message.', 'Sent {n} messages.') : '';
// Real mail goes to the Outbox and out at once. It's only called sent once the SMTP
// server has taken it; otherwise it waits in the Outbox and the status says why.
let askedSubject = 0; // when Send last asked about a missing subject
async function send(){
  if (![$('c-to'), $('c-cc'), $('c-bcc')].some(f => trimList(f?.value ?? ''))){ say(t('Add at least one recipient in To.')); $('c-to').focus(); return; }
  if (pref().ask_subject && !$('c-subj').value.trim() && Date.now() - askedSubject > 8000){
    askedSubject = Date.now();
    say(t('No subject. Press Send or Ctrl+Enter again to send it without one.'), 8000);
    $('c-subj').focus();
    return;
  }
  askedSubject = 0;
  const btn = document.querySelector('#cform .btn.primary');
  if (!btn || btn.disabled) return;
  const label = btn.querySelector('span');
  btn.disabled = true; label.textContent = t('Sending…');
  say(t('Sending…'), 0);
  let r;
  try { r = await call('send_message', {draft:draftOf(readCompose())}); }
  catch { btn.disabled = false; label.textContent = t('Send'); return; } // e.g. a bad address: fix it and try again
  state.compose = null;
  closeSuggestions();
  forgetRecipients(); // someone new may be in it
  renderPreview();
  await refresh();
  listEl.focus();
  if (r.wait){ // it waits a moment in the Outbox, so Z can still take it back
    state.undo = {kind:'send', id:r.queued, until:Date.now() + r.wait * 1000};
    say(t('Sending in {n} s. Press Z to undo.', {n:r.wait}), r.wait * 1000 + 500);
    return;
  }
  const sent = sentText(r.sent);
  if (r.errors.length) say([sent, r.errors.join(' · ')].filter(Boolean).join(' '), 15000);
  else say(sent || t('In the Outbox. It goes out with the next check.'), 6000);
}
// Mail that waited its moment has gone out (or not): what the SMTP server said.
window.__TAURI__?.event?.listen('outbox-sent', ({payload: r}) => {
  if (state.undo?.kind === 'send' && state.undo.id === r.queued) state.undo = null;
  refresh();
  const sent = sentText(r.sent);
  if (r.errors.length) say([sent, r.errors.join(' · ')].filter(Boolean).join(' '), 15000);
  else if (sent) say(sent, 6000);
});
// Z within the moment: the message stays in the Outbox, held, and opens again to be changed.
async function unsend(u){
  state.undo = null;
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
  const m = current(); if (!m) return;
  const flagged = !m.flagged;
  changed(m, {flagged}); if (!state.compose) renderPreview();
  call('set_flagged', {id:m.id, flagged})
    .then(() => { say(flagged ? t('Flagged.') : t('Flag removed.'), 2000); reorder(); })
    .catch(() => { changed(m, {flagged:!flagged}); if (!state.compose) renderPreview(); });
}
function toggleUnread(){
  clearTimeout(readTimer);
  if (state.picked.size > 1){
    // Any unread among them: all read. All read: all unread.
    const rows = pickedRows(), unread = rows.length > 0 && !rows.some(r => r.unread), n = state.picked.size;
    call('set_unread_many', {ids:chosen(), unread}).then(() => {
      say(unread ? tn(n, 'Marked {n} message as unread.', 'Marked {n} messages as unread.', {n:num(n)}) : tn(n, 'Marked {n} message as read.', 'Marked {n} messages as read.', {n:num(n)}), 3000);
      refresh();
    }).catch(() => {});
    return;
  }
  const m = current(); if (!m) return;
  const unread = !m.unread;
  call('set_unread', {id:m.id, unread}).then(() => {
    changed(m, {unread}); reorder();
    say(unread ? t('Marked as unread.') : t('Marked as read.'), 2000);
  }).catch(() => {});
}
// Shift+U: every message in the folder (or in the list, in all mail) read.
async function markAllRead(){
  if (!list.total) return;
  let n;
  try {
    if (allMail()) n = await call('set_unread_many', {ids:await invoke('list_ids', {view:viewArgs(), from:null, to:null}), unread:false});
    else n = await call('mark_folder_read', {folder:state.folder});
  } catch { return; }
  await refresh();
  say(n ? tn(n, 'Marked {n} message as read.', 'Marked {n} messages as read.', {n:num(n)}) : t('Everything here was read already.'), 3000);
}
// Deleting from Trash can't be undone (on the server either), so it takes a second Del.
let armedDelete = null; // {key, at}
async function del(){
  const ids = chosen(); if (!ids.length) return;
  const many = state.picked.size > 1, m = current();
  const inTrash = many ? !allMail() && kindOf(state.folder) === 'trash' : m && kindOf(m.f) === 'trash';
  if (inTrash){
    const key = ids.join(',');
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
  state.undo = r.trashed.length ? {kind:'move', items:r.trashed.map(([id, from]) => ({id, from}))} : null;
  const n = ids.length;
  say(r.trashed.length ? (n === 1 ? t('Moved to Trash. Press Z to undo.') : tn(n, 'Moved {n} message to Trash. Press Z to undo.', 'Moved {n} messages to Trash. Press Z to undo.', {n:num(n)}))
    : n === 1 ? t('Deleted for good.') : tn(n, 'Deleted {n} message for good.', 'Deleted {n} messages for good.', {n:num(n)}));
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
  state.undo = {kind:'move', items:places.filter(([, from]) => from !== folder).map(([id, from]) => ({id, from}))};
  const n = ids.length, where = folderName(folder);
  say(n === 1 ? t('Moved to {folder}. Press Z to undo.', {folder:where}) : tn(n, 'Moved {n} message to {folder}. Press Z to undo.', 'Moved {n} messages to {folder}. Press Z to undo.', {n:num(n), folder:where}));
  unpick();
  if (next){ state.sel[selKey()] = next.id; selRow = next; }
  await refresh({skip:n === 1 ? ids[0] : undefined});
  if (!state.compose) renderPreview();
}
// Z: put back what was deleted or moved last, or take back mail still waiting to be sent.
async function undo(){
  const u = state.undo; if (!u) return;
  if (u.kind === 'send'){ await unsend(u); return; }
  const groups = new Map();
  for (const it of u.items){ if (!groups.has(it.from)) groups.set(it.from, []); groups.get(it.from).push(it.id); }
  try { for (const [folder, ids] of groups) await call('move_messages', {ids, folder}); } catch { return; }
  state.undo = null;
  const first = u.items[0];
  if (first && (allMail() || state.folder === first.from)){ state.sel[selKey()] = first.id; selRow = null; }
  await loadTree().catch(() => {});
  renderTree();
  await loadView();
  say(u.items.length === 1 ? t('Restored to {folder}.', {folder:folderName(first.from)}) : tn(u.items.length, 'Restored {n} message.', 'Restored {n} messages.', {n:num(u.items.length)}));
}
/* ---------- folder picker: M moves, G goes ---------- */
// Type part of a folder's name (diacritics don't matter), ↑ ↓ to choose, Enter. Folders used
// last come first. M lists the folders of the selected messages' account; G every folder.
const pdlg = $('pick-dlg');
let pick = null; // {mode, items:[{key, name, acct}], shown:[], at}
function openPicker(mode){
  if (state.compose || !ACCOUNTS.length) return;
  let accts = ACCOUNTS.map(a => a.id), skip = null;
  if (mode === 'move'){
    const ids = chosen();
    if (!ids.length){ say(t('Select a message first.')); return; }
    const rows = state.picked.size > 1 ? pickedRows() : [current()];
    const from = [...new Set(rows.map(r => acctId(r.f)))];
    if (from.length > 1){ say(t('Messages from different accounts can’t be moved together.')); return; }
    accts = from;
    const folders = new Set(rows.map(r => r.f));
    if (folders.size === 1) skip = [...folders][0];
  }
  const items = accts.flatMap(a => (ACCT[a]?.folders ?? [])
    .filter(f => mode !== 'move' || f.kind !== 'outbox')
    .map(f => ({key:a + '/' + f.id, name:folderName(a + '/' + f.id), acct:a}))
    .filter(it => it.key !== skip));
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
      : fold(ACCT[it.acct]?.email ?? '').startsWith(q) ? 80 + i / 1000 : -1;
    return {it, r};
  }).filter(x => x.r >= 0).sort((a, b) => a.r - b.r);
  pick.shown = ranked.map(x => x.it);
  pick.at = Math.min(pick.at, Math.max(0, pick.shown.length - 1));
  const many = new Set(pick.items.map(it => it.acct)).size > 1; // say whose folder only when it isn't obvious
  $('pk-list').innerHTML = pick.shown.length ? pick.shown.map((it, i) => {
    const f = folderOf(it.key);
    return `<li role="option" id="pk-${i}" data-i="${i}" aria-selected="${i === pick.at}">${I[KIND_ICON[f?.kind] || 'folder']}${many ? adot(it.acct) : ''}<span class="nm">${esc(it.name)}</span>${many ? `<span class="em">${esc(ACCT[it.acct]?.email ?? '')}</span>` : ''}</li>`;
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
  const n = ACCOUNTS.filter(a => a.imap).length;
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
    await refresh();
    loadUsage(); // the quota and how the sync went came with the check
    const ids = r.new;
    let text = '';
    if (first) text = tn(ids.length, '{n} message downloaded.', '{n} messages downloaded.');
    else if (ids.length){
      const where = r.new_in && folderOf(r.new_in) ? `${ACCT[acctId(r.new_in)].email} › ${folderName(r.new_in)}` : '';
      text = where ? tn(ids.length, '{n} new message in {where}.', '{n} new messages in {where}.', {where})
        : tn(ids.length, '{n} new message.', '{n} new messages.');
    } else if (!quiet) text = t('No new messages.');
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
window.__TAURI__?.event?.listen('inbox-changed', ({payload: account}) => {
  clearTimeout(inboxTimer);
  inboxTimer = setTimeout(() => getMail({quiet:true, account}), 800);
});

// Progress while a folder downloads; each finished folder shows up right away.
let reloadTimer;
window.__TAURI__?.event?.listen('sync-progress', ({payload: p}) => {
  say(t('{account} › {folder}: {done} of {total}', p), 0);
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
  state.q = text; state.inText = false;
  if (state.thread && text.trim()) state.thread = null; // the filter looks in the folder or all mail again
  unpick();
  paintScope(); renderHead();
  clearTimeout(searchTimer);
  loadView();
  if (state.q.trim()) searchTimer = setTimeout(() => { state.inText = true; loadView(); }, 150);
}
qEl.addEventListener('input', () => setFilter(qEl.value));
function paintScope(){
  const all = state.scope === 'all';
  win.classList.toggle('allmail', allMail());
  $('q').closest('.search').classList.toggle('active', !!state.q);
  $('q-scope').textContent = all ? t('All mail') : t('This folder');
  $('q-scope').setAttribute('aria-pressed', String(all));
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
const ACCOUNT_VIEWS = {home:'a-home', start:'w-start', work:'w-work', form:'acct-form', ids:'i-list-view', idform:'i-form'};
function showAccountView(view){
  accountView = view;
  for (const [v, id] of Object.entries(ACCOUNT_VIEWS)) $(id).hidden = v !== view;
  if (view === 'home') renderAccountList();
  sdlg.querySelector('.setpanes').scrollTop = 0;
}

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
  const real = ACCOUNTS.filter(a => a.imap), usage = Object.fromEntries((USAGE?.accounts ?? []).map(u => [u.account, u]));
  $('a-list').innerHTML = real.length ? real.map(a => {
    const where = `${a.email} · ${a.imap.host}${a.smtp ? '' : ' · ' + t('no SMTP server yet')}`, u = usage[a.id];
    const sync = u?.sync_error ? `<div class="st warn" title="${esc(u.sync_error)}">${esc(t('Last check failed: {reason}', {reason:u.sync_error}))}</div>`
      : u?.synced_at ? `<div class="st">${esc(t('Checked {when}', {when:fmtWhen(parseDate(u.synced_at))}))}</div>` : '';
    return `<div class="sacct">${adot(a.id)}<div class="satext"><div class="nm">${esc(a.name)}</div><div class="em" title="${esc(where)}">${esc(where)}</div>${sync}</div>`
      + `<button type="button" class="btn" data-ids="${esc(a.id)}">${esc(t('Identities'))}</button><button type="button" class="btn" data-edit="${esc(a.id)}">${esc(t('Edit'))}</button><button type="button" class="btn danger" data-remove="${esc(a.id)}">${esc(t('Remove'))}</button></div>`;
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
window.__TAURI__?.event?.listen('detect-progress', ({payload}) => {
  if (wizardBusy !== 'find' || !steps.length) return;
  const domain = $('w-email').value.trim().split('@')[1] || '';
  steps[steps.length - 1].detail = payload === 'guess' ? t('Trying the usual server names…') : t('Asking {domain} and its DNS records…', {domain});
  renderSteps();
});
const serverNames = s => s.host === s.smtp_host ? s.host : t('{a} and {b}', {a:s.host, b:s.smtp_host});

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
  $('a-host').value = v.host ?? ''; $('a-port').value = v.port ?? 993; $('a-isec').value = v.imap_security ?? 'tls';
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
  $('a-isec').value = a.imap.security ?? 'tls';
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
  if (badPort(port) || !port){ aErr.textContent = t('The IMAP port is a number from 1 to 65535: usually 993 for SSL/TLS, 143 for STARTTLS.'); $('a-port').focus(); return; }
  if (badPort(smtpPort)){ aErr.textContent = t('The SMTP port is a number from 1 to 65535: usually 465 for SSL/TLS, 587 for STARTTLS.'); $('a-sport').focus(); return; }
  const setup = {name:$('a-name').value, email:$('a-email').value, host:$('a-host').value, port, username:$('a-user').value,
    imap_security:$('a-isec').value, smtp_host:$('a-smtp').value, smtp_port:smtpPort, smtp_security:$('a-sec').value};
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
  const ed = e.target.closest('[data-edit]');
  if (ed){ editAccount(ed.dataset.edit); return; }
  const ids = e.target.closest('[data-ids]');
  if (ids){ openIdentities(ids.dataset.ids); return; }
  const b = e.target.closest('[data-remove]');
  if (!b) return;
  if (b.dataset.armed !== '1'){ b.dataset.armed = '1'; b.textContent = t('Remove it and its local mail'); return; }
  try { await invoke('remove_account', {id:b.dataset.remove}); } catch (err){ $('a-list-err').textContent = tr(err); return; }
  await refresh();
  await loadUsage();
  renderAccountList();
  say(t('Account removed from Telex. The mail on the server is untouched.'));
});

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
    reply_header:base.reply_header ?? '', forward_header:base.forward_header ?? '', is_default:false};
  showAccountView('idform');
  $('i-form-err').textContent = '';
  $('i-form-title').textContent = existing ? t('Edit identity') : t('New identity');
  $('i-name').value = identity.name; $('i-email').value = identity.email; $('i-sig').value = identity.signature;
  $('i-reply').value = identity.reply_header; $('i-fwd').value = identity.forward_header;
  $('i-default').checked = identity.is_default;
  $('i-default').disabled = existing?.is_default === true; // switch by making another one the default
  $('i-name').focus();
}
$('i-form').addEventListener('submit', async e => {
  e.preventDefault();
  Object.assign(identity, {name:$('i-name').value, email:$('i-email').value, signature:$('i-sig').value,
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
$('i-new').addEventListener('click', () => showIdentityForm(null));
$('i-back').addEventListener('click', showIdentityList);
$('i-close').addEventListener('click', () => showAccountView('home'));

/* ---------- rules ---------- */
const rdlg = $('rules-dlg');
let RULES = [], rule = null; // rule: the one being edited
// Labels are English keys, translated where they are drawn.
const FIELDS = [['from', N_('From')], ['to', N_('To or Cc')], ['subject', N_('Subject')], ['body', N_('Text|rule field')], ['anywhere', N_('Anywhere')]];
const OPS = [['contains', N_('contains')], ['not_contains', N_('doesn’t contain')], ['is', N_('is')], ['starts_with', N_('starts with')], ['ends_with', N_('ends with')]];
const ACTIONS = [['move', N_('Move to folder')], ['mark_read', N_('Mark as read')], ['flag', N_('Flag|rule action')], ['delete', N_('Move to Trash')], ['auto_reply', N_('Answer automatically')]];
const labelOf = (list, v) => { const l = list.find(([k]) => k === v)?.[1]; return l ? t(l) : v; };
const options = (list, v) => list.map(([k, l]) => `<option value="${esc(k)}"${k === v ? ' selected' : ''}>${esc(t(l))}</option>`).join('');
// Folders a rule can move mail to: the account's own, not its Outbox. Their names are the server's.
const moveTargets = acct => (ACCT[acct]?.folders ?? []).filter(f => f.kind !== 'outbox').map(f => [acct + '/' + f.id, f.name]);
const folderOptions = (list, v) => list.map(([k, l]) => `<option value="${esc(k)}"${k === v ? ' selected' : ''}>${esc(l)}</option>`).join('');

function ruleSummary(r){
  const conds = r.conditions.map(c => t('{field} {op} “{value}”', {field:labelOf(FIELDS, c.field), op:labelOf(OPS, c.op), value:c.value}))
    .join(r.match_all ? t(' and ') : t(' or '));
  const acts = r.actions.map(a => a.type === 'move' ? t('move to {folder}', {folder:folderName(a.folder)}) : labelOf(ACTIONS, a.type).toLowerCase()).join(', ');
  return t('If {conds}: {acts}.', {conds, acts}) + (r.account ? ' ' + t('{name} only.', {name:ACCT[r.account]?.email ?? r.account}) : '') + (r.stop ? ' ' + t('Then stop.') : '');
}
async function openRules(){
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
<div class="rtext"><div class="rname">${esc(r.name)}${r.enabled ? '' : ` <span class="tag">${esc(t('Off|rule'))}</span>`}</div><div class="rsum" title="${esc(ruleSummary(r))}">${esc(ruleSummary(r))}</div></div>
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
  return {id:0, name:out ? nameOf(m.to) : (m.from || m.email), enabled:true, account:acctId(m.f), match_all:true,
    conditions:[out ? {field:'to', op:'is', value:to} : {field:'from', op:'is', value:m.email}],
    actions:[kindOf(m.f) === 'custom' ? {type:'move', folder:m.f} : {type:'flag'}], stop:false};
}
function showRuleForm(existing, start = null){
  const firstAccount = ACCOUNTS.find(a => a.imap)?.id ?? ACCOUNTS[0]?.id ?? null;
  rule = existing ? structuredClone(existing) : start ?? {id:0, name:'', enabled:true, account:firstAccount, match_all:true,
    conditions:[{field:'from', op:'contains', value:''}], actions:[{type:'flag'}], stop:false};
  $('r-list-view').hidden = true; $('r-form').hidden = false; $('r-form-err').textContent = '';
  $('r-form-title').textContent = existing ? t('Edit rule') : t('New rule');
  $('r-name').value = rule.name;
  $('r-acct').innerHTML = `<option value="">${esc(t('All accounts'))}</option>` + ACCOUNTS.map(a => `<option value="${esc(a.id)}"${a.id === rule.account ? ' selected' : ''}>${esc(a.name)} · ${esc(a.email)}</option>`).join('');
  $('r-match').value = rule.match_all ? 'all' : 'any';
  $('r-stop').checked = rule.stop;
  renderConditions(); renderActions();
  $('r-name').focus();
}
function renderConditions(){
  $('r-conds').innerHTML = rule.conditions.map((c, i) => `<div class="rrow" data-i="${i}">
<select data-k="field" aria-label="${esc(t('Where'))}">${options(FIELDS, c.field)}</select><select data-k="op" aria-label="${esc(t('How'))}">${options(OPS, c.op)}</select>
<input data-k="value" value="${esc(c.value)}" placeholder="${esc(t('text to look for'))}" autocomplete="off" spellcheck="false" aria-label="${esc(t('Text to look for'))}">
<button type="button" class="btn icon" data-x="${i}" title="${esc(t('Remove this condition'))}" aria-label="${esc(t('Remove condition'))}">×</button></div>`).join('');
}
function renderActions(){
  $('r-acts').innerHTML = rule.actions.map((a, i) => {
    let param = '<span class="none"></span>';
    if (a.type === 'move'){
      const targets = rule.account ? moveTargets(rule.account) : [];
      param = targets.length ? `<select data-k="folder" aria-label="${esc(t('Folder'))}">${folderOptions(targets, a.folder)}</select>`
        : `<span class="none">${esc(t('Pick the rule’s account to choose a folder.'))}</span>`;
    } else if (a.type === 'auto_reply'){
      param = `<textarea data-k="body" aria-label="${esc(t('Reply text'))}" placeholder="${esc(t('Hello, I’m away until 20 October.'))}">${esc(a.body || '')}</textarea>`;
    }
    return `<div class="rrow act" data-i="${i}"><select data-k="type" aria-label="${esc(t('Action'))}">${options(ACTIONS, a.type)}</select>${param}
<button type="button" class="btn icon" data-x="${i}" title="${esc(t('Remove this action'))}" aria-label="${esc(t('Remove action'))}">×</button></div>`;
  }).join('');
}
// A new or changed action gets sensible defaults: a folder to move to, empty reply text.
function fixAction(a){
  if (a.type === 'move'){
    const targets = rule.account ? moveTargets(rule.account) : [];
    if (!targets.some(([k]) => k === a.folder)) a.folder = (targets.find(([k]) => folderOf(k)?.kind === 'custom') ?? targets[0])?.[0] ?? '';
  } else delete a.folder;
  if (a.type === 'auto_reply') a.body = a.body ?? ''; else delete a.body;
  return a;
}
$('r-conds').addEventListener('input', e => {
  const row = e.target.closest('[data-i]'); if (!row) return;
  rule.conditions[row.dataset.i][e.target.dataset.k] = e.target.value;
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
$('r-acct').addEventListener('change', () => { rule.account = $('r-acct').value || null; rule.actions.forEach(fixAction); renderActions(); });
$('r-form').addEventListener('submit', async e => {
  e.preventDefault();
  Object.assign(rule, {name:$('r-name').value, match_all:$('r-match').value === 'all', stop:$('r-stop').checked});
  try { await invoke('save_rule', {rule}); } catch (err){ $('r-form-err').textContent = tr(err); return; }
  say(t('Rule “{name}” saved. It runs on new mail as it arrives.', {name:rule.name.trim()}));
  await showRuleList();
});
$('r-back').addEventListener('click', showRuleList);
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
  $('s-themes').innerHTML = themeCards();
  // The level comes from the theme itself (--wcag in themes.css), so it can't drift.
  $('s-themes').querySelectorAll('.tlvl').forEach(el => {
    const lvl = getComputedStyle(el.closest('.tcard').querySelector('.mini')).getPropertyValue('--wcag').trim() || 'AA';
    el.textContent = 'WCAG ' + lvl;
    el.title = t('Meets WCAG 2.2 {level} contrast', {level:lvl});
  });
  $('s-forced').hidden = !forcedColors.matches;
  $('s-density').innerHTML = segs('density', [['compact', esc(t('Compact'))], ['comfortable', esc(t('Comfortable'))]], s.density);
  $('s-size').innerHTML = segs('text_size', TEXT_SIZES.map(n => [n, n + '%']), s.text_size);
  const auto = LANG_NAMES[SETTINGS.settings.language === 'auto' ? SETTINGS.language : ''] ?? '';
  $('s-langs').innerHTML = radios('language', [
    ['auto', esc(t('Automatic, as in Windows')), auto ? `<small class="sdim">${esc(auto)}</small>` : ''],
    ['en', '<span lang="en">English</span>'], ['pl', '<span lang="pl">Polski</span>'],
  ], s.language);
  $('s-every').innerHTML = CHECK_EVERY.map(n => `<option value="${n}"${n === s.check_every ? ' selected' : ''}>${esc(
    n === 0 ? t('Only when I click Get mail') : n === 60 ? t('Every hour') : tn(n, 'Every {n} minute', 'Every {n} minutes'))}</option>`).join('');
  $('s-read').innerHTML = radios('mark_read', [['moment', esc(t('After a moment on screen'))], ['at_once', esc(t('As soon as I select them'))], ['manual', esc(t('Only when I press U'))]], s.mark_read);
  $('s-view').innerHTML = radios('prefer_text', [['html', esc(t('Show the HTML'))], ['text', esc(t('Show the plain text'))]], s.prefer_text ? 'text' : 'html');
  $('s-paper').innerHTML = radios('mail_paper', [['theme', esc(t('On the theme’s dark paper'))], ['white', esc(t('On white, as sent'))]], s.mail_paper);
  $('s-format').innerHTML = radios('compose_format', [['plain', esc(t('Plain text'))], ['html', esc(t('HTML'))]], s.compose_format);
  $('s-reply').innerHTML = radios('reply_format', [['same', esc(t('In the format of the message'))], ['plain', esc(t('Always plain text'))], ['html', esc(t('Always HTML'))]], s.reply_format);
  $('s-subject').checked = s.ask_subject; $('s-spell').checked = s.spell_check;
  $('s-push').checked = s.push_mail; $('s-notify').checked = s.notify;
  $('s-undo').innerHTML = UNDO_SEND.map(n => `<option value="${n}"${n === s.undo_send ? ' selected' : ''}>${esc(undoText(n))}</option>`).join('');
  const can = !!upd.build?.updatable;
  $('s-updates').checked = s.updates === 'daily'; $('s-updates').disabled = !can;
  $('s-upd-none').hidden = can; $('s-check').hidden = !can;
  renderMove();
  paneOpened(setTab);
  if (again) sdlg.querySelector(again)?.focus();
}
// What a tab draws when it opens: the lists that change while Telex runs.
function paneOpened(tab){
  if (tab === 'accts'){
    if (accountView === 'home') renderAccountList();
    else if (accountView === 'ids') showIdentityList();
    else if (accountView === 'start') renderLangPick();
  }
  if (tab === 'read') renderTrusted();
  if (tab === 'use') renderUsage();
  if (tab === 'move') renderMove();
}
// After a change that keeps the words: only the ticks move, so focus stays where it was.
function syncSettings(){
  const s = pref();
  const want = {theme:s.theme, density:s.density, text_size:String(s.text_size), language:s.language, mark_read:s.mark_read, prefer_text:s.prefer_text ? 'text' : 'html',
    mail_paper:s.mail_paper, compose_format:s.compose_format, reply_format:s.reply_format};
  sdlg.querySelectorAll('input[type="radio"]').forEach(r => { if (r.name in want) r.checked = r.value === want[r.name]; });
  $('s-every').value = String(s.check_every);
  $('s-updates').checked = s.updates === 'daily';
  $('s-subject').checked = s.ask_subject; $('s-spell').checked = s.spell_check;
  $('s-push').checked = s.push_mail; $('s-notify').checked = s.notify;
  $('s-undo').value = String(s.undo_send);
}
// How long sent mail waits so it can be taken back.
const undoText = n => n === 0 ? t('Off: send at once') : tn(n, '{n} second', '{n} seconds');
function openSettings(tab){
  if (tab) setTab = tab;
  move_ = {plan:null, choice:null, report:null, exported:move_.exported, error:'', counts:null};
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
  else if (el.name === 'density') changeSettings({density:v});
  else if (el.name === 'text_size') changeSettings({text_size:Number(v)});
  else if (el.name === 'language') changeSettings({language:v});
  else if (el.name === 'mark_read') changeSettings({mark_read:v});
  else if (el.name === 'prefer_text') changeSettings({prefer_text:v === 'text'});
  else if (el.name === 'mail_paper'){ state.white.clear(); changeSettings({mail_paper:v}).then(() => { if (!state.compose) renderPreview(); }); }
  else if (el.name === 'compose_format') changeSettings({compose_format:v});
  else if (el.name === 'reply_format') changeSettings({reply_format:v});
  else if (el.id === 's-subject') changeSettings({ask_subject:el.checked});
  else if (el.id === 's-spell') changeSettings({spell_check:el.checked});
  else if (el.id === 's-every') changeSettings({check_every:Number(v)});
  else if (el.id === 's-push') changeSettings({push_mail:el.checked});
  else if (el.id === 's-notify') changeSettings({notify:el.checked});
  else if (el.id === 's-undo') changeSettings({undo_send:Number(v)});
  else if (el.id === 's-updates') changeSettings({updates:el.checked ? 'daily' : 'off'});
  else if (el.dataset.part) move_.choice[el.dataset.part] = el.checked;
});
forcedColors.addEventListener('change', () => { if (sdlg.open) $('s-forced').hidden = !forcedColors.matches; });
$('s-check').addEventListener('click', () => { sdlg.close(); openUpdates(); });
// A login that works saves the account, so it isn't left halfway: no closing until it answers.
$('s-close').addEventListener('click', () => { if (wizardBusy !== 'login') sdlg.close(); });
sdlg.addEventListener('cancel', e => { if (wizardBusy === 'login') e.preventDefault(); });
sdlg.addEventListener('close', () => {
  ++wizardSeq; wizardBusy = null;
  if (accountView === 'work') accountView = 'start';
  listEl.focus({preventScroll:true});
});

// Import and export. The file holds settings, rules, identities and account servers, never
// passwords. Importing shows what would change first; nothing changes until Import.
const SETTING_LABELS = {theme:N_('Theme'), language:N_('Language'), density:N_('Rows'), text_size:N_('Text size'), prefer_text:N_('HTML mail'),
  mark_read:N_('Mark messages as read'), check_every:N_('Check for new mail'), mail_paper:N_('HTML mail in a dark theme'),
  compose_format:N_('New messages'), reply_format:N_('Replies and forwards'), ask_subject:N_('Ask before sending a message without a subject'),
  spell_check:N_('Underline spelling mistakes'), updates:N_('Updates'), push_mail:N_('Show new mail at once'),
  notify:N_('Notify me of new mail while Telex is in the background'), undo_send:N_('Undo sending')};
function settingValue(key, v){
  switch (key){
    case 'theme': return THEME_NAMES[v] ? t(THEME_NAMES[v]) : v;
    case 'language': return v === 'auto' ? t('Automatic, as in Windows') : LANG_NAMES[v] ?? v;
    case 'density': return v === 'comfortable' ? t('Comfortable') : t('Compact');
    case 'text_size': return v + '%';
    case 'prefer_text': return v ? t('Show the plain text') : t('Show the HTML');
    case 'mark_read': return {moment:t('After a moment on screen'), at_once:t('As soon as I select them'), manual:t('Only when I press U')}[v] ?? v;
    case 'check_every': return v === 0 ? t('Only when I click Get mail') : v === 60 ? t('Every hour') : tn(v, 'Every {n} minute', 'Every {n} minutes');
    case 'updates': return {ask:t('Not decided yet'), daily:t('Once a day'), off:t('Never')}[v] ?? v;
    case 'mail_paper': return {theme:t('On the theme’s dark paper'), white:t('On white, as sent')}[v] ?? v;
    case 'compose_format': return {plain:t('Plain text'), html:t('HTML')}[v] ?? v;
    case 'reply_format': return {same:t('In the format of the message'), plain:t('Always plain text'), html:t('Always HTML')}[v] ?? v;
    case 'ask_subject': case 'spell_check': case 'push_mail': case 'notify': return v ? t('On') : t('Off');
    case 'undo_send': return undoText(v);
    default: return String(v);
  }
}
const STATUS = {add:N_('New|import'), update:N_('Updated|import'), same:N_('Unchanged|import'), skip:N_('Skipped|import')};
function skipText(s){
  if (!s) return '';
  if (s.code === 'no_account') return t('{email} isn’t set up here yet', {email:s.email});
  if (s.code === 'no_folder') return t('no folder “{folder}” here', {folder:s.folder});
  if (s.code === 'unreadable') return t('from a newer Telex, or edited by hand');
  return tr(s.detail ?? '');
}
function counted(items){
  const n = k => items.filter(i => i.status === k).length;
  return [[n('add'), N_('{n} new')], [n('update'), N_('{n} updated')], [n('same'), N_('{n} unchanged')], [n('skip'), N_('{n} skipped')]]
    .filter(([c]) => c).map(([c, key]) => tn(c, key, key)).join(', ');
}
const why = i => i.status === 'skip' ? skipText(i.skip) : i.account ?? t('All accounts');
function planItems(items){
  return `<ul class="ilist">${items.map(i => `<li><span class="ist ${i.status}">${esc(t(STATUS[i.status]))}</span><span class="iname">${esc(i.name)}</span>`
    + `<span class="iwhy" title="${esc(why(i))}">${esc(why(i))}</span></li>`).join('')}</ul>`;
}
function planSection(part, title, items){
  if (!items.length) return '';
  const doable = items.some(i => i.status === 'add' || i.status === 'update');
  return `<section class="ipart"><label class="ihead"><input type="checkbox" data-part="${part}"${move_.choice[part] ? ' checked' : ''}${doable ? '' : ' disabled'}>`
    + `<b>${esc(title)}</b><span>${esc(counted(items))}</span></label>${planItems(items)}</section>`;
}
function renderMove(){
  const el = $('s-move'), p = move_.plan;
  if (!p){
    const c = move_.counts;
    const n = (k, text) => c ? ` <small class="sdim">${esc(tn(c[k], text, text))}</small>` : '';
    el.innerHTML = `<h3 class="rh">${esc(t('Export'))}</h3>
<p class="dnote">${esc(t('Save your settings to a file, to set up Telex on another PC or keep a copy. The file holds no passwords.'))}</p>
<div class="sopts">
  <label class="sopt"><input type="checkbox" data-ex="settings" checked><span>${esc(t('Settings: theme, language, rows, mail checks'))}</span></label>
  <label class="sopt"><input type="checkbox" data-ex="rules" checked><span>${esc(t('Rules'))}</span>${n('rules', N_('{n} rule'))}</label>
  <label class="sopt"><input type="checkbox" data-ex="identities" checked><span>${esc(t('Identities and signatures'))}</span>${n('identities', N_('{n} identity'))}</label>
  <label class="sopt"><input type="checkbox" data-ex="servers" checked><span>${esc(t('Account servers and user names'))}</span>${n('accounts', N_('{n} account'))}</label>
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
    r.identities_added ? tn(r.identities_added, '{n} identity added', '{n} identities added') : '',
    r.identities_updated ? tn(r.identities_updated, '{n} identity updated', '{n} identities updated') : '',
  ].filter(Boolean) : [];
  el.innerHTML = `<h3 class="rh">${esc(r ? t('Imported') : t('Import'))}</h3>
${made ? `<p class="dnote">${esc(t('From {made}.', {made}))}</p>` : ''}
${p.newer ? `<p class="whint warn">${esc(t('This file comes from a newer Telex. What this version doesn’t know is left out.'))}</p>` : ''}
${r ? `<p class="whint">${esc(doneLines.length ? doneLines.join(', ') + '.' : t('Nothing needed changing.'))}</p>` + (r.problems.length ? `<p class="derr">${esc(r.problems.map(tr).join(' · '))}</p>` : '') : ''}
${r ? accounts : settings + planSection('rules', t('Rules'), p.rules) + planSection('identities', t('Identities and signatures'), p.identities) + accounts}
<p class="derr" role="alert">${esc(move_.error)}</p>
<div class="sbtns">${r ? `<button class="btn" type="button" id="s-back">${esc(t('Done'))}</button>`
  : `<button class="btn primary" type="button" id="s-apply">${esc(t('Import'))}</button><button class="btn" type="button" id="s-back">${esc(t('Cancel'))}</button>`}</div>`;
}
// How many rules, identities and accounts an export would hold, for the checkboxes.
async function countThings(){
  let rules = [];
  try { rules = await invoke('list_rules'); } catch {}
  const real = ACCOUNTS.filter(a => a.imap);
  move_.counts = {rules:rules.length, identities:real.reduce((n, a) => n + (a.identities?.length ?? 0), 0), accounts:real.length};
  if (sdlg.open && !move_.plan) renderMove();
}
const dateStamp = d => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
function importError(e){
  if (e?.code === 'not_json' || e?.code === 'not_telex') return t('This isn’t a Telex settings file. Choose a file that Telex exported.');
  if (e?.code === 'unreadable') return t('Telex couldn’t read the file: {detail}', {detail:e.detail});
  return tr(e?.detail ?? e);
}
$('s-move').addEventListener('click', async e => {
  const b = e.target.closest('button');
  if (!b) return;
  move_.error = '';
  if (b.id === 's-export'){
    const choice = {};
    $('s-move').querySelectorAll('[data-ex]').forEach(c => { choice[c.dataset.ex] = c.checked; });
    if (!Object.values(choice).some(Boolean)){ move_.error = t('Tick at least one part to export.'); renderMove(); return; }
    let path;
    try {
      path = await invoke('export_settings', {choice, words:{title:t('Export Telex settings'), filter:t('Telex settings'),
        file_name:t('Telex settings {date}', {date:dateStamp(new Date())}) + '.json'}});
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
    move_ = {...move_, plan, report:null, choice:{settings:plan.settings.length > 0, rules:doable(plan.rules), identities:doable(plan.identities)}};
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
// Settings › Usage, a page per account in the spirit of a phone's storage screen: the
// server's storage when it says, what each kind of folder takes on this PC, mail in and out
// over the last 30 days, who sends the most, the largest messages, and what still waits.
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
function accountUsageHtml(u, since){
  const a = ACCT[u.account]; if (!a) return '';
  const localKb = u.folders.reduce((n, f) => n + f.size_kb, 0), count = u.folders.reduce((n, f) => n + f.count, 0);
  const sync = u.sync_error ? `<span class="ustat warn" title="${esc(u.sync_error)}">${esc(t('Last check failed'))}</span>`
    : u.synced_at ? `<span class="ustat">${esc(t('Checked {when}', {when:fmtWhen(parseDate(u.synced_at))}))}</span>` : '';
  let html = `<article class="ucard"><header class="uhead">${adot(a.id)}<span class="uname" title="${esc(a.name)}">${esc(a.email)}</span>${sync}</header>`;
  if (u.sync_error) html += `<p class="unote warn">${esc(u.sync_error)}</p>`;
  // Storage on the server, when the server reports it.
  if (u.quota){
    const pct = pctOf(u.quota), level = levelOf(pct);
    html += `<div class="ublock"><div class="urow"><span class="ulabel">${esc(t('On the server'))}</span><span class="uval">${esc(t('{used} of {limit}', {used:fmtSize(u.quota.used_kb), limit:fmtSize(u.quota.limit_kb)}))} · ${pct}%</span></div>${bar(pct, level)}`
      + (pct >= 80 ? `<p class="unote ${level}">${esc(t('The mailbox is nearly full. Once it is, new mail bounces back to its senders: empty Trash and Spam, or move old mail out.'))}</p>` : '') + '</div>';
  }
  // This PC, by kind of folder: one stacked bar and its legend.
  const groups = USE_GROUPS.map(([g, label, c]) => {
    const fs = u.folders.filter(f => groupOf(f.kind) === g);
    return {label, c, kb:fs.reduce((n, f) => n + f.size_kb, 0), n:fs.reduce((n, f) => n + f.count, 0)};
  }).filter(x => x.n);
  html += `<div class="ublock"><div class="urow"><span class="ulabel">${esc(t('On this PC'))}</span><span class="uval">${esc(fmtSize(localKb))} · ${esc(tn(count, '{n} message', '{n} messages', {n:num(count)}))}</span></div>`
    + `<div class="ustack" aria-hidden="true">${groups.map(x => `<i class="u${x.c}" style="flex-grow:${Math.max(x.kb, localKb / 150, 1)}"></i>`).join('')}</div>`
    + `<ul class="ulegend">${groups.map(x => `<li><i class="u${x.c}" aria-hidden="true"></i>${esc(t(x.label))} <span>${esc(fmtSize(x.kb))}</span></li>`).join('')}</ul></div>`;
  // Mail in and out, day by day.
  const sum = k => u.flow.reduce((n, d) => n + d[k], 0);
  const today = new Date(since.getFullYear(), since.getMonth(), since.getDate() + u.flow.length - 1);
  html += `<div class="ublock"><div class="urow"><span class="ulabel">${esc(t('Last 30 days'))}</span><span class="uval"><i class="ukey in" aria-hidden="true"></i>${esc(tn(sum('received'), '{n} received', '{n} received'))}`
    + ` · <i class="ukey out" aria-hidden="true"></i>${esc(tn(sum('sent'), '{n} sent', '{n} sent'))}${sum('spam') ? ` · <i class="ukey junk" aria-hidden="true"></i>${esc(tn(sum('spam'), '{n} in Spam', '{n} in Spam'))}` : ''}</span></div>`
    + flowSvg(u.flow, since) + `<div class="uaxis"><span>${dm(since)}</span><span>${esc(t('today'))} ${dm(today)}</span></div></div>`;
  // Who sends the most, and the largest messages: each a way to tidy up.
  const most = Math.max(1, ...u.senders.map(s => s.count));
  if (u.senders.length) html += `<div class="ublock"><span class="ulabel">${esc(t('Most mail from'))}</span><ul class="ulist">${u.senders.map(s =>
    `<li><button type="button" class="ulink" data-sender="${esc(s.email)}" title="${esc(t('Show all mail from {email}', {email:s.email}))}"><span class="who">${esc(s.name || s.email)}</span>`
    + `<span class="spark" aria-hidden="true"><i style="width:${s.count / most * 100}%"></i></span><span class="n">${num(s.count)}</span></button></li>`).join('')}</ul></div>`;
  if (u.largest.length) html += `<div class="ublock"><span class="ulabel">${esc(t('Largest messages'))}</span><ul class="ulist">${u.largest.map(m =>
    `<li><button type="button" class="ulink" data-open="${m.id}" data-folder="${esc(m.folder)}" title="${esc(t('Open it in {folder}', {folder:folderName(m.folder)}))}"><span class="who">${esc(m.subject)}</span>`
    + `<span class="from">${esc(isOut(m.folder) ? nameOf(m.to) : m.from_name)}</span><span class="n">${esc(fmtSize(m.size_kb))}</span></button></li>`).join('')}</ul></div>`;
  // What still waits.
  const outbox = u.folders.find(f => f.kind === 'outbox')?.count ?? 0, waits = [];
  if (u.queued) waits.push(tn(u.queued, '{n} change waits for the server', '{n} changes wait for the server'));
  if (outbox) waits.push(tn(outbox, '{n} message waits in the Outbox', '{n} messages wait in the Outbox'));
  if (waits.length) html += `<p class="unote">${esc(waits.join(' · '))}.</p>`;
  return html + '</article>';
}
let usageSeq = 0;
async function renderUsage(){
  const box = $('u-usage'), seq = ++usageSeq;
  if (!USAGE) box.innerHTML = `<p class="dnote">${esc(t('Counting…'))}</p>`;
  try { USAGE = await invoke('usage'); } catch (e){ box.innerHTML = `<p class="derr">${esc(tr(e))}</p>`; return; }
  if (seq !== usageSeq) return;
  renderMeter(); renderTree();
  if (!ACCOUNTS.length){ box.innerHTML = `<p class="dnote">${esc(t('No accounts yet. Add one to start: Telex finds the servers itself.'))}</p>`; return; }
  const since = parseDate(USAGE.since + 'T00:00:00');
  box.innerHTML = `<div class="ubase"><span>${esc(t('Mail base on this PC: {size}', {size:fmtBytes(USAGE.base_bytes)}))}</span>`
    + (USAGE.free_bytes > 1048576 ? `<span class="sdim">${esc(t('{size} of it is free space Telex fills before the file grows', {size:fmtBytes(USAGE.free_bytes)}))}</span>` : '') + '</div>'
    + USAGE.accounts.map(u => accountUsageHtml(u, since)).join('')
    + `<p class="dnote">${esc(t('The counts cover the mail Telex has downloaded: the newest 300 messages of each folder at the first check, then everything new.'))}</p>`;
}
const fmtBytes = b => fmtSize(Math.ceil(b / 1024));
$('u-usage').addEventListener('click', e => {
  const s = e.target.closest('[data-sender]'), o = e.target.closest('[data-open]');
  if (s){ sdlg.close(); filterAllMail(s.dataset.sender); }
  if (o){ sdlg.close(); openMessage(Number(o.dataset.open), o.dataset.folder); }
});

/* ---------- updates ---------- */
// Only release builds can update themselves. Telex checks once a day if the user said yes
// to that, and whenever they click the version number. The core asks GitHub and installs;
// the window never goes online.
const udlg = $('upd-dlg');
const upd = {build:null, found:null, checking:false, installing:false, text:'', error:''};
function renderUpdates(){
  const {build, found} = upd, can = !!build?.updatable;
  $('st-ver').hidden = !build;
  if (!build) return;
  $('st-ver').textContent = build.version;
  $('st-ver').title = t('Telex {version}.', {version:build.version}) + ' ' + (can ? t('Click to check for updates.') : t('This build can’t update itself.'));
  $('st-ask').hidden = !can || pref().updates !== 'ask';
  $('st-update').hidden = !found;
  if (found) $('st-update').textContent = t('Update to {version}', {version:found.version});
  $('upd-title').textContent = 'Telex ' + build.version;
  $('u-state').textContent = can ? upd.text : t('This build of Telex can’t update itself. Release builds from telexmail.app can.');
  $('u-err').textContent = upd.error;
  $('u-notes').hidden = !found?.notes;
  $('u-notes').textContent = found?.notes ?? '';
  $('u-install').hidden = !found;
  if (found) $('u-install').textContent = t('Install {version} and restart', {version:found.version});
  $('u-install').disabled = $('u-check').disabled = upd.checking || upd.installing;
  $('u-auto').checked = pref().updates === 'daily';
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
  Object.assign(upd, {installing:true, text:t('Downloading Telex {version}…', {version:upd.found.version}), error:''});
  renderUpdates();
  let r;
  try { r = await invoke('install_update'); }
  catch (e){
    Object.assign(upd, {installing:false, text:'', error:tr(e)}); renderUpdates();
    if (!udlg.open) say(tr(e), 10000); // replaces the download progress
    return;
  }
  if (r.busy){ // a check or a send is running: start when it's done
    upd.text = t('Waiting for Telex to finish with the mail server…'); renderUpdates();
    setTimeout(() => { upd.installing = false; installUpdate(); }, 2000);
  }
}
window.__TAURI__?.event?.listen('update-progress', ({payload: p}) => {
  const mb = n => I18n.decimal(n / 1048576);
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
function autoCheck(){
  if (pref().updates === 'daily' && Date.now() - state.updateChecked >= 864e5) checkUpdate({quiet:true});
}
async function bootUpdates(){
  try { upd.build = await invoke('update_info'); } catch { return; }
  renderUpdates();
  if (!upd.build?.updatable) return;
  setTimeout(autoCheck, 20e3); // after the first mail check
  setInterval(autoCheck, 60 * 60e3);
}
$('st-ask').addEventListener('click', async e => {
  const b = e.target.closest('[data-ask]'); if (!b) return;
  if (!await changeSettings({updates:b.dataset.ask})) return;
  if (pref().updates === 'daily'){ say(t('Telex will check for updates once a day.')); autoCheck(); }
  else say(t('Telex won’t check for updates. Click the version number to check yourself.'), 7000);
});
$('u-auto').addEventListener('change', e => { changeSettings({updates:e.target.checked ? 'daily' : 'off'}); });
$('st-ver').addEventListener('click', openUpdates);
$('st-update').addEventListener('click', openUpdates);
$('u-check').addEventListener('click', () => checkUpdate());
$('u-install').addEventListener('click', installUpdate);
$('u-close').addEventListener('click', () => udlg.close());
udlg.addEventListener('close', () => listEl.focus({preventScroll:true}));

/* ---------- keys and tips ---------- */
// The "?" in the status bar shows every key and the features that aren't obvious while the
// pointer is over it; a click, ? or F1 keeps it open until Esc, a click elsewhere or another
// press. Keys are written "Shift+R"; a space separates keys that do the same or go together.
const helpBtn = $('st-help'), helpPop = $('help-pop');
const HELP_KEYS = [
  [N_('Read|help'), [['↑ ↓', N_('previous or next message')], ['Home End', N_('first or last message')],
    ['H', N_('HTML or plain text')], ['T', N_('the whole conversation; T or Esc goes back')], ['Enter', N_('in all mail and conversations: the message’s folder')]]],
  [N_('Write|help'), [['N', N_('new message')], ['R', N_('reply')], ['Shift+R', N_('reply to all')], ['Shift+F', N_('forward')],
    ['E', N_('edit a draft or a message in the Outbox')], ['Ctrl+Enter', N_('send')], ['Esc', N_('save as a draft and close')],
    ['Ctrl+B I U K', N_('bold, italic, underline, link in HTML')]]],
  [N_('Sort out|help'), [['F', N_('flag or unflag')], ['U', N_('read or unread')], ['Shift+U', N_('everything in the folder read')],
    ['Del', N_('to Trash; in Trash, twice to delete for good')], ['M', N_('move to a folder: type part of its name')],
    ['G', N_('go to a folder')], ['Z', N_('undo a delete or a move; take back mail just sent')]]],
  [N_('Select|help'), [['Shift+↑ ↓', N_('one more or one fewer')], ['Shift+Home End', N_('everything to the top or bottom')],
    ['Ctrl+A', N_('the whole list')], ['Esc', N_('just the one message again')]]],
  [N_('Find|help'), [['/', N_('quick filter')], ['Tab', N_('this folder or all mail')], ['Esc', N_('clear the filter')]]],
  [N_('Window|help'), [['Ctrl+,', N_('settings')], ['Ctrl+= − 0', N_('larger, smaller or normal text')], ['? F1', N_('this help')]]],
];
const HELP_TIPS = [
  N_('Shift+click selects a range, Ctrl+click one more message.'),
  N_('Click the sender above a message to see all their mail.'),
  N_('Drop files on the window to attach them, even with no message open.'),
  N_('Rules › “New rule for mail from …” starts a rule from the selected message.'),
  N_('The storage bar in the status bar shows how full the mailbox on the server is. Click it for more.'),
  N_('In a dark theme, the sun button above HTML mail shows it on white, as it was sent.'),
  N_('Errors and notes from rules show above the message they are about.'),
];
const OPERATORS = 'from: to: subject: is:unread is:flagged has:file after:2026-09 before:2026 larger:5m smaller:100k';
const keysHtml = spec => spec.split(' ').map(c => c.split(/\+(?=.)/).map(k => `<kbd>${esc(k)}</kbd>`).join('+')).join(' ');
function renderHelp(){
  helpPop.innerHTML = `<div class="hgrid">${HELP_KEYS.map(([title, rows]) => `<section class="hsec"><h3>${esc(t(title))}</h3>`
    + rows.map(([keys, what]) => `<div class="hrow"><span class="k">${keysHtml(keys)}</span><span>${esc(t(what))}</span></div>`).join('')
    + (title === 'Find|help' ? `<p class="hops">${OPERATORS.split(' ').map(o => `<code>${esc(o)}</code>`).join(' ')}</p>` : '') + '</section>').join('')}</div>`
    + `<h3 class="htitle">${esc(t('Good to know'))}</h3><ul class="htips">${HELP_TIPS.map(tip => `<li>${esc(t(tip))}</li>`).join('')}</ul>`;
}
let helpPinned = false, helpTimer;
function showHelp(pin){
  clearTimeout(helpTimer);
  if (pin !== undefined) helpPinned = pin;
  if (helpPop.hidden){ renderHelp(); helpPop.hidden = false; }
  helpBtn.setAttribute('aria-expanded', 'true');
}
function hideHelp(){
  clearTimeout(helpTimer);
  helpPinned = false;
  helpPop.hidden = true;
  helpBtn.setAttribute('aria-expanded', 'false');
}
const toggleHelp = () => helpPop.hidden || !helpPinned ? showHelp(true) : hideHelp();
// Hovering opens it after a moment and leaving closes it, unless it was opened on purpose.
const helpHover = on => {
  clearTimeout(helpTimer);
  if (helpPinned) return;
  helpTimer = setTimeout(() => on ? showHelp() : hideHelp(), on ? 150 : 250);
};
helpBtn.addEventListener('pointerenter', () => helpHover(true));
helpBtn.addEventListener('pointerleave', () => helpHover(false));
helpPop.addEventListener('pointerenter', () => clearTimeout(helpTimer));
helpPop.addEventListener('pointerleave', () => helpHover(false));
helpBtn.addEventListener('click', toggleHelp);
document.addEventListener('pointerdown', e => {
  if (!helpPop.hidden && !helpPop.contains(e.target) && !helpBtn.contains(e.target)) hideHelp();
});

/* ---------- keyboard ---------- */
// Also attached to the HTML message frame, so keys work after clicking into it.
function onKey(e){
  const mod = (e.ctrlKey || e.metaKey) && !e.altKey;
  // Text size works everywhere, in dialogs too.
  if (mod && ['=', '+', '-', '0'].includes(e.key)){ e.preventDefault(); stepTextSize(e.key === '0' ? 0 : e.key === '-' ? -1 : 1); return; }
  if (document.querySelector('dialog[open]')) return; // dialogs handle their own Enter and Esc
  if (mod && e.key === ','){ e.preventDefault(); openSettings(); return; }
  const tg = e.target;
  if (state.compose && e.key === 'Enter' && (e.ctrlKey || e.metaKey)){ e.preventDefault(); send(); return; }
  if (e.key === 'F1'){ e.preventDefault(); toggleHelp(); return; }
  if (e.key === 'Escape'){
    if (!helpPop.hidden){ e.preventDefault(); hideHelp(); return; }
    if (state.compose){ e.preventDefault(); closeCompose().then(ok => ok && say(t('Saved to Drafts.'))); return; }
    if (tg === qEl){ qEl.value = ''; setFilter(''); listEl.focus(); return; }
    if (unpick()){ e.preventDefault(); afterPick(); return; }
    if (state.thread){ e.preventDefault(); leaveThread(); return; }
  }
  // Tab in the filter box: this folder or all mail, while there is something to look for.
  if (tg === qEl && e.key === 'Tab' && !e.shiftKey && !e.ctrlKey && !e.altKey && qEl.value.trim()){ e.preventDefault(); toggleScope(); return; }
  if (tg.closest && (tg.closest('input, textarea, select, [contenteditable="true"]') || tg.closest('.split'))) return;
  // Ctrl+A in the list takes every message; elsewhere it selects text as always.
  if (mod && !e.shiftKey && e.key.toLowerCase() === 'a' && listEl.contains(tg)){ e.preventDefault(); pickAll(); return; }
  if (e.ctrlKey || e.metaKey || e.altKey) return;
  const k = e.key, shift = e.shiftKey;
  if (k === 'ArrowDown') shift ? pickMove(1) : move(1);
  else if (k === 'ArrowUp') shift ? pickMove(-1) : move(-1);
  else if (k === 'Home') shift ? pickEnd(false) : move(-Infinity);
  else if (k === 'End') shift ? pickEnd(true) : move(Infinity);
  else if (k === 'Delete') del();
  else if (k === 'Enter' && allMail()) openInFolder();
  else if (k === 'r' || k === 'R') openCompose(shift ? 'replyall' : 'reply');
  else if (k === 'F' && shift) openCompose('forward');
  else if (k === 'f') toggleFlag();
  else if ((k === 'u' || k === 'U') && shift) markAllRead();
  else if (k === 'u' || k === 'U') toggleUnread();
  else if (k === 'n' || k === 'N') openCompose('new');
  else if (k === 'e' || k === 'E') openCompose('edit');
  else if (k === 'h' || k === 'H') setView(!pref().prefer_text);
  else if (k === 'm' || k === 'M') openPicker('move');
  else if (k === 'g' || k === 'G') openPicker('go');
  else if (k === 't' || k === 'T') toggleThread();
  else if (k === 'z' || k === 'Z') undo();
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
async function boot(){
  I18n.translatePage();
  paintToolbar(); paintDensity();
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
  loadUsage(); // the status-bar meter, from what the last check saved
  // Real accounts: check now, then as often as the settings say, quietly.
  if (ACCOUNTS.length) getMail({quiet:true});
  else openAccountDialog(); // first start: straight to adding an account
}
boot();
})();
