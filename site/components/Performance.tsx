import { PageLink as Link } from '@/components/PageNavigation';
import { benchmarkDate, performance, performanceFacts, ramSavedPercent } from '@/lib/performance';
import { pages } from '@/lib/pages';
import type { Release } from '@/lib/release';
import s from './Performance.module.css';

export function Performance({ release, compact = false }: { release: Release | null; compact?: boolean }) {
  return (
    <section className={`section ${s.band}${compact ? ` ${s.compact}` : ''}`} aria-labelledby="performance-heading">
      <div className="container">
        <header className="section-head">
          <p className="eyebrow">Measured on Windows</p>
          <h2 id="performance-heading" className="h2">Light on your PC.<br />Quick through your inbox.</h2>
          <p className="lead">A small download, a quick start, and room for years of mail. Here is what Telex did with 100,000 messages.</p>
        </header>
        <dl className={s.stats}>
          {performanceFacts(release).map(fact => (
            <div key={fact.label}>
              <dt>{fact.label}</dt>
              <dd>{fact.value}</dd>
              <dd className={s.statNote}>{fact.note}</dd>
            </div>
          ))}
        </dl>
        {!compact && ramSavedPercent !== null && ramSavedPercent > 0 && <p className={s.memory}>
          <strong>{ramSavedPercent}% less idle private RAM than Thunderbird.</strong> In our offline Windows test with the same 100,000 full messages,
          including every app process. Both windows visible, unminimized and in the background.
        </p>}
        <div className={s.after}>
          <p>Desktop timings: Telex {performance.version} benchmark build · {benchmarkDate} · one PC. Search includes the typing delay and rendering.</p>
          <Link className="more" href={`${pages.performance.path}${compact ? '#method' : ''}`}>{compact ? 'How we measured' : 'Results, comparisons & how we measured'}</Link>
        </div>
      </div>
    </section>
  );
}
