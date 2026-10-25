// The private /stats page, as one HTML document: downloads from GitHub and copies counted.
// No scripts: the site's CSP allows inline styles, and bars are plain boxes.
import { type Copies, KINDS, type ReleaseDownloads } from './count';

export type StatsData = {
  releases: ReleaseDownloads[] | null;
  perDay: { day: string; installers: number; latest: number }[] | null;
  copies: Copies | null;
  problems: string[];
};

const esc = (s: string) => s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
const n = (v: number) => v.toLocaleString('en-US');
const KIND_NAMES = { exe: 'Windows .exe', msi: 'Windows .msi', appimage: 'AppImage', deb: '.deb' };

function bars(points: { label: string; value: number; extra?: number }[], title: string, extraName = '') {
  const max = Math.max(1, ...points.map((p) => p.value));
  const cols = points.map((p) => {
    const h = Math.round((p.value / max) * 100);
    const e = p.extra ? Math.round((p.extra / max) * 100) : 0;
    const tip = `${p.label}: ${n(p.value)}${p.extra ? ` (${n(p.extra)} ${extraName})` : ''}`;
    return `<div class="col" title="${esc(tip)}"><div class="bar" style="height:${h}%"><div class="extra" style="height:${h ? Math.round((e / h) * 100) : 0}%"></div></div></div>`;
  }).join('');
  const first = points[0]?.label ?? '', last = points.at(-1)?.label ?? '';
  return `<figure><figcaption>${esc(title)} <span class="muted">max ${n(max)}</span></figcaption><div class="chart">${cols}</div><div class="axis"><span>${esc(first)}</span><span>${esc(last)}</span></div></figure>`;
}

function shares(title: string, counts: Record<string, number>) {
  const total = Object.values(counts).reduce((a, b) => a + b, 0);
  const rows = Object.entries(counts).sort((a, b) => b[1] - a[1]).map(([k, v]) =>
    `<tr><td>${esc(k)}</td><td class="num">${n(v)}</td><td class="wide"><div class="hbar" style="width:${total ? Math.round((v / total) * 100) : 0}%"></div></td></tr>`).join('');
  return `<div><h3>${esc(title)}</h3>${rows ? `<table>${rows}</table>` : '<p class="muted">None yesterday.</p>'}</div>`;
}

const tile = (label: string, value: number, note = '') =>
  `<div class="tile"><div class="label">${esc(label)}</div><div class="value">${n(value)}</div>${note ? `<div class="muted">${esc(note)}</div>` : ''}</div>`;

