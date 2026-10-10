'use client';

import { setTheme, useThemeChoice, type ThemeChoice } from '@/lib/theme';
import s from './ThemeToggle.module.css';

const ICONS: Record<ThemeChoice, React.ReactNode> = {
  system: (
    <>
      <rect x="2" y="3" width="12" height="8.5" rx="1.2" />
      <path d="M6 14h4M8 11.5V14" />
    </>
  ),
  light: (
    <>
      <circle cx="8" cy="8" r="3" />
      <path d="M8 1.5v1.6M8 12.9v1.6M1.5 8h1.6M12.9 8h1.6M3.4 3.4l1.1 1.1M11.5 11.5l1.1 1.1M3.4 12.6l1.1-1.1M11.5 4.5l1.1-1.1" />
    </>
  ),
  dark: <path d="M13.2 9.6A5.6 5.6 0 0 1 6.4 2.8a5.6 5.6 0 1 0 6.8 6.8z" />,
};
const LABELS: Record<ThemeChoice, string> = { system: 'Match system', light: 'Light', dark: 'Dark' };

/** System / light / dark. The choice is remembered in this browser only. */
export function ThemeToggle({ className = '' }: { className?: string }) {
  const current = useThemeChoice();
  return (
    <div className={`${s.toggle} ${className}`} role="group" aria-label="Color theme">
      {(Object.keys(ICONS) as ThemeChoice[]).map((c) => (
        <button
          key={c}
          type="button"
          className={s.btn}
          aria-pressed={current === c}
          aria-label={LABELS[c]}
          title={LABELS[c]}
          onClick={() => setTheme(c)}
        >
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            {ICONS[c]}
          </svg>
        </button>
      ))}
    </div>
  );
}
