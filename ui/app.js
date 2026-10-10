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
  mail: sv('<rect x="1.5" y="3" width="13" height="10" rx="1.2"/><path d="M2 4l6 5 6-5"/>'),
};
const KIND_ICON = {inbox:'inbox', drafts:'drafts', sent:'sent', archive:'archive', spam:'spam', trash:'trash', custom:'folder'};

/* ---------- data (filled from Rust) ---------- */
let ACCOUNTS = [], ACCT = {}, MSGS = [];
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
async function load(){
  const snap = await invoke('snapshot');
  ACCOUNTS = snap.accounts;
  ACCT = Object.fromEntries(ACCOUNTS.map(a => [a.id, a]));
  MSGS = snap.messages.map(fromSummary);
}
async function call(cmd, args){
  try { return await invoke(cmd, args); }
  catch (e){ say(String(e), 8000); throw e; }
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
const isOut = key => kindOf(key) === 'sent' || kindOf(key) === 'drafts';
const keyOfKind = (acct, kind) => { const f = ACCT[acct]?.folders.find(x => x.kind === kind); return f ? acct + '/' + f.id : null; };
const inFolder = key => MSGS.filter(m => m.f === key);
const unreadIn = key => MSGS.filter(m => m.f === key && m.unread).length;
const store = {
  get(){ try { return JSON.parse(localStorage.getItem('telex-ui') || '{}'); } catch { return {}; } },
  set(v){ try { localStorage.setItem('telex-ui', JSON.stringify(v)); } catch {} },
};

/* ---------- state ---------- */
const saved = store.get();
const state = {
  folder:'', sel:{}, sort:{key:'date', dir:-1}, q:'',
  collapsed:{}, compose:null, images:new Set(), lastDeleted:null,
  density: saved.density === 'comfortable' ? 'comfortable' : 'compact',
};
const win = $('win'), rowsEl = $('rows'), listEl = $('list'), previewEl = $('preview'), treeEl = $('tree'), qEl = $('q');
if (saved.treeW) win.style.setProperty('--tree-w', saved.treeW + 'px');
if (saved.listH) win.style.setProperty('--list-h', saved.listH + '%');
const persist = () => store.set({
  treeW: parseInt(win.style.getPropertyValue('--tree-w')) || undefined,
  listH: parseFloat(win.style.getPropertyValue('--list-h')) || undefined,
  density: state.density,
});

/* ---------- toolbar ---------- */
[['b-get','get','Get mail'],['b-new','neu','New'],['b-reply','reply','Reply'],['b-replyall','replyall','Reply all'],
 ['b-forward','forward','Forward'],['b-flag','flag','Flag'],['b-delete','trash','Delete']]
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
  const all = inFolder(state.folder), n = unreadIn(state.folder);
  $('st-folder').textContent = `${folderName(state.folder)} · ${all.length} ${all.length === 1 ? 'message' : 'messages'}${n ? `, ${n} unread` : ''}`;
  $('st-check').textContent = lastCheck ? 'Last check ' + hm(lastCheck) : '';
  const title = `${folderName(state.folder)} · ${ACCT[acctId(state.folder)]?.email ?? ''} – Telex`;
  if (title !== lastTitle){
    lastTitle = title;
    document.title = title;
    try { window.__TAURI__?.window?.getCurrentWindow().setTitle(title); } catch {}
  }
}

