import { PageLink as Link } from '@/components/PageNavigation';
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
  ['Local mail base', 'Mail is stored on your disk in full and searchable offline. Saved queries put reusable views beside your folders, without copying mail.'],
  ['Powerful filters', 'Match text, named headers and attachments, preview results, then sort, flag, move, copy or forward new mail, with notes explaining changes.'],
  ['Private by default', 'Remote images need permission or sender trust. The spam filter learns on your PC; links and scam warning signs are checked there too, without uploading mail.'],
  ['Fluent in Polish', 'Old charsets like Windows-1250 and ISO-8859-2 decode correctly; replies quote the Polish way.'],
  ['HTML when it helps', 'New messages and replies start in HTML and always go out with a plain-text copy. Plain text in a mono face is a setting, or a switch on any one message.'],
  ['Readable', 'Fourteen themes with measured AA contrast, two at AAA, any accent color, six fonts and five toolbar layouts. Optional threads keep conversations compact without hiding unread replies from Space.'],
  ['Mail follows the theme', 'In a dark theme HTML mail goes on the theme’s dark paper with its colors turned around; one click shows it on white, as sent.'],
  ['Honest about mail', '“Sent” only once your server has accepted the message. Until then it waits in the Outbox, with the reason.'],
  ['Forgiving', 'Z undoes a delete or a move, and takes back a message in the moment after Send. Esc keeps a draft, never loses it.'],
];

const never = ['Electron', 'AI features switched on by default', 'A web-app look', 'Anything that phones home without asking'];

const stack = [
  ['Mail core', 'Rust: IMAP with IDLE (async-imap), SMTP (lettre), MIME (mail-parser), TLS (rustls), HTML cleaning (ammonia), rules, a learning spam filter and offline scam warning checks'],
  ['Mail base', 'SQLite with FTS5 full-text search, one file on your disk'],
  ['Window', 'Tauri 2: WebView2 on Windows, WebKitGTK on Linux, so no browser is bundled'],
  ['Interface', 'Plain HTML, CSS and JavaScript, with six fonts, fourteen themes and five toolbar layouts'],
  ['This site', 'Next.js on our own server behind Cloudflare, with no cookies and cookieless page counts. The live demo is the app’s own interface'],
];

export default function AboutPage() {
  return (
    <>
      <PageHeader
        page={pages.about}
        title="A mail client built like a tool."
        lead="Telex is for people who get a lot of mail and want to get through it fast: a compact, keyboard-driven client for Windows and Linux in the spirit of The Bat!, with your mail kept on your own disk."
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
          <p>
            <Link href={pages.history.path}>Explore the history of telex and email</Link>: the machines, the networks,
            and the ideas that still shape written communication.
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
