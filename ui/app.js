// Telex UI. Plain JS, no build step: Tauri serves this folder as-is.
// All mail data comes from the Rust core (crates/telex-core) through `invoke`.
(() => {
'use strict';

const invoke = window.__TAURI__?.core?.invoke;

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
  flagFill: '<svg class="flag" viewBox="0 0 16 16" fill="currentColor" aria-label="Flagged"><path d="M3 15V2h10l-2.2 3.5L13 9H4.5v6z"/></svg>',
  clip: sv('<path d="M10.5 5L6 9.5a1.4 1.4 0 002 2L13 6.5a2.8 2.8 0 00-4-4L4 7.5a4.2 4.2 0 006 6l3.5-3.5"/>', 'class="clip"'),
  rows: sv('<path d="M2.5 4h11M2.5 8h11M2.5 12h11"/>'),
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
// Commands that change mail; the core queues them for the server and we push shortly after.
// send_message counts: a sent message's copy for the server's Sent folder is queued too.
const CHANGES = new Set(['set_unread', 'set_flagged', 'move_message', 'delete_message', 'send_message', 'run_rules']);
async function call(cmd, args){
  try {
    const result = await invoke(cmd, args);
    if (CHANGES.has(cmd)) schedulePush();
    return result;
  } catch (e){ say(String(e), 8000); throw e; }
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
  try { r = await invoke('push_changes'); } catch (e){ say(String(e), 8000); return; }
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
const DOW = ['Sun','Mon','Tue','Wed','Thu','Fri','Sat'];
const hm = d => pad(d.getHours()) + ':' + pad(d.getMinutes());
const dmy = d => pad(d.getDate()) + '.' + pad(d.getMonth() + 1) + '.' + d.getFullYear();
function fmtList(d){
  const day = x => new Date(x.getFullYear(), x.getMonth(), x.getDate());
  const diff = Math.round((day(new Date()) - day(d)) / 864e5);
  if (diff <= 0) return hm(d);
  if (diff < 7) return DOW[d.getDay()] + ' ' + hm(d);
  return dmy(d);
}
const fmtFull = d => DOW[d.getDay()] + ', ' + dmy(d) + ' ' + hm(d);
const fmtSize = kb => kb >= 1024 ? (kb / 1024).toFixed(1) + ' MB' : kb + ' KB';
const nameOf = addr => {
  const s = String(addr || '').trim();
  if (!s) return '(no recipient)';
  const m = s.match(/^\s*"?([^"<]+?)"?\s*</);
  return m ? m[1] : s;
};
const acctId = key => key.split('/')[0];
const folderOf = key => { const [a, f] = key.split('/'); return ACCT[a]?.folders.find(x => x.id === f); };
const folderName = key => folderOf(key)?.name ?? key;
const kindOf = key => folderOf(key)?.kind;
const isOut = key => ['sent', 'drafts', 'outbox'].includes(kindOf(key));
const keyOfKind = (acct, kind) => { const f = ACCT[acct]?.folders.find(x => x.kind === kind); return f ? acct + '/' + f.id : null; };
const homeOf = acct => keyOfKind(acct, 'inbox') || (ACCT[acct]?.folders[0] ? acct + '/' + ACCT[acct].folders[0].id : '');
const countOf = key => COUNTS[key] ?? {total:0, unread:0};
const num = n => n.toLocaleString('en');
// Each account has a color, shown as a dot once there is more than one account.
const adot = acct => { const i = ACCOUNTS.findIndex(a => a.id === acct); return i < 0 ? '' : `<i class="adot a${i % 6}" aria-hidden="true"></i>`; };
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
  density: saved.density === 'comfortable' ? 'comfortable' : 'compact',
  preferText: saved.preferText === true, // show plain text even when a message has HTML
  updates: ['daily', 'off'].includes(saved.updates) ? saved.updates : null, // null: not asked yet
  updateChecked: Number(saved.updateChecked) || 0, // when a check last got an answer (ms)
};
const win = $('win'), rowsEl = $('rows'), listEl = $('list'), previewEl = $('preview'), treeEl = $('tree'), qEl = $('q');
if (saved.treeW) win.style.setProperty('--tree-w', saved.treeW + 'px');
if (saved.listH) win.style.setProperty('--list-h', saved.listH + '%');
const persist = () => store.set({
  treeW: parseInt(win.style.getPropertyValue('--tree-w')) || undefined,
  listH: parseFloat(win.style.getPropertyValue('--list-h')) || undefined,
  density: state.density,
  preferText: state.preferText || undefined,
  updates: state.updates || undefined,
  updateChecked: state.updateChecked || undefined,
  scope: state.scope === 'all' ? 'all' : undefined,
  more: Object.keys(state.more).length ? state.more : undefined,
});

/* ---------- toolbar ---------- */
[['b-get','get','Get mail'],['b-new','neu','New'],['b-reply','reply','Reply'],['b-replyall','replyall','Reply all'],
 ['b-forward','forward','Forward'],['b-flag','flag','Flag'],['b-delete','trash','Delete'],['b-rules','rules','Rules']]
  .forEach(([id, ic, label]) => { $(id).innerHTML = I[ic] + `<span class="lbl">${label}</span>`; });
$('search-ic').innerHTML = I.search;
function paintDensity(){
  win.dataset.density = state.density;
  $('b-density').innerHTML = I.rows + `<span class="lbl">${state.density === 'compact' ? 'Compact' : 'Comfortable'}</span>`;
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
  $('st-folder').innerHTML = !ACCOUNTS.length ? 'No account yet'
    : allMail() ? `All mail · ${num(found)} ${found === 1 ? 'match' : 'matches'}`
    : `${adot(acctId(state.folder))}${esc(folderName(state.folder))} · ${num(total)} ${total === 1 ? 'message' : 'messages'}${unread ? `, ${num(unread)} unread` : ''}`;
  $('st-check').textContent = lastCheck ? 'Last check ' + hm(lastCheck) : '';
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
        items += `<li><button type="button" class="fold more" data-more="${esc(a.id)}" aria-expanded="${more}">${I.caret}<span>${more ? 'Fewer folders' : `${rest.length} more`}</span>${unread ? `<span class="cnt">${num(unread)}</span>` : ''}</button></li>`;
      }
      items += '</ul>';
    }
    return `<button type="button" class="acct" data-acct="${esc(a.id)}" aria-expanded="${open}" title="${esc(a.name)} · ${esc(a.email)}">${I.caret}${adot(a.id)}<span class="acct-mail">${esc(a.email)}</span></button>${items}`;
  }).join('') + `<button type="button" class="add-acct" data-act="add-account">${I.plus}<span>${ACCOUNTS.length ? 'Add account' : 'Add your account'}</span></button>`;
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
  const out = isOut(m.f), who = out ? (allMail() ? 'To: ' : '') + nameOf(m.to) : m.from, tip = out ? m.to : `${m.from} <${m.email}>`;
  const cls = (m.unread ? 'unread ' : '') + (m.id === sel ? 'sel' : '');
  return `<tr data-id="${m.id}" class="${cls}" aria-selected="${m.id === sel}">
<td class="c-ic"><span class="ic"><i class="dot"></i>${m.flagged ? I.flagFill : '<i class="sp"></i>'}${m.hasAtt ? I.clip : '<i class="sp"></i>'}</span></td>
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
    rowsEl.innerHTML = !ACCOUNTS.length || list.seq === 0 ? '' : `<tr class="empty"><td colspan="${cols()}">${state.q ? `No messages match “${esc(state.q)}”${allMail() ? ' in any account' : ''}. Press Esc to clear the filter.` : 'This folder is empty.'}</td></tr>`;
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
  catch { try { p = await fetchPage('first'); } catch (e){ say(String(e), 8000); return; } }
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
  $('from-label').textContent = out ? 'To' : 'From';
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

// Mark as read after the message has been on screen for a moment, so arrowing past doesn't.
let readTimer;
function select(id, silent){
  state.sel[selKey()] = id;
  selRow = list.rows.find(r => r.id === id) ?? (selRow?.id === id ? selRow : null);
  clearTimeout(readTimer);
  const m = current();
  if (m && m.unread && !silent){
    readTimer = setTimeout(() => {
      call('set_unread', {id:m.id, unread:false}).then(() => { changed(m, {unread:false}); reorder(); }).catch(() => {});
    }, 600);
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
    if (img) return `<div class="imgph${loaded ? ' loaded' : ''}">${loaded ? esc(img[1]) : 'Blocked image: ' + esc(img[1])}</div>`;
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
  if (spam){ say('Links are off in Spam. Move the message out of Spam to use them.'); return; }
  if (/^mailto:/i.test(h)){
    const to = decodeURIComponent(h.slice(7).split('?')[0]);
    openCompose('new', {to});
  } else if (/^https?:\/\//i.test(h)){
    call('open_link', {url:h}).then(() => say('Opened in your browser.', 2500)).catch(() => {});
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
    const n = hv.blocked === 1 ? 'A remote image is' : `${hv.blocked} remote images are`;
    const always = spam ? '' : ` <button type="button" class="linkbtn" data-act="trust">Always load from ${who}</button>`;
    return `<div class="bar">${n} blocked so the sender can’t see when you open this. <button type="button" class="linkbtn" data-act="images">Load images</button>${always}</div>`;
  }
  if (hv.loaded && hv.trusted && !spam){
    return `<div class="bar quiet">Images from ${who} load automatically. <button type="button" class="linkbtn" data-act="untrust">Stop</button></div>`;
  }
  if (hv.loaded && hv.blocked) return `<div class="bar quiet">${hv.blocked === 1 ? 'One image' : hv.blocked + ' images'} couldn’t be loaded.</div>`;
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
    previewEl.innerHTML = `<div class="welcome">${I.mail}<h2>Welcome to Telex</h2>
