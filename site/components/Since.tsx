import type { Release } from '@/lib/release';
import { sinceLabel } from '@/lib/versions';

/** "Coming in 0.5" or "New in 0.5" next to something built for a version (see NEXT in lib/content.ts). */
export function Since({ since, release, className = '' }: { since?: string; release: Release | null; className?: string }) {
  const label = sinceLabel(since, release);
  if (!label) return null;
  return <span className={`badge ${label.released ? 'badge-new' : 'badge-coming'} ${className}`}>{label.text}</span>;
}
