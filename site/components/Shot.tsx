import { getImageProps } from 'next/image';

export type ShotTheme = 'auto' | 'light' | 'dark';

// Screenshots are 1280×800 windows captured at 2× (scripts/screens.mjs).
const W = 2560;
const H = 1600;
const SIZES = '(max-width: 1224px) calc(100vw - 32px), 1160px';

/**
 * A screenshot in the light or dark variant. "auto" follows the visitor's system theme, and only
 * the matching file is downloaded (a <picture> source instead of two hidden images).
 */
export function Shot({ id, alt, theme = 'auto', eager = false }: { id: string; alt: string; theme?: ShotTheme; eager?: boolean }) {
  const common = { alt, width: W, height: H, quality: 90, sizes: SIZES };
  const dark = getImageProps({ ...common, src: `/screens/${id}-dark.png` }).props;
  const { srcSet, ...img } = getImageProps({
    ...common,
    src: `/screens/${id}-light.png`,
    loading: eager ? 'eager' : 'lazy',
    fetchPriority: eager ? 'high' : 'auto',
  }).props;
  // "all" always picks the dark source, "not all" never does.
  const media = theme === 'auto' ? '(prefers-color-scheme: dark)' : theme === 'dark' ? 'all' : 'not all';
  return (
    <picture>
      <source media={media} srcSet={dark.srcSet} sizes={SIZES} />
      <img {...img} srcSet={srcSet} alt={alt} />
    </picture>
  );
}
