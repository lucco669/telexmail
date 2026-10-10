'use client';

import { useRef, useState } from 'react';
import type { Screen } from '@/lib/content';
import { useResolvedTheme } from '@/lib/theme';
import { Shot, type ShotTheme } from './Shot';
import { WindowFrame } from './WindowFrame';
import s from './Gallery.module.css';

const THUMB_SIZES = '(max-width: 720px) calc(100vw - 32px), 560px';

/** All screenshots as a grid; one opens full size in a dialog, where ← → move and Esc closes. */
export function Gallery({ items }: { items: Screen[] }) {
  const [choice, setChoice] = useState<ShotTheme>('site');
  const site = useResolvedTheme() ?? 'light';
  const shown = choice === 'site' ? site : choice;
  const [open, setOpen] = useState(0);
  const dialog = useRef<HTMLDialogElement>(null);
  const current = items[open];

  const show = (i: number) => {
    setOpen(i);
    dialog.current?.showModal();
  };
  const step = (d: number) => setOpen((i) => (i + d + items.length) % items.length);

  return (
    <>
      <div className={s.bar}>
        <p className={s.count}>{items.length} screenshots · 1280 × 800</p>
        <div className={s.theme} role="group" aria-label="Screenshot theme">
          {(['light', 'dark'] as const).map((t) => (
            <button key={t} type="button" aria-pressed={shown === t} className={s.themeBtn} onClick={() => setChoice(t)}>
              {t === 'light' ? 'Light' : 'Dark'}
            </button>
          ))}
        </div>
      </div>

      <ul className={s.grid}>
        {items.map((sc, i) => (
          <li key={sc.id}>
            <button type="button" className={s.card} onClick={() => show(i)} aria-label={`Open full size: ${sc.label}`}>
              <WindowFrame title={sc.title} theme={choice}>
                <Shot id={sc.id} alt="" theme={choice} sizes={THUMB_SIZES} />
              </WindowFrame>
            </button>
            <h2 className={s.label}>{sc.label}</h2>
            <p className={s.caption}>{sc.caption}</p>
          </li>
        ))}
      </ul>

      <dialog
        ref={dialog}
        className={s.dialog}
        aria-label={current.label}
        onKeyDown={(e) => {
          if (e.key === 'ArrowRight') step(1);
          if (e.key === 'ArrowLeft') step(-1);
        }}
        onClick={(e) => {
          if (e.target === e.currentTarget) dialog.current?.close(); // click on the backdrop
        }}
      >
        <div className={s.viewer}>
          <WindowFrame title={current.title} theme={choice}>
            <Shot key={current.id} id={current.id} alt={`Telex: ${current.caption}`} theme={choice} sizes="100vw" />
          </WindowFrame>
          <div className={s.viewerBar}>
            <p>
              <span className={s.counter}>
                {open + 1}/{items.length}
              </span>
              {current.caption}
            </p>
            <div className={s.viewerBtns}>
              <button type="button" className="btn btn-sm" onClick={() => step(-1)} aria-label="Previous screenshot">
                ←
              </button>
              <button type="button" className="btn btn-sm" onClick={() => step(1)} aria-label="Next screenshot">
                →
              </button>
              <button type="button" className="btn btn-sm" onClick={() => dialog.current?.close()}>
                Close <kbd className="kbd">Esc</kbd>
              </button>
            </div>
          </div>
        </div>
      </dialog>
    </>
  );
}
