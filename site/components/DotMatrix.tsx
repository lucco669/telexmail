import s from './DotMatrix.module.css';

// 5×7 dot-matrix letters, as on a teleprinter's printout. Same idea as the logo's T.
const FONT: Record<string, string[]> = {
  T: ['11111', '00100', '00100', '00100', '00100', '00100', '00100'],
  E: ['11111', '10000', '10000', '11110', '10000', '10000', '11111'],
  L: ['10000', '10000', '10000', '10000', '10000', '10000', '11111'],
  X: ['10001', '10001', '01010', '00100', '01010', '10001', '10001'],
};
const PITCH = 10;

/**
 * A word in big dot-matrix letters over a faint grid. A path per letter and a repeating pattern
 * keep the same dots with very few DOM nodes; hovering a letter gives it a phosphor glow.
 */
export function DotMatrix({ word }: { word: string }) {
  const letters = word.toUpperCase().split('').filter((c) => FONT[c]);
  const cols = letters.length * 6 - 1; // one blank column between letters
  const glyphs = letters.map((letter, i) => FONT[letter].flatMap((row, r) => [...row].flatMap((on, c) => {
    const x = (i * 6 + c) * PITCH + PITCH / 2;
    const y = r * PITCH + PITCH / 2;
    return on === '1' ? [`M${x - 3.6},${y}a3.6,3.6 0 1,0 7.2,0a3.6,3.6 0 1,0 -7.2,0`] : [];
  })).join(''));
  return (
    <svg className={s.matrix} viewBox={`0 0 ${cols * PITCH} ${7 * PITCH}`} role="img" aria-label={word}>
      <defs><pattern id="telex-word-grid" width={PITCH} height={PITCH} patternUnits="userSpaceOnUse"><circle cx={PITCH / 2} cy={PITCH / 2} r={PITCH * 0.36} className={s.off} /></pattern></defs>
      <rect width={cols * PITCH} height={7 * PITCH} fill="url(#telex-word-grid)" />
      {glyphs.map((d, i) => <path key={i} d={d} className={s.on} />)}
    </svg>
  );
}
