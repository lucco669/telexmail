'use client';

import { useState } from 'react';
import { PageLink as Link } from '@/components/PageNavigation';
import { hero } from '@/lib/content';
import { pages } from '@/lib/pages';
import { DemoFrame } from './DemoFrame';
import { Shot } from './Shot';
import { WindowFrame } from './WindowFrame';
import s from './HeroStage.module.css';

// Wide enough for the app's window to be usable in place; narrower screens open the demo page.
const WIDE = '(min-width: 900px)';

/**
 * The hero's screenshot, which turns into the live demo where it stands. The picture stays the
 * first paint (and the page's largest image); the demo loads only on a click. Without script, or on
 * a narrow screen, it is a link to the demo page.
 */
export function HeroStage() {
  const [live, setLive] = useState(false);
  if (live) return <DemoFrame />;
  return (
    <WindowFrame title={hero.title}>
      <Link
        href={pages.demo.path}
        className={s.poster}
        onClick={(e) => {
          if (e.metaKey || e.ctrlKey || e.shiftKey || e.button !== 0 || !window.matchMedia(WIDE).matches) return;
          e.preventDefault();
          setLive(true);
        }}
      >
        <Shot id={hero.id} alt="Telex main window: folder tree, message list and the selected message." eager />
        <span className={s.cta}>
          <span className={s.play} aria-hidden="true">
            <svg viewBox="0 0 16 16">
              <path d="M5 3.2v9.6a.6.6 0 0 0 .9.5l7.6-4.8a.6.6 0 0 0 0-1L5.9 2.7a.6.6 0 0 0-.9.5z" />
            </svg>
          </span>
          <span className={s.words}>
            <b className={s.wide}>Run it live, right here</b>
            <b className={s.narrow}>Try the live demo</b>
            <span>The real interface · nothing to install</span>
          </span>
        </span>
      </Link>
    </WindowFrame>
  );
}