<p>Add your mail account to start. Telex keeps your mail on this PC and talks only to your mail servers.</p>
<button type="button" class="btn primary" data-act="add-account">Add your mail account</button></div>`;
    return;
  }
  if (!s){
    previewEl.innerHTML = `<div class="pempty">${I.mail}<div>${list.total || allMail() ? 'Select a message to read it here.' : 'Nothing in ' + esc(folderName(state.folder)) + '.'}</div></div>`;
    return;
  }
  let m, hv;
  try {
    m = await full(s.id);
    hv = await htmlView(s.id, state.preferText ? false : state.images.has(s.id) ? true : null);
  } catch (e){ if (seq === pvSeq) previewEl.innerHTML = `<div class="pempty">${esc(String(e))}</div>`; return; }
  if (seq !== pvSeq || state.compose) return;
  const spam = kindOf(s.f) === 'spam'; // links and trusted images stay off in Spam
  const asHtml = !!hv && !state.preferText;
  const views = hv ? `<span class="seg" role="group" aria-label="Show as"><button type="button" data-view="html" aria-pressed="${asHtml}" title="Show the HTML (H)">HTML</button><button type="button" data-view="text" aria-pressed="${!asHtml}" title="Show plain text (H)">Text</button></span>` : '';
  const images = asHtml ? imagesBar(m, hv, spam)
    : !hv && m.remote_images && !state.images.has(m.id) ? `<div class="bar">Remote images are blocked so the sender can’t see when you open this. <button type="button" class="linkbtn" data-act="images">Load images</button></div>` : '';
  previewEl.innerHTML = `<article>
