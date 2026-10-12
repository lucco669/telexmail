import { PageLink as Link } from '@/components/PageNavigation';
import { facts, hero, NEXT, nextTeaser, versions } from '@/lib/content';
import { pages } from '@/lib/pages';
import { megabytes, type Release } from '@/lib/release';
import { isReleased, versionAnchor, versionName } from '@/lib/versions';
import { Compat } from './DownloadChoice';
import { DownloadButton } from './DownloadButton';
import { ArrowIcon } from './Icons';
import { Shot } from './Shot';
import { WindowFrame } from './WindowFrame';
import s from './Hero.module.css';

/** The line in the pill above the title: what the next version brings, or what the newest one did. */
function news(release: Release | null) {
  if (!release || !isReleased(NEXT, release)) {
    return { lead: `Coming in ${versionName(NEXT)}:`, text: nextTeaser, anchor: NEXT };
  }
  const notes = versions.find((v) => v.version === release.version);
  if (!notes) return { lead: `Version ${release.version} is out:`, text: 'see what’s new.', anchor: release.version };
  return { lead: `New in ${versionName(release.version)}:`, text: `${notes.title}.`, anchor: release.version };
}

/** Price, status and size under the buttons. */
function meta(release: Release | null, unknown: boolean) {
  const file = release?.exe ?? release?.msi;
  if (release && file) {
    return (
      <>
        Free · preview {release.version} · {megabytes(file.size)} · Windows 10 and 11
        {(release.appimage || release.deb) && (
          <>
            {' · '}
            <Link href={`${pages.download.path}#linux`}>Linux</Link>
          </>
        )}
      </>
    );
  }
  if (unknown) return 'Free · Windows 10 and 11, and Linux · every release is on GitHub';
  return 'Free · Windows 10 and 11 · the first preview build is on its way';
}

export function Hero({ release, unknown = false }: { release: Release | null; unknown?: boolean }) {
  const pill = news(release);
  return (
    <section className={s.hero}>
      <div className={`container ${s.top}`}>
        <div className={s.intro}>
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
        </div>
        <div className={s.side}>
          <p className={s.lead}>
            Light on your PC. Quick through your inbox. Telex is an email client for Windows and Linux, with dense lists, one key for every action, and all
            your mail on your own disk, searchable offline.
          </p>
          <div className={s.actions}>
            <DownloadButton release={release} unknown={unknown} />
            <Link href={pages.demo.path} className="btn btn-lg">
              <span className={s.liveDot} aria-hidden="true" />
              Try it in your browser
            </Link>
          </div>
          <p className={s.meta}>{meta(release, unknown)}</p>
          <Compat className={s.compat} />
        </div>
      </div>

      <div className={s.stage}>
        <div className="container">
          <WindowFrame title={hero.title}>
            <Shot id={hero.id} alt="Telex main window: folder tree, message list and the selected message." eager />
          </WindowFrame>
        </div>
        <div className="container">
          <ul className={s.facts} aria-label="What you get">
            {facts.map((f) => (
              <li key={f}>{f}</li>
            ))}
          </ul>
        </div>
      </div>
    </section>
  );
}
