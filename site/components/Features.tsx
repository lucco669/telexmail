import Link from 'next/link';
import { features } from '@/lib/content';
import { pages } from '@/lib/pages';
import s from './Features.module.css';

export function Features() {
  return (
    <section id="features" className="section">
      <div className="container">
        <header className="section-head">
          <p className="eyebrow">
            <b>01</b> · Why Telex
          </p>
          <h2 className="h2">Built like a tool.</h2>
          <p className="lead">
            Everything in Telex is tuned for getting through a lot of mail quickly, without reaching for the mouse and
            without anyone looking over your shoulder.
          </p>
          <p>
            <Link className="more" href={pages.features.path}>
              All features, with screenshots
            </Link>
          </p>
          <p>
            <Link className="more" href={pages.about.path}>
              Why it’s built this way
            </Link>
          </p>
        </header>
        <ol className={s.grid}>
          {features.map((f, i) => (
            <li key={f.title} className={s.item}>
              <span className={s.num}>{String(i + 1).padStart(2, '0')}</span>
              <h3 className={s.title}>{f.title}</h3>
              <p className={s.text}>{f.text}</p>
            </li>
          ))}
        </ol>
      </div>
    </section>
  );
}
