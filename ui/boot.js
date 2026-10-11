// Runs in <head>, before the first paint: puts the saved theme, language, text size, font and account mark on the
// page, so Telex never flashes the default colors or English first. The settings themselves are
// in settings.json, kept by the Rust core; app.js keeps this copy ("telex-look") in step with it.
(() => {
  'use strict';
  let look = {};
  try { look = JSON.parse(localStorage.getItem('telex-look') || '{}') || {}; } catch {}
  const root = document.documentElement;
  if (look.theme && look.theme !== 'system') root.dataset.theme = look.theme;
  else if (!root.dataset.theme) root.dataset.theme = matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  // Before the first start's settings arrive: the browser's languages, which follow Windows.
  const guess = (navigator.languages || [navigator.language]).map(l => String(l).slice(0, 2).toLowerCase());
  root.lang = look.lang || guess.find(l => l === 'en' || l === 'pl') || 'en';
  root.dataset.mark = look.mark || 'rail';
  root.dataset.font = look.font || 'plex';
  if (look.zoom && look.zoom !== 100){
    try { window.__TAURI__?.webview?.getCurrentWebview().setZoom(look.zoom / 100); } catch {}
  }
})();
