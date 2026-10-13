import { CtaBand } from '@/components/CtaBand';
import { PageHeader } from '@/components/PageHeader';
import { faqGroups } from '@/lib/content';
import { pages } from '@/lib/pages';
import { faqLd, pageMetadata } from '@/lib/seo';
import s from './page.module.css';

export const metadata = pageMetadata(pages.faq);

const slug = (t: string) => t.toLowerCase().replace(/[^a-z]+/g, '-').replace(/^-|-$/g, '');

export default function FaqPage() {
  return (
    <>
      <PageHeader
        page={pages.faq}
        title="Questions and answers."
        lead="Which providers work, how saved searches and richer rules behave, how to customize threads and toolbars, what local scam checks mean, and what to expect when you install."
        extraLd={[faqLd(faqGroups.flatMap((g) => g.items))]}
      />
      <div className={`container ${s.layout}`}>
        <nav className={s.toc} aria-label="Topics">
          {faqGroups.map((g) => (
            <a key={g.title} href={`#${slug(g.title)}`}>
              {g.title}
            </a>
          ))}
        </nav>
        <div>
          {faqGroups.map((g) => (
            <section key={g.title} id={slug(g.title)} className={s.group} aria-labelledby={`${slug(g.title)}-h`}>
              <h2 id={`${slug(g.title)}-h`} className={s.groupTitle}>
                {g.title}
              </h2>
              {g.items.map((item) => (
                <details key={item.q} className={s.item}>
                  <summary className={s.q}>{item.q}</summary>
                  <p className={s.a}>{item.a}</p>
                </details>
              ))}
            </section>
          ))}
        </div>
      </div>
      <CtaBand secondary={pages.features} />
    </>
  );
}