/* ---------- tree ---------- */
function renderTree(){
  treeEl.innerHTML = ACCOUNTS.map(a => {
    const open = !state.collapsed[a.id];
    const items = open ? '<ul>' + a.folders.map(f => {
      const key = a.id + '/' + f.id, drafts = f.kind === 'drafts';
      const n = drafts ? inFolder(key).length : unreadIn(key);
      const cnt = n ? `<span class="cnt${drafts ? ' dim' : ''}">${n}</span>` : '';
      const on = key === state.folder;
      return `<li><button type="button" class="fold${on ? ' active' : ''}" data-folder="${key}"${on ? ' aria-current="true"' : ''}>${I[KIND_ICON[f.kind] || 'folder']}<span>${esc(f.name)}</span>${cnt}</button></li>`;
    }).join('') + '</ul>' : '';
    return `<button type="button" class="acct" data-acct="${esc(a.id)}" aria-expanded="${open}">${I.caret}<span class="acct-name">${esc(a.name)}</span><span class="acct-mail">${esc(a.email)}</span></button>${items}`;
  }).join('');
  $('folder-select').innerHTML = ACCOUNTS.map(a => `<optgroup label="${esc(a.name)}">` + a.folders.map(f => {
    const key = a.id + '/' + f.id, n = unreadIn(key);
    return `<option value="${key}"${key === state.folder ? ' selected' : ''}>${esc(f.name)}${n ? ` (${n})` : ''}</option>`;
  }).join('') + '</optgroup>').join('');
}
treeEl.addEventListener('click', e => {
  const f = e.target.closest('[data-folder]'), a = e.target.closest('[data-acct]');
  if (f) setFolder(f.dataset.folder);
  else if (a){ state.collapsed[a.dataset.acct] = !state.collapsed[a.dataset.acct]; renderTree(); }
});
$('folder-select').addEventListener('change', e => setFolder(e.target.value));

function setFolder(key){
  state.folder = key; state.q = ''; qEl.value = '';
  const v = visible();
  if (!v.some(m => m.id === state.sel[key])) state.sel[key] = v[0]?.id;
  renderAll();
}

/* ---------- list ---------- */
function visible(){
  const q = state.q.trim().toLowerCase();
  const arr = MSGS.filter(m => m.f === state.folder &&
    (!q || [m.from, m.email, m.subj, m.to].join(' ').toLowerCase().includes(q)));
  const {key, dir} = state.sort, out = isOut(state.folder);
  const who = m => (out ? nameOf(m.to) : m.from || '').toLowerCase();
  const val = m => key === 'date' ? +m.date : key === 'size' ? m.kb
    : key === 'flag' ? (m.flagged ? 2 : 0) + (m.unread ? 1 : 0)
    : key === 'from' ? who(m) : m.subj.toLowerCase().replace(/^((re|fwd?):\s*)+/i, '');
  return arr.sort((a, b) => { const x = val(a), y = val(b); return (x < y ? -1 : x > y ? 1 : 0) * dir || (b.date - a.date); });
}
const current = () => MSGS.find(m => m.id === state.sel[state.folder] && m.f === state.folder);

function renderList(){
  const arr = visible(), sel = state.sel[state.folder], out = isOut(state.folder);
  $('from-label').textContent = out ? 'To' : 'From';
  document.querySelectorAll('th button[data-sort]').forEach(b => {
    const on = b.dataset.sort === state.sort.key, th = b.parentElement;
    if (on) th.setAttribute('aria-sort', state.sort.dir > 0 ? 'ascending' : 'descending'); else th.removeAttribute('aria-sort');
    b.querySelector('.arr').textContent = on ? (state.sort.dir > 0 ? '▲' : '▼') : '';
  });
  if (!arr.length){
    rowsEl.innerHTML = `<tr class="empty"><td colspan="5">${state.q ? `No messages match “${esc(state.q)}”. Press Esc to clear the filter.` : 'This folder is empty.'}</td></tr>`;
    return;
  }
  rowsEl.innerHTML = arr.map(m => {
    const who = out ? nameOf(m.to) : m.from;
    const tip = out ? m.to : `${m.from} <${m.email}>`;
    const cls = (m.unread ? 'unread ' : '') + (m.id === sel ? 'sel' : '');
    return `<tr data-id="${m.id}" class="${cls}" aria-selected="${m.id === sel}">
<td class="c-ic"><span class="ic"><i class="dot"></i>${m.flagged ? I.flagFill : '<i class="sp"></i>'}${m.hasAtt ? I.clip : '<i class="sp"></i>'}</span></td>
<td class="c-from" title="${esc(tip)}">${esc(who)}</td>
<td class="c-subj" title="${esc(m.subj)}">${esc(m.subj)}</td>
<td class="c-date">${fmtList(m.date)}</td>
<td class="c-size">${fmtSize(m.kb)}</td></tr>`;
  }).join('');
  const r = rowsEl.querySelector('tr.sel');
  if (r){
    const head = 22, top = r.offsetTop, h = r.offsetHeight;
    if (top - head < listEl.scrollTop) listEl.scrollTop = top - head;
    else if (top + h > listEl.scrollTop + listEl.clientHeight) listEl.scrollTop = top + h - listEl.clientHeight;
  }
}
rowsEl.addEventListener('click', e => {
  const tr = e.target.closest('tr[data-id]');
  if (!tr) return;
  select(Number(tr.dataset.id));
  listEl.focus({preventScroll:true});
});
document.querySelectorAll('th button[data-sort]').forEach(b => b.addEventListener('click', () => {
  const k = b.dataset.sort;
  state.sort = state.sort.key === k ? {key:k, dir:-state.sort.dir} : {key:k, dir:(k === 'date' || k === 'size' || k === 'flag') ? -1 : 1};
  renderList();
}));

