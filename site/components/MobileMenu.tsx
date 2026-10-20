'use client';

import { useRef } from 'react';
import { NavLinks, type NavItem } from './NavLinks';
import { ThemeToggle } from './ThemeToggle';
import s from './Header.module.css';

/** The menu on narrow screens: a popover, so Esc and clicking outside close it. */
export function MobileMenu({ items }: { items: NavItem[] }) {
  const menu = useRef<HTMLDivElement>(null);
  const close = () => menu.current?.hidePopover();
  return (
    <>
      <button type="button" className={s.menuBtn} popoverTarget="site-menu" aria-label="Menu">
        <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" aria-hidden="true">
          <path d="M2.5 4.5h11M2.5 8h11M2.5 11.5h11" />
        </svg>
      </button>
      <div id="site-menu" popover="auto" ref={menu} className={s.menu}>
        <nav aria-label="Pages" className={s.menuNav}>
          <NavLinks items={items} onNavigate={close} />
        </nav>
        <div className={s.menuFoot}>
          <span>Theme</span>
          <ThemeToggle />
        </div>
      </div>
    </>
  );
}
