// Operator names are public aliases on the shared board. This keeps them short, in one script
// (so "Аnna" with a Cyrillic А can't pose as "Anna") and free of the obvious slurs and swearing
// in English and Polish, including leetspeak and spaced-out spellings. It is a basic filter;
// published rows can still be removed by hand.

const allowed = /^[\p{L}\p{N}][\p{L}\p{N} ._'-]{0,18}[\p{L}\p{N}]$/u;

// Letters and digits that look like Latin letters, folded before the word check.
const lookalikes: Record<string, string> = {
  а: 'a', в: 'b', е: 'e', ё: 'e', к: 'k', м: 'm', н: 'h', о: 'o', р: 'p', с: 'c', т: 't', у: 'y', х: 'x',
  і: 'i', ї: 'i', ј: 'j', ѕ: 's', ԁ: 'd', ӏ: 'l', ɡ: 'g',
  α: 'a', β: 'b', ε: 'e', η: 'n', ι: 'i', κ: 'k', ν: 'v', ο: 'o', ρ: 'p', τ: 't', υ: 'u', χ: 'x',
  '0': 'o', '1': 'i', '3': 'e', '4': 'a', '5': 's', '7': 't', '8': 'b', '9': 'g', ł: 'l',
};

// Matched anywhere in the folded name (separators removed, and once more with repeated letters
// squeezed), so these are stems that rarely occur inside an innocent word or surname.
const stems = [
  'fuck', 'fck', 'shit', 'cunt', 'nigg', 'faggot', 'fagot', 'bitch', 'biatch', 'whore', 'slut', 'hitler',
  'kike', 'tranny', 'retard', 'pussy', 'penis', 'vagina', 'porn', 'dildo', 'twat', 'bastard', 'asshole', 'motherf',
  'kurw', 'kurew', 'chuj', 'pierdol', 'pierdl', 'jeban', 'jebac', 'jebie', 'jebn', 'zajeb', 'wyjeb', 'pizd',
  'skurw', 'spierd', 'dziwk', 'cwel', 'ciota', 'szmata', 'kutas', 'ciapat',
];
// Whole words only, as they occur inside names (Dickens, Hancock, Nazir, principal, swank).
const words = new Set([
  'ass', 'fag', 'fags', 'cum', 'spic', 'chink', 'dick', 'cock', 'nazi', 'nazis', 'heil', 'kkk', 'rape', 'rapist',
  'wank', 'wanker', 'huj', 'cipa', 'cipka', 'pedal', 'dupa', 'dupek', 'debil',
]);
// Names that would read as the site itself speaking.
const reserved = new Set([
  'admin', 'administrator', 'moderator', 'mod', 'official', 'staff', 'support', 'telex', 'telexmail',
  'telexteam', 'teamtelex', 'telexofficial', 'telexsupport',
]);

function scriptOf(character: string) {
  if (/\p{Script=Latin}/u.test(character)) return 'latin';
  if (/\p{Script=Cyrillic}/u.test(character)) return 'cyrillic';
  if (/\p{Script=Greek}/u.test(character)) return 'greek';
  if (/[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}]/u.test(character)) return 'cjk';
  if (/\p{Script=Hangul}/u.test(character)) return 'hangul';
  return /\p{L}/u.test(character) ? 'other' : null;
}

const squeeze = (text: string) => text.replace(/(.)\1+/g, '$1');

function offensive(name: string) {
  const folded = Array.from(name.normalize('NFKD').replace(/\p{M}/gu, '').toLowerCase()).map((c) => lookalikes[c] ?? c).join('');
  const plain = folded.replace(/[^a-z]/g, '');
  const tokens = folded.split(/[^a-z]+/).filter(Boolean);
  // Squeezed, "fuuuck" is "fuck"; a stem that loses a letter that way must stay long enough
  // not to match innocent words ("nigg" squeezed would match "night").
  const squeezed = squeeze(plain);
  return stems.some((stem) => plain.includes(stem) || ((squeeze(stem) === stem || squeeze(stem).length >= 5) && squeezed.includes(squeeze(stem))))
    || tokens.some((token) => words.has(token) || words.has(squeeze(token)))
    || words.has(plain) || reserved.has(plain);
}

// The name as it will be stored, or an error a player can act on.
export function checkOperator(raw: unknown): { name: string } | { error: string } {
  const name = typeof raw === 'string' ? raw.normalize('NFKC').trim().replace(/\s+/g, ' ') : '';
  const length = Array.from(name).length;
  if (length < 2 || length > 20) return { error: 'Use an operator name of 2–20 characters.' };
  if (!allowed.test(name)) return { error: 'Use an operator name of letters and numbers, with spaces or . _ \' - only between them.' };
  if (/[ ._'-]{2}/.test(name)) return { error: 'Use one space or punctuation mark at a time in the operator name.' };
  if ((name.match(/\p{L}/gu) ?? []).length < 2) return { error: 'Use at least two letters in the operator name.' };
  const scripts = new Set(Array.from(name).map(scriptOf).filter(Boolean));
  if (scripts.size > 1 || scripts.has('other')) return { error: 'Use one alphabet in the operator name: Latin, Greek, Cyrillic, Chinese, Japanese or Korean.' };
  if (offensive(name)) return { error: 'Please choose a different operator name.' };
  return { name };
}