// Mark as read after the message has been on screen for a moment, so arrowing past doesn't.
let readTimer;
function select(id, silent){
  state.sel[state.folder] = id;
  clearTimeout(readTimer);
  const m = current();
  if (m && m.unread && !silent){
    readTimer = setTimeout(() => {
      call('set_unread', {id:m.id, unread:false})
        .then(() => { m.unread = false; renderTree(); renderList(); renderStatus(); })
        .catch(() => {});
    }, 600);
  }
  renderList();
  if (!state.compose) renderPreview();
}
function move(delta){
  const v = visible(); if (!v.length) return;
  const i = v.findIndex(m => m.id === state.sel[state.folder]);
  const n = delta === Infinity ? v.length - 1 : delta === -Infinity ? 0 : Math.max(0, Math.min(v.length - 1, (i < 0 ? 0 : i + delta)));
  select(v[n].id);
}

/* ---------- preview ---------- */
function bodyHtml(m){
  const loaded = state.images.has(m.id);
  return m.body.split('\n').map(line => {
    const img = line.match(/^\[img:(.+)\]$/);
    if (img) return `<div class="imgph${loaded ? ' loaded' : ''}">${loaded ? esc(img[1]) : 'Blocked image: ' + esc(img[1])}</div>`;
    const q = line.startsWith('>');
    let h = esc(q ? line.replace(/^> ?/, '') : line);
    if (!m.note) h = h.replace(/(https?:\/\/[^\s<]+)/g, '<a href="$1" target="_blank" rel="noopener">$1</a>');
    return `<div class="ln${q ? ' q' : ''}">${h}</div>`;
  }).join('');
}
function attsHtml(atts){
  if (!atts.length) return '';
  return `<div class="atts">${atts.map((a, i) => `<button type="button" class="att" data-att="${i}" title="${esc(a.name)}">${I.clip}<span class="nm">${esc(a.name)}</span><small>${fmtSize(a.size_kb)}</small></button>`).join('')}</div>`;
}
let pvSeq = 0;
async function renderPreview(){
  const seq = ++pvSeq, s = current();
  if (!s){
    previewEl.innerHTML = `<div class="pempty">${I.mail}<div>${inFolder(state.folder).length ? 'Select a message to read it here.' : 'Nothing in ' + esc(folderName(state.folder)) + '.'}</div></div>`;
    return;
  }
  let m;
  try { m = await full(s.id); } catch (e){ if (seq === pvSeq) previewEl.innerHTML = `<div class="pempty">${esc(String(e))}</div>`; return; }
  if (seq !== pvSeq || state.compose) return;
  previewEl.innerHTML = `<article>
<header class="ph">
  <div class="ph-top"><h2 class="ph-subj">${esc(m.subject)}</h2>${s.flagged ? `<span class="pill">${I.flag}Flagged</span>` : ''}</div>
  <dl class="ph-grid">
    <dt>From</dt><dd>${esc(m.from_name)} &lt;${esc(m.from_email)}&gt;</dd>
    <dt>To</dt><dd>${esc(m.to || '(no recipient)')}</dd>
    ${m.cc ? `<dt>Cc</dt><dd>${esc(m.cc)}</dd>` : ''}
    <dt>Date</dt><dd class="mono">${fmtFull(parseDate(m.date))} · ${fmtSize(m.size_kb)}</dd>
  </dl>
  ${attsHtml(m.attachments)}
</header>
${m.note ? `<div class="bar note">${esc(m.note)}</div>` : ''}
${m.remote_images && !state.images.has(m.id) ? `<div class="bar">Remote images are blocked so the sender can’t see when you open this. <button type="button" class="linkbtn" data-act="images">Load images</button></div>` : ''}
<div class="pbody">${bodyHtml(m)}</div>
</article>`;
  previewEl.scrollTop = 0;
}
previewEl.addEventListener('click', e => {
  const att = e.target.closest('[data-att]'), act = e.target.closest('[data-act]');
  if (att){ say(`Opening attachments isn't built yet (${att.title}).`); return; }
  if (!act) return;
  const m = current();
  if (act.dataset.act === 'images' && m){ state.images.add(m.id); renderPreview(); say('Images loaded for this message only.'); }
  if (act.dataset.act === 'discard'){ state.compose = null; renderPreview(); say('Message discarded.'); listEl.focus(); }
  if (act.dataset.act === 'draft') closeCompose(false).then(ok => ok && say('Saved to Drafts.'));
});

