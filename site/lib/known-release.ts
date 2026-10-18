import type { Release } from './release';
import { site } from './site';

// The newest release when this file was last updated. The site offers it only when GitHub can't be
// asked while the site is built (or in `npm run dev` offline); a live site keeps its last good answer
// from GitHub instead (lib/release.ts). Update it after each release (README, "After a release").

const version = '0.9.22';
const file = (name: string, size: number) => ({
  name,
  size,
  url: `https://github.com/${site.releasesRepo}/releases/download/v${version}/${name}`,
});

export const knownRelease: Release | null = {
  version,
  date: '2026-10-18T06:37:39Z',
  pageUrl: `https://github.com/${site.releasesRepo}/releases/tag/v${version}`,
  exe: file(`Telex_${version}_x64-setup.exe`, 7607449),
  msi: file(`Telex_${version}_x64_en-US.msi`, 10551296),
  appimage: file(`Telex_${version}_amd64.AppImage`, 89180664),
  deb: file(`Telex_${version}_amd64.deb`, 11289436),
  sums: file('SHA256SUMS', 832),
};
