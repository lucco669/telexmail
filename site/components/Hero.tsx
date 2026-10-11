import Link from 'next/link';
import { facts, hero, NEXT, nextTeaser, versions } from '@/lib/content';
import { pages } from '@/lib/pages';
import { megabytes, type Release } from '@/lib/release';
import { isReleased, shortVersion, versionAnchor } from '@/lib/versions';
import { DownloadButton } from './DownloadButton';
import { ArrowIcon } from './Icons';
import { Shot } from './Shot';
import { WindowFrame } from './WindowFrame';
import s from './Hero.module.css';

/** The line in the pill above the title: what the next version brings, or what the newest one did. */
function news(release: Release | null) {
  if (!release || !isReleased(NEXT, release)) {
    return { lead: `Coming in ${shortVersion(NEXT)}:`, text: nextTeaser, anchor: NEXT };
  }
  const notes = versions.find((v) => v.version === release.version);
  if (!notes) return { lead: `Version ${release.version} is out:`, text: 'see what’s new.', anchor: release.version };
  return { lead: `New in ${shortVersion(release.version)}:`, text: `${notes.title}.`, anchor: release.version };
}

export function Hero({ release }: { release: Release | null }) {
  const file = release?.exe ?? release?.msi;
  const pill = news(release);
  return (
    <section className={s.hero}>
      <div className="container">
        <Link className={s.status} href={`${pages.changelog.path}#${versionAnchor(pill.anchor)}`}>
          <span className={s.pulse} aria-hidden="true" />
          <span>
            <b>{pill.lead}</b> {pill.text}
          </span>
          <ArrowIcon />
        </Link>
        <h1 className={s.title}>
          Email at the speed <span className={s.nowrap}>of your keyboard.</span>
        </h1>
        <p className={s.lead}>
          Telex is a compact, fast email client for Windows and Linux. Dense lists, one key for every action, and all your
          mail on your own disk, searchable offline.
        </p>
        <div className={s.actions}>
          <DownloadButton release={release} />
          <Link href={pages.demo.path} className="btn btn-lg">
            <span className={s.liveDot} aria-hidden="true" />
            Try it in your browser
          </Link>
        </div>
        <p className={s.meta}>
          {release && file ? (
            <>
              Version {release.version} · {megabytes(file.size)} · Windows 10 and 11, 64-bit
              {(release.appimage || release.deb) && (
                <>
                  {' · '}
                  <Link href={`${pages.download.path}#linux`}>Linux</Link>
                </>
              )}
            </>
          ) : (
            'Windows 10 and 11, 64-bit · the first preview build is on its way'
          )}
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
