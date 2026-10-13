import { PageLink as Link } from '@/components/PageNavigation';
import { CtaBand } from '@/components/CtaBand';
import { DownloadButton } from '@/components/DownloadButton';
import { PageHeader } from '@/components/PageHeader';
import { versions, type Version } from '@/lib/content';
import { pages } from '@/lib/pages';
import { allReleasesPage, getReleaseInfo, megabytes, type Release } from '@/lib/release';
import { pageMetadata } from '@/lib/seo';
import { compareVersions, shortVersion, versionAnchor } from '@/lib/versions';
import s from './page.module.css';

export const metadata = pageMetadata(pages.changelog);

const dateFmt = new Intl.DateTimeFormat('en-GB', { day: 'numeric', month: 'long', year: 'numeric', timeZone: 'UTC' });

type Entry = { version: string; notes: Version | null; release: Release | null };

/** The versions this page knows, and any newer release on GitHub it doesn't describe yet. */
function entries(releases: Release[]): Entry[] {
  const all = new Map<string, Entry>();
  for (const v of versions) all.set(v.version, { version: v.version, notes: v, release: null });
  for (const r of releases) all.set(r.version, { version: r.version, notes: all.get(r.version)?.notes ?? null, release: r });
  return [...all.values()].sort((a, b) => compareVersions(b.version, a.version));
}

function Files({ r }: { r: Release }) {
  const files = [
    r.exe && { label: `Windows .exe (${megabytes(r.exe.size)})`, url: r.exe.url },
    r.msi && { label: '.msi', url: r.msi.url },
    r.appimage && { label: 'Linux AppImage', url: r.appimage.url },
    r.deb && { label: '.deb', url: r.deb.url },
  ].filter((f): f is { label: string; url: string } => !!f);
  return (
    <p className={s.files}>
      {files.map((f) => (
        <a key={f.url} href={f.url}>
          {f.label}
        </a>
      ))}
      <a href={r.pageUrl}>Release notes on GitHub</a>
    </p>
  );
}

export default async function ChangelogPage() {
  const { list: releases, live } = await getReleaseInfo();
  const latest = releases[0] ?? null;
  const list = entries(releases);
  // Without GitHub's answer the site knows only its newest release; anything up to it is out too.
  const isOut = (e: Entry) => !!e.release || (!!latest && compareVersions(e.version, latest.version) <= 0);
  return (
    <>
      <PageHeader
        page={pages.changelog}
        title="What’s new."
        lead="Every release of Telex, newest first, with downloads and upgrade notes. Version 0.9.5 adds saved searches, richer rules, optional threads, toolbar layouts and local scam checks. Click the version number in Telex’s status bar to check for an update."
      >
        <DownloadButton release={latest} unknown={!live && !latest} />
        <Link className="btn btn-lg" href={pages.demo.path}>
          Try the newest in your browser
        </Link>
      </PageHeader>

      <nav className={`container ${s.jump}`} aria-label="Versions">
        {list.map((e) => (
          <a key={e.version} href={`#${versionAnchor(e.version)}`}>
            {e.version}
          </a>
        ))}
      </nav>

      <div className={`container ${s.list}`}>
        {list.map((e) => {
          const date = e.release?.date || e.notes?.date;
          const out = isOut(e);
          const isLatest = !!latest && e.version === latest.version;
          return (
            <article key={e.version} id={versionAnchor(e.version)} className={s.entry} data-released={out || undefined}>
              <header className={s.meta}>
                <p className={s.ver}>{shortVersion(e.version)}</p>
                <p className={s.full}>Version {e.version}</p>
                {date ? (
                  <p className={s.date}>{dateFmt.format(new Date(date))}</p>
                ) : (
                  !out && <span className="badge badge-coming">In the works</span>
                )}
                {isLatest && <span className="badge badge-new">Newest</span>}
              </header>
              <div className={s.body}>
                <h2 className={s.title}>{e.notes?.title ?? `Telex ${e.version}`}</h2>
                {e.notes && <p className={s.summary}>{e.notes.summary}</p>}
                {e.notes && (
                  <ul className={s.points}>
                    {e.notes.points.map((p) => (
                      <li key={p}>{p}</li>
                    ))}
                  </ul>
                )}
                {!e.notes && e.release && <p className={s.summary}>The notes for this version are on GitHub.</p>}
                {e.notes?.upgrade && <p className={s.upgrade}>{e.notes.upgrade}</p>}
                {e.release ? (
                  <Files r={e.release} />
                ) : out && !live ? (
                  <p className={s.files}>
                    <a href={allReleasesPage}>Its downloads are on GitHub</a>
                  </p>
                ) : (
                  !date && (
                    <p className={s.pending}>
                      Not released yet: it comes as an update to everyone who has Telex. The{' '}
                      <Link href={pages.demo.path}>live demo</Link> already runs it.
                    </p>
                  )
                )}
              </div>
            </article>
          );
        })}
      </div>
      <CtaBand title="See what comes after this." secondary={pages.roadmap} />
    </>
  );
}
