import Link from 'next/link';
import { roadmap, type Step } from '@/lib/content';
import { pages } from '@/lib/pages';
import type { Release } from '@/lib/release';
import { isReleased, shortVersion } from '@/lib/versions';
import { CheckIcon } from './Icons';
import s from './Roadmap.module.css';

const LABEL = { done: 'Done', now: 'In progress', next: 'Up next', later: 'Planned' } as const;

/** A step built for a version is "now" until that version is released. */
export function stepStatus(step: Step, release: Release | null): Step['status'] {
  return step.since && !isReleased(step.since, release) ? 'now' : step.status;
}

/**
 * The roadmap as a rail of steps. `details` adds what each finished step doesn't do yet; `recent`
 * folds all but that many finished steps into one line.
 */
export function RoadmapList({ release, details = false, recent }: { release: Release | null; details?: boolean; recent?: number }) {
  const steps = roadmap.map((step, i) => ({ step, i, status: stepStatus(step, release) }));
  const done = steps.filter((x) => x.status === 'done');
  const folded = recent === undefined ? [] : done.slice(0, Math.max(0, done.length - recent));
  const shown = steps.filter((x) => !folded.includes(x));
  return (
    <ol className={s.list}>
      {folded.length > 0 && (
        <li className={s.step} data-status="done">
          <span className={s.marker} aria-hidden="true">
            <CheckIcon />
          </span>
          <div className={s.body}>
            <div className={s.top}>
              <span className={s.num}>
                01–{String(folded.length).padStart(2, '0')}
              </span>
              <h3 className={s.title}>{folded.length} steps done before these</h3>
            </div>
            <p className={s.text}>{folded.map((x) => x.step.title).join(' · ')}</p>
          </div>
        </li>
      )}
      {shown.map(({ step, i, status }) => {
        const label = status === 'now' && step.since ? `Coming in ${shortVersion(step.since)}` : LABEL[status];
        return (
          <li key={step.title} className={s.step} data-status={status}>
            <span className={s.marker} aria-hidden="true">
              {status === 'done' ? <CheckIcon /> : null}
            </span>
            <div className={s.body}>
              <div className={s.top}>
                <span className={s.num}>{String(i + 1).padStart(2, '0')}</span>
                <h3 className={s.title}>{step.title}</h3>
                <span className={s.badge}>{label}</span>
              </div>
              <p className={s.text}>{step.text}</p>
              {details && step.notYet && (
                <p className={s.notYet}>
                  <span>Not yet:</span> {step.notYet}
                </p>
              )}
            </div>
          </li>
        );
      })}
    </ol>
  );
}

/** The roadmap section on the home page. */
export function Roadmap({ release }: { release: Release | null }) {
  return (
    <section id="roadmap" className={`section ${s.section}`}>
      <div className={`container ${s.layout}`}>
        <header className="section-head">
          <p className="eyebrow">
            <b>06</b> · Roadmap
          </p>
          <h2 className="h2">Early, and honest about it.</h2>
          <p className="lead">
            Telex reads, syncs and sends real mail today, on Windows and Linux. This is what was built, in order, and what
            comes next.
          </p>
          <p>
            <Link className="more" href={pages.roadmap.path}>
              The full roadmap
            </Link>
          </p>
          <p>
            <Link className="more" href={pages.changelog.path}>
              What each version brought
            </Link>
          </p>
        </header>
        <RoadmapList release={release} recent={2} />
      </div>
    </section>
  );
}
