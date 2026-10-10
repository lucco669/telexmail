import { roadmap } from '@/lib/content';
import { CheckIcon } from './Icons';
import s from './Roadmap.module.css';

const LABEL = { done: 'Done', now: 'In progress', next: 'Up next', later: 'Planned' } as const;

export function Roadmap() {
  return (
    <section id="roadmap" className={`section ${s.section}`}>
      <div className={`container ${s.layout}`}>
        <header className="section-head">
          <p className="eyebrow">
            <b>04</b> · Roadmap
          </p>
          <h2 className="h2">Early, and honest about it.</h2>
          <p className="lead">Telex keeps real mail in sync with your server today, but it doesn’t send yet. This is what comes next, in order.</p>
        </header>
        <ol className={s.list}>
          {roadmap.map((step, i) => (
            <li key={step.title} className={s.step} data-status={step.status}>
              <span className={s.marker} aria-hidden="true">
                {step.status === 'done' ? <CheckIcon /> : null}
              </span>
              <div className={s.body}>
                <div className={s.top}>
                  <span className={s.num}>{String(i + 1).padStart(2, '0')}</span>
                  <h3 className={s.title}>{step.title}</h3>
                  <span className={s.badge}>{LABEL[step.status]}</span>
                </div>
                <p className={s.text}>{step.text}</p>
              </div>
            </li>
          ))}
        </ol>
      </div>
    </section>
  );
}