<header class="ph">
  <div class="ph-top"><h2 class="ph-subj">${esc(m.subject)}</h2>${views}${s.flagged ? `<span class="pill">${I.flag}Flagged</span>` : ''}</div>
  <dl class="ph-grid">
    <dt>From</dt><dd>${esc(m.from_name)} &lt;${esc(m.from_email)}&gt;</dd>
    <dt>To</dt><dd>${esc(m.to || '(no recipient)')}</dd>
    ${m.cc ? `<dt>Cc</dt><dd>${esc(m.cc)}</dd>` : ''}
    <dt>Date</dt><dd class="mono">${fmtFull(parseDate(m.date))} · ${fmtSize(m.size_kb)}</dd>
  </dl>
  ${attsHtml(m.attachments)}
</header>
${m.note ? `<div class="bar note${spam || kindOf(s.f) === 'outbox' ? ' warn' : ''}">${esc(m.note)}</div>` : ''}
${images}
${asHtml ? '<div class="pbody html"><iframe class="hframe" sandbox="allow-same-origin" referrerpolicy="no-referrer" title="Message"></iframe></div>' : `<div class="pbody">${bodyHtml(m)}</div>`}
</article>`;
  if (asHtml) mountFrame(previewEl.querySelector('.hframe'), hv.document, spam);
  previewEl.scrollTop = 0;
}
function setView(text){
  state.preferText = text; persist();
  if (!state.compose) renderPreview();
  say(text ? 'Showing plain text. Press H for HTML.' : 'Showing HTML. Press H for plain text.', 2500);
}
previewEl.addEventListener('click', async e => {
  const link = e.target.closest('.pbody a[href]');
  if (link){ e.preventDefault(); const s = current(); followLink(link.getAttribute('href'), s && (kindOf(s.f) === 'spam')); return; }
  const view = e.target.closest('[data-view]');
  if (view){ setView(view.dataset.view === 'text'); return; }
  const att = e.target.closest('[data-att]'), act = e.target.closest('[data-act]');
  if (att){ say(`Opening attachments isn't built yet (${att.title}).`); return; }
  if (!act) return;
  const m = current();
  if (act.dataset.act === 'images' && m){
    state.images.add(m.id); say('Loading images…', 0);
    await renderPreview(); say('Images loaded for this message only.');
  }
  if ((act.dataset.act === 'trust' || act.dataset.act === 'untrust') && m){
    const trust = act.dataset.act === 'trust', email = (await full(m.id)).from_email;
    try { await call('trust_sender', {email, trusted:trust}); } catch { return; }
    htmlViews.clear();
    if (trust) state.images.add(m.id); else state.images.delete(m.id);
    if (trust) say('Loading images…', 0);
    await renderPreview();
    say(trust ? `Images from ${email} will load automatically, except in Spam.` : `Images from ${email} are blocked again.`);
  }
  if (act.dataset.act === 'discard'){ state.compose = null; renderPreview(); say('Message discarded.'); listEl.focus(); }
  if (act.dataset.act === 'draft') closeCompose().then(ok => ok && say('Saved to Drafts.'));
  if (act.dataset.act === 'add-account') openAccountDialog();
});

