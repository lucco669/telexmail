'use client';

import { useId, useRef, useState, useSyncExternalStore } from 'react';
import { screens } from '@/lib/content';
import { Shot, type ShotTheme } from './Shot';
import { WindowFrame } from './WindowFrame';
import s from './Screens.module.css';

const DARK = '(prefers-color-scheme: dark)';
const subscribe = (cb: () => void) => {
  const mq = window.matchMedia(DARK);
  mq.addEventListener('change', cb);
  return () => mq.removeEventListener('change', cb);
};
const systemTheme = () => (window.matchMedia(DARK).matches ? 'dark' : 'light');

/** Tabbed screenshots with a light/dark switch. Arrow keys move between tabs. */
export function Screens() {
  const [index, setIndex] = useState(0);
  const [choice, setChoice] = useState<ShotTheme>('auto');
  const system = useSyncExternalStore(subscribe, systemTheme, () => 'light' as const);
  const shown = choice === 'auto' ? system : choice;
  const tabs = useRef<(HTMLButtonElement | null)[]>([]);
  const id = useId();
  const screen = screens[index];

  const onTabKey = (e: React.KeyboardEvent) => {
    const step = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0;
    const to = e.key === 'Home' ? 0 : e.key === 'End' ? screens.length - 1 : (index + step + screens.length) % screens.length;
    if (!step && e.key !== 'Home' && e.key !== 'End') return;
    e.preventDefault();
    setIndex(to);
    tabs.current[to]?.focus();
  };

  return (
    <section id="screens" className="section">
      <div className="container">
        <header className="section-head">
          <p className="eyebrow">
            <b>03</b> · Screens
          </p>
          <h2 className="h2">A closer look.</h2>
          <p className="lead">
            Real screenshots of the current build, showing the sample mailbox Telex opens with before you add an
            account.
          </p>
        </header>

        <div className={s.bar}>
          <div className={s.tabs} role="tablist" aria-label="Screenshots" onKeyDown={onTabKey}>
            {screens.map((sc, i) => (
              <button
                key={sc.id}
                ref={(el) => {
                  tabs.current[i] = el;
                }}
                type="button"
                role="tab"
                id={`${id}-tab-${sc.id}`}
                aria-selected={i === index}
                aria-controls={`${id}-panel`}
                tabIndex={i === index ? 0 : -1}
                className={s.tab}
                onClick={() => setIndex(i)}
              >
                {sc.label}
              </button>
            ))}
          </div>
          <div className={s.theme} role="group" aria-label="Screenshot theme">
            {(['light', 'dark'] as const).map((t) => (
              <button key={t} type="button" aria-pressed={shown === t} className={s.themeBtn} onClick={() => setChoice(t)}>
                {t === 'light' ? 'Light' : 'Dark'}
              </button>
            ))}
          </div>
        </div>

        <div role="tabpanel" id={`${id}-panel`} aria-labelledby={`${id}-tab-${screen.id}`} className={s.panel}>
          <WindowFrame title={screen.title} theme={choice}>
            <Shot key={screen.id} id={screen.id} alt={`Telex: ${screen.caption}`} theme={choice} />
          </WindowFrame>
          <p className={s.caption}>
            <span className={s.counter}>
              {index + 1}/{screens.length}
            </span>
            {screen.caption}
          </p>
        </div>
      </div>
    </section>
  );
}
