'use client';

import { Fragment, useEffect, useRef, useState } from 'react';
import { shortcuts, type Shortcut } from '@/lib/content';
import s from './Keyboard.module.css';

function matches(sc: Shortcut, e: KeyboardEvent) {
  return sc.match.some((m) => {
    const letter = /^[a-z]$/i.test(m.key);
    if (letter ? e.key.toLowerCase() !== m.key.toLowerCase() : e.key !== m.key) return false;
    // Shift matters for letters (R vs Shift+R); symbols like / need Shift on some layouts.
    if (letter && !!m.shift !== e.shiftKey) return false;
    return !!m.ctrl === (e.ctrlKey || e.metaKey);
  });
}

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
 * app, and the readout above says what it does. `grouped` adds Move / Write / Organize / Find headings.
 */
export function KeyboardDemo({ grouped = false }: { grouped?: boolean }) {
  const [hit, setHit] = useState<string | null>(null);
  const [tried, setTried] = useState<Set<string>>(() => new Set());
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.altKey) return; // AltGr on Polish keyboards arrives as Ctrl+Alt
      const t = e.target as HTMLElement | null;
      if (t?.closest?.('input, textarea, select, [contenteditable], dialog')) return;
      const sc = shortcuts.find((x) => matches(x, e));
      if (!sc) return;
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
  }, []);

  const last = shortcuts.find((x) => x.id === hit);
  const groups = grouped ? [...new Set(shortcuts.map((x) => x.group))] : [null];

  return (
    <div className={s.demo}>
      <div className={`${s.readout} no-print`} aria-live="polite">
        <span className={s.readKeys}>{last ? <Keys sc={last} /> : <span className={s.idle}>Press a key from the list…</span>}</span>
        <span className={s.readAction}>{last?.action}</span>
        <span className={s.count}>
          {tried.size}/{shortcuts.length} tried
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
              {shortcuts
                .filter((sc) => !g || sc.group === g)
                .map((sc) => (
                  <tr key={sc.id} className={s.row} data-hit={sc.id === hit || undefined} data-tried={tried.has(sc.id) || undefined}>
                    <td className={s.keys}>
                      <Keys sc={sc} />
                    </td>
                    <td className={s.action}>{sc.action}</td>
                  </tr>
                ))}
            </Fragment>
          ))}
        </tbody>
      </table>
    </div>
  );
}
