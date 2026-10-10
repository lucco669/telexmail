import Link from 'next/link';
import { facts, hero } from '@/lib/content';
import { pages } from '@/lib/pages';
import { megabytes, type Release } from '@/lib/release';
import { DownloadButton } from './DownloadButton';
import { ArrowIcon } from './Icons';
import { Shot } from './Shot';
import { WindowFrame } from './WindowFrame';
import s from './Hero.module.css';

export function Hero({ release }: { release: Release | null }) {
  const file = release?.exe ?? release?.msi;
  return (
    <section className={s.hero}>
      <div className="container">
        <Link className={s.status} href={pages.roadmap.path}>
          <span className={s.pulse} aria-hidden="true" />
          <span>
            <b>Early preview.</b> Sync, sending, HTML mail and filters work; Gmail sign-in is next.
          </span>
          <ArrowIcon />
        </Link>
        <h1 className={s.title}>
          Email at the speed <span className={s.nowrap}>of your keyboard.</span>
        </h1>
        <p className={s.lead}>
          Telex is a compact, fast email client for Windows. Dense lists, one key for every action, and all your mail on
          your own disk, searchable offline.
        </p>
        <div className={s.actions}>
          {/* Until there is a build to download, the tour is the main action. */}
          {file ? (
            <>
              <DownloadButton release={release} />
              <a href="#screens" className="btn btn-lg">
                See it in action
              </a>
            </>
          ) : (
            <>
              <a href="#screens" className="btn btn-primary btn-lg">
                See it in action
              </a>
              <DownloadButton release={release} />
            </>
          )}
        </div>
        <p className={s.meta}>
          {release && file
            ? `Version ${release.version} · ${megabytes(file.size)} · Windows 10 and 11, 64-bit`
            : 'Windows 10 and 11, 64-bit · the first preview build is on its way'}
        </p>
      </div>

      <div className={s.stage}>
        <div className="container">
          <WindowFrame title={hero.title}>
            <Shot id={hero.id} alt="Telex main window: folder tree, message list and the selected message." eager />
          </WindowFrame>
        </div>
        <div className="container">
          <ul className={s.facts} aria-label="Built with">
            {facts.map((f) => (
              <li key={f}>{f}</li>
            ))}
          </ul>
        </div>
      </div>
    </section>
  );
}
