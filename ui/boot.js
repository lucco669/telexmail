// Runs in <head>, before the first paint: puts the saved theme, accent, language, text size, font, toolbar,
// account mark, unread style, spacing, message pane and reading choices on the page, so Telex never flashes the default colors or English first.
// The settings themselves are in settings.json, kept by the Rust core; app.js keeps this copy
// ("telex-look") in step with it.
(() => {
  'use strict';
  let look = {};
  try { look = JSON.parse(localStorage.getItem('telex-look') || '{}') || {}; } catch {}
  const root = document.documentElement;
  if (look.theme && look.theme !== 'system') root.dataset.theme = look.theme;
  else if (!root.dataset.theme) root.dataset.theme = matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  // Before the first start's settings arrive: the browser's languages, which follow Windows.
  const spoken = ['en', 'pl', 'de', 'cs', 'sk', 'uk', 'es', 'fr', 'it'];
  const guess = (navigator.languages || [navigator.language]).map(l => String(l).slice(0, 2).toLowerCase());
  root.lang = spoken.includes(look.lang) ? look.lang : guess.find(l => spoken.includes(l)) || 'en';
  // A language other than English and Polish has its words in a file of its own: load it now,
  // before i18n.js and the first paint (a script written here runs before the next one).
  if (root.lang !== 'en' && root.lang !== 'pl') document.write(`<script src="i18n/${root.lang}.js"><\/script>`);
  root.dataset.mark = look.mark || 'rail';
  root.dataset.unread = look.unread || 'quiet';
  root.dataset.font = look.font || 'plex';
  root.dataset.toolbar = ['classic', 'icons', 'large', 'below', 'text'].includes(look.toolbar) ? look.toolbar : 'classic';
  const one = (value, list) => list.includes(value) ? value : list[0];
  root.dataset.density = one(look.density, ['compact', 'normal', 'comfortable', 'spacious']);
  root.dataset.pane = one(look.pane, ['below', 'right', 'off']);
  root.dataset.lines = one(look.lines, ['normal', 'tight', 'relaxed', 'loose']);
  root.dataset.width = one(look.width, ['normal', 'narrow', 'wide', 'full']);
  // The accent as app.js last worked it out for this theme (accent.js needs the theme's colors,
  // which aren't loaded yet): only plain colors, only when the theme is the same.
  const accent = look.accent;
  if (accent && accent.theme === root.dataset.theme && accent.vars && typeof accent.vars === 'object'){
    for (const [name, value] of Object.entries(accent.vars)){
      if (/^[a-z-]+$/.test(name) && /^#[0-9a-f]{6}$/i.test(String(value))) root.style.setProperty('--' + name, value);
    }
  }
  if (look.zoom && look.zoom !== 100){
    try { window.__TAURI__?.webview?.getCurrentWebview().setZoom(look.zoom / 100); } catch {}
  }
})();
