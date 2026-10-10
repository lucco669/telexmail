import Link from 'next/link';
import { CtaBand } from '@/components/CtaBand';
import { PageHeader } from '@/components/PageHeader';
import { pages } from '@/lib/pages';
import { pageMetadata } from '@/lib/seo';
import s from './page.module.css';

export const metadata = pageMetadata(pages.about);

const principles = [
  ['Dense', '20-pixel rows by default, a condensed face in the list, mono for dates and sizes. No spacious padding; a Comfortable setting for people who want air.'],
  ['Keyboard-first', 'Every common action has a single key. The mouse is optional.'],
  ['Fast and light', 'Starts quickly, installs small, and needs no account or cloud service.'],
  ['Local mail base', 'Mail is stored on your disk in full and searchable offline.'],
  ['Powerful filters', 'Rules that sort, flag, move and auto-reply, with the reason shown on every message they touch.'],
  ['Private by default', 'Remote images blocked until you allow them. Links in spam not clickable. Nothing phones home.'],
  ['Fluent in Polish', 'Old charsets like Windows-1250 and ISO-8859-2 decode correctly; replies quote the Polish way.'],
  ['Plain text first', 'Compose is plain text in a mono face. Mail between people reads best that way.'],
];

const never = ['Electron', 'AI features switched on by default', 'A web-app look', 'Anything that phones home'];

const stack = [
  ['Mail core', 'Rust: IMAP (async-imap), SMTP (lettre), MIME (mail-parser), TLS (rustls)'],
  ['Mail base', 'SQLite with FTS5 full-text search, one file on your disk'],
  ['Window', 'Tauri 2 and WebView2, which Windows already has'],
  ['Interface', 'Plain HTML, CSS and JavaScript, set in IBM Plex'],
  ['This site', 'Next.js on Vercel, with no cookies and cookieless page counts'],
];

export default function AboutPage() {
  return (
    <>
      <PageHeader
        page={pages.about}
        title="A mail client built like a tool."
        lead="Telex is for people who get a lot of mail and want to get through it fast: a compact, keyboard-driven client for Windows in the spirit of The Bat!, with your mail kept on your own disk."
      />

      <section className={`container ${s.section}`} aria-labelledby="feel">
        <h2 id="feel" className={s.h2}>
          What it should feel like
        </h2>
        <ol className={s.principles}>
          {principles.map(([title, text], i) => (
            <li key={title}>
              <span className={s.num}>{String(i + 1).padStart(2, '0')}</span>
              <h3>{title}</h3>
              <p>{text}</p>
            </li>
          ))}
        </ol>
      </section>

      <section className={`container ${s.section} ${s.split}`}>
        <div>
          <h2 className={s.h2}>What it will never be</h2>
          <ul className={s.never}>
            {never.map((n) => (
              <li key={n}>{n}</li>
            ))}
          </ul>
        </div>
        <div className="prose">
          <h2 className={s.h2}>Where it comes from</h2>
          <p>
            For years The Bat! showed what a mail client can be when it is built for speed: dense lists, a key for
            everything, and a Sorting Office of rules that does the filing for you. Telex aims for that feel, rebuilt
            from scratch for today’s Windows. It isn’t affiliated with Ritlabs.
          </p>
          <p>
            The name comes from telex, the network of teleprinters that carried typed messages around the world before
            email. The dot-matrix T in the logo is printed the way a teleprinter prints, and the paper tape above the
            footer is punched in ITA2, the code real telex machines used. It spells out what you read under it.
          </p>
        </div>
      </section>

      <section className={`container ${s.section}`} aria-labelledby="stack">
        <h2 id="stack" className={s.h2}>
          Built with
        </h2>
        <dl className={s.stack}>
          {stack.map(([k, v]) => (
            <div key={k}>
              <dt>{k}</dt>
              <dd>{v}</dd>
            </div>
          ))}
        </dl>
        <p className={s.after}>
          Curious where it’s going? <Link href={pages.roadmap.path}>See the roadmap</Link>.
        </p>
      </section>
      <CtaBand secondary={pages.features} />
    </>
  );
}
