'use client';

import { useState } from 'react';
import type measurements from '@/lib/themes.json';
import s from './page.module.css';

type Measurements = Pick<(typeof measurements)['themes'][number], 'tokens' | 'pairs'>;
const ratio = (r: number) => `${r.toFixed(2)}:1`;

/** Keep the summary in HTML; fetch the detailed evidence only when someone opens it. */
export function ThemeMeasurements({ id, name, count }: { id: string; name: string; count: number }) {
  const [data, setData] = useState<Measurements | null>(null);
  const [loading, setLoading] = useState(false);
  const [failed, setFailed] = useState(false);
  const url = `/accessibility/contrast/${id}.json`;

  async function load() {
    if (data || loading) return;
    setLoading(true);
    setFailed(false);
    try {
      const response = await fetch(url);
      if (!response.ok) throw new Error('Measurements unavailable');
      setData(await response.json());
    } catch {
      setFailed(true);
    } finally {
      setLoading(false);
    }
  }

  return (
    <details onToggle={(event) => { if (event.currentTarget.open) void load(); }}>
      <summary>Every pair in {name} <span>({count})</span></summary>
      <p className={s.evidenceLink}><a href={url}>All {count} measurements as JSON</a></p>
      {loading && <p className={s.evidenceLink} role="status">Loading measurements…</p>}
      {failed && <p className={s.evidenceLink} role="alert">Measurements could not load. <button type="button" className="btn btn-sm" onClick={() => void load()}>Try again</button></p>}
      {data && (
        <table className={s.pairs}>
          <caption className="visually-hidden">Contrast measurements for {name}</caption>
          <thead>
            <tr><th scope="col">Where</th><th scope="col">Color on background</th><th scope="col">Contrast</th><th scope="col">Needs</th></tr>
          </thead>
          <tbody>
            {data.pairs.map((p) => {
              const tokens = data.tokens as Record<string, string>;
              return (
                <tr key={`${p.kind}-${p.fg}-${p.bg}`}>
                  <td>{p.what}</td>
                  <td><span className={s.chip} style={{ background: (p as { color?: string }).color ?? tokens[p.bg], color: tokens[p.fg] }} aria-hidden="true">Aa</span>{p.fg} on {p.bg}</td>
                  <td>{ratio(p.ratio)}</td>
                  <td>{p.need}:1</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </details>
  );
}