/* ---------- compose ---------- */
// kind: new, reply, replyall or forward, started by the core with the identity's signature
// and templates; or edit, which reopens a draft or an unsent message in the Outbox.
async function openCompose(kind, preset = {}){
  if (!ACCOUNTS.length){ say('Add a mail account first.'); openAccountDialog(); return; }
  const s = current();
  if (kind !== 'new' && !s){ say('Select a message first.'); return; }
  if (kind === 'edit' && !['drafts', 'outbox'].includes(kindOf(s.f))){ say('Only drafts and unsent messages in the Outbox can be edited.'); return; }
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
  } catch (e){ say(String(e), 8000); return; }
  Object.assign(c, preset); // e.g. {to} from a mailto: link
  state.compose = c;
  renderCompose();
}
const allIdentities = () => ACCOUNTS.flatMap(a => (a.identities || []).map(i => ({...i, account:a.id})));
const composeNote = c => !ACCT[c.account]?.smtp ? 'This account has no SMTP server yet: edit it under Add account to send.' : '';
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
  const c = state.compose, a = ACCT[c.account];
  const title = {new:'New message', reply:'Reply', replyall:'Reply to all', forward:'Forward', edit:'Edit'}[c.kind];
  const note = composeNote(c);
  previewEl.innerHTML = `<form class="compose" id="cform" novalidate>
<div class="ctitle">${title}<label class="cfrom">from <select id="c-from" aria-label="From">${fromOptions(c)}</select></label></div>
<div class="cf">
  <label for="c-to">To</label><input id="c-to" value="${esc(c.to)}" autocomplete="off" spellcheck="false">
  <label for="c-cc">Cc</label><input id="c-cc" value="${esc(c.cc)}" autocomplete="off" spellcheck="false">
  <label for="c-subj">Subject</label><input id="c-subj" value="${esc(c.subject)}" autocomplete="off">
</div>
${attsHtml(c.attachments)}
<textarea id="c-body" aria-label="Message text">
${esc(c.body)}</textarea>
<div class="cbar">
  <button class="btn primary" type="submit">Send <kbd>Ctrl+Enter</kbd></button>
  <button class="btn" type="button" data-act="draft">Save draft <kbd>Esc</kbd></button>
  <button class="btn" type="button" data-act="discard">Discard</button>
  <span class="cnote">${esc(note)}</span>
</div></form>`;
  $('cform').addEventListener('submit', e => { e.preventDefault(); send(); });
  $('c-from').addEventListener('change', e => changeIdentity(Number(e.target.value)));
  const t = $('c-body');
  if (c.kind === 'new' || c.kind === 'forward' || (c.kind === 'edit' && !c.to)) $('c-to').focus();
  else { t.focus(); t.setSelectionRange(0, 0); t.scrollTop = 0; }
}
const draftOf = c => ({account:c.account, to:c.to, cc:c.cc, subject:c.subject || '(no subject)', body:c.body.replace(/^\n+/, ''),
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
// Real mail goes to the Outbox and out at once. It's only called sent once the SMTP
// server has taken it; otherwise it waits in the Outbox and the status says why.
async function send(){
  if (!$('c-to').value.trim() && !$('c-cc').value.trim()){ say('Add at least one recipient in To.'); $('c-to').focus(); return; }
  const btn = document.querySelector('#cform .btn.primary');
  if (!btn || btn.disabled) return;
  btn.disabled = true; btn.firstChild.textContent = 'Sending… ';
  say('Sending…', 0);
  let r;
  try { r = await call('send_message', {draft:draftOf(readCompose())}); }
  catch { btn.disabled = false; btn.firstChild.textContent = 'Send '; return; } // e.g. a bad address: fix it and try again
  state.compose = null;
  renderPreview();
  await refresh();
  listEl.focus();
  const sent = r.sent.length === 1 ? `Sent ${r.sent[0]}.` : r.sent.length ? `Sent ${r.sent.length} messages.` : '';
  if (r.errors.length) say([sent, r.errors.join(' · ')].filter(Boolean).join(' '), 15000);
  else say(sent || 'In the Outbox. It goes out with the next check.', 6000);
}

/* ---------- actions ---------- */
function toggleFlag(){
  const m = current(); if (!m) return;
  const flagged = !m.flagged;
  changed(m, {flagged}); if (!state.compose) renderPreview();
  call('set_flagged', {id:m.id, flagged})
    .then(() => { say(flagged ? 'Flagged.' : 'Flag removed.', 2000); reorder(); })
    .catch(() => { changed(m, {flagged:!flagged}); if (!state.compose) renderPreview(); });
}
function toggleUnread(){
  const m = current(); if (!m) return;
  clearTimeout(readTimer);
  const unread = !m.unread;
  call('set_unread', {id:m.id, unread}).then(() => {
    changed(m, {unread}); reorder();
    say(unread ? 'Marked as unread.' : 'Marked as read.', 2000);
  }).catch(() => {});
}
// Deleting from Trash can't be undone (on the server either), so it takes a second Del.
let armedDelete = null; // {id, at}
async function del(){
  const m = current(); if (!m) return;
  if (kindOf(m.f) === 'trash'){
    if (armedDelete?.id !== m.id || Date.now() - armedDelete.at > 5000){
      armedDelete = {id:m.id, at:Date.now()};
      say('Press Del again to delete this message for good.', 5000);
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
    say('Deleted for good.');
  } else {
    state.lastDeleted = {id:m.id, from:m.f};
    say('Moved to Trash. Press Z to undo.');
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
  say('Restored to ' + folderName(d.from) + '.');
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
  if (!quiet) say(`Checking ${n} ${n === 1 ? 'account' : 'accounts'}…`, 0);
  try {
    const r = await invoke('check_mail');
    if (r.busy){ setTimeout(() => getMail({quiet, first}), 2000); return; } // changes are being sent
    lastCheck = new Date();
    await refresh();
    const ids = r.new, count = `${ids.length} ${first ? '' : 'new '}${ids.length === 1 ? 'message' : 'messages'}`;
    let text = '';
    if (first) text = `${count} downloaded.`;
    else if (ids.length){
      const where = r.new_in && folderOf(r.new_in) ? `${ACCT[acctId(r.new_in)].email} › ${folderName(r.new_in)}` : '';
      text = `${count}${where ? ' in ' + where : ''}.`;
    } else if (!quiet) text = 'No new messages.';
    if (r.sorted) text = [text, `Rules sorted ${r.sorted}.`].filter(Boolean).join(' ');
    if (r.sent?.length) text = [r.sent.length === 1 ? `Sent ${r.sent[0]}.` : `Sent ${r.sent.length} messages.`, text].filter(Boolean).join(' ');
    if (r.errors.length) text = [r.errors.join(' · '), text].filter(Boolean).join(' · ');
    say(text, r.errors.length ? 15000 : 6000);
  } catch (e){ if (!quiet) say(String(e), 8000); }
  finally {
    b.classList.remove('busy');
    if (checkAgain){ const next = checkAgain; checkAgain = null; getMail(next); }
  }
}

// Progress while a folder downloads; each finished folder shows up right away.
let reloadTimer;
window.__TAURI__?.event?.listen('sync-progress', ({payload: p}) => {
  say(`${p.account} › ${p.folder}: ${p.done} of ${p.total}`, 0);
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
$('b-density').addEventListener('click', () => { state.density = state.density === 'compact' ? 'comfortable' : 'compact'; paintDensity(); persist(); drawRows(); });
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
  $('q-scope').textContent = all ? 'All mail' : 'This folder';
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
  app_password: p => `${p} takes an app password here, not your usual password. You make one in your ${p} account’s security settings.`,
  turn_on_imap: p => `Turn on access for mail programs (IMAP) in your ${p} settings first.`,
  turn_on_imap_and_app_password: p => `Turn on IMAP in your ${p} settings first, and use an app password made there, not your usual password.`,
};
const UNSUPPORTED_TEXT = {
  microsoft_sign_in: p => `${p} lets other mail apps in only with Microsoft sign-in, which Telex doesn’t have yet. It’s next on the roadmap.`,
  proton_bridge: () => 'Proton Mail works with other mail apps only through Proton Mail Bridge, which Telex can’t use yet.',
};
const SOURCE_TEXT = {
  list: d => `Telex knows ${d.provider}`,
  autoconfig: d => `from ${d.provider ? d.provider + '’s' : 'your provider’s'} setup file`,
  srv: () => 'from your domain’s DNS records',
  mx: d => `your domain’s mail is handled by ${d.provider}`,
  guess: () => 'found by trying the usual server names',
};
function hint(el, d){
  const p = d?.provider ?? 'your provider';
  const text = d?.unsupported ? UNSUPPORTED_TEXT[d.unsupported]?.(p) : d?.note ? NOTE_TEXT[d.note]?.(p) : '';
  el.hidden = !text;
  el.classList.toggle('warn', !!d?.unsupported);
  el.innerHTML = text ? esc(text) + (d.help ? ` <button type="button" class="linkbtn" data-help="${esc(d.help)}">Open that page</button>` : '') : '';
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
  showView('start');
  if (!dlg.open) dlg.showModal();
  $(me ? 'w-email' : 'w-name').focus();
}
function renderAccountList(){
  const real = ACCOUNTS.filter(a => a.imap);
  $('a-list').innerHTML = real.length ? '<h3>Connected</h3>' + real.map(a => {
    const where = `${a.email} · ${a.imap.host}${a.smtp ? '' : ' · no SMTP server yet'}`;
    return `<div class="dacct"><span class="nm">${esc(a.name)}</span><span class="em" title="${esc(where)}">${esc(where)}</span>`
      + `<button type="button" class="btn" data-ids="${esc(a.id)}">Identities</button><button type="button" class="btn" data-edit="${esc(a.id)}">Edit</button><button type="button" class="btn danger" data-remove="${esc(a.id)}">Remove</button></div>`;
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
  steps[steps.length - 1].detail = payload === 'guess' ? 'Trying the usual server names…' : `Asking ${domain} and its DNS records…`;
  renderSteps();
});
const serverNames = s => s.host === s.smtp_host ? s.host : `${s.host} and ${s.smtp_host}`;

wForm.addEventListener('submit', async e => {
  e.preventDefault();
  if (wizardBusy) return;
  const name = $('w-name').value.trim(), email = $('w-email').value.trim(), password = $('w-pass').value;
  const domain = email.split('@')[1] || '';
  wErr.textContent = '';
  if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email)){ wErr.textContent = 'Enter your email address, e.g. anna@example.com.'; $('w-email').focus(); return; }
  if (!password){ wErr.textContent = 'Enter the password.'; $('w-pass').focus(); return; }
  const seq = ++wizardSeq;
  steps = [];
  $('w-work-title').textContent = 'Setting up ' + email;
  $('w-stop').disabled = false;
  showView('work');
  wizardBusy = 'find';
  step(`Looking for the mail servers of ${domain}`);
  let d = null;
  try { d = await invoke('detect_settings', {email}); } catch {}
  if (seq !== wizardSeq) return; // cancelled meanwhile
  wizardBusy = null;
  if (!d){
    openForm({name, email, password}, `Telex couldn’t find the mail servers for ${domain}. Enter them here: your provider’s help pages list them as IMAP and SMTP servers.`);
    return;
  }
  if (d.unsupported){ showView('start'); hint($('w-hint'), d); $('w-email').focus(); return; }
  finishStep('done', `Found ${serverNames(d.setup)}`, SOURCE_TEXT[d.source]?.(d) ?? '');
  const setup = {...d.setup, name, email};
  wizardBusy = 'login';
  $('w-stop').disabled = true; // the account is saved the moment the login works
  step(`Logging in to ${serverNames(d.setup)}`);
  let account;
  try { account = await invoke('add_account', {setup, password}); }
  catch (err){
    wizardBusy = null;
    openForm({...setup, password}, String(err), d);
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
function openForm(v, error = '', d = null){
  aForm.reset();
  $('a-title').textContent = 'Server settings';
  $('a-go').querySelector('span').textContent = 'Connect';
  $('a-email').value = v.email ?? ''; $('a-name').value = v.name ?? '';
  $('a-host').value = v.host ?? ''; $('a-port').value = v.port ?? 993;
  $('a-user').value = v.username && v.username !== v.email ? v.username : '';
  $('a-pass').value = v.password ?? ''; $('a-pass').placeholder = '';
  $('a-smtp').value = v.smtp_host ?? ''; $('a-sport').value = v.smtp_port ?? 465; $('a-sec').value = v.smtp_security ?? 'tls';
  placeholders();
  aErr.textContent = error;
  hint($('a-hint'), d);
  showView('form');
  $(!v.email ? 'a-email' : !v.host ? 'a-host' : 'a-go').focus();
}
// Fill the form with an account's settings; the password stays empty and is kept if left so.
function editAccount(id){
  const a = ACCT[id]; if (!a?.imap) return;
  editing = id;
  aForm.reset();
  aErr.textContent = ''; hint($('a-hint'), null);
  $('a-title').textContent = 'Edit ' + a.name;
  $('a-go').querySelector('span').textContent = 'Save';
  $('a-email').value = a.email; $('a-name').value = a.name;
  $('a-host').value = a.imap.host; $('a-port').value = a.imap.port; $('a-user').value = a.imap.username;
  $('a-smtp').value = a.smtp?.host ?? ''; $('a-sec').value = a.smtp?.security ?? 'tls';
  $('a-sport').value = a.smtp?.port ?? 465;
  $('a-pass').value = ''; $('a-pass').placeholder = 'Leave empty to keep the saved password';
  placeholders();
  showView('form');
  $(a.smtp ? 'a-pass' : 'a-smtp').focus();
}
function placeholders(){
  const email = $('a-email').value.trim(), domain = email.split('@')[1] || '';
  $('a-host').placeholder = domain.includes('.') ? 'e.g. mail.' + domain : 'mail.example.com';
  $('a-user').placeholder = email || 'Same as email';
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
  if (badPort(port) || !port){ aErr.textContent = 'The IMAP port is a number from 1 to 65535. For SSL/TLS it is usually 993.'; $('a-port').focus(); return; }
  if (badPort(smtpPort)){ aErr.textContent = 'The SMTP port is a number from 1 to 65535: usually 465 for SSL/TLS, 587 for STARTTLS.'; $('a-sport').focus(); return; }
  const setup = {name:$('a-name').value, email:$('a-email').value, host:$('a-host').value, port, username:$('a-user').value,
    smtp_host:$('a-smtp').value, smtp_port:smtpPort, smtp_security:$('a-sec').value};
  const wasEditing = editing;
  aErr.textContent = '';
  go.disabled = true; label.textContent = 'Connecting…';
  wizardBusy = 'login';
  let account;
  try { account = await invoke('add_account', {setup, password:$('a-pass').value}); }
  catch (err){ aErr.textContent = String(err); return; }
  finally { wizardBusy = null; go.disabled = false; label.textContent = wasEditing ? 'Save' : 'Connect'; }
  await connected(account, !!wasEditing);
});
async function connected(account, wasEditing){
  wForm.reset(); aForm.reset();
  dlg.close();
  await loadTree();
  if (wasEditing){
    refresh();
    say(`Saved ${account.name}.`);
    getMail({quiet:true}); // the Outbox may have waited for these settings
    return;
  }
  setFolder(homeOf(account.id));
  say(`Connected to ${account.email}. Downloading mail…`, 0);
  getMail({first:true});
}
$('a-list').addEventListener('click', async e => {
  const ed = e.target.closest('[data-edit]');
  if (ed){ editAccount(ed.dataset.edit); return; }
  const ids = e.target.closest('[data-ids]');
  if (ids){ dlg.close(); openIdentities(ids.dataset.ids); return; }
  const b = e.target.closest('[data-remove]');
  if (!b) return;
  if (b.dataset.armed !== '1'){ b.dataset.armed = '1'; b.textContent = 'Remove it and its local mail'; return; }
  try { await invoke('remove_account', {id:b.dataset.remove}); } catch (err){ wErr.textContent = String(err); return; }
  await refresh();
  renderAccountList();
  say('Account removed from Telex. The mail on the server is untouched.');
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
  $('ids-title').textContent = 'Identities · ' + a.name;
  $('i-list').innerHTML = (a.identities || []).map(i => `<div class="rrule" data-id="${i.id}">
<div class="rtext"><div class="rname">${esc(i.name)}${i.is_default ? ' <span class="tag">Default</span>' : ''}</div>
<div class="rsum">${esc(i.email)}${i.signature ? ' · ' + esc(i.signature.split('\n')[0]) : ''}</div></div>
<button type="button" class="btn" data-i="edit">Edit</button><button type="button" class="btn danger" data-i="delete">Delete</button></div>`).join('');
  $('i-new').focus();
}
function showIdentityForm(existing){
  const a = ACCT[idAccount], base = a.identities.find(i => i.is_default) ?? a.identities[0] ?? {};
  identity = existing ? structuredClone(existing) : {id:0, account:idAccount, name:'', email:a.email, signature:base.signature ?? '',
    reply_header:base.reply_header ?? '', forward_header:base.forward_header ?? '', is_default:false};
  $('i-list-view').hidden = true; $('i-form').hidden = false; $('i-form-err').textContent = '';
  $('i-form-title').textContent = existing ? 'Edit identity' : 'New identity';
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
  try { await invoke('save_identity', {identity}); } catch (err){ $('i-form-err').textContent = String(err); return; }
  await loadTree();
  say(`Saved ${identity.name.trim()}.`);
  showIdentityList();
});
$('i-list').addEventListener('click', async e => {
  const b = e.target.closest('[data-i]'), row = e.target.closest('[data-id]');
  if (!b || !row) return;
  const i = ACCT[idAccount].identities.find(x => x.id === Number(row.dataset.id));
  if (!i) return;
  if (b.dataset.i === 'edit'){ showIdentityForm(i); return; }
  if (b.dataset.armed !== '1'){ b.dataset.armed = '1'; b.textContent = 'Delete it'; return; }
  try { await invoke('delete_identity', {id:i.id}); } catch (err){ $('i-err').textContent = String(err); return; }
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
const FIELDS = [['from', 'From'], ['to', 'To or Cc'], ['subject', 'Subject'], ['body', 'Text'], ['anywhere', 'Anywhere']];
const OPS = [['contains', 'contains'], ['not_contains', 'doesn’t contain'], ['is', 'is'], ['starts_with', 'starts with'], ['ends_with', 'ends with']];
const ACTIONS = [['move', 'Move to folder'], ['mark_read', 'Mark as read'], ['flag', 'Flag'], ['delete', 'Move to Trash'], ['auto_reply', 'Answer automatically']];
const labelOf = (list, v) => list.find(([k]) => k === v)?.[1] ?? v;
const options = (list, v) => list.map(([k, l]) => `<option value="${esc(k)}"${k === v ? ' selected' : ''}>${esc(l)}</option>`).join('');
// Folders a rule can move mail to: the account's own, not its Outbox.
const moveTargets = acct => (ACCT[acct]?.folders ?? []).filter(f => f.kind !== 'outbox').map(f => [acct + '/' + f.id, f.name]);

function ruleSummary(r){
  const conds = r.conditions.map(c => `${labelOf(FIELDS, c.field)} ${labelOf(OPS, c.op)} “${c.value}”`).join(r.match_all ? ' and ' : ' or ');
  const acts = r.actions.map(a => a.type === 'move' ? 'move to ' + folderName(a.folder) : labelOf(ACTIONS, a.type).toLowerCase()).join(', ');
  return `If ${conds}: ${acts}.` + (r.account ? ` ${ACCT[r.account]?.email ?? r.account} only.` : '') + (r.stop ? ' Then stop.' : '');
}
async function openRules(){
  rdlg.showModal();
  await showRuleList();
}
async function showRuleList(){
  $('r-form').hidden = true; $('r-list-view').hidden = false; $('r-err').textContent = '';
  try { RULES = await invoke('list_rules'); } catch (e){ $('r-err').textContent = String(e); return; }
  $('r-folder').textContent = folderName(state.folder);
  $('r-list').innerHTML = RULES.length ? RULES.map((r, i) => `<div class="rrule${r.enabled ? '' : ' off'}" data-id="${r.id}">
<input type="checkbox" data-r="toggle"${r.enabled ? ' checked' : ''} title="On or off" aria-label="Rule ${esc(r.name)} on">
<div class="rtext"><div class="rname">${esc(r.name)}</div><div class="rsum" title="${esc(ruleSummary(r))}">${esc(ruleSummary(r))}</div></div>
<button type="button" class="btn icon" data-r="up" title="Run earlier"${i ? '' : ' disabled'}>↑</button>
<button type="button" class="btn icon" data-r="down" title="Run later"${i < RULES.length - 1 ? '' : ' disabled'}>↓</button>
<button type="button" class="btn" data-r="edit">Edit</button><button type="button" class="btn danger" data-r="delete">Delete</button></div>`).join('')
    : '<p class="dnote">No rules yet. A rule can file invoices, flag mail from a client, or answer while you’re away.</p>';
  $('r-new').focus();
}
function showRuleForm(existing){
  const firstAccount = ACCOUNTS.find(a => a.imap)?.id ?? ACCOUNTS[0]?.id ?? null;
  rule = existing ? structuredClone(existing) : {id:0, name:'', enabled:true, account:firstAccount, match_all:true,
    conditions:[{field:'from', op:'contains', value:''}], actions:[{type:'flag'}], stop:false};
  $('r-list-view').hidden = true; $('r-form').hidden = false; $('r-form-err').textContent = '';
  $('r-form-title').textContent = existing ? 'Edit rule' : 'New rule';
  $('r-name').value = rule.name;
  $('r-acct').innerHTML = '<option value="">All accounts</option>' + ACCOUNTS.map(a => `<option value="${esc(a.id)}"${a.id === rule.account ? ' selected' : ''}>${esc(a.name)} · ${esc(a.email)}</option>`).join('');
  $('r-match').value = rule.match_all ? 'all' : 'any';
  $('r-stop').checked = rule.stop;
  renderConditions(); renderActions();
  $('r-name').focus();
}
function renderConditions(){
  $('r-conds').innerHTML = rule.conditions.map((c, i) => `<div class="rrow" data-i="${i}">
<select data-k="field" aria-label="Where">${options(FIELDS, c.field)}</select><select data-k="op" aria-label="How">${options(OPS, c.op)}</select>
<input data-k="value" value="${esc(c.value)}" placeholder="text to look for" autocomplete="off" spellcheck="false" aria-label="Text">
<button type="button" class="btn icon" data-x="${i}" title="Remove this condition" aria-label="Remove condition">×</button></div>`).join('');
}
function renderActions(){
  $('r-acts').innerHTML = rule.actions.map((a, i) => {
    let param = '<span class="none"></span>';
    if (a.type === 'move'){
      const targets = rule.account ? moveTargets(rule.account) : [];
      param = targets.length ? `<select data-k="folder" aria-label="Folder">${options(targets, a.folder)}</select>`
        : '<span class="none">Pick the rule’s account to choose a folder.</span>';
    } else if (a.type === 'auto_reply'){
      param = `<textarea data-k="body" aria-label="Reply text" placeholder="Dzień dobry, jestem na urlopie do 20 października.">${esc(a.body || '')}</textarea>`;
    }
    return `<div class="rrow act" data-i="${i}"><select data-k="type" aria-label="Action">${options(ACTIONS, a.type)}</select>${param}
<button type="button" class="btn icon" data-x="${i}" title="Remove this action" aria-label="Remove action">×</button></div>`;
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
  try { await invoke('save_rule', {rule}); } catch (err){ $('r-form-err').textContent = String(err); return; }
  say(`Rule “${rule.name.trim()}” saved. It runs on new mail as it arrives.`);
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
      if (b.dataset.armed !== '1'){ b.dataset.armed = '1'; b.textContent = 'Delete it'; return; }
      await invoke('delete_rule', {id:r.id});
    }
  } catch (err){ $('r-err').textContent = String(err); return; }
  await showRuleList();
});
$('r-run').addEventListener('click', async () => {
  const where = folderName(state.folder);
  let r;
  try { r = await call('run_rules', {folder:state.folder}); } catch { return; }
  rdlg.close();
  await refresh();
  const n = r.matched;
  const text = n ? `Rules changed ${n} ${n === 1 ? 'message' : 'messages'} in ${where}.` : `No rule changed anything in ${where}.`;
  say([text, ...r.problems].join(' · '), r.problems.length ? 12000 : 6000);
});
$('b-rules').addEventListener('click', openRules);

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
  $('st-ver').title = `Telex ${build.version}. ` + (can ? 'Click to check for updates.' : 'This build can’t update itself.');
  $('st-ask').hidden = !can || state.updates !== null;
  $('st-update').hidden = !found;
  if (found) $('st-update').textContent = 'Update to ' + found.version;
  $('upd-title').textContent = 'Telex ' + build.version;
  $('u-state').textContent = can ? upd.text : 'This build of Telex can’t update itself. Release builds from telexmail.app can.';
  $('u-err').textContent = upd.error;
  $('u-notes').hidden = !found?.notes;
  $('u-notes').textContent = found?.notes ?? '';
  $('u-install').hidden = !found;
  if (found) $('u-install').textContent = `Install ${found.version} and restart`;
  $('u-install').disabled = $('u-check').disabled = upd.checking || upd.installing;
  $('u-auto').checked = state.updates === 'daily';
  ['u-auto-row', 'u-note', 'u-check'].forEach(id => { $(id).hidden = !can; });
}
// quiet: the daily check, which speaks up only when there is an update.
async function checkUpdate({quiet = false} = {}){
  if (!upd.build?.updatable || upd.checking || upd.installing) return;
  Object.assign(upd, {checking:true, text:'Checking for updates…', error:''});
  renderUpdates();
  try {
    const found = upd.found = await invoke('check_update');
    state.updateChecked = Date.now(); persist();
    upd.text = found ? `Telex ${found.version} is out${found.date ? ', from ' + dmy(parseDate(found.date)) : ''}. You have ${upd.build.version}.`
      : `You have the newest version. Checked at ${hm(new Date())}.`;
    if (found && quiet) say(`Telex ${found.version} is out. Click Update in the status bar to see what’s new.`, 10000);
  } catch (e){
    Object.assign(upd, {text:'', error:String(e)});
    if (!quiet && !udlg.open) say(String(e), 8000);
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
  Object.assign(upd, {installing:true, text:`Downloading Telex ${upd.found.version}…`, error:''});
  renderUpdates();
  let r;
  try { r = await invoke('install_update'); }
  catch (e){
    Object.assign(upd, {installing:false, text:'', error:String(e)}); renderUpdates();
    if (!udlg.open) say(String(e), 10000); // replaces the download progress
    return;
  }
  if (r.busy){ // a check or a send is running: start when it's done
    upd.text = 'Waiting for Telex to finish with the mail server…'; renderUpdates();
    setTimeout(() => { upd.installing = false; installUpdate(); }, 2000);
  }
}
window.__TAURI__?.event?.listen('update-progress', ({payload: p}) => {
  const mb = n => (n / 1048576).toFixed(1);
  upd.text = p.total && p.done >= p.total ? 'Checking the signature. Telex will close, update and open again.'
    : `Downloading Telex ${upd.found?.version ?? ''}: ${mb(p.done)}${p.total ? ' of ' + mb(p.total) : ''} MB`;
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
  if (state.updates === 'daily' && Date.now() - state.updateChecked >= 864e5) checkUpdate({quiet:true});
}
async function bootUpdates(){
  try { upd.build = await invoke('update_info'); } catch { return; }
  renderUpdates();
  if (!upd.build?.updatable) return;
  setTimeout(autoCheck, 20e3); // after the first mail check
  setInterval(autoCheck, 60 * 60e3);
}
$('st-ask').addEventListener('click', e => {
  const b = e.target.closest('[data-ask]'); if (!b) return;
  state.updates = b.dataset.ask; persist(); renderUpdates();
  if (state.updates === 'daily'){ say('Telex will check for updates once a day.'); autoCheck(); }
  else say('Telex won’t check for updates. Click the version number to check yourself.', 7000);
});
$('u-auto').addEventListener('change', e => { state.updates = e.target.checked ? 'daily' : 'off'; persist(); renderUpdates(); });
$('st-ver').addEventListener('click', openUpdates);
$('st-update').addEventListener('click', openUpdates);
$('u-check').addEventListener('click', () => checkUpdate());
$('u-install').addEventListener('click', installUpdate);
$('u-close').addEventListener('click', () => udlg.close());
udlg.addEventListener('close', () => listEl.focus({preventScroll:true}));

/* ---------- keyboard ---------- */
// Also attached to the HTML message frame, so keys work after clicking into it.
function onKey(e){
  if (document.querySelector('dialog[open]')) return; // dialogs handle their own Enter and Esc
  const t = e.target;
  if (state.compose && e.key === 'Enter' && (e.ctrlKey || e.metaKey)){ e.preventDefault(); send(); return; }
  if (e.key === 'Escape'){
    if (state.compose){ e.preventDefault(); closeCompose().then(ok => ok && say('Saved to Drafts.')); return; }
    if (t === qEl){ qEl.value = ''; setFilter(''); listEl.focus(); return; }
  }
  // Tab in the filter box: this folder or all mail, while there is something to look for.
  if (t === qEl && e.key === 'Tab' && !e.shiftKey && !e.ctrlKey && !e.altKey && qEl.value.trim()){ e.preventDefault(); toggleScope(); return; }
  if (t.closest && (t.closest('input, textarea, select') || t.closest('.split'))) return;
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
  else if (k === 'h' || k === 'H') setView(!state.preferText);
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
  paintDensity();
  if (!invoke){
    previewEl.innerHTML = `<div class="pempty">${I.mail}<div>This page needs the Rust core. Start Telex with <code>npm run dev</code>.</div></div>`;
    return;
  }
  bootUpdates(); // first, so a version that can't load the mail can still be updated
  try { await loadTree(); } catch (e){ previewEl.innerHTML = `<div class="pempty">Couldn’t load mail: ${esc(String(e))}</div>`; return; }
  paintScope();
  await setFolder(homeOf(ACCOUNTS[0]?.id));
  listEl.focus({preventScroll:true});
  // Real accounts: check now and every 10 minutes, quietly.
  if (ACCOUNTS.length) getMail({quiet:true});
  else openAccountDialog(); // first start: straight to adding an account
  setInterval(() => getMail({quiet:true}), 10 * 60e3);
}
boot();
})();
