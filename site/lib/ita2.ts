// ITA2 (Baudot–Murray), the 5-bit code telex machines punched into paper tape. Channel 1 first.
// Letters and figures share codes; the FIGS and LTRS columns switch between the two sets.

const LETTERS: Record<string, string> = {
  A: '11000', B: '10011', C: '01110', D: '10010', E: '10000', F: '10110', G: '01011', H: '00101', I: '01100',
  J: '11010', K: '11110', L: '01001', M: '00111', N: '00110', O: '00011', P: '01101', Q: '11101', R: '01010',
  S: '10100', T: '00001', U: '11100', V: '01111', W: '11001', X: '10111', Y: '10101', Z: '10001',
};
const FIGURES: Record<string, string> = {
  '1': '11101', '2': '11001', '3': '10000', '4': '01010', '5': '00001', '6': '10101', '7': '11100', '8': '01100',
  '9': '00011', '0': '01101', '-': '11000', '?': '10011', ':': '01110', '.': '00111', ',': '00110', '(': '11110',
  ')': '01001', "'": '11010', '/': '10111', '+': '10001', '=': '01111',
};
const SPACE = '00100';
const FIGS = '11011';
const LTRS = '11111';

/** `shift` marks the FIGS/LTRS columns, which print nothing on a real teleprinter. */
export type Column = { code: string; char: string; shift?: 'figs' | 'ltrs' };

/** Text to tape columns, with shift columns where it changes between letters and figures. */
export function encode(text: string): Column[] {
  const out: Column[] = [];
  let figures = false;
  for (const raw of text.toUpperCase()) {
    if (LETTERS[raw]) {
      if (figures) out.push({ code: LTRS, char: '', shift: 'ltrs' });
      figures = false;
      out.push({ code: LETTERS[raw], char: raw });
    } else if (FIGURES[raw]) {
      if (!figures) out.push({ code: FIGS, char: '', shift: 'figs' });
      figures = true;
      out.push({ code: FIGURES[raw], char: raw });
    } else {
      out.push({ code: SPACE, char: ' ' });
    }
  }
  return out;
}
