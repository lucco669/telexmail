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
import { getLatestRelease } from '@/lib/release';
import { appLd, organizationLd, pageMetadata, websiteLd } from '@/lib/seo';

export const metadata = pageMetadata(pages.home);

export default async function Home() {
  const release = await getLatestRelease();
  return (
    <>
      <JsonLd items={[organizationLd(), websiteLd(), appLd(release)]} />
      <Hero release={release} />
      <Features />
      <Keyboard release={release} />
      <DemoBand />
      <Screens release={release} />
      <WhatsNew release={release} />
      <Roadmap release={release} />
      <Download release={release} />
    </>
  );
}
