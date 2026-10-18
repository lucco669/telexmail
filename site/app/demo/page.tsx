import { PageLink as Link } from '@/components/PageNavigation';
import { CtaBand } from '@/components/CtaBand';
import { DemoFirst } from '@/components/DemoFirst';
import { DemoFrame } from '@/components/DemoFrame';
import { PageHeader } from '@/components/PageHeader';
import { demoTries } from '@/lib/content';
import { pages } from '@/lib/pages';
import { pageMetadata } from '@/lib/seo';
import s from './page.module.css';

export const metadata = pageMetadata(pages.demo);

export default function DemoPage() {
  return (
    <>
      <PageHeader
        page={pages.demo}
        compact
        title="Try Telex in your browser."
        lead="Telex’s real interface on a made-up mailbox. Try saved searches, rule previews, expandable threads and five toolbar layouts, then read, reply and organize with the keyboard. Your mailbox edits stay in this tab."
      />

      <section className={`container ${s.stage}`} aria-label="Live demo">
        <DemoFirst title="Try these first" className={s.first} />
        <DemoFrame />
        <p className={s.small}>
          Telex is a desktop app, so the demo is best on a computer with a keyboard. On a phone it works, but cramped.
        </p>
      </section>

      <section className={`container ${s.section}`} aria-labelledby="try">
        <h2 id="try" className={s.h2}>
          Things to try
        </h2>
        <ol className={s.tries}>
          {demoTries.map((t) => (
            <li key={t.title}>
              <p className={s.keys}>
                {t.keys.map((k, i) => (
                  <span key={i} className={s.combo}>
                    {i > 0 && <span className={s.then}>then</span>}
                    {k.split('+').map((part, j) => (
                      <span key={j} className={s.combo}>
                        {j > 0 && <span className={s.plus}>+</span>}
                        <kbd className="kbd">{part}</kbd>
                      </span>
                    ))}
                  </span>
                ))}
              </p>
              <h3>{t.title}</h3>
              <p>{t.text}</p>
            </li>
          ))}
        </ol>
        <p className={s.more}>
          <Link className="more" href={pages.keyboard.path}>
            Every shortcut
          </Link>
        </p>
      </section>

      <section className={`container ${s.section} ${s.split}`} aria-label="About this demo">
        <div>
          <h2 className={s.h3}>What is real</h2>
          <ul className={s.list}>
            <li>The whole interface, from the folder tree to the settings, exactly as the app has it.</li>
            <li>Every key, the selection, search with its suggestions and operators, compose in text or HTML.</li>
            <li>Saved queries, header and attachment conditions, rule previews and copying inside the demo mailbox.</li>
            <li>Optional thread rows, all five toolbar layouts, fourteen themes, accent colors, six fonts, text size and spacing.</li>
            <li>Link destinations and copying, including in Spam, and the interface in English and Polish.</li>
          </ul>
        </div>
        <div>
          <h2 className={s.h3}>What is pretend</h2>
          <ul className={s.list}>
            <li>The mailbox: Sam Carter of Linden Studio, and the people who write to him, are made up.</li>
            <li>
              The servers. In the app, a Rust core syncs with IMAP and sends over SMTP; here a small script in the page
              answers in its place. “Sent” mail only moves to Sent, and attachments can’t be opened.
            </li>
            <li>
              The scam report and the spam filter. Warning reasons and the spam note are fixed illustrative examples;
              the desktop app’s offline Rust scanner and spam filter don’t run here, so Not spam moves mail without
              learning. Automatic forwarding cannot send mail from the demo.
            </li>
            <li>Memory. Reload, or press Start over, and everything is as it was.</li>
          </ul>
        </div>
        <div>
          <h2 className={s.h3}>What leaves your browser</h2>
          <ul className={s.list}>
            <li>Nothing. What you type, send or attach stays in this tab; files you attach aren’t even read.</li>
            <li>
              Like the app, the demo remembers your theme and pane sizes in this browser’s local storage. The{' '}
              <Link href={`${pages.privacy.path}#demo`}>privacy page</Link> has the details.
            </li>
          </ul>
        </div>
      </section>
      <CtaBand title="Like it? Use it on your own mail." secondary={pages.features} />
    </>
  );
}