/* ---------- compose ---------- */
const stripPrefix = s => s.replace(/^((re|fwd?):\s*)+/i, '');
function quoted(m){
  const head = `Dnia ${dmy(parseDate(m.date))} o ${hm(parseDate(m.date))}, ${m.from_name} napisał(a):`;
  const lines = m.body.split('\n').filter(l => !/^\[img:/.test(l)).map(l => l.startsWith('>') ? '>' + l : '> ' + l);
  return head + '\n' + lines.join('\n');
}
async function openCompose(kind){
  const s = current();
  if (kind !== 'new' && !s){ say('Select a message first.'); return; }
  const c = {kind, account:acctId(state.folder), to:'', cc:'', subject:'', body:'\n\nPozdrawiam\n', attachments:[]};
  if (kind !== 'new'){
    let m;
    try { m = await full(s.id); } catch (e){ say(String(e)); return; }
    const out = isOut(s.f);
    if (kind === 'reply' || kind === 'replyall'){
      c.to = out ? m.to : `${m.from_name} <${m.from_email}>`;
      if (kind === 'replyall') c.cc = m.cc;
      c.subject = 'Re: ' + stripPrefix(m.subject);
      c.body = '\n\nPozdrawiam\n\n' + quoted(m);
    } else {
      c.subject = 'Fwd: ' + stripPrefix(m.subject);
      c.attachments = m.attachments.slice();
      c.body = `\n\n-------- Przekazana wiadomość --------\nOd: ${m.from_name} <${m.from_email}>\nData: ${fmtFull(parseDate(m.date))}\nTemat: ${m.subject}\n\n`
        + m.body.split('\n').filter(l => !/^\[img:/.test(l)).join('\n');
    }
  }
  state.compose = c;
  renderCompose();
}
function renderCompose(){
  const c = state.compose, a = ACCT[c.account];
  const title = {new:'New message', reply:'Reply', replyall:'Reply to all', forward:'Forward'}[c.kind];
  previewEl.innerHTML = `<form class="compose" id="cform" novalidate>
<div class="ctitle">${title}<span>from ${esc(a.email)}</span></div>
<div class="cf">
  <label for="c-to">To</label><input id="c-to" value="${esc(c.to)}" autocomplete="off" spellcheck="false">
  <label for="c-cc">Cc</label><input id="c-cc" value="${esc(c.cc)}" autocomplete="off" spellcheck="false">
  <label for="c-subj">Subject</label><input id="c-subj" value="${esc(c.subject)}" autocomplete="off">
</div>
${attsHtml(c.attachments)}
<textarea id="c-body" aria-label="Message text">${esc(c.body)}</textarea>
<div class="cbar">
  <button class="btn primary" type="submit">Send <kbd>Ctrl+Enter</kbd></button>
  <button class="btn" type="button" data-act="draft">Save draft <kbd>Esc</kbd></button>
  <button class="btn" type="button" data-act="discard">Discard</button>
  <span class="cnote">Sending isn’t connected yet: Send files the message in Sent.</span>
</div></form>`;
  $('cform').addEventListener('submit', e => { e.preventDefault(); send(); });
  const t = $('c-body');
  if (c.kind === 'new' || c.kind === 'forward') $('c-to').focus();
  else { t.focus(); t.setSelectionRange(0, 0); t.scrollTop = 0; }
}
function readCompose(){
  const c = state.compose;
  c.to = $('c-to').value.trim(); c.cc = $('c-cc').value.trim(); c.subject = $('c-subj').value.trim(); c.body = $('c-body').value;
  return c;
}
// Files the compose content in Sent (send = true) or Drafts. Resolves to true when it worked.
async function closeCompose(send){
  const c = readCompose();
  const draft = {account:c.account, to:c.to, cc:c.cc, subject:c.subject || '(no subject)', body:c.body.replace(/^\n+/, ''), attachments:c.attachments};
  try { await call('save_outgoing', {draft, send}); } catch { return false; }
  state.compose = null;
  await load();
  renderAll();
  listEl.focus();
  return true;
}
async function send(){
  if (!$('c-to').value.trim()){ say('Add at least one recipient in To.'); $('c-to').focus(); return; }
  if (await closeCompose(true)) say('Filed in Sent. Sending isn’t connected yet, so nothing left your computer.', 7000);
}

/* ---------- actions ---------- */
function toggleFlag(){
  const m = current(); if (!m) return;
  m.flagged = !m.flagged; renderList(); if (!state.compose) renderPreview();
  call('set_flagged', {id:m.id, flagged:m.flagged})
    .then(() => say(m.flagged ? 'Flagged.' : 'Flag removed.', 2000))
    .catch(() => { m.flagged = !m.flagged; renderList(); if (!state.compose) renderPreview(); });
}
function toggleUnread(){
  const m = current(); if (!m) return;
  clearTimeout(readTimer);
  const next = !m.unread;
  call('set_unread', {id:m.id, unread:next}).then(() => {
    m.unread = next; renderTree(); renderList(); renderStatus();
    say(next ? 'Marked as unread.' : 'Marked as read.', 2000);
  }).catch(() => {});
}
async function del(){
  const m = current(); if (!m) return;
  const v = visible(), i = v.findIndex(x => x.id === m.id);
  const next = v[i + 1] || v[i - 1];
  let outcome;
  try { outcome = await call('delete_message', {id:m.id}); } catch { return; }
  if (outcome === 'removed'){
    MSGS.splice(MSGS.indexOf(m), 1); bodies.delete(m.id); state.lastDeleted = null;
    say('Removed from Trash.');
  } else {
    state.lastDeleted = {m, from:m.f};
    m.f = keyOfKind(acctId(m.f), 'trash');
    say('Moved to Trash. Press Z to undo.');
  }
  state.sel[state.folder] = next ? next.id : undefined;
  renderAll();
}
async function undo(){
  const d = state.lastDeleted; if (!d) return;
  try { await call('move_message', {id:d.m.id, folder:d.from}); } catch { return; }
  d.m.f = d.from; state.lastDeleted = null;
  if (state.folder === d.from) state.sel[state.folder] = d.m.id;
  renderAll(); say('Restored to ' + folderName(d.from) + '.');
}
async function getMail(){
  const b = $('b-get');
  if (b.classList.contains('busy')) return;
  b.classList.add('busy'); say(`Checking ${ACCOUNTS.length} accounts…`, 0);
  try {
    const ids = await call('check_mail');
    lastCheck = new Date();
    await load();
    renderAll();
    if (ids.length){
      const first = MSGS.find(m => m.id === ids[0]);
      const where = first ? `${ACCT[acctId(first.f)].name} › ${folderName(first.f)}` : '';
      say(`${ids.length} new ${ids.length === 1 ? 'message' : 'messages'}${where ? ' in ' + where : ''}.`);
    } else say('No new messages.');
  } catch {} finally { b.classList.remove('busy'); }
}

$('b-get').addEventListener('click', getMail);
$('b-new').addEventListener('click', () => openCompose('new'));
$('b-reply').addEventListener('click', () => openCompose('reply'));
$('b-replyall').addEventListener('click', () => openCompose('replyall'));
$('b-forward').addEventListener('click', () => openCompose('forward'));
$('b-flag').addEventListener('click', toggleFlag);
$('b-delete').addEventListener('click', del);
$('b-density').addEventListener('click', () => { state.density = state.density === 'compact' ? 'comfortable' : 'compact'; paintDensity(); persist(); });
qEl.addEventListener('input', () => { state.q = qEl.value; renderList(); });

/* ---------- keyboard ---------- */
document.addEventListener('keydown', e => {
  const t = e.target;
  if (state.compose && e.key === 'Enter' && (e.ctrlKey || e.metaKey)){ e.preventDefault(); send(); return; }
  if (e.key === 'Escape'){
    if (state.compose){ e.preventDefault(); closeCompose(false).then(ok => ok && say('Saved to Drafts.')); return; }
    if (t === qEl){ qEl.value = ''; state.q = ''; renderList(); listEl.focus(); return; }
  }
  if (t.closest && (t.closest('input, textarea, select') || t.closest('.split'))) return;
  if (e.ctrlKey || e.metaKey || e.altKey) return;
  const k = e.key;
  if (k === 'ArrowDown') move(1);
  else if (k === 'ArrowUp') move(-1);
  else if (k === 'Home') move(-Infinity);
  else if (k === 'End') move(Infinity);
  else if (k === 'Delete') del();
  else if (k === 'r' || k === 'R') openCompose(e.shiftKey ? 'replyall' : 'reply');
  else if (k === 'F' && e.shiftKey) openCompose('forward');
  else if (k === 'f') toggleFlag();
  else if (k === 'u' || k === 'U') toggleUnread();
  else if (k === 'n' || k === 'N') openCompose('new');
  else if (k === 'z' || k === 'Z') undo();
  else if (k === '/') qEl.focus();
  else return;
  e.preventDefault();
});

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
function renderAll(){ renderTree(); renderList(); if (!state.compose) renderPreview(); renderStatus(); }
async function boot(){
  paintDensity();
  if (!invoke){
    previewEl.innerHTML = `<div class="pempty">${I.mail}<div>This page needs the Rust core. Start Telex with <code>npm run dev</code>.</div></div>`;
    return;
  }
  try { await load(); } catch (e){ previewEl.innerHTML = `<div class="pempty">Couldn’t load mail: ${esc(String(e))}</div>`; return; }
  const first = ACCOUNTS[0];
  state.folder = keyOfKind(first.id, 'inbox') || first.id + '/' + first.folders[0].id;
  state.sel[state.folder] = visible()[0]?.id;
  renderAll();
  listEl.focus({preventScroll:true});
}
boot();
})();
