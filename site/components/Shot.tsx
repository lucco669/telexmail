'use client';

import { getImageProps } from 'next/image';
import { useResolvedTheme, useThemeChoice } from '@/lib/theme';
import posters from '@/lib/hero-posters.json';

/** "site" follows the page's theme; "light" and "dark" pin the screenshot. */
export type ShotTheme = 'site' | 'light' | 'dark';

// Screenshots are 1280×800 windows captured at 2× (scripts/screens.mjs).
const W = 2560;
const H = 1600;
const DEFAULT_SIZES = '(max-width: 1224px) calc(100vw - 32px), 1160px';

/**
 * A screenshot in its light or dark variant, as a <picture> so only the visible file downloads.
 * Before hydration the system theme decides (a media query); a theme picked on the page takes
 * over right after.
 */
export function Shot({
  id,
  alt,
  theme = 'site',
  eager = false,
  poster = false,
  sizes = DEFAULT_SIZES,
}: {
  id: string;
  alt: string;
  theme?: ShotTheme;
  eager?: boolean;
  poster?: boolean;
  sizes?: string;
}) {
  const choice = useThemeChoice();
  const site = useResolvedTheme();
  const want = theme === 'site' ? (choice === 'system' ? null : site) : theme;
  // "all" always picks the dark source, "not all" never does.
  const media = want === 'dark' ? 'all' : want === 'light' ? 'not all' : '(prefers-color-scheme: dark)';
  const lightMedia = media === 'all' ? 'not all' : media === 'not all' ? 'all' : '(prefers-color-scheme: light)';
  const usePoster = poster && id === posters.id;

  const common = { alt, width: W, height: H, sizes };
  const responsive = (variant: 'light' | 'dark', format: 'src' | 'avifSrc' = 'src') => ({
    ...common,
    src: posters[variant][2][format],
    srcSet: posters[variant].map((item) => `${item[format]} ${item.width}w`).join(', '),
  });
  const dark = usePoster ? responsive('dark') : getImageProps({ ...common, quality: 90, src: `/screens/${id}-dark.webp` }).props;
  const { srcSet, ...img } = usePoster ? responsive('light') : getImageProps({ ...common, quality: 90, src: `/screens/${id}-light.webp` }).props;
  const darkAvif = usePoster ? responsive('dark', 'avifSrc') : null;
  const lightAvif = usePoster ? responsive('light', 'avifSrc') : null;
  return (
    <>
      {usePoster && eager && <>
        <link rel="preload" as="image" type="image/avif" imageSrcSet={darkAvif!.srcSet} imageSizes={sizes} media={media} fetchPriority="high" />
        <link rel="preload" as="image" type="image/avif" imageSrcSet={lightAvif!.srcSet} imageSizes={sizes} media={lightMedia} fetchPriority="high" />
      </>}
      <picture>
        {darkAvif && <source type="image/avif" media={media} srcSet={darkAvif.srcSet} sizes={sizes} />}
        {lightAvif && <source type="image/avif" media={lightMedia} srcSet={lightAvif.srcSet} sizes={sizes} />}
        <source media={media} srcSet={dark.srcSet} sizes={sizes} />
        <img {...img} srcSet={srcSet} alt={alt} loading={eager ? 'eager' : 'lazy'} fetchPriority={eager ? 'high' : 'auto'} />
      </picture>
    </>
  );
}
