import Link from 'next/link';
import { NEXT, ticker } from '@/lib/content';
import { footerGroups } from '@/lib/pages';
import { getLatestRelease } from '@/lib/release';
import { site } from '@/lib/site';
import { isReleased, shortVersion } from '@/lib/versions';
import { DotMatrix } from './DotMatrix';
import { Logo } from './Logo';
import { TelexTicker } from './TelexTicker';
import { ThemeToggle } from './ThemeToggle';
import s from './Footer.module.css';

const year = (process.env.BUILD_DATE ?? '2026').slice(0, 4);

export async function Footer() {
  const release = await getLatestRelease();
  const out = release
    ? [`VERSION ${release.version} IS OUT${release.date ? ` ${release.date.slice(0, 10).split('-').reverse().join('.')}` : ''}`]
    : [];
  const next = `${isReleased(NEXT, release) ? 'NEW' : 'COMING'} IN ${shortVersion(NEXT)}: ATTACHMENTS, CONVERSATIONS, UNDO SEND`;
  const messages = [ticker[0], ...out, next, ...ticker.slice(1)];
  return (
    <footer className={`${s.footer} no-print`}>
      <TelexTicker messages={messages} />
      <div className={`container ${s.top}`}>
        <div className={s.brand}>
          <Logo />
          <p>{site.description}</p>
          <ThemeToggle />
        </div>
        <nav className={s.groups} aria-label="Footer">
          {footerGroups.map((g) => (
            <div key={g.title} className={s.group}>
              <h2 className={s.groupTitle}>{g.title}</h2>
              <ul>
                {g.items.map((p) => (
                  <li key={p.path}>
                    <Link href={p.path}>{p.label}</Link>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </nav>
      </div>
      <div className={`container ${s.word}`}>
        <DotMatrix word="Telex" />
      </div>
      <div className={`container ${s.bottom}`}>
        <span>
          © {year} {site.name}. Made for Windows and Linux, with Rust and Tauri.
        </span>
        {site.contact && (
          <a href={`mailto:${site.contact}`} className={s.top_link}>
            {site.contact}
          </a>
        )}
        <a href="#top" className={s.top_link}>
          Back to top ↑
        </a>
      </div>
    </footer>
  );
}
