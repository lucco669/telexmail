'use client';

import { Fragment, useEffect, useRef, useState } from 'react';
import type { Shortcut } from '@/lib/content';
import type { Release } from '@/lib/release';
import { Since } from './Since';
import s from './Keyboard.module.css';

function matches(sc: Shortcut, e: KeyboardEvent) {
  return sc.match.some((m) => {
    const letter = /^[a-z]$/i.test(m.key);
    if (letter ? e.key.toLowerCase() !== m.key.toLowerCase() : e.key !== m.key) return false;
    // Shift matters for letters (R vs Shift+R) and where a row says so (↓ vs Shift+↓); symbols like /
    // need Shift on some layouts.
    if ((letter || m.shift !== undefined) && !!m.shift !== e.shiftKey) return false;
    return !!m.ctrl === (e.ctrlKey || e.metaKey);
  });
}

// Rows whose keys would also do something in the browser (select the page, save or print it, open
// view-source):
// here they only light up.
const KEEP_FROM_BROWSER = new Set(['selectall', 'format', 'saveas', 'print', 'source']);

function Keys({ sc }: { sc: Shortcut }) {
  return (
    <>
      {sc.keys.map((combo, i) => (
        <span key={i} className={s.combo}>
          {i > 0 && <span className={s.or}>/</span>}
          {combo.map((k, j) => (
            <span key={k} className={s.combo}>
              {j > 0 && <span className={s.plus}>+</span>}
              <kbd className="kbd">{k}</kbd>
            </span>
          ))}
        </span>
      ))}
    </>
  );
}

/**
 * The shortcut table. Pressing a listed key lights up its row, like selecting a message in the
 * app, and the readout above says what it does. `grouped` adds Move / Select / Write … headings;
 * the caller supplies only the rows needed by this page.
 */
export function KeyboardDemo({
  release,
  grouped = false,
  items: list,
}: {
  release: Release | null;
  grouped?: boolean;
  items: Shortcut[];
}) {
  const [hit, setHit] = useState<string | null>(null);
  const [tried, setTried] = useState<Set<string>>(() => new Set());
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.altKey) return; // AltGr on Polish keyboards arrives as Ctrl+Alt
      const t = e.target as HTMLElement | null;
      if (t?.closest?.('input, textarea, select, [contenteditable], dialog')) return;
      const sc = list.find((x) => matches(x, e));
      if (!sc) return;
      if (KEEP_FROM_BROWSER.has(sc.id)) e.preventDefault();
      setHit(sc.id);
      setTried((prev) => (prev.has(sc.id) ? prev : new Set(prev).add(sc.id)));
      clearTimeout(timer.current);
      timer.current = setTimeout(() => setHit(null), 900);
    };
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('keydown', onKey);
      clearTimeout(timer.current);
    };
  }, [list]);

  const last = list.find((x) => x.id === hit);
  const groups = grouped ? [...new Set(list.map((x) => x.group))] : [null];

  return (
    <div className={s.demo}>
      <div className={`${s.readout} no-print`} aria-live="polite">
        <span className={s.readKeys}>{last ? <Keys sc={last} /> : <span className={s.idle}>Press a key from the list…</span>}</span>
        <span className={s.readAction}>{last?.action}</span>
        <span className={s.count}>
          {tried.size}/{list.length} tried
        </span>
      </div>
      <table className={s.table}>
        <caption className="visually-hidden">Keyboard shortcuts</caption>
        <thead className="visually-hidden">
          <tr>
            <th scope="col">Keys</th>
            <th scope="col">Action</th>
          </tr>
        </thead>
        <tbody>
          {groups.map((g) => (
            <Fragment key={g ?? 'all'}>
              {g && (
                <tr className={s.groupRow}>
                  <th colSpan={2} scope="colgroup">
                    {g}
                  </th>
                </tr>
              )}
              {list
                .filter((sc) => !g || sc.group === g)
                .map((sc) => (
                  <tr key={sc.id} className={s.row} data-hit={sc.id === hit || undefined} data-tried={tried.has(sc.id) || undefined}>
                    <td className={s.keys}>
                      <Keys sc={sc} />
                    </td>
                    <td className={s.action}>
                      {sc.action} <Since since={sc.since} release={release} className={s.badge} />
                    </td>
                  </tr>
                ))}
            </Fragment>
          ))}
        </tbody>
      </table>
    </div>
  );
}
