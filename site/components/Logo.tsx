import { site } from '@/lib/site';
import s from './Logo.module.css';

// The app icon is still a placeholder (src-tauri/icons). Replace this mark and app/icon.svg together.
export function LogoMark({ size = 24 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" aria-hidden="true" className={s.mark}>
      <rect width="32" height="32" rx="7" fill="var(--brand)" />
      <rect x="7" y="9.5" width="18" height="13" rx="1.8" fill="none" stroke="#fff" strokeWidth="2.2" />
      <path d="M8.2 10.8 16 17l7.8-6.2" fill="none" stroke="#fff" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

export function Logo() {
  return (
    <span className={s.logo}>
      <LogoMark />
      <span className={s.word}>{site.name}</span>
    </span>
  );
}
