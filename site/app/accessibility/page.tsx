import Image from 'next/image';
import Link from 'next/link';
import { CtaBand } from '@/components/CtaBand';
import { PageHeader } from '@/components/PageHeader';
import { pages } from '@/lib/pages';
import { pageMetadata } from '@/lib/seo';
import { site } from '@/lib/site';
// Measured from ui/themes.css by scripts/check-contrast.mjs (`npm run themes` writes this file).
import data from '@/lib/themes.json';
import s from './page.module.css';

export const metadata = pageMetadata(pages.accessibility);

const about: Record<string, { name: string; note: string }> = {
  light: { name: 'Light', note: 'The default when Windows is light. Cool greys, a blue accent.' },
  dark: { name: 'Dark', note: 'The default when Windows is dark. Soft on the eyes at night.' },
  paper: { name: 'Paper', note: 'Teleprinter paper and petrol ink, with a brick-red flag. Selected rows look highlighted.' },
  heather: { name: 'Heather', note: 'Lavender greys, a violet accent and a raspberry flag.' },
  fjord: { name: 'Fjord', note: 'A deep blue-slate night with frost-cyan accents and an amber flag.' },
  phosphor: { name: 'Phosphor', note: 'A green phosphor terminal screen, with an amber flag.' },
  'contrast-light': { name: 'High contrast light', note: 'Black on white. Every text at 7:1 or more, borders you can’t miss.' },
  'contrast-dark': { name: 'High contrast dark', note: 'White on black. Every text at 7:1 or more, borders you can’t miss.' },
};
const swatches = ['surface', 'panel', 'chrome', 'fg', 'muted', 'accent', 'sel', 'flag'];
const ratio = (r: number) => `${r.toFixed(2)}:1`;

const themes = data.themes;
const lowestText = Math.min(...themes.map((t) => t.minText));
const aaa = themes.filter((t) => t.level === 'AAA').length;

const beyond = [
  {
    title: 'Every action on a key',
    text: 'Reading, replying, selecting, moving, filtering and the settings themselves work from the keyboard. Ctrl+, opens Settings; arrow keys move through its tabs and choices. Folder pickers and address suggestions are listboxes a screen reader can follow.',
    link: { href: pages.keyboard.path, label: 'All shortcuts' },
  },
  {
    title: 'A focus ring you can see',
    text: 'Two pixels in the theme’s accent color, at 3:1 or more against what surrounds it. Around main buttons it sits outside the button, so it never disappears into the button’s own color.',
  },
  {
    title: 'Text up to 200%',
    text: 'Settings › Appearance, or Ctrl+= and Ctrl+− (Ctrl+0 for normal). Everything grows together, and the layout reflows: at large sizes the folder pane turns into a menu.',
  },
  {
    title: 'Windows contrast themes',
    text: 'When a Windows contrast theme is on, Windows picks every color and Telex steps aside: the selection, the open folder and pressed choices take the system highlight, so they stay visible.',
  },
  {
    title: 'Less motion on request',
    text: 'If Windows is set to show fewer animations, nothing in Telex spins or slides.',
  },
  {
    title: 'Fields you can find',
    text: 'The border of every text field, menu and button is at least 3:1 against its background (WCAG 1.4.11), not the faint hairline many apps use.',
  },
  {
    title: 'Named for screen readers',
    text: 'Icon buttons have names, the settings use real tabs, radio buttons and dialogs, the list says how it is sorted, and the status bar and the message pane announce changes.',
  },
  {
    title: 'English and Polish',
    text: 'Telex follows the Windows display language, or the one you pick. The page’s language changes with it, so a screen reader reads with the right voice.',
  },
];

const gaps = [
  'Moving through the message list with the arrow keys isn’t announced row by row yet; the message pane reads out the message that opens.',
  'In a light theme HTML mail keeps its sender’s colors on white, so its contrast is the sender’s; dark themes turn its colors around to the theme. Press H for the plain text in your theme.',
  'Messages from the mail servers (errors, refusals) come through as the servers word them, usually in English.',
  'These are our own measurements and checks. Telex hasn’t had an outside accessibility audit yet.',
];

