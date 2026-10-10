import { Logo } from './Logo';
import s from './Header.module.css';

const links = [
  ['#features', 'Features'],
  ['#keyboard', 'Keyboard'],
  ['#screens', 'Screens'],
  ['#roadmap', 'Roadmap'],
];

export function Header() {
  return (
    <header className={s.header}>
      <div className={`container ${s.bar}`}>
        <a href="#top" className={s.brand} aria-label="Telex, back to top">
          <Logo />
        </a>
        <nav className={s.nav} aria-label="Sections">
          {links.map(([href, label]) => (
            <a key={href} href={href}>
              {label}
            </a>
          ))}
        </nav>
        <a href="#download" className="btn btn-primary btn-sm">
          Download
        </a>
      </div>
    </header>
  );
}
