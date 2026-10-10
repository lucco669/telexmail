import Link from 'next/link';
import { CtaBand } from '@/components/CtaBand';
import { PageHeader } from '@/components/PageHeader';
import { Shot } from '@/components/Shot';
import { WindowFrame } from '@/components/WindowFrame';
import { featureRows, notYet, screens, hero } from '@/lib/content';
import { pages } from '@/lib/pages';
import { pageMetadata } from '@/lib/seo';
import s from './page.module.css';

export const metadata = pageMetadata(pages.features);

const screenById = Object.fromEntries([hero, ...screens].map((x) => [x.id, x]));
const ROW_SIZES = '(max-width: 960px) calc(100vw - 32px), 680px';

export default function FeaturesPage() {
  return (
    <>
      <PageHeader
        page={pages.features}
        title="Everything Telex does today."
        lead="A fast, dense mail client for people who read a lot of mail. Here is what works in the current build, with screenshots, and what doesn’t yet."
      />
      <nav className={`container ${s.toc}`} aria-label="On this page">
        {featureRows.map((f) => (
          <a key={f.id} href={`#${f.id}`}>
            {f.title}
          </a>
        ))}
      </nav>
      <div className="container">
        {featureRows.map((f, i) => {
          const screen = screenById[f.screen];
          return (
            <section key={f.id} id={f.id} className={s.row}>
              <div className={s.text}>
                <p className="eyebrow">
                  <b>{String(i + 1).padStart(2, '0')}</b>
                  {f.soon && <span className="badge"> Coming</span>}
                </p>
                <h2 className={s.title}>{f.title}</h2>
                <p className={s.body}>{f.text}</p>
                <ul className={s.points}>
                  {f.points.map((p) => (
                    <li key={p}>{p}</li>
                  ))}
                </ul>
              </div>
              <div className={s.shot}>
                <WindowFrame title={screen.title}>
                  <Shot id={screen.id} alt={`Telex: ${screen.caption}`} sizes={ROW_SIZES} />
                </WindowFrame>
              </div>
            </section>
          );
        })}

        <aside className={s.gaps} aria-labelledby="not-yet">
          <h2 id="not-yet" className={s.gapsTitle}>
            Not there yet
          </h2>
          <ul>
            {notYet.map((g) => (
              <li key={g}>{g}</li>
            ))}
          </ul>
          <p className={s.gapsMore}>
            What comes when is on the <Link href={pages.roadmap.path}>roadmap</Link>. Wondering whether your provider
            works? See the <Link href={pages.faq.path}>questions and answers</Link>, or read{' '}
            <Link href={pages.about.path}>why Telex is built this way</Link>.
          </p>
        </aside>
      </div>
      <CtaBand secondary={pages.keyboard} />
    </>
  );
}
