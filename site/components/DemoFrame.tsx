'use client';

import { useEffect, useRef, useState } from 'react';
import { useResolvedTheme } from '@/lib/theme';
import { Shot } from './Shot';
import { WindowFrame } from './WindowFrame';
import s from './DemoFrame.module.css';

// The app as `npm run demo` builds it into public/demo-app: Telex's own interface with a stand-in
// for its Rust core (site/demo/backend.js), so everything happens in this browser tab.
const APP = '/demo-app/index.html';

/**
 * The live demo in a window frame. It starts in the page's theme, shows the inbox screenshot
 * until the app has drawn itself, takes the keyboard focus once it has, and puts the app's window
 * title in the frame's title bar.
 */
export function DemoFrame() {
  const theme = useResolvedTheme();
  const frame = useRef<HTMLIFrameElement>(null);
  const [src, setSrc] = useState<string | null>(null);
  const [ready, setReady] = useState(false);
  const [title, setTitle] = useState('Inbox · sam@lindenstudio.com – Telex');
  const [run, setRun] = useState(0);

  // The theme is known only in the browser; the frame starts once it is.
  useEffect(() => {
    if (theme && !src) setSrc(`${APP}?theme=${theme}`);
  }, [theme, src]);

  useEffect(() => {
    const onMessage = (e: MessageEvent) => {
      if (e.origin !== window.location.origin || e.data?.type !== 'telex-demo-title') return;
      setTitle(String(e.data.title));
    };
    window.addEventListener('message', onMessage);
    return () => window.removeEventListener('message', onMessage);
  }, []);

  const focusApp = () => {
    frame.current?.focus({ preventScroll: true });
    frame.current?.contentWindow?.focus();
  };

  const onLoad = () => {
    setReady(true);
    // Keys go straight to Telex, unless the visitor is already busy elsewhere on the page.
    if (document.activeElement === document.body || document.activeElement === null) focusApp();
  };

  const restart = () => {
    setReady(false);
    setRun((n) => n + 1);
  };

  return (
    <div className={s.demo}>
      <div className={s.bar}>
        <p className={s.live}>
          <span className={s.dot} aria-hidden="true" />
          Live · runs in your browser
        </p>
        <p className={s.hint}>Click inside the window, then use the keys.</p>
        <div className={s.tools}>
          <button type="button" className="btn btn-sm" onClick={restart} disabled={!src}>
            Start over
          </button>
          <a className="btn btn-sm" href={src ?? APP} target="_blank" rel="noopener">
            Full window
            <span className="visually-hidden"> (opens in a new tab)</span>
          </a>
        </div>
      </div>
      <WindowFrame title={title}>
        <div className={s.viewport}>
          <div className={s.poster} data-hidden={ready || undefined} aria-hidden="true">
            <Shot id="inbox" alt="" eager sizes="(max-width: 1224px) calc(100vw - 32px), 1160px" />
            <span className={s.starting}>Starting Telex…</span>
          </div>
          {src && (
            <iframe
              key={run}
              ref={frame}
              src={src}
              title="Telex live demo: the app’s window. Press Tab or click inside to use it."
              className={s.app}
              data-ready={ready || undefined}
              onLoad={onLoad}
            />
          )}
        </div>
      </WindowFrame>
    </div>
  );
}
