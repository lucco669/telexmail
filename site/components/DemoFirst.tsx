import { Fragment } from 'react';
import { demoFirst } from '@/lib/content';
import s from './DemoFirst.module.css';

/** Text with keys in square brackets, "[Z]", drawn as keycaps. */
function KeyText({ text }: { text: string }) {
  return text.split(/\[([^\]]+)\]/).map((part, i) =>
    i % 2 ? (
      <kbd key={i} className="kbd">
        {part}
      </kbd>
    ) : (
      <Fragment key={i}>{part}</Fragment>
    ),
  );
}

/** The three things to try first in the live demo (`demoFirst` in lib/content.ts). */
export function DemoFirst({ title, className = '' }: { title?: string; className?: string }) {
  return (
    <div className={`${s.wrap} ${className}`}>
      {title && <p className={s.title}>{title}</p>}
      <ol className={s.list}>
        {demoFirst.map((t) => (
          <li key={t.text}>
            {t.keys.map((k) => (
              <kbd key={k} className="kbd">
                {k}
              </kbd>
            ))}{' '}
            <KeyText text={t.text} />
          </li>
        ))}
      </ol>
    </div>
  );
}
