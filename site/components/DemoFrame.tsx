'use client';

import { useEffect, useRef, useState } from 'react';
import { useResolvedTheme, type Theme } from '@/lib/theme';
import { Shot } from './Shot';
import { WindowFrame } from './WindowFrame';
import s from './DemoFrame.module.css';

// The app as `npm run demo` builds it into public/demo-app: Telex's own interface with a stand-in
// for its Rust core (site/demo/backend.js), so everything happens in this browser tab.
const APP = '/demo-app/index.html';
const appUrl = (theme: Theme) => `${APP}?theme=${theme}`;

/**
 * The live demo in a window frame. It starts in the page's theme and follows the page's light/dark
 * switch while it runs (a `telex-demo-theme` message, which backend.js turns into the app's own
 * "settings-changed"), shows the inbox screenshot until the app has drawn itself, takes the keyboard
 * focus once it has, and puts the app's window title in the frame's title bar.
 */
export function DemoFrame() {
  const theme = useResolvedTheme();
  const frame = useRef<HTMLIFrameElement>(null);
  // The theme the app was started in. Changing it would reload the frame, so a later switch is sent
  // as a message instead.
  const [started, setStarted] = useState<Theme | null>(null);
  const [ready, setReady] = useState(false);
  const [title, setTitle] = useState('Inbox · sam@lindenstudio.com – Telex');
  const [run, setRun] = useState(0);

  // The theme is known only in the browser; the frame starts once it is.
  useEffect(() => {
    if (theme && !started) setStarted(theme);
  }, [theme, started]);

  // The page's theme changed (or changed while the app was starting): the running app follows.
  useEffect(() => {
    if (!theme || !ready) return;
    frame.current?.contentWindow?.postMessage({ type: 'telex-demo-theme', theme }, window.location.origin);
  }, [theme, ready]);

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
    if (theme) setStarted(theme);
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
          <button type="button" className="btn btn-sm" onClick={restart} disabled={!started}>
            Start over
          </button>
          <a className="btn btn-sm" href={appUrl(theme ?? started ?? 'light')} target="_blank" rel="noopener">
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
          {started && (
            <iframe
              key={run}
              ref={frame}
              src={appUrl(started)}
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
