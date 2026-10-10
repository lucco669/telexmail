import { cacheLife } from 'next/cache';
import { site } from './site';

export type Asset = { name: string; url: string; size: number };
export type Release = { version: string; date: string; pageUrl: string; exe: Asset | null; msi: Asset | null };

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

/**
 * Releases that have a Windows installer, newest first (up to 10). Empty when there are none yet,
 * they are still drafts, the repository is private, or GitHub can't be reached. Checked at most
 * once an hour, so a new
 * release shows up on the site without a redeploy.
 */
export async function getReleases(): Promise<Release[]> {
  'use cache';
  cacheLife('hours');

  const headers: Record<string, string> = {
    Accept: 'application/vnd.github+json',
    'X-GitHub-Api-Version': '2022-11-28',
  };
  // Optional: only raises GitHub's rate limit. Visitors still need a public repository to download.
  if (process.env.GITHUB_TOKEN) headers.Authorization = `Bearer ${process.env.GITHUB_TOKEN}`;

  try {
    const res = await fetch(`https://api.github.com/repos/${site.releasesRepo}/releases?per_page=10`, { headers });
    if (!res.ok) return [];
    const releases = (await res.json()) as GitHubRelease[];
    return releases
      .filter((r) => !r.draft)
      .map((r) => ({
        version: r.tag_name.replace(/^v/, ''),
        date: r.published_at ?? '',
        pageUrl: r.html_url,
        // Tauri names them Telex_0.1.0_x64-setup.exe and Telex_0.1.0_x64_en-US.msi.
        exe: pick(r.assets, '.exe'),
        msi: pick(r.assets, '.msi'),
      }))
      .filter((r) => r.exe || r.msi);
  } catch {
    return [];
  }
}

/** The newest release with a Windows installer, or null. */
export async function getLatestRelease(): Promise<Release | null> {
  return (await getReleases())[0] ?? null;
}

export const megabytes = (bytes: number) => `${(bytes / 1024 / 1024).toFixed(1)} MB`;
