import Link from 'next/link';
import { compat } from '@/lib/content';
import { pages } from '@/lib/pages';
import { allReleasesPage, latestReleasePage, megabytes, type Release } from '@/lib/release';
import { site } from '@/lib/site';
import { DownloadIcon } from './Icons';
import s from './DownloadChoice.module.css';

const dateFmt = new Intl.DateTimeFormat('en-GB', { day: 'numeric', month: 'long', year: 'numeric', timeZone: 'UTC' });

type Props = {
  release: Release | null;
  /** The site couldn't ask GitHub and knows no release: link to GitHub's page for the newest one. */
  unknown?: boolean;
};

/** A placeholder button, only while GitHub says nothing is released yet. */
const Soon = () => (
  <span className="btn btn-primary" aria-disabled="true">
    <DownloadIcon />
    Coming soon
  </span>
);

const FromGitHub = ({ what }: { what: string }) => (
  <a className="btn btn-primary" href={latestReleasePage}>
    <DownloadIcon />
    {what} on GitHub
  </a>
);

function Windows({ release, unknown }: Props) {
  const file = release?.exe ?? release?.msi;
  if (file) {
    return (
      <>
        <a className="btn btn-primary" href={file.url}>
          <DownloadIcon />
          Download installer
        </a>
        <p className={s.file}>
          Installer ({release?.exe ? '.exe' : '.msi'}) · {megabytes(file.size)} · updates itself
        </p>
        {release?.exe && release.msi && (
          <p className={s.alt}>
            Or the <a href={release.msi.url}>.msi package</a> ({megabytes(release.msi.size)}), for many PCs at once.
          </p>
        )}
        <p className={s.alt}>
          Not code-signed yet: if SmartScreen warns, choose <b>More info</b>, then <b>Run anyway</b>.
        </p>
      </>
    );
  }
  if (unknown) {
    return (
      <>
        <FromGitHub what="Windows installer" />
        <p className={s.file}>Pick the .exe (or the .msi for many PCs)</p>
      </>
    );
  }
  return <Soon />;
}

function Linux({ release, unknown }: Props) {
  const file = release?.appimage ?? release?.deb;
  if (file) {
    return (
      <>
        <a className="btn btn-primary" href={file.url}>
          <DownloadIcon />
          {release?.appimage ? 'Download AppImage' : 'Download .deb'}
        </a>
        <p className={s.file}>
          {release?.appimage ? 'AppImage' : '.deb package'} · {megabytes(file.size)}
          {release?.appimage && ' · updates itself'}
        </p>
        {release?.appimage && release.deb && (
          <p className={s.alt}>
            Or the <a href={release.deb.url}>.deb package</a> ({megabytes(release.deb.size)}), for Debian and Ubuntu.
          </p>
        )}
      </>
    );
  }
  if (release) {
    return (
      <>
        <a className="btn" href={allReleasesPage}>
          Every release on GitHub
        </a>
        <p className={s.file}>Version {release.version} has no Linux build</p>
      </>
    );
  }
  if (unknown) {
    return (
      <>
        <FromGitHub what="AppImage and .deb" />
        <p className={s.file}>The AppImage updates itself</p>
      </>
    );
  }
  return <Soon />;
}

/** Windows and Linux side by side: the file to pick for each, its size, and the alternative. */
export function DownloadChoice(props: Props) {
  return (
    <div className={s.wrap}>
      <div className={s.choice}>
        <div className={s.option}>
          <p className={s.os}>
            <span className={s.name}>Windows</span>
            <span>10 and 11, 64-bit</span>
          </p>
          <Windows {...props} />
        </div>
        <div className={s.option}>
          <p className={s.os}>
            <span className={s.name}>Linux</span>
            <span>64-bit, Ubuntu 22.04 or newer</span>
          </p>
          <Linux {...props} />
        </div>
      </div>
    </div>
  );
}

/** Price and status beside the buttons: free, a preview, and where the source is. */
export function ReleaseStatus({ release, unknown = false, className = '' }: Props & { className?: string }) {
  return (
    <p className={`${s.status} ${className}`}>
      <b>Free</b>
      <span className={s.sep}> · </span>
      {release ? (
        <>
          Preview release {release.version}
          {release.date && <span className={s.date}>, {dateFmt.format(new Date(release.date))}</span>}
        </>
      ) : unknown ? (
        'Preview releases (0.x)'
      ) : (
        'The first preview is on its way'
      )}
      <span className={s.sep}> · </span>
      <a href={`https://github.com/${site.releasesRepo}`}>Source on GitHub</a>
    </p>
  );
}

/** Words with a hyphen ("sign-in") kept on one line. */
const keepHyphened = (text: string) =>
  text.split(/(\S*\w-\w\S*)/).map((part, i) =>
    i % 2 ? (
      <span key={i} className={s.nowrap}>
        {part}
      </span>
    ) : (
      part
    ),
  );

/** Which mail works, in one line, with a link to the details on the download page. */
export function Compat({ className = '' }: { className?: string }) {
  return (
    <p className={`${s.compat} ${className}`}>
      {keepHyphened(`${compat.works} ${compat.later}`)} <Link href={`${pages.download.path}#mail`}>Which mail works</Link>
    </p>
  );
}

/** When the site couldn't ask GitHub: say so, so a missing or older version isn't mistaken for the newest. */
export function LookupNote({ release }: { release: Release | null }) {
  return (
    <p className={s.lookup}>
      {release
        ? `This page couldn’t check GitHub for a newer version just now, so it offers ${release.version}, the newest it knows. `
        : 'This page couldn’t check GitHub for the newest version just now. '}
      <a href={latestReleasePage}>The newest release on GitHub</a>
    </p>
  );
}
