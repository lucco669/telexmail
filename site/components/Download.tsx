import Link from 'next/link';
import { benefits, faq } from '@/lib/content';
import { pages } from '@/lib/pages';
import type { Release } from '@/lib/release';
import { Compat, DownloadChoice, ReleaseStatus } from './DownloadChoice';
import s from './Download.module.css';

/** The home page's download section: Windows or Linux, what it costs, what works, and what you get. */
export function Download({ release, unknown = false }: { release: Release | null; unknown?: boolean }) {
  return (
    <section id="download" className="section">
      <div className={`container ${s.layout}`}>
        <div className={s.card}>
          <p className="eyebrow">
            <b>07</b> · Download
          </p>
          <h2 className="h2">Get Telex.</h2>
          <ReleaseStatus release={release} unknown={unknown} />
          <DownloadChoice release={release} unknown={unknown} />
          <Compat />
          <ul className={s.benefits}>
            {benefits.map((b) => (
              <li key={b.title}>
                <b>{b.title}.</b> {b.text}
              </li>
            ))}
          </ul>
          <p className={s.links}>
            <Link className="more" href={pages.download.path}>
              Install steps and all versions
            </Link>
          </p>
        </div>

        <div>
          <h3 className={s.faqTitle}>Questions</h3>
          {faq.map((item) => (
            <details key={item.q} className={s.item} name="faq">
              <summary className={s.q}>{item.q}</summary>
              <p className={s.a}>{item.a}</p>
            </details>
          ))}
          <p className={s.faqMore}>
            <Link className="more" href={pages.faq.path}>
              All questions
            </Link>
          </p>
        </div>
      </div>
    </section>
  );
}
