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
let ACCOUNTS = [], ACCT = {}, MSGS = [];
let SAMPLE = true; // showing the built-in sample mailbox, not the user's mail
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
  if (snap.sample !== SAMPLE){
    // Switched between the sample and the real mail base: ids now mean other messages.
    bodies.clear(); state.sel = {}; state.images.clear(); state.lastDeleted = null; state.hits = new Set();
  }
  SAMPLE = snap.sample;
  ACCOUNTS = snap.accounts;
  ACCT = Object.fromEntries(ACCOUNTS.map(a => [a.id, a]));
  MSGS = snap.messages.map(fromSummary);
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
  if (SAMPLE) return;
  clearTimeout(pushTimer);
  pushTimer = setTimeout(pushNow, delay);
}
async function pushNow(){
  let r;
  try { r = await invoke('push_changes'); } catch (e){ say(String(e), 8000); return; }
  if (r.busy){ schedulePush(4000); return; } // a check is running; try again after it
  if (r.errors.length) say(r.errors.join(' · '), 15000);
  // A refused move puts the message back; one deleted elsewhere disappears.
  try { await load(); renderAll(); } catch {}
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
const inFolder = key => MSGS.filter(m => m.f === key);
const unreadIn = key => MSGS.filter(m => m.f === key && m.unread).length;
const store = {
  get(){ try { return JSON.parse(localStorage.getItem('telex-ui') || '{}'); } catch { return {}; } },
  set(v){ try { localStorage.setItem('telex-ui', JSON.stringify(v)); } catch {} },
};

/* ---------- state ---------- */
const saved = store.get();
const state = {
  folder:'', sel:{}, sort:{key:'date', dir:-1}, q:'', hits:new Set(),
  collapsed:{}, compose:null, images:new Set(), lastDeleted:null,
  density: saved.density === 'comfortable' ? 'comfortable' : 'compact',
  preferText: saved.preferText === true, // show plain text even when a message has HTML
};
const win = $('win'), rowsEl = $('rows'), listEl = $('list'), previewEl = $('preview'), treeEl = $('tree'), qEl = $('q');
if (saved.treeW) win.style.setProperty('--tree-w', saved.treeW + 'px');
if (saved.listH) win.style.setProperty('--list-h', saved.listH + '%');
const persist = () => store.set({
  treeW: parseInt(win.style.getPropertyValue('--tree-w')) || undefined,
  listH: parseFloat(win.style.getPropertyValue('--list-h')) || undefined,
  density: state.density,
  preferText: state.preferText || undefined,
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
  const all = inFolder(state.folder), n = unreadIn(state.folder);
  $('st-folder').textContent = `${folderName(state.folder)} · ${all.length} ${all.length === 1 ? 'message' : 'messages'}${n ? `, ${n} unread` : ''}`;
  $('st-check').textContent = lastCheck ? 'Last check ' + hm(lastCheck) : '';
  $('st-sample').hidden = !SAMPLE;
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
      const key = a.id + '/' + f.id, drafts = f.kind === 'drafts' || f.kind === 'outbox'; // these count everything, not unread
      const n = drafts ? inFolder(key).length : unreadIn(key);
      const cnt = n ? `<span class="cnt${drafts ? ' dim' : ''}">${n}</span>` : '';
      const on = key === state.folder;
      return `<li><button type="button" class="fold${on ? ' active' : ''}" data-folder="${key}"${on ? ' aria-current="true"' : ''}>${I[KIND_ICON[f.kind] || 'folder']}<span>${esc(f.name)}</span>${cnt}</button></li>`;
    }).join('') + '</ul>' : '';
    return `<button type="button" class="acct" data-acct="${esc(a.id)}" aria-expanded="${open}">${I.caret}<span class="acct-name">${esc(a.name)}</span><span class="acct-mail">${esc(a.email)}</span></button>${items}`;
  }).join('') + `<button type="button" class="add-acct" data-act="add-account">${I.plus}<span>${SAMPLE ? 'Add your account' : 'Add account'}</span></button>`;
  $('folder-select').innerHTML = ACCOUNTS.map(a => `<optgroup label="${esc(a.name)}">` + a.folders.map(f => {
    const key = a.id + '/' + f.id, n = unreadIn(key);
    return `<option value="${key}"${key === state.folder ? ' selected' : ''}>${esc(f.name)}${n ? ` (${n})` : ''}</option>`;
  }).join('') + '</optgroup>').join('');
}
treeEl.addEventListener('click', e => {
  const f = e.target.closest('[data-folder]'), a = e.target.closest('[data-acct]');
  if (e.target.closest('[data-act="add-account"]')) openAccountDialog();
  else if (f) setFolder(f.dataset.folder);
  else if (a){ state.collapsed[a.dataset.acct] = !state.collapsed[a.dataset.acct]; renderTree(); }
});
$('folder-select').addEventListener('change', e => setFolder(e.target.value));

