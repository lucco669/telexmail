import { CtaBand } from '@/components/CtaBand';
import { Gallery } from '@/components/Gallery';
import { PageHeader } from '@/components/PageHeader';
import { hero, screens } from '@/lib/content';
import { pages } from '@/lib/pages';
import { pageMetadata } from '@/lib/seo';

export const metadata = pageMetadata(pages.screenshots);

export default function ScreenshotsPage() {
  return (
    <>
      <PageHeader
        page={pages.screenshots}
        title="Screenshots."
        lead="The current build, in light and dark, with a demo mailbox. Open any of them full size."
      />
      <div className="container">
        <Gallery items={[hero, ...screens]} />
      </div>
      <CtaBand secondary={pages.features} />
    </>
  );
}
