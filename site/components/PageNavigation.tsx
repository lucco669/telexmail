'use client';

import NextLink from 'next/link';
import { usePathname } from 'next/navigation';
import { createContext, useCallback, useContext, useLayoutEffect, useRef, type ComponentProps, type ReactNode } from 'react';

const NavigationContext = createContext<(pathname: string | null) => void>(() => {});

/** Reset after a page-link navigation commits, while preserving browser history and section links. */
export function PageNavigation({ children }: { children: ReactNode }) {
  const pathname = usePathname();
  const pendingPath = useRef<string | null>(null);
  const prepareNavigation = useCallback((path: string | null) => { pendingPath.current = path; }, []);

  useLayoutEffect(() => {
    if (pendingPath.current === pathname) {
      window.scrollTo({ top: 0, left: 0, behavior: 'instant' });
    }
    pendingPath.current = null;
  }, [pathname]);

  return <NavigationContext.Provider value={prepareNavigation}>{children}</NavigationContext.Provider>;
}

type PageLinkProps = Omit<ComponentProps<typeof NextLink>, 'href'> & { href: string };

/** Ordinary page links start at the top; explicit hash links keep their section target. */
export function PageLink({ href, onNavigate, ...props }: PageLinkProps) {
  const prepareNavigation = useContext(NavigationContext);

  return (
    <NextLink
      {...props}
      href={href}
      onNavigate={(event) => {
        let prevented = false;
        onNavigate?.({ preventDefault: () => { prevented = true; event.preventDefault(); } });
        if (prevented) return;

        prepareNavigation(null);
        const target = new URL(href, window.location.href);
        if (target.hash) return;

        if (target.pathname === window.location.pathname) {
          // Clicking the current page (including the home logo) needs no route refresh.
          const sameUrl = target.href === window.location.href;
          if (sameUrl) event.preventDefault();
          const smooth = sameUrl && !window.matchMedia('(prefers-reduced-motion: reduce)').matches;
          window.scrollTo({ top: 0, left: 0, behavior: smooth ? 'smooth' : 'instant' });
        } else {
          prepareNavigation(target.pathname);
        }
      }}
    />
  );
}
