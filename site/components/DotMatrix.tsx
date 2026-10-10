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
 * A word in big dot-matrix letters over a faint grid of unlit dots. Pointing at dots lights them
 * in the accent color and they fade back slowly, like phosphor (CSS only).
 */
export function DotMatrix({ word }: { word: string }) {
  const letters = word.toUpperCase().split('').filter((c) => FONT[c]);
  const cols = letters.length * 6 - 1; // one blank column between letters
  const dots: { x: number; y: number; on: boolean }[] = [];
  for (let c = 0; c < cols; c++) {
    const letter = FONT[letters[Math.floor(c / 6)]];
    const lc = c % 6;
    for (let r = 0; r < 7; r++) dots.push({ x: c, y: r, on: lc < 5 && letter[r][lc] === '1' });
  }
  return (
    <svg className={s.matrix} viewBox={`0 0 ${cols * PITCH} ${7 * PITCH}`} role="img" aria-label={word}>
      {dots.map((d) => (
        <circle
          key={`${d.x}-${d.y}`}
          cx={d.x * PITCH + PITCH / 2}
          cy={d.y * PITCH + PITCH / 2}
          r={PITCH * 0.36}
          className={d.on ? s.on : s.off}
        />
      ))}
    </svg>
  );
}