function setFolder(key){
  state.folder = key; state.q = ''; qEl.value = ''; state.hits = new Set();
  const v = visible();
  if (!v.some(m => m.id === state.sel[key])) state.sel[key] = v[0]?.id;
  renderAll();
}

/* ---------- list ---------- */
function visible(){
  const q = state.q.trim().toLowerCase();
  // Header fields match as you type; bodies match through the core's full-text search (state.hits).
  const arr = MSGS.filter(m => m.f === state.folder &&
    (!q || state.hits.has(m.id) || [m.from, m.email, m.subj, m.to].join(' ').toLowerCase().includes(q)));
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
  if (!s){
    previewEl.innerHTML = `<div class="pempty">${I.mail}<div>${inFolder(state.folder).length ? 'Select a message to read it here.' : 'Nothing in ' + esc(folderName(state.folder)) + '.'}</div></div>`;
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
  if (act.dataset.act === 'draft') closeCompose(false).then(ok => ok && say('Saved to Drafts.'));
});

/* ---------- compose ---------- */
// kind: new, reply, replyall or forward, started by the core with the identity's signature
// and templates; or edit, which reopens a draft or an unsent message in the Outbox.
async function openCompose(kind, preset = {}){
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
const composeNote = c => SAMPLE ? 'Sample mailbox: Send only files the message in Sent.'
  : !ACCT[c.account]?.smtp ? 'This account has no SMTP server yet: edit it under Add account to send.' : '';
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
// Files the compose content in Sent (send = true) or Drafts. Resolves to true when it worked.
async function closeCompose(send){
  const draft = draftOf(readCompose());
  try { await call('save_outgoing', {draft, send}); } catch { return false; }
  state.compose = null;
  await load();
  renderAll();
  listEl.focus();
  return true;
}
// Real mail goes to the Outbox and out at once. It's only called sent once the SMTP
// server has taken it; otherwise it waits in the Outbox and the status says why.
async function send(){
  if (!$('c-to').value.trim() && !$('c-cc').value.trim()){ say('Add at least one recipient in To.'); $('c-to').focus(); return; }
  if (SAMPLE){
    if (await closeCompose(true)) say('Filed in Sent. This is the sample mailbox, so nothing left your computer.', 7000);
    return;
  }
  const btn = document.querySelector('#cform .btn.primary');
  if (!btn || btn.disabled) return;
  btn.disabled = true; btn.firstChild.textContent = 'Sending… ';
  say('Sending…', 0);
  let r;
  try { r = await call('send_message', {draft:draftOf(readCompose())}); }
  catch { btn.disabled = false; btn.firstChild.textContent = 'Send '; return; } // e.g. a bad address: fix it and try again
  state.compose = null;
  await load();
  renderAll();
  listEl.focus();
  const sent = r.sent.length === 1 ? `Sent ${r.sent[0]}.` : r.sent.length ? `Sent ${r.sent.length} messages.` : '';
  if (r.errors.length) say([sent, r.errors.join(' · ')].filter(Boolean).join(' '), 15000);
  else say(sent || 'In the Outbox. It goes out with the next check.', 6000);
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
  const v = visible(), i = v.findIndex(x => x.id === m.id);
  const next = v[i + 1] || v[i - 1];
  let outcome;
  try { outcome = await call('delete_message', {id:m.id}); } catch { return; }
  if (outcome === 'removed'){
    MSGS.splice(MSGS.indexOf(m), 1); bodies.delete(m.id); state.lastDeleted = null;
    say('Deleted for good.');
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
// quiet: a background check, which speaks up only for new mail or errors.
// first: the download right after adding an account.
let checkAgain = null; // asked for while a check was running; runs when it ends
async function getMail({quiet = false, first = false} = {}){
  const b = $('b-get');
  if (b.classList.contains('busy')){ if (first || !quiet) checkAgain = {quiet, first}; return; }
  b.classList.add('busy');
  const n = SAMPLE ? ACCOUNTS.length : ACCOUNTS.filter(a => a.imap).length;
  if (!quiet) say(`Checking ${n} ${n === 1 ? 'account' : 'accounts'}…`, 0);
  try {
    const r = await invoke('check_mail');
    if (r.busy){ setTimeout(() => getMail({quiet, first}), 2000); return; } // changes are being sent
    lastCheck = new Date();
    await load();
    renderAll();
    const ids = r.new, count = `${ids.length} ${first ? '' : 'new '}${ids.length === 1 ? 'message' : 'messages'}`;
    let text = '';
    if (first) text = `${count} downloaded.`;
    else if (ids.length){
      const m = MSGS.find(x => x.id === ids[ids.length - 1]);
      const where = m ? `${ACCT[acctId(m.f)].name} › ${folderName(m.f)}` : '';
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
    reloadTimer = setTimeout(() => load().then(renderAll).catch(() => {}), 200);
  }
});

$('b-get').addEventListener('click', () => getMail());
$('b-new').addEventListener('click', () => openCompose('new'));
$('b-reply').addEventListener('click', () => openCompose('reply'));
$('b-replyall').addEventListener('click', () => openCompose('replyall'));
$('b-forward').addEventListener('click', () => openCompose('forward'));
$('b-flag').addEventListener('click', toggleFlag);
$('b-delete').addEventListener('click', del);
$('b-density').addEventListener('click', () => { state.density = state.density === 'compact' ? 'comfortable' : 'compact'; paintDensity(); persist(); });
// The filter matches headers at once, then message bodies when the full-text search answers.
let searchTimer, searchSeq = 0;
qEl.addEventListener('input', () => {
  state.q = qEl.value;
  renderList();
  clearTimeout(searchTimer);
  const q = state.q.trim();
  if (!q){ state.hits = new Set(); return; }
  searchTimer = setTimeout(async () => {
    const seq = ++searchSeq;
    let ids;
    try { ids = await invoke('search', {query:q}); } catch { return; }
    if (seq !== searchSeq || state.q.trim() !== q) return;
    state.hits = new Set(ids);
    renderList();
  }, 150);
});

/* ---------- account dialog ---------- */
const dlg = $('acct-dlg'), aForm = $('acct-form'), aErr = $('a-err');
let editing = null; // id of the account being edited, or null when adding
function openAccountDialog(){
  aForm.reset();
  editing = null;
  aErr.textContent = '';
  $('acct-title').textContent = 'Add a mail account';
  $('a-go').querySelector('span').textContent = 'Connect';
  $('a-host').placeholder = 'mail.example.com';
  $('a-user').placeholder = 'Same as email';
  $('a-pass').placeholder = '';
  renderAccountList();
  dlg.showModal();
  $('a-email').focus();
}
// Fill the form with an account's settings; the password stays empty and is kept if left so.
function editAccount(id){
  const a = ACCT[id]; if (!a?.imap) return;
  editing = id;
  aErr.textContent = '';
  $('acct-title').textContent = 'Edit ' + a.name;
  $('a-go').querySelector('span').textContent = 'Save';
  $('a-email').value = a.email; $('a-name').value = a.name;
  $('a-host').value = a.imap.host; $('a-port').value = a.imap.port; $('a-user').value = a.imap.username;
  $('a-smtp').value = a.smtp?.host ?? ''; $('a-sec').value = a.smtp?.security ?? 'tls';
  $('a-sport').value = a.smtp?.port ?? 465;
  $('a-pass').value = ''; $('a-pass').placeholder = 'Leave empty to keep the saved password';
  $(a.smtp ? 'a-pass' : 'a-smtp').focus();
}
function renderAccountList(){
  const real = ACCOUNTS.filter(a => a.imap);
  $('a-list').innerHTML = real.length ? '<h3>Connected</h3>' + real.map(a => {
    const where = `${a.email} · ${a.imap.host}${a.smtp ? '' : ' · no SMTP server yet'}`;
    return `<div class="dacct"><span class="nm">${esc(a.name)}</span><span class="em" title="${esc(where)}">${esc(where)}</span>`
      + `<button type="button" class="btn" data-ids="${esc(a.id)}">Identities</button><button type="button" class="btn" data-edit="${esc(a.id)}">Edit</button><button type="button" class="btn danger" data-remove="${esc(a.id)}">Remove</button></div>`;
  }).join('') : '';
}
$('a-email').addEventListener('input', () => {
  const email = $('a-email').value.trim(), domain = email.split('@')[1] || '';
  $('a-host').placeholder = domain.includes('.') ? 'e.g. mail.' + domain : 'mail.example.com';
  $('a-user').placeholder = email || 'Same as email';
});
// The usual ports follow the security choice, unless the user typed another one.
$('a-sec').addEventListener('change', () => {
  const p = $('a-sport');
  if (['465', '587', ''].includes(p.value)) p.value = $('a-sec').value === 'starttls' ? 587 : 465;
});
$('st-sample').addEventListener('click', openAccountDialog);
$('a-cancel').addEventListener('click', () => dlg.close());
dlg.addEventListener('close', () => listEl.focus({preventScroll:true}));
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
  let account;
  try { account = await invoke('add_account', {setup, password:$('a-pass').value}); }
  catch (err){ aErr.textContent = String(err); return; }
  finally { go.disabled = false; label.textContent = editing ? 'Save' : 'Connect'; }
  aForm.reset();
  dlg.close();
  await load();
  if (wasEditing){
    renderAll();
    say(`Saved ${account.name}.`);
    getMail({quiet:true}); // the Outbox may have waited for these settings
    return;
  }
  setFolder(homeOf(account.id));
  say(`Connected to ${account.email}. Downloading mail…`, 0);
  getMail({first:true});
});
$('a-list').addEventListener('click', async e => {
  const ed = e.target.closest('[data-edit]');
  if (ed){ editAccount(ed.dataset.edit); return; }
  const ids = e.target.closest('[data-ids]');
  if (ids){ dlg.close(); openIdentities(ids.dataset.ids); return; }
  const b = e.target.closest('[data-remove]');
  if (!b) return;
  if (b.dataset.armed !== '1'){ b.dataset.armed = '1'; b.textContent = 'Remove it and its local mail'; return; }
  try { await invoke('remove_account', {id:b.dataset.remove}); } catch (err){ aErr.textContent = String(err); return; }
  await load();
  if (!ACCT[acctId(state.folder)]) setFolder(homeOf(ACCOUNTS[0]?.id)); else renderAll();
  renderAccountList();
  say('Account removed from Telex. The mail on the server is untouched.');
  if (SAMPLE) dlg.close();
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
  await load();
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
  await load();
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
  return `If ${conds}: ${acts}.` + (r.account ? ` ${ACCT[r.account]?.name ?? r.account} only.` : '') + (r.stop ? ' Then stop.' : '');
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
  await load(); renderAll();
  const n = r.matched;
  const text = n ? `Rules changed ${n} ${n === 1 ? 'message' : 'messages'} in ${where}.` : `No rule changed anything in ${where}.`;
  say([text, ...r.problems].join(' · '), r.problems.length ? 12000 : 6000);
});
$('b-rules').addEventListener('click', openRules);

/* ---------- keyboard ---------- */
// Also attached to the HTML message frame, so keys work after clicking into it.
function onKey(e){
  if (document.querySelector('dialog[open]')) return; // dialogs handle their own Enter and Esc
  const t = e.target;
  if (state.compose && e.key === 'Enter' && (e.ctrlKey || e.metaKey)){ e.preventDefault(); send(); return; }
  if (e.key === 'Escape'){
    if (state.compose){ e.preventDefault(); closeCompose(false).then(ok => ok && say('Saved to Drafts.')); return; }
    if (t === qEl){ qEl.value = ''; state.q = ''; state.hits = new Set(); renderList(); listEl.focus(); return; }
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
function renderAll(){
  if (!folderOf(state.folder)) state.folder = homeOf(ACCOUNTS[0]?.id); // folder deleted on the server
  if (!current()) state.sel[state.folder] = visible()[0]?.id; // e.g. mail just arrived in an empty folder
  renderTree(); renderList(); if (!state.compose) renderPreview(); renderStatus();
}
async function boot(){
  paintDensity();
  if (!invoke){
    previewEl.innerHTML = `<div class="pempty">${I.mail}<div>This page needs the Rust core. Start Telex with <code>npm run dev</code>.</div></div>`;
    return;
  }
  try { await load(); } catch (e){ previewEl.innerHTML = `<div class="pempty">Couldn’t load mail: ${esc(String(e))}</div>`; return; }
  state.folder = homeOf(ACCOUNTS[0]?.id);
  state.sel[state.folder] = visible()[0]?.id;
  renderAll();
  listEl.focus({preventScroll:true});
  // Real accounts: check now and every 10 minutes, quietly.
  if (!SAMPLE) getMail({quiet:true});
  setInterval(() => { if (!SAMPLE) getMail({quiet:true}); }, 10 * 60e3);
}
boot();
})();
