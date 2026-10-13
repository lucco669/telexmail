import type { Release } from '@/lib/release';
import { sinceLabel } from '@/lib/versions';

/** "Coming" or "New" next to a feature's fixed introduction version. */
export function Since({ since, release, className = '' }: { since?: string; release: Release | null; className?: string }) {
  const label = sinceLabel(since, release);
  if (!label) return null;
  return <span className={`badge ${label.released ? 'badge-new' : 'badge-coming'} ${className}`}>{label.text}</span>;
}