export default function AccessibilityPage() {
  return (
    <>
      <PageHeader
        page={pages.accessibility}
        title="Readable in every theme."
        lead={`Eight themes, from plain light to a green phosphor screen, in English and Polish. Every one meets WCAG 2.2 AA contrast, the ${aaa} high-contrast ones meet AAA, and a script measures every color pair whenever a color changes.`}
      >
        <dl className={s.stats}>
          <div>
            <dt>Themes</dt>
            <dd>{themes.length}</dd>
          </div>
          <div>
            <dt>Pairs measured in each</dt>
            <dd>{data.checked}</dd>
          </div>
          <div>
            <dt>Lowest text contrast</dt>
            <dd>{ratio(lowestText)}</dd>
          </div>
          <div>
            <dt>At level AAA</dt>
            <dd>{aaa}</dd>
          </div>
        </dl>
      </PageHeader>

      <section className={`container ${s.section}`} aria-labelledby="themes">
        <h2 id="themes" className={s.h2}>
          Eight themes
        </h2>
        <p className={s.intro}>
          Pick one in Settings › Appearance; each card there is drawn in its own colors. “Match Windows” switches between
          Light and Dark with the Windows setting.
        </p>
        <ul className={s.gallery}>
          {themes.map((t) => (
            <li key={t.id} className={s.card}>
              <Image
                src={`/screens/theme-${t.id}.png`}
                alt={`Telex in the ${about[t.id]?.name ?? t.id} theme: the folder list, the message list and a message.`}
                width={1280}
                height={800}
                sizes="(max-width: 700px) calc(100vw - 32px), (max-width: 1100px) 50vw, 380px"
                className={s.shot}
              />
              <div className={s.cardHead}>
                <h3>{about[t.id]?.name ?? t.id}</h3>
                <span className={t.level === 'AAA' ? `${s.level} ${s.aaa}` : s.level}>WCAG {t.level}</span>
              </div>
              <p className={s.note}>{about[t.id]?.note}</p>
              <ul className={s.swatches} aria-label="Main colors">
                {swatches.map((k) => {
                  const hex = (t.tokens as Record<string, string>)[k];
                  return (
                    <li key={k} title={`${k} ${hex}`}>
                      <span style={{ background: hex }} aria-hidden="true" />
                      <span className="visually-hidden">
                        {k} {hex}
                      </span>
                    </li>
                  );
                })}
              </ul>
              <p className={s.measured}>
                Text ≥ {ratio(t.minText)} · icons and borders ≥ {ratio(t.minUi)}
              </p>
            </li>
          ))}
        </ul>
      </section>

      <section className={`container ${s.section}`} aria-labelledby="measured">
        <h2 id="measured" className={s.h2}>
          Measured, not eyeballed
        </h2>
        <div className={s.split}>
          <div className="prose">
            <p>
              Every color in Telex is a named token, and every theme sets all of them. For each theme a script works out
              the contrast of every text color on every background it can appear on, including a row under the pointer,
              the selected row and the open folder: {data.checked} pairs per theme.
            </p>
            <ul>
              <li>
                <strong>Text</strong> needs 4.5:1 for level AA (WCAG 1.4.3), 7:1 for AAA (1.4.6). That includes the
                quietest text: dates, sizes, addresses, hints.
              </li>
              <li>
                <strong>Icons, focus rings and field borders</strong> need 3:1 (1.4.11): the unread dot, the flag, the
                account dots and the outline of every field and button.
              </li>
            </ul>
            <p>
              It runs on every change to the code, and a color that misses its level stops the build. Run it yourself:{' '}
              <code>node scripts/check-contrast.mjs</code>, in the{' '}
              <a href={`https://github.com/${site.releasesRepo}/blob/main/scripts/check-contrast.mjs`}>Telex repository</a>.
            </p>
          </div>
          <div className={s.tableWrap}>
            <table className={s.table}>
              <caption className="visually-hidden">The lowest contrast in each theme</caption>
              <thead>
                <tr>
                  <th scope="col">Theme</th>
                  <th scope="col">Level</th>
                  <th scope="col">Lowest text</th>
                  <th scope="col">Lowest icon or border</th>
                </tr>
              </thead>
              <tbody>
                {themes.map((t) => (
                  <tr key={t.id}>
                    <th scope="row">{about[t.id]?.name ?? t.id}</th>
                    <td>{t.level}</td>
                    <td>{ratio(t.minText)}</td>
                    <td>{ratio(t.minUi)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
        <div className={s.details}>
          {themes.map((t) => (
            <details key={t.id}>
              <summary>
                Every pair in {about[t.id]?.name ?? t.id} <span>({t.pairs.length})</span>
              </summary>
              <table className={s.pairs}>
                <thead>
                  <tr>
                    <th scope="col">Where</th>
                    <th scope="col">Color on background</th>
                    <th scope="col">Contrast</th>
                    <th scope="col">Needs</th>
                  </tr>
                </thead>
                <tbody>
                  {t.pairs.map((p) => {
                    const tokens = t.tokens as Record<string, string>;
                    return (
                      <tr key={`${p.kind}-${p.fg}-${p.bg}`}>
                        <td>{p.what}</td>
                        <td>
                          <span className={s.chip} style={{ background: tokens[p.bg], color: tokens[p.fg] }} aria-hidden="true">
                            Aa
                          </span>
                          {p.fg} on {p.bg}
                        </td>
                        <td>{ratio(p.ratio)}</td>
                        <td>{p.need}:1</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </details>
          ))}
        </div>
      </section>

      <section className={`container ${s.section}`} aria-labelledby="beyond">
        <h2 id="beyond" className={s.h2}>
          Beyond color
        </h2>
        <ul className={s.grid}>
          {beyond.map((b) => (
            <li key={b.title}>
              <h3>{b.title}</h3>
              <p>{b.text}</p>
              {b.link && (
                <Link href={b.link.href} className="more">
                  {b.link.label}
                </Link>
              )}
            </li>
          ))}
        </ul>
      </section>

      <section className={`container ${s.section}`} aria-labelledby="gaps">
        <div className={s.gaps}>
          <h2 id="gaps" className={s.gapsTitle}>
            Not there yet
          </h2>
          <ul>
            {gaps.map((g) => (
              <li key={g}>{g}</li>
            ))}
          </ul>
          <p className={s.gapsMore}>
            Something in Telex hard to use with your setup? Write to <a href={`mailto:${site.contact}`}>{site.contact}</a>.
          </p>
        </div>
      </section>
      <CtaBand secondary={pages.keyboard} />
    </>
  );
}
