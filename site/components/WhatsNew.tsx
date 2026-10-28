import { PageLink as Link } from '@/components/PageNavigation';
import { versions } from '@/lib/content';
import { pages } from '@/lib/pages';
import type { Release } from '@/lib/release';
import { isReleased, versionAnchor, versionName } from '@/lib/versions';
import s from './WhatsNew.module.css';

/** How many highlights the home page shows; the changelog has the rest. */
const SHOWN = 6;

/** The home page's look at the newest version this site describes: coming soon, or just out. */
export function WhatsNew({ release }: { release: Release | null }) {
  const v = versions[0];
  const out = isReleased(v.version, release);
  const all = v.highlights ?? v.points.map((text) => ({ title: '', text, keys: undefined, href: undefined }));
  const cards = all.slice(0, SHOWN);
  const anchor = `${pages.changelog.path}#${versionAnchor(v.version)}`;
  return (
    <section id="new" className="section">
      <div className="container">
        <header className="section-head">
          <p className="eyebrow">
            <b>05</b> · What’s new
          </p>
          <h2 className="h2">
            {out ? 'New' : 'Coming'} in {versionName(v.version)}.
          </h2>
          <p className="lead">
            {v.summary}{' '}
            {out
              ? 'If you enabled update checks, Telex offers it in the status bar. You can also click the version number to check manually.'
              : 'It comes as an update to everyone who has Telex; the live demo already runs it.'}
          </p>
          {!out && (
            <p>
              <Link className="more" href={pages.demo.path}>
                Try it now
              </Link>
            </p>
          )}
        </header>
        <ul className={s.grid}>
          {cards.map((c) => (
            <li key={c.text} className={s.card}>
              {(c.title || c.keys) && (
                <div className={s.top}>
                  {c.title && <h3 className={s.title}>{c.title}</h3>}
                  {c.keys && (
                    <span className={s.keys} aria-label={`Key: ${c.keys}`}>
                      {c.keys.split(/\s*·\s*/).map((k) => (
                        <kbd key={k} className="kbd">
                          {k}
                        </kbd>
                      ))}
                    </span>
                  )}
                </div>
              )}
              <p className={s.text}>{c.text}</p>
              {c.href && <Link className="more" href={c.href} aria-label={`See details: ${c.title}`}>See details</Link>}
            </li>
          ))}
        </ul>
        <p className={s.links}>
          <Link className="more" href={anchor}>
            {all.length > SHOWN ? `All ${all.length} highlights of ${versionName(v.version)}` : `Everything in ${versionName(v.version)}`}
          </Link>
          <Link className="more" href={pages.changelog.path}>
            What each version brought
          </Link>
        </p>
      </div>
    </section>
  );
}
