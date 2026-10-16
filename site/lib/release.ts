import { cacheLife } from 'next/cache';
import { knownRelease } from './known-release';
import { site } from './site';

export type Asset = { name: string; url: string; size: number };
export type Release = {
  version: string;
  date: string;
  pageUrl: string;
  exe: Asset | null;
  msi: Asset | null;
  /** The Linux builds, from releases that have them. */
  appimage: Asset | null;
  deb: Asset | null;
  /** SHA256SUMS of every file, from releases made after 0.9.10 (with attestations of each file). */
  sums: Asset | null;
};

/** What the site knows about the releases. */
export type Releases = {
  /** Newest first: GitHub's answer, or, when GitHub couldn't be asked, the last release this site knew of. */
  list: Release[];
  /** GitHub answered. When false, an empty list means "not known", not "nothing released yet". */
  live: boolean;
};

/** GitHub's page for the newest release: always right, whatever this site knows. */
export const latestReleasePage = `https://github.com/${site.releasesRepo}/releases/latest`;
/** Every release on GitHub. */
export const allReleasesPage = `https://github.com/${site.releasesRepo}/releases`;

type GitHubAsset = { name: string; browser_download_url: string; size: number };
type GitHubRelease = {
  tag_name: string;
  draft: boolean;
  published_at: string | null;
  html_url: string;
  assets: GitHubAsset[];
};

// The 64-bit Intel/AMD installer with this extension, if the release has one; any other build otherwise.
function pick(assets: GitHubAsset[], ext: string): Asset | null {
  const files = assets.filter((a) => a.name.toLowerCase().endsWith(ext));
  const a = files.find((f) => /x64|amd64/i.test(f.name) && !/arm64/i.test(f.name)) ?? files[0];
  return a ? { name: a.name, url: a.browser_download_url, size: a.size } : null;
}

function named(assets: GitHubAsset[], name: string): Asset | null {
  const a = assets.find((f) => f.name === name);
  return a ? { name: a.name, url: a.browser_download_url, size: a.size } : null;
}

/**
 * Releases that have a Windows installer (and the Linux AppImage and .deb, if they have them), newest
 * first (up to 10); empty when there are none yet or they are still drafts. Asked at most once an hour,
 * so a new release shows up without a redeploy.
 *
 * When GitHub can't be reached or refuses (rate limit, a private repository), this throws on a live
 * site: the page's background refresh then fails, and Next.js keeps serving the page as it was with
 * the last good answer, trying again on the next visit. (No `expire`: the default profile's "never",
 * so a long outage never turns that page into an error.) At build time and in development there is
 * no earlier answer to keep, so it returns null and the site offers the last release it knew of.
 */
async function lookUp(): Promise<Release[] | null> {
  'use cache';
  cacheLife({ stale: 300, revalidate: 3600 });

  const headers: Record<string, string> = {
    Accept: 'application/vnd.github+json',
    'X-GitHub-Api-Version': '2022-11-28',
  };
  // Optional: only raises GitHub's rate limit. Visitors still need a public repository to download.
  if (process.env.GITHUB_TOKEN) headers.Authorization = `Bearer ${process.env.GITHUB_TOKEN}`;

  try {
    const res = await fetch(`https://api.github.com/repos/${site.releasesRepo}/releases?per_page=10`, { headers });
    if (!res.ok) throw new Error(`GitHub answered ${res.status} ${res.statusText}`);
    const releases = (await res.json()) as GitHubRelease[];
    return releases
      .filter((r) => !r.draft)
      .map((r) => ({
        version: r.tag_name.replace(/^v/, ''),
        date: r.published_at ?? '',
        pageUrl: r.html_url,
        // Tauri names them Telex_0.1.0_x64-setup.exe, Telex_0.1.0_x64_en-US.msi,
        // Telex_0.1.0_amd64.AppImage and Telex_0.1.0_amd64.deb.
        exe: pick(r.assets, '.exe'),
        msi: pick(r.assets, '.msi'),
        appimage: pick(r.assets, '.appimage'),
        deb: pick(r.assets, '.deb'),
        sums: named(r.assets, 'SHA256SUMS'),
      }))
      .filter((r) => r.exe || r.msi);
  } catch (error) {
    if (process.env.NEXT_PHASE === 'phase-production-build' || process.env.NODE_ENV === 'development') {
      console.warn(`Couldn't look up the releases of ${site.releasesRepo} (${error}); using lib/known-release.ts.`);
      return null;
    }
    throw error;
  }
}

/** The releases, and whether GitHub said so (see `Releases`). */
export async function getReleaseInfo(): Promise<Releases> {
  const list = await lookUp();
  if (list) return { list, live: true };
  return { list: knownRelease ? [knownRelease] : [], live: false };
}

/** The releases this site knows, newest first. */
export async function getReleases(): Promise<Release[]> {
  return (await getReleaseInfo()).list;
}

/** The newest release with a Windows installer, or null. */
export async function getLatestRelease(): Promise<Release | null> {
  return (await getReleases())[0] ?? null;
}

export const megabytes = (bytes: number) => `${(bytes / 1024 / 1024).toFixed(1)} MiB`;
