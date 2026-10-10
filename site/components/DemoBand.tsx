import Link from 'next/link';
import { screens } from '@/lib/content';
import { pages } from '@/lib/pages';
import { ArrowIcon } from './Icons';
import { Shot } from './Shot';
import { WindowFrame } from './WindowFrame';
import s from './DemoBand.module.css';

const thread = screens.find((x) => x.id === 'thread')!;

const hints: { keys: string[]; text: string }[] = [
  { keys: ['T'], text: 'follow a conversation across folders' },
  { keys: ['Shift', '↓'], text: 'then M: move a pile in four keys' },
  { keys: ['Ctrl', 'Enter'], text: 'then Z: take back what you just sent' },
];

/** The home page's way into the live demo. */
export function DemoBand() {
  return (
    <section id="demo" className={`section ${s.section}`}>
      <div className={`container ${s.layout}`}>
        <header className={`section-head ${s.head}`}>
          <p className="eyebrow">
            <b>03</b> · Live demo
          </p>
          <h2 className="h2">Try it before you install.</h2>
          <p className="lead">
            The live demo runs Telex’s real interface in your browser, on a made-up mailbox. Every key works, the settings
            and themes too, and nothing you do leaves the page.
          </p>
          <ul className={s.hints}>
            {hints.map((h) => (
              <li key={h.text}>
                <span className={s.keys}>
                  {h.keys.map((k, i) => (
                    <span key={k} className={s.keys}>
                      {i > 0 && <span className={s.plus}>+</span>}
                      <kbd className="kbd">{k}</kbd>
                    </span>
                  ))}
                </span>
                {h.text}
              </li>
            ))}
          </ul>
          <p>
            <Link href={pages.demo.path} className="btn btn-primary btn-lg">
              Open the live demo
              <ArrowIcon />
            </Link>
          </p>
        </header>
        <Link href={pages.demo.path} className={s.preview} tabIndex={-1} aria-hidden="true">
          <WindowFrame title={thread.title}>
            <Shot id={thread.id} alt="" sizes="(max-width: 960px) calc(100vw - 32px), 640px" />
          </WindowFrame>
          <span className={s.play}>
            <svg viewBox="0 0 16 16" aria-hidden="true">
              <path d="M5 3.5v9l7.5-4.5z" fill="currentColor" />
            </svg>
            Start the demo
          </span>
        </Link>
      </div>
    </section>
  );
}
