import { DemoBand } from '@/components/DemoBand';
import { Download } from '@/components/Download';
import { Features } from '@/components/Features';
import { Hero } from '@/components/Hero';
import { JsonLd } from '@/components/JsonLd';
import { Keyboard } from '@/components/Keyboard';
import { Roadmap } from '@/components/Roadmap';
import { Screens } from '@/components/Screens';
import { WhatsNew } from '@/components/WhatsNew';
import { pages } from '@/lib/pages';
import { getReleaseInfo } from '@/lib/release';
import { appLd, organizationLd, pageMetadata, websiteLd } from '@/lib/seo';

export const metadata = pageMetadata(pages.home);

export default async function Home() {
  const { list, live } = await getReleaseInfo();
  const release = list[0] ?? null;
  // GitHub couldn't be asked and no release is known: link to GitHub rather than say "coming soon".
  const unknown = !live && !release;
  return (
    <>
      <JsonLd items={[organizationLd(), websiteLd(), appLd(release)]} />
      <Hero release={release} unknown={unknown} />
      <Features />
      <Keyboard release={release} />
      <DemoBand />
      <Screens release={release} />
      <WhatsNew release={release} />
      <Roadmap release={release} />
      <Download release={release} unknown={unknown} />
    </>
  );
}
