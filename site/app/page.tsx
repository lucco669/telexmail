import { Download } from '@/components/Download';
import { Features } from '@/components/Features';
import { Hero } from '@/components/Hero';
import { JsonLd } from '@/components/JsonLd';
import { Keyboard } from '@/components/Keyboard';
import { Roadmap } from '@/components/Roadmap';
import { Screens } from '@/components/Screens';
import { pages } from '@/lib/pages';
import { getLatestRelease } from '@/lib/release';
import { appLd, pageMetadata, websiteLd } from '@/lib/seo';

export const metadata = pageMetadata(pages.home);

export default async function Home() {
  const release = await getLatestRelease();
  return (
    <>
      <JsonLd items={[websiteLd(), appLd(release)]} />
      <Hero release={release} />
      <Features />
      <Keyboard />
      <Screens />
      <Roadmap />
      <Download release={release} />
    </>
  );
}
