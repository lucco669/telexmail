'use client';

import { useEffect, useState } from 'react';
import type { LeaderboardEntry } from '@/lib/night-shift-ranking';
import s from './NightShift.module.css';

// The same order as night_shift_leaderboard() in the migration.
function better(a: LeaderboardEntry, b: LeaderboardEntry, sort: string) {
  if (sort === 'speed' && a.wpm !== b.wpm) return b.wpm - a.wpm;
  if (sort === 'reaction' && a.reaction_ms !== b.reaction_ms) return a.reaction_ms - b.reaction_ms;
  return b.score - a.score || b.delivered - a.delivered || b.accuracy - a.accuracy || a.created_at.localeCompare(b.created_at);
}

// The board is cached for 30 seconds, so a run just published is put in by hand, where the
// database will put it: one best run per operator name, 20 rows.
function withPublished(entries: LeaderboardEntry[], published: LeaderboardEntry | null, sort: string) {
  if (!published || entries.some((entry) => entry.id === published.id)) return entries;
  const same = entries.find((entry) => entry.operator.toLowerCase() === published.operator.toLowerCase());
  if (same && better(same, published, sort) <= 0) return entries;
  return [...entries.filter((entry) => entry !== same), published].sort((a, b) => better(a, b, sort)).slice(0, 20);
}

export function NightShiftLeaderboard({ refresh, published }: { refresh: number; published: LeaderboardEntry | null }) {
  const highlight = published?.id ?? null;
  const [fetched, setEntries] = useState<LeaderboardEntry[]>([]);
  const [sort, setSort] = useState('score');
  const [period, setPeriod] = useState('all');
  const [retry, setRetry] = useState(0);
  const [status, setStatus] = useState<'loading' | 'ready' | 'offline' | 'error'>('loading');
  const entries = withPublished(fetched, published, sort);
  useEffect(() => {
    const controller = new AbortController();
    setStatus('loading');
    void fetch(`/api/night-shift/leaderboard?sort=${sort}&period=${period}`, { signal: controller.signal }).then(async (response) => {
      if (!response.ok) throw new Error('Board unavailable');
      const body = await response.json();
      setEntries(body.entries ?? []); setStatus(body.available ? 'ready' : 'offline');
    }).catch(() => { if (!controller.signal.aborted) setStatus('error'); });
    return () => controller.abort();
  }, [sort, period, retry, refresh]);
  return <section className={s.leaderboard} aria-labelledby="night-shift-leaderboard">
    <div className={s.boardHeading}>
      <div><p className={s.boardKicker}>THE INTERNATIONAL NIGHT DESK</p><h3 id="night-shift-leaderboard">The operators’ honour roll.</h3><p>One best run per operator name. Shared by players everywhere.</p></div>
      <button type="button" onClick={() => setRetry(retry + 1)} disabled={status === 'loading'}>Refresh ↻</button>
    </div>
    <div className={s.boardFilters}>
      <div role="group" aria-label="Leaderboard period">{[['all','All time'],['week','Last 7 days'],['today','Today (UTC)']].map(([id, label]) => <button type="button" key={id} aria-pressed={period === id} onClick={() => setPeriod(id)}>{label}</button>)}</div>
      <label>Rank by <select value={sort} onChange={(event) => setSort(event.target.value)}><option value="score">Highest score</option><option value="speed">Typing speed</option><option value="reaction">Reaction time</option></select></label>
    </div>
    {status === 'loading' && <p className={s.boardNotice} role="status">Calling the international desk…</p>}
    {status === 'offline' && <div className={s.boardEmpty}><span aria-hidden="true">◎</span><h4>The world board is getting connected.</h4><p>Play a shift and explore your timing stats. Online scores will appear here when the desk is ready.</p></div>}
    {status === 'error' && <p className={s.boardNotice} role="status">The online board is taking a break. Your game still works. Try Refresh shortly.</p>}
    {status === 'ready' && !entries.length && <div className={s.boardEmpty}><span aria-hidden="true">✦</span><h4>The first shift could be yours.</h4><p>Finish a scored shift and publish it under your operator name.</p></div>}
    {status === 'ready' && entries.length > 0 && <>
      <div className={s.podium}>{entries.slice(0, 3).map((entry, i) => <div key={entry.id} data-highlight={entry.id === highlight}><span>{String(i + 1).padStart(2,'0')} / {i === 0 ? 'LEAD OPERATOR' : 'NIGHT CREW'}</span><h4>{entry.operator}</h4><strong>{sort === 'speed' ? `${entry.wpm} WPM` : sort === 'reaction' ? `${(entry.reaction_ms / 1000).toFixed(2)}s` : entry.score.toLocaleString('en-US')}</strong><p>{entry.delivered} delivered · {entry.accuracy}% accuracy</p></div>)}</div>
      <div className={s.boardTableWrap} tabIndex={0} role="region" aria-label="Leaderboard statistics; scroll horizontally for all columns">
        <table className={s.boardTable}><thead><tr><th scope="col">Rank / operator</th><th scope="col">Score</th><th scope="col">Sent</th><th scope="col">Reaction</th><th scope="col">Writing</th><th scope="col">WPM</th><th scope="col">Accuracy</th><th scope="col">Streak</th></tr></thead><tbody>{entries.map((entry, i) => <tr key={entry.id} data-highlight={entry.id === highlight}><th scope="row"><span>{String(i+1).padStart(2,'0')}</span>{entry.operator}</th><td>{entry.score.toLocaleString('en-US')}</td><td>{entry.delivered}<small>{entry.clean} clean / {entry.missed} missed</small></td><td>{(entry.reaction_ms / 1000).toFixed(2)}s</td><td>{(entry.writing_ms / 1000).toFixed(2)}s</td><td>{entry.wpm}</td><td>{entry.accuracy}%</td><td>{entry.best_streak}</td></tr>)}</tbody></table>
      </div>
    </>}
    <p className={s.boardFootnote}>Reaction = first viewing a telegram → first character. Writing = active time on that line after the first character → transmit. Both exclude pauses; writing excludes time in other cities. WPM uses five characters per word. Only delivered telegrams count toward averages.</p>
  </section>;
}
