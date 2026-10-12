import { PageLink as Link } from '@/components/PageNavigation';
import { nav, pages } from '@/lib/pages';
import { Logo } from './Logo';
import { MobileMenu } from './MobileMenu';
import { NavLinks } from './NavLinks';
import { ThemeToggle } from './ThemeToggle';
import s from './Header.module.css';

export function Header() {
  return (
    <header className={`${s.header} no-print`}>
      <div className={`container ${s.bar}`}>
        <Link href="/" className={s.brand} aria-label="Telex home">
          <Logo />
        </Link>
        <nav className={s.nav} aria-label="Pages">
          <NavLinks items={nav} className={s.link} />
        </nav>
        <ThemeToggle className={s.theme} />
        <Link href={pages.download.path} className="btn btn-primary btn-sm">
          Download
        </Link>
        <MobileMenu />
      </div>
    </header>
  );
}
