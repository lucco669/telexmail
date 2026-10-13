import { PageLink as Link } from '@/components/PageNavigation';
import { CtaBand } from '@/components/CtaBand';
import { PageHeader } from '@/components/PageHeader';
import { RoadmapList, stepStatus } from '@/components/Roadmap';
import { roadmap } from '@/lib/content';
import { pages } from '@/lib/pages';
import { getLatestRelease } from '@/lib/release';
import { pageMetadata } from '@/lib/seo';
import s from './page.module.css';

export const metadata = pageMetadata(pages.roadmap);

export default async function RoadmapPage() {
  const release = await getLatestRelease();
  const done = roadmap.filter((x) => stepStatus(x, release) === 'done').length;
  return (
    <>
      <PageHeader
        page={pages.roadmap}
        title="Early, and honest about it."
        lead="Saved searches, header and attachment rules, previews, optional threads, toolbar layouts and local mail inspection have shipped. Here is every completed step, its remaining limits, and what comes next."
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
          {release && <p className={s.label}>Newest release: {release.version}</p>}
          <p className={s.note}>
            The order can change when something turns out to matter more. This page follows the app as each step
            lands; the <Link href={pages.changelog.path}>changelog</Link> says what each version brought.
          </p>
        </aside>
        <RoadmapList release={release} details />
      </div>
      <CtaBand title="Want to see where it is today?" secondary={pages.demo} />
    </>
  );
}
