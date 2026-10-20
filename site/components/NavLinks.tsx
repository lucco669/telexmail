'use client';

import { PageLink as Link } from '@/components/PageNavigation';
import { usePathname } from 'next/navigation';
import type { PageInfo } from '@/lib/pages';

export type NavItem = Pick<PageInfo, 'path' | 'label'>;

/** Page links that mark the page you're on. `onNavigate` lets the phone menu close itself. */
export function NavLinks({ items, className, onNavigate }: { items: NavItem[]; className?: string; onNavigate?: () => void }) {
  const path = usePathname();
  return (
    <>
      {items.map((p) => (
        <Link
          key={p.path}
          href={p.path}
          className={className}
          aria-current={path === p.path ? 'page' : undefined}
          onClick={onNavigate}
        >
          {p.label}
        </Link>
      ))}
    </>
  );
}
