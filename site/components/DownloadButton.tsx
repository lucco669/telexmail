import { latestReleasePage, type Release } from '@/lib/release';
import { DownloadIcon } from './Icons';

/**
 * The big download button: the newest Windows installer. When the site couldn't ask GitHub and knows
 * no release (`unknown`), it opens GitHub's page for the newest release instead; the placeholder is
 * only for when GitHub says nothing is released yet.
 */
export function DownloadButton({ release, unknown = false }: { release: Release | null; unknown?: boolean }) {
  const file = release?.exe ?? release?.msi;
  if (file) {
    return (
      <a className="btn btn-primary btn-lg" href={file.url}>
        <DownloadIcon />
        Download for Windows
      </a>
    );
  }
  if (unknown) {
    return (
      <a className="btn btn-primary btn-lg" href={latestReleasePage}>
        <DownloadIcon />
        Download from GitHub
      </a>
    );
  }
  return (
    <span className="btn btn-primary btn-lg" aria-disabled="true">
      <DownloadIcon />
      Coming soon for Windows
    </span>
  );
}
