import { CtaBand } from '@/components/CtaBand';
import { PageHeader } from '@/components/PageHeader';
import { RoadmapList } from '@/components/Roadmap';
import { roadmap } from '@/lib/content';
import { pages } from '@/lib/pages';
import { pageMetadata } from '@/lib/seo';
import s from './page.module.css';

export const metadata = pageMetadata(pages.roadmap);

export default function RoadmapPage() {
  const done = roadmap.filter((x) => x.status === 'done').length;
  return (
    <>
      <PageHeader
        page={pages.roadmap}
        title="Early, and honest about it."
        lead="Telex is built in order: first mail that works, then mail that looks right, then the power tools. Here is every step, what is done, and what each finished step still leaves out."
      />
      <div className={`container ${s.layout}`}>
        <aside className={s.summary}>
          <p className={s.big}>
            {done}
            <span>/{roadmap.length}</span>
          </p>
          <p className={s.label}>steps done</p>
          <div className={s.meter} aria-hidden="true">
            <span style={{ width: `${(done / roadmap.length) * 100}%` }} />
          </div>
          <p className={s.note}>
            The order can change when something turns out to matter more. This page follows the app as each step
            lands.
          </p>
        </aside>
        <RoadmapList details />
      </div>
      <CtaBand title="Want to see where it is today?" secondary={pages.screenshots} />
    </>
  );
}
