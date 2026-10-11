import Link from 'next/link';
import { shortcuts } from '@/lib/content';
import { pages } from '@/lib/pages';
import type { Release } from '@/lib/release';
import { KeyboardDemo } from './KeyboardDemo';
import s from './Keyboard.module.css';

/** The keyboard section on the home page: the keys used most, and the way to all of them. */
export function Keyboard({ release }: { release: Release | null }) {
  return (
    <section id="keyboard" className={`section ${s.section}`}>
      <div className={`container ${s.layout}`}>
        <header className={`section-head ${s.head}`}>
          <p className="eyebrow">
            <b>02</b> · Keyboard
          </p>
          <h2 className="h2">One key per action.</h2>
          <p className="lead">
            The keys you use all day are single keys, with no Ctrl chords to remember, and they work on one message or a
            whole selection. Try them here: press any key from the list.
          </p>
          <p>
            <Link className="more" href={pages.demo.path}>
              Or press them in the live demo
            </Link>
          </p>
        </header>
        <div className={s.side}>
          <KeyboardDemo release={release} onlyHome />
          <p>
            <Link className="more" href={pages.keyboard.path}>
              All {shortcuts.length} shortcuts, with tips
            </Link>
          </p>
        </div>
      </div>
    </section>
  );
}
