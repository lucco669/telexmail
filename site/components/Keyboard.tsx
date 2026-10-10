import Link from 'next/link';
import { pages } from '@/lib/pages';
import { KeyboardDemo } from './KeyboardDemo';
import s from './Keyboard.module.css';

/** The keyboard section on the home page. */
export function Keyboard() {
  return (
    <section id="keyboard" className={`section ${s.section}`}>
      <div className={`container ${s.layout}`}>
        <header className={`section-head ${s.head}`}>
          <p className="eyebrow">
            <b>02</b> · Keyboard
          </p>
          <h2 className="h2">One key per action.</h2>
          <p className="lead">
            The keys you use all day are single keys, with no Ctrl chords to remember. Try them here: press any key from
            the list.
          </p>
          <p>
            <Link className="more" href={pages.keyboard.path}>
              Every shortcut, with tips
            </Link>
          </p>
        </header>
        <KeyboardDemo />
      </div>
    </section>
  );
}
