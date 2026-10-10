import { Logo } from './Logo';
import { Tape } from './Tape';
import s from './Footer.module.css';

const links = [
  ['#features', 'Features'],
  ['#keyboard', 'Keyboard'],
  ['#screens', 'Screens'],
  ['#roadmap', 'Roadmap'],
  ['#download', 'Download'],
];

export function Footer() {
  return (
    <footer className={s.footer}>
      <Tape text="Telex  fast keyboard mail for Windows  " />
      <div className={`container ${s.inner}`}>
        <div className={s.brand}>
          <Logo />
          <p>Fast, keyboard-driven email for Windows.</p>
        </div>
        <nav className={s.nav} aria-label="Footer">
          {links.map(([href, label]) => (
            <a key={href} href={href}>
              {label}
            </a>
          ))}
        </nav>
      </div>
      <div className={`container ${s.fine}`}>
        <p>Screenshots show the sample mailbox built into the app.</p>
        <p>The tape above is punched in ITA2, the code real telex machines used. It says what Telex is.</p>
      </div>
    </footer>
  );
}
