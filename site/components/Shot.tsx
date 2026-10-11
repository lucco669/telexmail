'use client';

import { getImageProps } from 'next/image';
import { useResolvedTheme, useThemeChoice } from '@/lib/theme';

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
  sizes = DEFAULT_SIZES,
}: {
  id: string;
  alt: string;
  theme?: ShotTheme;
  eager?: boolean;
  sizes?: string;
}) {
  const choice = useThemeChoice();
  const site = useResolvedTheme();
  const want = theme === 'site' ? (choice === 'system' ? null : site) : theme;
  // "all" always picks the dark source, "not all" never does.
  const media = want === 'dark' ? 'all' : want === 'light' ? 'not all' : '(prefers-color-scheme: dark)';

  const common = { alt, width: W, height: H, quality: 90, sizes };
  const dark = getImageProps({ ...common, src: `/screens/${id}-dark.webp` }).props;
  const { srcSet, ...img } = getImageProps({
    ...common,
    src: `/screens/${id}-light.webp`,
    loading: eager ? 'eager' : 'lazy',
    fetchPriority: eager ? 'high' : 'auto',
  }).props;
  return (
    <picture>
      <source media={media} srcSet={dark.srcSet} sizes={sizes} />
      <img {...img} srcSet={srcSet} alt={alt} />
    </picture>
  );
}
