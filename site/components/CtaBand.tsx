import Link from 'next/link';
import { pages } from '@/lib/pages';
import s from './CtaBand.module.css';

/** The closing band on subpages: one line and the two next steps. */
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
        <div className={s.actions}>
          <Link href={pages.download.path} className="btn btn-primary btn-lg">
            Download for Windows
          </Link>
          <Link href={secondary.path} className="btn btn-lg">
            {secondary.label}
          </Link>
        </div>
      </div>
    </section>
  );
}
