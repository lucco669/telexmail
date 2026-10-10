import s from './Tape.module.css';

// ITA2 (Baudot–Murray), the 5-bit code telex machines punched into paper tape. Channel 1 first.
const ITA2: Record<string, string> = {
  A: '11000', B: '10011', C: '01110', D: '10010', E: '10000', F: '10110', G: '01011', H: '00101', I: '01100',
  J: '11010', K: '11110', L: '01001', M: '00111', N: '00110', O: '00011', P: '01101', Q: '11101', R: '01010',
  S: '10100', T: '00001', U: '11100', V: '01111', W: '11001', X: '10111', Y: '10101', Z: '10001', ' ': '00100',
};

const COL = 11; // column pitch
const ROW = 9; // row pitch
const PAD = 12; // paper above and below the holes
// Five data channels with the smaller feed hole between channels 2 and 3.
const ROWS = [0, 1, 3, 4, 5];

/** A strip of punched tape that spells `text` in ITA2. Decorative. */
export function Tape({ text, repeat = 6 }: { text: string; repeat?: number }) {
  const chars = text.toUpperCase().repeat(repeat).split('');
  const width = chars.length * COL;
  const height = PAD * 2 + ROW * 5;
  const y = (r: number) => PAD + r * ROW;
  return (
    <div className={s.tape} aria-hidden="true">
      <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`}>
        {chars.map((ch, i) => {
          const x = i * COL + COL / 2;
          const code = ITA2[ch] ?? ITA2[' '];
          return (
            <g key={i}>
              <circle cx={x} cy={y(2)} r={1.6} className={s.feed} />
              {ROWS.map((r, bit) =>
                code[bit] === '1' ? <circle key={r} cx={x} cy={y(r)} r={3.4} className={s.hole} /> : null,
              )}
            </g>
          );
        })}
      </svg>
    </div>
  );
}
