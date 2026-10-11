import Link from 'next/link';
import { pages } from '@/lib/pages';
import s from './CtaBand.module.css';

/** The closing band on subpages: one line, the two next steps, and what the download costs. */
export function CtaBand({
  title = 'Try Telex on your own mail.',
  secondary = pages.features,
}: {
  title?: string;
  secondary?: { path: string; label: string };
}) {
  return (
    <section className={`${s.band} no-print`}>
      <div className={`container ${s.inner}`}>
        <h2 className={s.title}>{title}</h2>
        <div className={s.side}>
          <div className={s.actions}>
            <Link href={pages.download.path} className="btn btn-primary btn-lg">
              Download Telex
            </Link>
            <Link href={secondary.path} className="btn btn-lg">
              {secondary.label}
            </Link>
          </div>
          <p className={s.note}>Free · Windows 10 and 11, and Linux · a preview (0.x) that updates itself</p>
        </div>
      </div>
    </section>
  );
}
