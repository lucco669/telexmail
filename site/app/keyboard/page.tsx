import { PageLink as Link } from '@/components/PageNavigation';
import { CtaBand } from '@/components/CtaBand';
import { KeyboardDemo } from '@/components/KeyboardDemo';
import { PageHeader } from '@/components/PageHeader';
import { PrintButton } from '@/components/PrintButton';
import { Since } from '@/components/Since';
import { keyboardTips } from '@/lib/content';
import { pages } from '@/lib/pages';
import { getLatestRelease } from '@/lib/release';
import { pageMetadata } from '@/lib/seo';
import s from './page.module.css';

export const metadata = pageMetadata(pages.keyboard);

export default async function KeyboardPage() {
  const release = await getLatestRelease();
  return (
    <>
      <PageHeader
        page={pages.keyboard}
        title="Keyboard shortcuts."
        lead="Common mail actions take one key, for one message or a whole selection. Expand threads with the arrows, read on with Space, and inspect links on keyboard focus. Press a shortcut here to highlight it, or print the list as a cheat sheet."
      >
        <PrintButton label="Print as a cheat sheet" />
        <Link href={pages.demo.path} className="btn no-print">
          Press them in the live demo
        </Link>
      </PageHeader>
      <div className={`container ${s.layout}`}>
        <KeyboardDemo release={release} grouped />
        <aside className={s.tips} aria-labelledby="tips">
          <h2 id="tips" className={s.tipsTitle}>
            Good to know
          </h2>
          {keyboardTips.map((t) => (
            <div key={t.title} className={s.tip}>
              <h3>
                {t.title} <Since since={t.since} release={release} className={s.badge} />
              </h3>
              <p>{t.text}</p>
            </div>
          ))}
        </aside>
      </div>
      <CtaBand secondary={pages.demo} />
    </>
  );
}
