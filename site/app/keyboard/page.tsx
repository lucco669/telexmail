import { CtaBand } from '@/components/CtaBand';
import { KeyboardDemo } from '@/components/KeyboardDemo';
import { PageHeader } from '@/components/PageHeader';
import { PrintButton } from '@/components/PrintButton';
import { keyboardTips } from '@/lib/content';
import { pages } from '@/lib/pages';
import { pageMetadata } from '@/lib/seo';
import s from './page.module.css';

export const metadata = pageMetadata(pages.keyboard);

export default function KeyboardPage() {
  return (
    <>
      <PageHeader
        page={pages.keyboard}
        title="Keyboard shortcuts."
        lead="Everything you do all day is one key, no Ctrl chords. Press any of them here to try it, or print the list and keep it next to your keyboard."
      >
        <PrintButton label="Print as a cheat sheet" />
      </PageHeader>
      <div className={`container ${s.layout}`}>
        <KeyboardDemo grouped />
        <aside className={s.tips} aria-labelledby="tips">
          <h2 id="tips" className={s.tipsTitle}>
            Good to know
          </h2>
          {keyboardTips.map((t) => (
            <div key={t.title} className={s.tip}>
              <h3>{t.title}</h3>
              <p>{t.text}</p>
            </div>
          ))}
        </aside>
      </div>
      <CtaBand secondary={pages.features} />
    </>
  );
}
