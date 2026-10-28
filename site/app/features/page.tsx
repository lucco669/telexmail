import { PageLink as Link } from '@/components/PageNavigation';
import { CtaBand } from '@/components/CtaBand';
import { PageHeader } from '@/components/PageHeader';
import { Shot } from '@/components/Shot';
import { Since } from '@/components/Since';
import { WindowFrame } from '@/components/WindowFrame';
import { featureRows, hero, notYet, screens } from '@/lib/content';
import { pages } from '@/lib/pages';
import { getLatestRelease } from '@/lib/release';
import { pageMetadata } from '@/lib/seo';
import s from './page.module.css';

export const metadata = pageMetadata(pages.features);

const screenById = Object.fromEntries([hero, ...screens].map((x) => [x.id, x]));
const ROW_SIZES = '(max-width: 960px) calc(100vw - 32px), 680px';

export default async function FeaturesPage() {
  const release = await getLatestRelease();
  return (
    <>
      <PageHeader
        page={pages.features}
        title="Everything Telex does today."
        lead="A fast, dense mail client for people who read a lot of mail. Mailbox-first checks, preview reply buttons, today’s unread markers and local scam filtering join offline search, rules, encrypted mail and fourteen themes. Explore each feature with screenshots."
      >
        <Link href={pages.demo.path} className="btn btn-primary">
          Try it in your browser
        </Link>
        <Link href={pages.changelog.path} className="btn">
          What’s new
        </Link>
      </PageHeader>
      <nav className={`container ${s.toc}`} aria-label="On this page">
        {featureRows.map((f) => (
          <a key={f.id} href={`#${f.id}`}>
            {f.title}
            <Since since={f.since} release={release} className={s.tocBadge} />
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
                  <b>{String(i + 1).padStart(2, '0')}</b> <Since since={f.since} release={release} />
                </p>
                <h2 className={s.title}>{f.title}</h2>
                <p className={s.body}>{f.text}</p>
                <ul className={s.points}>
                  {f.points.map((p) => {
                    const point = typeof p === 'string' ? { text: p, since: undefined } : p;
                    return (
                      <li key={point.text}>
                        {point.text} <Since since={point.since} release={release} className={s.pointBadge} />
                      </li>
                    );
                  })}
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
            What comes when is on the <Link href={pages.roadmap.path}>roadmap</Link>, and what each version brought is in
            the <Link href={pages.changelog.path}>changelog</Link>. Wondering whether your provider works? See the{' '}
            <Link href={pages.faq.path}>questions and answers</Link>, or read{' '}
            <Link href={pages.about.path}>why Telex is built this way</Link>.
          </p>
        </aside>
      </div>
      <CtaBand secondary={pages.demo} />
    </>
  );
}
