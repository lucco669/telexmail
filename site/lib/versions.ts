import type { Release } from './release';

// What the site says about a feature built for a version: "Coming in 0.5" until GitHub has a release
// that new, "New in 0.5" while it is the newest minor version, and nothing once a newer one is out.

const parts = (v: string) => v.split('.').map((n) => parseInt(n, 10) || 0);

/** -1, 0 or 1, comparing dotted version numbers. */
export function compareVersions(a: string, b: string): number {
  const x = parts(a), y = parts(b);
  for (let i = 0; i < Math.max(x.length, y.length); i++) {
    const d = (x[i] ?? 0) - (y[i] ?? 0);
    if (d) return Math.sign(d);
  }
  return 0;
}

/** "0.5.0" → "0.5". */
export const shortVersion = (v: string) => parts(v).slice(0, 2).join('.');

/** A version as the site says it: "0.9.0" → "0.9", but a patch keeps its number ("0.9.1"). */
export const versionName = (v: string) => (parts(v)[2] ? v : shortVersion(v));

/** Whether the newest release already has what `since` brings. */
export const isReleased = (since: string, release: Release | null) => !!release && compareVersions(release.version, since) >= 0;

export type SinceLabel = { text: string; released: boolean };

/** The badge for something built for `since`, or null when there is nothing to say. */
export function sinceLabel(since: string | undefined, release: Release | null): SinceLabel | null {
  if (!since) return null;
  if (!isReleased(since, release)) return { text: `Coming in ${versionName(since)}`, released: false };
  if (shortVersion(release!.version) === shortVersion(since)) return { text: `New in ${versionName(since)}`, released: true };
  return null;
}

/** The changelog's anchor for a version: "0.5.0" → "v0-5-0". */
export const versionAnchor = (v: string) => `v${v.replaceAll('.', '-')}`;
