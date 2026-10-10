'use client';

import { useSyncExternalStore } from 'react';
import { THEME_KEY } from './theme-script';

export type ThemeChoice = 'system' | 'light' | 'dark';
export type Theme = 'light' | 'dark';

const DARK = '(prefers-color-scheme: dark)';
const CHANGED = 'telex-theme';

function subscribe(onChange: () => void) {
  const mq = window.matchMedia(DARK);
  // Another tab changed it: apply the same choice here.
  const onStorage = (e: StorageEvent) => {
    if (e.key !== THEME_KEY) return;
    apply(e.newValue === 'light' || e.newValue === 'dark' ? e.newValue : 'system');
    onChange();
  };
  mq.addEventListener('change', onChange);
  window.addEventListener(CHANGED, onChange);
  window.addEventListener('storage', onStorage);
  return () => {
    mq.removeEventListener('change', onChange);
    window.removeEventListener(CHANGED, onChange);
    window.removeEventListener('storage', onStorage);
  };
}

const choice = (): ThemeChoice => (document.documentElement.dataset.theme as Theme | undefined) ?? 'system';
const resolved = (): Theme => {
  const c = choice();
  return c === 'system' ? (window.matchMedia(DARK).matches ? 'dark' : 'light') : c;
};

function apply(c: ThemeChoice) {
  const root = document.documentElement;
  if (c === 'system') delete root.dataset.theme;
  else root.dataset.theme = c;
}

/** What the visitor picked. "system" on the server and before hydration. */
export const useThemeChoice = () => useSyncExternalStore(subscribe, choice, () => 'system' as const);

/** The theme on screen, or null on the server (where only CSS knows). */
export const useResolvedTheme = () => useSyncExternalStore(subscribe, resolved, () => null);

export function setTheme(c: ThemeChoice) {
  apply(c);
  try {
    if (c === 'system') localStorage.removeItem(THEME_KEY);
    else localStorage.setItem(THEME_KEY, c);
  } catch {
    // Private mode or blocked storage: the choice lasts for this page only.
  }
  window.dispatchEvent(new Event(CHANGED));
}
