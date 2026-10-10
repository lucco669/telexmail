import Link from 'next/link';
import { versions } from '@/lib/content';
import { pages } from '@/lib/pages';
import type { Release } from '@/lib/release';
import { isReleased, shortVersion, versionAnchor } from '@/lib/versions';
import s from './WhatsNew.module.css';

/** The home page's look at the newest version this site describes: coming soon, or just out. */
export function WhatsNew({ release }: { release: Release | null }) {
  const v = versions[0];
  const out = isReleased(v.version, release);
  const cards = v.highlights ?? v.points.map((text) => ({ title: '', text, keys: undefined }));
  return (
    <section id="new" className="section">
      <div className="container">
        <header className="section-head">
          <p className="eyebrow">
            <b>05</b> · What’s new
          </p>
          <h2 className="h2">
            {out ? 'New' : 'Coming'} in {shortVersion(v.version)}.
          </h2>
          <p className="lead">
            {v.summary}{' '}
            {out
              ? 'Telex offers the update by itself, or click the version number in its status bar.'
              : 'It comes as an update to everyone who has Telex; the live demo already runs it.'}
          </p>
          <p className={s.links}>
            <Link className="more" href={`${pages.changelog.path}#${versionAnchor(v.version)}`}>
              Everything in {shortVersion(v.version)}
            </Link>
            {!out && (
              <Link className="more" href={pages.demo.path}>
                Try it now
              </Link>
            )}
          </p>
        </header>
        <ul className={s.grid}>
          {cards.map((c) => (
            <li key={c.text} className={s.card}>
              {(c.title || c.keys) && (
                <div className={s.top}>
                  {c.title && <h3 className={s.title}>{c.title}</h3>}
                  {c.keys && (
                    <span className={s.keys} aria-label={`Key: ${c.keys}`}>
                      {c.keys.split('+').map((k) => (
                        <kbd key={k} className="kbd">
                          {k}
                        </kbd>
                      ))}
                    </span>
                  )}
                </div>
              )}
              <p className={s.text}>{c.text}</p>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}
