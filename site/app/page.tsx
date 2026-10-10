import { Download } from '@/components/Download';
import { Features } from '@/components/Features';
import { Footer } from '@/components/Footer';
import { Header } from '@/components/Header';
import { Hero } from '@/components/Hero';
import { Keyboard } from '@/components/Keyboard';
import { Roadmap } from '@/components/Roadmap';
import { Screens } from '@/components/Screens';
import { getLatestRelease } from '@/lib/release';

export default async function Home() {
  const release = await getLatestRelease();
  return (
    <>
      <Header />
      <main>
        <Hero release={release} />
        <Features />
        <Keyboard />
        <Screens />
        <Roadmap />
        <Download release={release} />
      </main>
      <Footer />
    </>
  );
}