export function statsPage(data: StatsData, now = new Date()) {
  const parts: string[] = [];
  const { releases, perDay, copies } = data;

  parts.push('<h2>Downloads</h2>');
  if (releases) {
    const totals = Object.fromEntries(KINDS.map((k) => [k, releases.reduce((s, r) => s + r.kinds[k], 0)]));
    const all = releases.reduce((s, r) => s + r.installers, 0);
    const latest = releases.reduce((s, r) => s + r.latest, 0);
    parts.push(`<div class="tiles">${tile('Installer downloads, all time', all)}${KINDS.map((k) => tile(KIND_NAMES[k], totals[k])).join('')}${tile('Update checks (latest.json)', latest)}</div>`);
    if (perDay && perDay.length) {
      const last = perDay.slice(-60);
      parts.push(bars(last.map((d) => ({ label: d.day, value: d.installers })), 'Installer downloads per day'));
      parts.push(bars(last.map((d) => ({ label: d.day, value: d.latest })), 'Update checks per day (latest.json fetches)'));
    } else {
      parts.push('<p class="muted">Downloads per day appear after two daily snapshots (the daily scheduled task, or opening this page on two days).</p>');
    }
    const rows = releases.map((r) => `<tr><td>${esc(r.release)}</td><td>${esc(r.date?.slice(0, 10) ?? '')}</td>${KINDS.map((k) => `<td class="num">${n(r.kinds[k])}</td>`).join('')}<td class="num"><b>${n(r.installers)}</b></td><td class="num">${n(r.latest)}</td></tr>`).join('');
    parts.push(`<table class="grid"><thead><tr><th>Release</th><th>Published</th>${KINDS.map((k) => `<th class="num">${KIND_NAMES[k]}</th>`).join('')}<th class="num">Installers</th><th class="num">latest.json</th></tr></thead><tbody>${rows}</tbody></table>`);
    parts.push('<p class="muted">GitHub counts every download of a file, including the ones the updater makes: the newest release’s .exe and AppImage include updates from older versions. latest.json is fetched by each update check, so it counts checks, not people. Robots and repeated downloads count too.</p>');
  } else {
    parts.push('<p class="muted">GitHub couldn’t be asked.</p>');
  }

  parts.push('<h2>Copies counted</h2>');
  if (copies) {
    parts.push(`<div class="tiles">${tile('Today', copies.day.today, 'so far, UTC')}${tile('Yesterday', copies.day.yesterday)}${tile('This week', copies.week.now, `last week ${n(copies.week.last)}`)}${tile('This month', copies.month.now, `last month ${n(copies.month.last)}`)}${tile('New, last 30 days', copies.newIn30)}</div>`);
    parts.push(bars(copies.daily.map((d) => ({ label: d.day, value: d.copies, extra: d.new })), 'Active copies per day (the bottom part: new ones)', 'new'));
    parts.push(`<div class="cols3">${shares('Version, yesterday', copies.yesterday.version)}${shares('System, yesterday', copies.yesterday.os)}${shares('Language, yesterday', copies.yesterday.lang)}</div>`);
    if (copies.cohorts.length) {
      const head = copies.months.map((m) => `<th class="num">${esc(m)}</th>`).join('');
      const body = copies.cohorts.map((c) => `<tr><td>${esc(c.since)}</td><td class="num">${n(c.started)}</td>${copies.months.map((m) => {
        const v = c.active[m];
        if (m < c.since || v === undefined) return '<td></td>';
        return `<td class="num">${n(v)}${c.started ? ` <span class="muted">${Math.round((v / c.started) * 100)}%</span>` : ''}</td>`;
      }).join('')}</tr>`).join('');
      parts.push(`<h3>Still used, by the month a copy was first counted</h3><table class="grid"><thead><tr><th>First counted</th><th class="num">Started</th>${head}</tr></thead><tbody>${body}</tbody></table>`);
    }
    parts.push('<p class="muted">Only copies whose user said yes to counting (release builds since the version that asks). Each copy counts at most once a UTC day, so a day’s number is that day’s active copies; a week’s and a month’s count each copy once. No IDs exist: a reinstall or a second PC is another copy.</p>');
  } else {
    parts.push('<p class="muted">Count storage isn’t connected (SUPABASE_URL and SUPABASE_SECRET_KEY), or it didn’t answer.</p>');
  }
  if (data.problems.length) parts.push(`<p class="bad">${data.problems.map(esc).join('<br>')}</p>`);

  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="robots" content="noindex,nofollow"><title>Telex stats</title><style>
:root{--bg:#fafaf7;--fg:#1d1d1b;--muted:#6b6b66;--line:#e2e1da;--bar:#3a6ea5;--extra:#1d3f66;--bad:#a12a2a;color-scheme:light dark}
@media (prefers-color-scheme:dark){:root{--bg:#161615;--fg:#ecebe6;--muted:#a3a29b;--line:#34332f;--bar:#6d9fd6;--extra:#b7d3f2;--bad:#ff8f8f}}
body{margin:0;background:var(--bg);color:var(--fg);font:14px/1.45 system-ui,sans-serif}
main{max-width:1100px;margin:0 auto;padding:24px 16px 48px}
h1{font-size:20px;margin:0 0 4px}h2{font-size:17px;margin:32px 0 12px;border-bottom:1px solid var(--line);padding-bottom:6px}h3{font-size:14px;margin:16px 0 8px}
.muted{color:var(--muted);font-size:12px}.bad{color:var(--bad)}
.tiles{display:grid;grid-template-columns:repeat(auto-fill,minmax(150px,1fr));gap:8px}
.tile{border:1px solid var(--line);border-radius:6px;padding:10px}.label{font-size:12px;color:var(--muted)}.value{font-size:22px;font-variant-numeric:tabular-nums}
figure{margin:16px 0}figcaption{font-size:13px;margin-bottom:6px}
.chart{display:flex;align-items:flex-end;gap:2px;height:120px;border-bottom:1px solid var(--line)}
.col{flex:1;height:100%;display:flex;align-items:flex-end}.bar{width:100%;background:var(--bar);display:flex;align-items:flex-end;min-height:0}.extra{width:100%;background:var(--extra)}
.axis{display:flex;justify-content:space-between;font-size:11px;color:var(--muted)}
table{border-collapse:collapse;font-variant-numeric:tabular-nums}td,th{padding:3px 8px;text-align:left;white-space:nowrap}
.grid{width:100%;display:block;overflow-x:auto}.grid th{font-weight:600;border-bottom:1px solid var(--line)}.grid td{border-bottom:1px solid var(--line)}
.num{text-align:right}.wide{width:100%;min-width:80px}.hbar{height:8px;background:var(--bar);border-radius:2px}
.cols3{display:grid;grid-template-columns:repeat(auto-fit,minmax(240px,1fr));gap:16px}
</style></head><body><main><h1>Telex stats</h1><p class="muted">As of ${esc(now.toISOString().slice(0, 16).replace('T', ' '))} UTC. Private page; not linked from the site.</p>${parts.join('\n')}</main></body></html>`;
}
