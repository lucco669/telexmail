// Runs in <head> before the page paints, so a chosen theme never flashes the other one first.
// Kept out of lib/theme.ts: a value exported from a 'use client' module can't be read on the server.
export const THEME_KEY = 'telex-theme';
export const THEME_SCRIPT = `try{var t=localStorage.getItem('${THEME_KEY}');if(t==='light'||t==='dark')document.documentElement.dataset.theme=t}catch(e){}`;
