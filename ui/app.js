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
};
const KIND_ICON = {inbox:'inbox', outbox:'outbox', drafts:'drafts', sent:'sent', archive:'archive', spam:'spam', trash:'trash', custom:'folder'};

/* ---------- data (filled from Rust) ---------- */
// The folder tree and its counts; the message list asks the core for pages (see "list").
let ACCOUNTS = [], ACCT = {}, COUNTS = {};
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
// The core's sentences are English; the common ones have a translation (i18n.js, FROM_CORE).
const tr = e => t(String(e));
// Commands that change mail; the core queues them for the server and we push shortly after.
// send_message counts: a sent message's copy for the server's Sent folder is queued too.
const CHANGES = new Set(['set_unread', 'set_flagged', 'move_message', 'delete_message', 'send_message', 'run_rules']);
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
const fmtSize = kb => kb >= 1024 ? I18n.decimal(kb / 1024) + ' MB' : kb + ' KB';
const nameOf = addr => {
  const s = String(addr || '').trim();
  if (!s) return t('(no recipient)');
  const m = s.match(/^\s*"?([^"<]+?)"?\s*</);
  return m ? m[1] : s;
};
// The core says what was sent as `"Subject" to Name`.
const sentLine = s => String(s).replace(/^"(.*)" to (.*)$/, (_, subject, who) => t('“{subject}” to {who}', {subject, who}));
const acctId = key => key.split('/')[0];
const folderOf = key => { const [a, f] = key.split('/'); return ACCT[a]?.folders.find(x => x.id === f); };
const folderName = key => folderOf(key)?.name ?? key;
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
  collapsed:{}, compose:null, images:new Set(), lastDeleted:null,
  updateChecked: Number(saved.updateChecked) || 0, // when a check last got an answer (ms)
  offered: saved.offered || null, // the language offered on the first start, once
};
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
const darkMode = matchMedia('(prefers-color-scheme: dark)'), forcedColors = matchMedia('(forced-colors: active)');
const shownTheme = id => id === 'system' ? (darkMode.matches ? 'dark' : 'light') : id;
let SETTINGS = {
  settings:{theme:'system', language:'auto', density:'compact', text_size:100, prefer_text:false, mark_read:'moment', check_every:10, updates:'ask'},
  language:I18n.lang(), offer:null, fresh:false,
};
const pref = () => SETTINGS.settings;
let zoomed = 100;
function applySettings(){
  const s = pref();
  document.documentElement.dataset.theme = shownTheme(s.theme);
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
  if (!state.compose) renderPreview();
  if (dlg.open){ renderAccountList(); renderLangPick(); }
  if (idlg.open && !$('i-list-view').hidden) showIdentityList();
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
function renderStatus(){
  const {total, unread} = countOf(state.folder), found = list.total;
  $('st-folder').innerHTML = !ACCOUNTS.length ? esc(t('No account yet'))
    : allMail() ? esc(t('All mail') + ' · ' + tn(found, '{n} match', '{n} matches', {n:num(found)}))
    : `${adot(acctId(state.folder))}${esc(folderName(state.folder))} · ${esc(tn(total, '{n} message', '{n} messages', {n:num(total)}))}${unread ? ', ' + esc(tn(unread, '{n} unread', '{n} unread', {n:num(unread)})) : ''}`;
  $('st-check').textContent = lastCheck ? t('Last check {time}', {time:hm(lastCheck)}) : '';
  const title = ACCOUNTS.length ? `${folderName(state.folder)} · ${ACCT[acctId(state.folder)]?.email ?? ''} – Telex` : 'Telex';
  if (title !== lastTitle){
    lastTitle = title;
    document.title = title;
    try { window.__TAURI__?.window?.getCurrentWindow().setTitle(title); } catch {}
  }
}

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
  return `<li><button type="button" class="fold${on ? ' active' : ''}" data-folder="${key}"${on ? ' aria-current="true"' : ''}>${I[KIND_ICON[f.kind] || 'folder']}<span>${esc(f.name)}</span>${cnt}</button></li>`;
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
    return `<button type="button" class="acct" data-acct="${esc(a.id)}" aria-expanded="${open}" title="${esc(a.name)} · ${esc(a.email)}">${I.caret}${adot(a.id)}<span class="acct-mail">${esc(a.email)}</span></button>${items}`;
  }).join('') + `<button type="button" class="add-acct" data-act="add-account">${I.plus}<span>${esc(ACCOUNTS.length ? t('Add account') : t('Add your account'))}</span></button>`;
  $('folder-select').innerHTML = ACCOUNTS.map(a => `<optgroup label="${esc(a.email)}">` + a.folders.map(f => {
    const key = a.id + '/' + f.id, n = countOf(key).unread;
    return `<option value="${key}"${key === state.folder ? ' selected' : ''}>${esc(f.name)}${n ? ` (${n})` : ''}</option>`;
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
  state.folder = key; state.q = ''; qEl.value = ''; state.inText = false;
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
const allMail = () => state.scope === 'all' && !!state.q.trim();
const selKey = () => allMail() ? '*' : state.folder;
const viewArgs = () => ({folder:allMail() ? null : state.folder, sort:state.sort.key, ascending:state.sort.dir > 0,
  filter:state.q, bodies:state.inText});
const current = () => selRow && selRow.id === state.sel[selKey()] && (allMail() || selRow.f === state.folder) ? selRow : null;

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
  const cls = (m.unread ? 'unread ' : '') + (m.id === sel ? 'sel' : '');
  return `<tr data-id="${m.id}" class="${cls}" aria-selected="${m.id === sel}">
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
  rowsEl.querySelectorAll('tr.sel').forEach(r => { r.classList.remove('sel'); r.setAttribute('aria-selected', 'false'); });
  const r = rowsEl.querySelector(`tr[data-id="${sel}"]`);
  if (r){ r.classList.add('sel'); r.setAttribute('aria-selected', 'true'); }
}
// Change fields of a message wherever the list keeps it.
function patch(id, fields){
  for (const r of list.rows) if (r.id === id) Object.assign(r, fields);
  if (selRow?.id === id) Object.assign(selRow, fields);
}
rowsEl.addEventListener('click', e => {
  const tr = e.target.closest('tr[data-id]');
  if (!tr) return;
  select(Number(tr.dataset.id));
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
async function moveBy(delta){
  if (!list.total) return;
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
// The HTML view of a message, cached per images setting. load: true loads remote images,
// false never does (only asks whether there is HTML), null lets the core decide (trusted
// sender, not spam). The core does any downloading; the window never talks to the network.
const htmlViews = new Map();
async function htmlView(id, load){
  const key = id + ':' + load;
  if (!htmlViews.has(key)) htmlViews.set(key, await invoke('get_html', {id, loadImages:load}));
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
function attsHtml(atts){
  if (!atts.length) return '';
  return `<div class="atts">${atts.map((a, i) => `<button type="button" class="att" data-att="${i}" title="${esc(a.name)}">${I.clip}<span class="nm">${esc(a.name)}</span><small>${fmtSize(a.size_kb)}</small></button>`).join('')}</div>`;
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
  if (!s){
    previewEl.innerHTML = `<div class="pempty">${I.mail}<div>${list.total || allMail() ? esc(t('Select a message to read it here.')) : t('Nothing in {folder}.', {folder:esc(folderName(state.folder))})}</div></div>`;
    return;
  }
  let m, hv;
  try {
    m = await full(s.id);
    hv = await htmlView(s.id, pref().prefer_text ? false : state.images.has(s.id) ? true : null);
  } catch (e){ if (seq === pvSeq) previewEl.innerHTML = `<div class="pempty">${esc(tr(e))}</div>`; return; }
  if (seq !== pvSeq || state.compose) return;
  const spam = kindOf(s.f) === 'spam'; // links and trusted images stay off in Spam
  const asHtml = !!hv && !pref().prefer_text;
  const views = hv ? `<span class="seg" role="group" aria-label="${esc(t('Show as'))}"><button type="button" data-view="html" aria-pressed="${asHtml}" title="${esc(t('Show the HTML (H)'))}">HTML</button><button type="button" data-view="text" aria-pressed="${!asHtml}" title="${esc(t('Show plain text (H)'))}">${esc(t('Text|view'))}</button></span>` : '';
  const images = asHtml ? imagesBar(m, hv, spam)
    : !hv && m.remote_images && !state.images.has(m.id) ? `<div class="bar">${esc(t('Remote images are blocked so the sender can’t see when you open this.'))} <button type="button" class="linkbtn" data-act="images">${esc(t('Load images'))}</button></div>` : '';
  previewEl.innerHTML = `<article>
<header class="ph">
  <div class="ph-top"><h2 class="ph-subj">${esc(m.subject)}</h2>${views}${s.flagged ? `<span class="pill">${I.flag}${esc(t('Flagged'))}</span>` : ''}</div>
  <dl class="ph-grid">
    <dt>${esc(t('From'))}</dt><dd>${esc(m.from_name)} &lt;${esc(m.from_email)}&gt;</dd>
    <dt>${esc(t('To'))}</dt><dd>${esc(m.to || t('(no recipient)'))}</dd>
    ${m.cc ? `<dt>${esc(t('Cc'))}</dt><dd>${esc(m.cc)}</dd>` : ''}
    <dt>${esc(t('Date'))}</dt><dd class="mono">${fmtFull(parseDate(m.date))} · ${fmtSize(m.size_kb)}</dd>
  </dl>
  ${attsHtml(m.attachments)}
</header>
${m.note ? `<div class="bar note${spam || kindOf(s.f) === 'outbox' ? ' warn' : ''}">${esc(m.note)}</div>` : ''}
${images}
${asHtml ? `<div class="pbody html"><iframe class="hframe" sandbox="allow-same-origin" referrerpolicy="no-referrer" title="${esc(t('Message'))}"></iframe></div>` : `<div class="pbody" lang="">${bodyHtml(m)}</div>`}
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
  const att = e.target.closest('[data-att]'), act = e.target.closest('[data-act]');
  if (att){ say(t('Opening attachments isn’t built yet ({name}).', {name:att.title})); return; }
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
  if (act.dataset.act === 'discard'){ state.compose = null; renderPreview(); say(t('Message discarded.')); listEl.focus(); }
  if (act.dataset.act === 'draft') closeCompose().then(ok => ok && say(t('Saved to Drafts.')));
  if (act.dataset.act === 'add-account') openAccountDialog();
});

/* ---------- compose ---------- */
// kind: new, reply, replyall or forward, started by the core with the identity's signature
// and templates; or edit, which reopens a draft or an unsent message in the Outbox.
async function openCompose(kind, preset = {}){
  if (!ACCOUNTS.length){ say(t('Add a mail account first.')); openAccountDialog(); return; }
  const s = current();
  if (kind !== 'new' && !s){ say(t('Select a message first.')); return; }
  if (kind === 'edit' && !['drafts', 'outbox'].includes(kindOf(s.f))){ say(t('Only drafts and unsent messages in the Outbox can be edited.')); return; }
  let c;
  try {
    if (kind === 'edit'){
      const m = await full(s.id), account = acctId(s.f), ids = ACCT[account]?.identities ?? [];
      const me = ids.find(i => i.email.toLowerCase() === m.from_email.toLowerCase()) ?? ids.find(i => i.is_default) ?? ids[0];
      // Attachments come from the old message itself, which Send replaces.
      c = {kind, account, identity:me?.id ?? null, to:m.to, cc:m.cc, subject:m.subject, body:m.body,
        attachments:m.attachments.slice(), replyTo:null, forwardOf:m.id, replaces:m.id};
    } else {
      const d = await invoke('start_draft', {kind:kind === 'replyall' ? 'reply_all' : kind, of:kind === 'new' ? null : s.id, account:acctId(state.folder)});
      c = {kind, account:d.account, identity:d.identity, to:d.to, cc:d.cc, subject:d.subject, body:d.body,
        attachments:d.attachments, replyTo:d.reply_to, forwardOf:d.forward_of, replaces:null};
    }
  } catch (e){ say(tr(e), 8000); return; }
  Object.assign(c, preset); // e.g. {to} from a mailto: link
  state.compose = c;
  renderCompose();
}
const allIdentities = () => ACCOUNTS.flatMap(a => (a.identities || []).map(i => ({...i, account:a.id})));
const composeNote = c => !ACCT[c.account]?.smtp ? t('This account has no SMTP server yet: edit it under Add account to send.') : '';
function fromOptions(c){
  return ACCOUNTS.map(a => `<optgroup label="${esc(a.name)}">` + (a.identities || []).map(i =>
    `<option value="${i.id}"${i.id === c.identity ? ' selected' : ''}>${esc(i.name)} &lt;${esc(i.email)}&gt;</option>`).join('') + '</optgroup>').join('');
}
// Another identity: the account follows it, and its signature replaces the old one.
function changeIdentity(id){
  const c = readCompose(), next = allIdentities().find(i => i.id === id);
  if (!next) return;
  const before = (allIdentities().find(i => i.id === c.identity)?.signature || '').trim(), after = (next.signature || '').trim();
  if (before && before !== after && c.body.includes(before)) c.body = c.body.replace(before, after);
  Object.assign(c, {identity:next.id, account:next.account});
  $('c-body').value = c.body;
  previewEl.querySelector('.cnote').textContent = composeNote(c);
}
function renderCompose(){
  const c = state.compose;
  const title = {new:t('New message'), reply:t('Reply|compose title'), replyall:t('Reply to all|compose title'), forward:t('Forward|compose title'), edit:t('Edit|compose title')}[c.kind];
  const note = composeNote(c);
  previewEl.innerHTML = `<form class="compose" id="cform" novalidate>
<div class="ctitle">${esc(title)}<label class="cfrom">${esc(t('from|compose'))} <select id="c-from" aria-label="${esc(t('From'))}">${fromOptions(c)}</select></label></div>
<div class="cf">
  <label for="c-to">${esc(t('To'))}</label><input id="c-to" value="${esc(c.to)}" autocomplete="off" spellcheck="false">
  <label for="c-cc">${esc(t('Cc'))}</label><input id="c-cc" value="${esc(c.cc)}" autocomplete="off" spellcheck="false">
  <label for="c-subj">${esc(t('Subject'))}</label><input id="c-subj" value="${esc(c.subject)}" autocomplete="off">
</div>
${attsHtml(c.attachments)}
<textarea id="c-body" aria-label="${esc(t('Message text'))}">
${esc(c.body)}</textarea>
<div class="cbar">
  <button class="btn primary" type="submit"><span>${esc(t('Send'))}</span> <kbd>Ctrl+Enter</kbd></button>
  <button class="btn" type="button" data-act="draft">${esc(t('Save draft'))} <kbd>Esc</kbd></button>
  <button class="btn" type="button" data-act="discard">${esc(t('Discard'))}</button>
  <span class="cnote">${esc(note)}</span>
</div></form>`;
  $('cform').addEventListener('submit', e => { e.preventDefault(); send(); });
  $('c-from').addEventListener('change', e => changeIdentity(Number(e.target.value)));
  const tx = $('c-body');
  if (c.kind === 'new' || c.kind === 'forward' || (c.kind === 'edit' && !c.to)) $('c-to').focus();
  else { tx.focus(); tx.setSelectionRange(0, 0); tx.scrollTop = 0; }
}
const draftOf = c => ({account:c.account, to:c.to, cc:c.cc, subject:c.subject || t('(no subject)'), body:c.body.replace(/^\n+/, ''),
  attachments:c.attachments, reply_to:c.replyTo, forward_of:c.forwardOf, replaces:c.replaces, identity:c.identity});
function readCompose(){
  const c = state.compose;
  c.to = $('c-to').value.trim(); c.cc = $('c-cc').value.trim(); c.subject = $('c-subj').value.trim(); c.body = $('c-body').value;
  return c;
}
// Saves the compose content as a draft and closes it. Resolves to true when it worked.
async function closeCompose(){
  const draft = draftOf(readCompose());
  try { await call('save_outgoing', {draft}); } catch { return false; }
  state.compose = null;
  renderPreview();
  await refresh();
  listEl.focus();
  return true;
}
const sentText = sent => sent.length === 1 ? t('Sent {what}.', {what:sentLine(sent[0])}) : sent.length ? tn(sent.length, 'Sent {n} message.', 'Sent {n} messages.') : '';
// Real mail goes to the Outbox and out at once. It's only called sent once the SMTP
// server has taken it; otherwise it waits in the Outbox and the status says why.
async function send(){
  if (!$('c-to').value.trim() && !$('c-cc').value.trim()){ say(t('Add at least one recipient in To.')); $('c-to').focus(); return; }
  const btn = document.querySelector('#cform .btn.primary');
  if (!btn || btn.disabled) return;
  const label = btn.querySelector('span');
  btn.disabled = true; label.textContent = t('Sending…');
  say(t('Sending…'), 0);
  let r;
  try { r = await call('send_message', {draft:draftOf(readCompose())}); }
  catch { btn.disabled = false; label.textContent = t('Send'); return; } // e.g. a bad address: fix it and try again
  state.compose = null;
  renderPreview();
  await refresh();
  listEl.focus();
  const sent = sentText(r.sent);
  if (r.errors.length) say([sent, r.errors.join(' · ')].filter(Boolean).join(' '), 15000);
  else say(sent || t('In the Outbox. It goes out with the next check.'), 6000);
}

/* ---------- actions ---------- */
function toggleFlag(){
  const m = current(); if (!m) return;
  const flagged = !m.flagged;
  changed(m, {flagged}); if (!state.compose) renderPreview();
  call('set_flagged', {id:m.id, flagged})
    .then(() => { say(flagged ? t('Flagged.') : t('Flag removed.'), 2000); reorder(); })
    .catch(() => { changed(m, {flagged:!flagged}); if (!state.compose) renderPreview(); });
}
function toggleUnread(){
  const m = current(); if (!m) return;
  clearTimeout(readTimer);
  const unread = !m.unread;
  call('set_unread', {id:m.id, unread}).then(() => {
    changed(m, {unread}); reorder();
    say(unread ? t('Marked as unread.') : t('Marked as read.'), 2000);
  }).catch(() => {});
}
// Deleting from Trash can't be undone (on the server either), so it takes a second Del.
let armedDelete = null; // {id, at}
async function del(){
  const m = current(); if (!m) return;
  if (kindOf(m.f) === 'trash'){
    if (armedDelete?.id !== m.id || Date.now() - armedDelete.at > 5000){
      armedDelete = {id:m.id, at:Date.now()};
      say(t('Press Del again to delete this message for good.'), 5000);
      return;
    }
    armedDelete = null;
  }
  // The next one gets the selection, or the one above at the end of the list.
  const pos = posOf(m.id);
  const next = pos < 0 ? null : (await rowAt(pos + 1)) ?? (await rowAt(pos - 1));
  let outcome;
  try { outcome = await call('delete_message', {id:m.id}); } catch { return; }
  if (outcome === 'removed'){
    bodies.delete(m.id); state.lastDeleted = null;
    say(t('Deleted for good.'));
  } else {
    state.lastDeleted = {id:m.id, from:m.f};
    say(t('Moved to Trash. Press Z to undo.'));
  }
  state.sel[selKey()] = next?.id;
  selRow = next;
  if (!state.compose) renderPreview();
  await refresh({skip:m.id});
}
async function undo(){
  const d = state.lastDeleted; if (!d) return;
  try { await call('move_message', {id:d.id, folder:d.from}); } catch { return; }
  state.lastDeleted = null;
  if (allMail() || state.folder === d.from){ state.sel[selKey()] = d.id; selRow = null; }
  await loadTree().catch(() => {});
  renderTree();
  await loadView();
  say(t('Restored to {folder}.', {folder:folderName(d.from)}));
}
// quiet: a background check, which speaks up only for new mail or errors.
// first: the download right after adding an account.
let checkAgain = null; // asked for while a check was running; runs when it ends
async function getMail({quiet = false, first = false} = {}){
  const n = ACCOUNTS.filter(a => a.imap).length;
  if (!n){ if (!quiet) openAccountDialog(); return; } // nothing to check yet
  const b = $('b-get');
  if (b.classList.contains('busy')){ if (first || !quiet) checkAgain = {quiet, first}; return; }
  b.classList.add('busy');
  if (!quiet) say(tn(n, 'Checking {n} account…', 'Checking {n} accounts…'), 0);
  try {
    const r = await invoke('check_mail');
    if (r.busy){ setTimeout(() => getMail({quiet, first}), 2000); return; } // changes are being sent
    lastCheck = new Date();
    await refresh();
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
  paintScope();
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

/* ---------- account dialog ---------- */
// Adding is a wizard: name, email and password. The core finds the servers (its provider
// list, the domain's setup file, its DNS zone, the usual names) and add_account logs in to
// both before anything is saved. When nothing is found or the login fails, the settings
// form opens, filled in, with the reason. Editing an account goes straight to the form.
const dlg = $('acct-dlg'), aForm = $('acct-form'), aErr = $('a-err'), wForm = $('w-start'), wErr = $('w-err');
let editing = null;     // id of the account being edited, or null when adding
let wizardSeq = 0;      // each run gets a number; answers for an older run are dropped
let wizardBusy = null;  // 'find' or 'login' while the wizard works

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
function showView(view){
  wForm.hidden = view !== 'start'; $('w-work').hidden = view !== 'work'; aForm.hidden = view !== 'form';
  dlg.setAttribute('aria-labelledby', {start:'acct-title', work:'w-work-title', form:'a-title'}[view]);
}
function openAccountDialog(){
  wForm.reset(); aForm.reset();
  editing = null; wizardBusy = null; ++wizardSeq;
  wErr.textContent = ''; hint($('w-hint'), null);
  // A second account is usually the same person: start from their name.
  const me = ACCOUNTS.flatMap(a => a.identities || []).find(i => i.is_default)?.name ?? '';
  $('w-name').value = me;
  renderAccountList();
  renderLangPick();
  showView('start');
  if (!dlg.open) dlg.showModal();
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
function renderAccountList(){
  const real = ACCOUNTS.filter(a => a.imap);
  $('a-list').innerHTML = real.length ? `<h3>${esc(t('Connected'))}</h3>` + real.map(a => {
    const where = `${a.email} · ${a.imap.host}${a.smtp ? '' : ' · ' + t('no SMTP server yet')}`;
    return `<div class="dacct"><span class="nm">${esc(a.name)}</span><span class="em" title="${esc(where)}">${esc(where)}</span>`
      + `<button type="button" class="btn" data-ids="${esc(a.id)}">${esc(t('Identities'))}</button><button type="button" class="btn" data-edit="${esc(a.id)}">${esc(t('Edit'))}</button><button type="button" class="btn danger" data-remove="${esc(a.id)}">${esc(t('Remove'))}</button></div>`;
  }).join('') : '';
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
  showView('work');
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
  if (d.unsupported){ showView('start'); hint($('w-hint'), d); $('w-email').focus(); return; }
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
$('w-stop').addEventListener('click', () => {
  if (wizardBusy !== 'find') return;
  ++wizardSeq; wizardBusy = null;
  showView('start');
  $('w-email').focus();
});

// The settings form, filled in from what the wizard found or typed (v), with a reason.
// note: a tip above the fields, e.g. that the servers came from an imported file.
function openForm(v, error = '', d = null, note = ''){
  aForm.reset();
  $('a-title').textContent = t('Server settings');
  $('a-go').querySelector('span').textContent = t('Connect');
  $('a-email').value = v.email ?? ''; $('a-name').value = v.name ?? '';
  $('a-host').value = v.host ?? ''; $('a-port').value = v.port ?? 993;
  $('a-user').value = v.username && v.username !== v.email ? v.username : '';
  $('a-pass').value = v.password ?? ''; $('a-pass').placeholder = '';
  $('a-smtp').value = v.smtp_host ?? ''; $('a-sport').value = v.smtp_port || 465; $('a-sec').value = v.smtp_security ?? 'tls';
  placeholders();
  aErr.textContent = error;
  hint($('a-hint'), d);
  if (note){ $('a-hint').hidden = false; $('a-hint').classList.add('warn'); $('a-hint').textContent = note; }
  showView('form');
  if (!dlg.open) dlg.showModal();
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
  $('a-smtp').value = a.smtp?.host ?? ''; $('a-sec').value = a.smtp?.security ?? 'tls';
  $('a-sport').value = a.smtp?.port ?? 465;
  $('a-pass').value = ''; $('a-pass').placeholder = t('Leave empty to keep the saved password');
  placeholders();
  showView('form');
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
$('a-back').addEventListener('click', () => {
  editing = null;
  renderAccountList();
  showView('start');
  $('w-email').focus();
});
dlg.addEventListener('click', e => {
  const help = e.target.closest('[data-help]');
  if (help) call('open_link', {url:help.dataset.help}).catch(() => {});
  if (e.target.closest('[data-close]')) dlg.close();
});
// Esc can't close the dialog while a login runs: a login that works saves the account.
dlg.addEventListener('cancel', e => { if (wizardBusy === 'login') e.preventDefault(); });
dlg.addEventListener('close', () => { ++wizardSeq; wizardBusy = null; listEl.focus({preventScroll:true}); });
aForm.addEventListener('submit', async e => {
  e.preventDefault();
  const go = $('a-go'), label = go.querySelector('span');
  if (go.disabled) return;
  const port = Number($('a-port').value), smtpPort = Number($('a-sport').value || 0);
  const badPort = p => !Number.isInteger(p) || p < 0 || p > 65535;
  if (badPort(port) || !port){ aErr.textContent = t('The IMAP port is a number from 1 to 65535. For SSL/TLS it is usually 993.'); $('a-port').focus(); return; }
  if (badPort(smtpPort)){ aErr.textContent = t('The SMTP port is a number from 1 to 65535: usually 465 for SSL/TLS, 587 for STARTTLS.'); $('a-sport').focus(); return; }
  const setup = {name:$('a-name').value, email:$('a-email').value, host:$('a-host').value, port, username:$('a-user').value,
    smtp_host:$('a-smtp').value, smtp_port:smtpPort, smtp_security:$('a-sec').value};
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
  dlg.close();
  await loadTree();
  if (wasEditing){
    refresh();
    say(t('Saved {name}.', {name:account.name}));
    getMail({quiet:true}); // the Outbox may have waited for these settings
    return;
  }
  setFolder(homeOf(account.id));
  say(t('Connected to {email}. Downloading mail…', {email:account.email}), 0);
  getMail({first:true});
}
$('a-list').addEventListener('click', async e => {
  const ed = e.target.closest('[data-edit]');
  if (ed){ editAccount(ed.dataset.edit); return; }
  const ids = e.target.closest('[data-ids]');
  if (ids){ dlg.close(); openIdentities(ids.dataset.ids); return; }
  const b = e.target.closest('[data-remove]');
  if (!b) return;
  if (b.dataset.armed !== '1'){ b.dataset.armed = '1'; b.textContent = t('Remove it and its local mail'); return; }
  try { await invoke('remove_account', {id:b.dataset.remove}); } catch (err){ wErr.textContent = tr(err); return; }
  await refresh();
  renderAccountList();
  say(t('Account removed from Telex. The mail on the server is untouched.'));
});

/* ---------- identities ---------- */
const idlg = $('ids-dlg');
let idAccount = null, identity = null; // identity: the one being edited
function openIdentities(account){
  idAccount = account;
  idlg.showModal();
  showIdentityList();
}
function showIdentityList(){
  const a = ACCT[idAccount];
  if (!a){ idlg.close(); return; }
  $('i-form').hidden = true; $('i-list-view').hidden = false; $('i-err').textContent = '';
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
  $('i-list-view').hidden = true; $('i-form').hidden = false; $('i-form-err').textContent = '';
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
$('i-close').addEventListener('click', () => idlg.close());
idlg.addEventListener('close', () => listEl.focus({preventScroll:true}));

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
  $('r-list').innerHTML = RULES.length ? RULES.map((r, i) => `<div class="rrule${r.enabled ? '' : ' off'}" data-id="${r.id}">
<input type="checkbox" data-r="toggle"${r.enabled ? ' checked' : ''} title="${esc(t('On or off'))}" aria-label="${esc(t('Rule {name} on', {name:r.name}))}">
<div class="rtext"><div class="rname">${esc(r.name)}${r.enabled ? '' : ` <span class="tag">${esc(t('Off|rule'))}</span>`}</div><div class="rsum" title="${esc(ruleSummary(r))}">${esc(ruleSummary(r))}</div></div>
<button type="button" class="btn icon" data-r="up" title="${esc(t('Run earlier'))}" aria-label="${esc(t('Run earlier'))}"${i ? '' : ' disabled'}>↑</button>
<button type="button" class="btn icon" data-r="down" title="${esc(t('Run later'))}" aria-label="${esc(t('Run later'))}"${i < RULES.length - 1 ? '' : ' disabled'}>↓</button>
<button type="button" class="btn" data-r="edit">${esc(t('Edit'))}</button><button type="button" class="btn danger" data-r="delete">${esc(t('Delete'))}</button></div>`).join('')
    : `<p class="dnote">${esc(t('No rules yet. A rule can file invoices, flag mail from a client, or answer while you’re away.'))}</p>`;
  $('r-new').focus();
}
function showRuleForm(existing){
  const firstAccount = ACCOUNTS.find(a => a.imap)?.id ?? ACCOUNTS[0]?.id ?? null;
  rule = existing ? structuredClone(existing) : {id:0, name:'', enabled:true, account:firstAccount, match_all:true,
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
// Five tabs. Every change is saved at once (changeSettings) and shows at once, theme cards
// included: each card carries its own data-theme, so it is drawn in its own colors.
const sdlg = $('set-dlg');
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
  const can = !!upd.build?.updatable;
  $('s-updates').checked = s.updates === 'daily'; $('s-updates').disabled = !can;
  $('s-upd-none').hidden = can; $('s-check').hidden = !can;
  renderMove();
  if (again) sdlg.querySelector(again)?.focus();
}
// After a change that keeps the words: only the ticks move, so focus stays where it was.
function syncSettings(){
  const s = pref();
  const want = {theme:s.theme, density:s.density, text_size:String(s.text_size), language:s.language, mark_read:s.mark_read, prefer_text:s.prefer_text ? 'text' : 'html'};
  sdlg.querySelectorAll('input[type="radio"]').forEach(r => { if (r.name in want) r.checked = r.value === want[r.name]; });
  $('s-every').value = String(s.check_every);
  $('s-updates').checked = s.updates === 'daily';
}
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
  if (tab === 'move') renderMove();
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
  else if (el.id === 's-every') changeSettings({check_every:Number(v)});
  else if (el.id === 's-updates') changeSettings({updates:el.checked ? 'daily' : 'off'});
  else if (el.dataset.part) move_.choice[el.dataset.part] = el.checked;
});
forcedColors.addEventListener('change', () => { if (sdlg.open) $('s-forced').hidden = !forcedColors.matches; });
$('s-check').addEventListener('click', () => { sdlg.close(); openUpdates(); });
$('s-close').addEventListener('click', () => sdlg.close());
sdlg.addEventListener('close', () => listEl.focus({preventScroll:true}));

// Import and export. The file holds settings, rules, identities and account servers, never
// passwords. Importing shows what would change first; nothing changes until Import.
const SETTING_LABELS = {theme:N_('Theme'), language:N_('Language'), density:N_('Rows'), text_size:N_('Text size'), prefer_text:N_('HTML mail'),
  mark_read:N_('Mark messages as read'), check_every:N_('Check for new mail'), updates:N_('Updates')};
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
    sdlg.close();
    editing = null;
    openForm(a.setup, '', null, t('The servers come from the settings file: {servers}. Check them before you enter your password.', {servers}));
  }
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
  if (e.key === 'Escape'){
    if (state.compose){ e.preventDefault(); closeCompose().then(ok => ok && say(t('Saved to Drafts.'))); return; }
    if (tg === qEl){ qEl.value = ''; setFilter(''); listEl.focus(); return; }
  }
  // Tab in the filter box: this folder or all mail, while there is something to look for.
  if (tg === qEl && e.key === 'Tab' && !e.shiftKey && !e.ctrlKey && !e.altKey && qEl.value.trim()){ e.preventDefault(); toggleScope(); return; }
  if (tg.closest && (tg.closest('input, textarea, select') || tg.closest('.split'))) return;
  if (e.ctrlKey || e.metaKey || e.altKey) return;
  const k = e.key;
  if (k === 'ArrowDown') move(1);
  else if (k === 'ArrowUp') move(-1);
  else if (k === 'Home') move(-Infinity);
  else if (k === 'End') move(Infinity);
  else if (k === 'Delete') del();
  else if (k === 'Enter' && allMail()) openInFolder();
  else if (k === 'r' || k === 'R') openCompose(e.shiftKey ? 'replyall' : 'reply');
  else if (k === 'F' && e.shiftKey) openCompose('forward');
  else if (k === 'f') toggleFlag();
  else if (k === 'u' || k === 'U') toggleUnread();
  else if (k === 'n' || k === 'N') openCompose('new');
  else if (k === 'e' || k === 'E') openCompose('edit');
  else if (k === 'h' || k === 'H') setView(!pref().prefer_text);
  else if (k === 'z' || k === 'Z') undo();
  else if (k === '/') qEl.focus();
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
  // Real accounts: check now, then as often as the settings say, quietly.
  if (ACCOUNTS.length) getMail({quiet:true});
  else openAccountDialog(); // first start: straight to adding an account
}
boot();
})();
