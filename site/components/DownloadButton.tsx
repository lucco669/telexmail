import { type Release } from '@/lib/release';
import { DownloadIcon } from './Icons';

/** The big download button: the .exe installer when there is a release, otherwise a placeholder. */
export function DownloadButton({ release }: { release: Release | null }) {
  const file = release?.exe ?? release?.msi;
  if (!file) {
    return (
      <span className="btn btn-primary btn-lg" aria-disabled="true">
        <DownloadIcon />
        Coming soon for Windows
      </span>
    );
  }
  return (
    <a className="btn btn-primary btn-lg" href={file.url}>
      <DownloadIcon />
      Download for Windows
    </a>
  );
}
