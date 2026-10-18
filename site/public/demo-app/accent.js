// Accent colors (Settings › Appearance › Accent color): the theme's own, a preset or a color the
// user picks, made to fit the theme. The pick gives the hue and how colorful; the lightness is
// found here, in OKLab, so that every pair the accent is part of still meets the theme's WCAG level
// (the same pairs scripts/check-contrast.mjs measures, which runs this file for every theme with
// every preset and a wheel of picked colors). The selection, the chips and the image stand-ins take
// the accent's hue at the theme's own lightness. Nothing fits: that token stays the theme's own.
// Loaded by index.html before app.js (window.TelexAccent); check-contrast.mjs runs it in a sandbox.
(function (root) {
  'use strict';
  // [id, color]: the hue and colorfulness; the words are app.js's (COLOR_NAMES).
  const PRESETS = [['blue', '#2f6fd6'], ['teal', '#0f8f88'], ['green', '#2f8a3c'], ['orange', '#c8640f'],
    ['red', '#cc3333'], ['pink', '#c43b84'], ['purple', '#7a4fd0'], ['gray', '#5f6b78']];
  const hex = s => { const m = /^#([0-9a-f]{6})$/i.exec(String(s).trim()); return m ? [0, 2, 4].map(i => parseInt(m[1].slice(i, i + 2), 16)) : null; };
  const toHex = rgb => '#' + rgb.map(v => v.toString(16).padStart(2, '0')).join('');
  const lin = v => ((v /= 255) <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
  const lum = rgb => { const [r, g, b] = rgb.map(lin); return 0.2126 * r + 0.7152 * g + 0.0722 * b; };
  const ratio = (a, b) => { const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p); return (x + 0.05) / (y + 0.05); };
  // sRGB ⇄ OKLab (Björn Ottosson's matrices), as check-contrast.mjs and colors.rs.
  const toLab = rgb => {
    const [r, g, b] = rgb.map(lin);
    const [l, m, s] = [0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b, 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b,
      0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b].map(Math.cbrt);
    return [0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s, 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
      0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s];
  };
  const linRgb = ([L, A, B]) => {
    const [l, m, s] = [L + 0.3963377774 * A + 0.2158037573 * B, L - 0.1055613458 * A - 0.0638541728 * B, L - 0.0894841775 * A - 1.291485548 * B].map(v => v ** 3);
    return [4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s, -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
      -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s];
  };
  const gam = v => Math.round(255 * Math.min(1, Math.max(0, v <= 0.0031308 ? 12.92 * v : 1.055 * v ** (1 / 2.4) - 0.055)));
  // A lightness, chroma and hue as sRGB: less chroma until it is a color the screen can show.
  function lch(L, C, h){
    for (let c = C; ; c *= 0.94){
      const rgb = linRgb([L, c * Math.cos(h), c * Math.sin(h)]);
      if (rgb.every(v => v >= -1e-4 && v <= 1 + 1e-4) || c < 1e-3) return rgb.map(gam);
    }
  }
  const polar = rgb => { const [L, a, b] = toLab(rgb); return {L, C: Math.hypot(a, b), h: Math.atan2(b, a)}; };
  // The pairs the accent and the tokens that take its hue are in, with the theme's tokens.
  // [token, other token or [a, share, b] mixed in OKLab, need: 'text' or 'ui'] as in check-contrast.mjs.
  const ROWS = [0, 1, 2, 3, 4, 5].map(i => [`acct-${i}`, 0.09, 'surface']);
  const PAIRS = {
    sel: [['fg', 'text'], ['muted', 'text'], ['flag', 'ui'], ['fg', 'ui']],
    accent: [['surface', 'text'], ['panel', 'text'], ['chrome', 'text'], ['sel', 'text'], ['hover', 'text'], ['accent-fg', 'text'],
      ['line-soft', 'ui'], ...ROWS.map(r => [r, 'text'])],
    'chip-bg': [['chip-fg', 'text']],
    'chip-fg': [['chip-bg', 'text'], ['img-a', 'text'], ['img-b', 'text']],
    'img-a': [['chip-fg', 'text']],
    'img-b': [['chip-fg', 'text']],
  };
  const mixLab = (a, p, b) => { const x = toLab(a), y = toLab(b); return linRgb(x.map((v, i) => v * p + y[i] * (1 - p))).map(gam); };
  const color = (tok, ref) => Array.isArray(ref) ? mixLab(tok[ref[0]], ref[1], tok[ref[2]]) : tok[ref];
  const fits = (tok, name, need) => PAIRS[name].every(([ref, kind]) => ratio(tok[name], color(tok, ref)) >= (kind === 'ui' ? 3 : need));

  // tokens: the theme's own, {name: '#rrggbb'} (at least those PAIRS names, the account colors and
  // wcag); pick: '#rrggbb'. Returns the tokens to set, as '#rrggbb', or {} when nothing changes.
  function derive(tokens, pick){
    const p = hex(pick);
    if (!p) return {};
    const tok = {}, out = {};
    for (const [k, v] of Object.entries(tokens)){ const c = hex(v); if (c) tok[k] = c; }
    const need = String(tokens.wcag).trim() === 'AAA' ? 7 : 4.5;
    const {C: pc, h} = polar(p);
    const dark = lum(tok.surface) < 0.2;
    // Hue first for the quiet tokens: the theme's lightness, colorful only as far as the theme's own.
    const tint = (name, steps) => {
      if (!tok[name]) return;
      const own = tok[name], {L, C} = polar(own);
      // A token that is almost grey in the theme stays almost grey; so does a grey pick.
      const c = Math.min(C, pc);
      for (let i = 0; i <= steps; i++){
        // Away from the text: lighter in a light theme, darker in a dark one.
        const l = Math.min(1, Math.max(0, L + (dark ? -1 : 1) * i * 0.004));
        tok[name] = lch(l, c, h);
        if (fits(tok, name, need)) return;
      }
      tok[name] = own; // nothing fits: the theme's own
    };
    // The accent: the pick's hue and chroma, from the theme accent's lightness away from what it is on.
    const solve = (name, from, up) => {
      const own = tok[name], {L} = polar(from);
      for (let i = 0; i <= 250; i++){
        const l = L + (up ? 1 : -1) * i * 0.004;
        if (l < 0 || l > 1) break;
        tok[name] = lch(l, pc, h);
        if (fits(tok, name, need)) return true;
      }
      tok[name] = own;
      return false;
    };
    tint('sel', 60); tint('img-a', 0); tint('img-b', 0); tint('chip-bg', 40);
    // chip-fg keeps the theme's lightness and goes away from chip-bg until it reads.
    if (tok['chip-fg']){
      const own = tok['chip-fg'];
      if (!solve('chip-fg', own, dark) || !fits(tok, 'chip-bg', need) || !fits(tok, 'img-a', need) || !fits(tok, 'img-b', need)){
        tok['chip-fg'] = own; tok['chip-bg'] = hex(tokens['chip-bg']); tok['img-a'] = hex(tokens['img-a']); tok['img-b'] = hex(tokens['img-b']);
      }
    }
    // The accent, then the selection checked again with it (it is drawn on the selection too).
    if (!solve('accent', hex(tokens.accent), dark)) tok.accent = hex(tokens.accent);
    if (!fits(tok, 'sel', need) || !fits(tok, 'accent', need)){ tok.sel = hex(tokens.sel); tok.accent = hex(tokens.accent); }
    for (const name of ['accent', 'sel', 'chip-bg', 'chip-fg', 'img-a', 'img-b']){
      if (tok[name] && toHex(tok[name]) !== String(tokens[name]).toLowerCase()) out[name] = toHex(tok[name]);
    }
    return out;
  }
  // The color a setting stands for: a preset's, a picked one, or null for the theme's own.
  const pickOf = setting => PRESETS.find(([id]) => id === setting)?.[1] ?? (hex(setting) ? String(setting).toLowerCase() : null);
  root.TelexAccent = {PRESETS, derive, pickOf, ratio, hex};
})(typeof window !== 'undefined' ? window : globalThis);
