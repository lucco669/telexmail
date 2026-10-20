'use client';

import { useState } from 'react';
import dynamic from 'next/dynamic';
import { PageLink as Link } from '@/components/PageNavigation';
import { Shot } from './Shot';
import { WindowFrame } from './WindowFrame';
import s from './HeroStage.module.css';

// Wide enough for the app's window to be usable in place; narrower screens open the demo page.
const WIDE = '(min-width: 900px)';
const DemoFrame = dynamic(() => import('./DemoFrame').then((module) => module.DemoFrame), {
  loading: () => <div aria-busy="true"><WindowFrame title="Starting Telex…"><Shot id="inbox" alt="Loading the Telex live demo." eager poster /></WindowFrame></div>,
});

/**
 * The hero's screenshot, which turns into the live demo where it stands. The picture stays the
 * first paint (and the page's largest image); the demo loads only on a click. Without script, or on
 * a narrow screen, it is a link to the demo page.
 */
export function HeroStage({ id, title, demoPath }: { id: string; title: string; demoPath: string }) {
  const [live, setLive] = useState(false);
  if (live) return <DemoFrame />;
  return (
    <WindowFrame title={title}>
      <Link
        href={demoPath}
        className={s.poster}
        onClick={(e) => {
          if (e.metaKey || e.ctrlKey || e.shiftKey || e.button !== 0 || !window.matchMedia(WIDE).matches) return;
          e.preventDefault();
          setLive(true);
        }}
      >
        <Shot id={id} alt="Telex main window: folder tree, message list and the selected message." eager poster />
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
