import { site } from '@/lib/site';
import s from './Logo.module.css';

// The mark at small sizes, same as brand/telex-mark-small.svg: a dot-matrix T, like a printed telex.
// The full mark (with the faint dot grid) is brand/telex-mark.svg; icons come from `npm run icons`.
const GRID = [14, 23, 32, 41, 50];
const T_DOTS = [...GRID.map((x) => [x, 14]), ...GRID.slice(1).map((y) => [32, y])];

export function LogoMark({ size = 30 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" aria-hidden="true" className={s.mark}>
      <rect width="64" height="64" rx="14" fill="var(--brand)" />
      <g fill="#fff">
        {T_DOTS.map(([cx, cy]) => (
          <circle key={`${cx}-${cy}`} cx={cx} cy={cy} r="4.4" />
        ))}
      </g>
    </svg>
  );
}

export function Logo() {
  return (
    <span className={s.logo}>
      <LogoMark />
      <span className={s.word}>{site.name}</span>
    </span>
  );
}
