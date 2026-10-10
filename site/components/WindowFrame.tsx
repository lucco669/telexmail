import { LogoMark } from './Logo';
import type { ShotTheme } from './Shot';
import s from './WindowFrame.module.css';

/** A Windows 11 style title bar around a screenshot, themed like the screenshot inside it. */
export function WindowFrame({ title, theme = 'site', children }: { title: string; theme?: ShotTheme; children: React.ReactNode }) {
  return (
    <div className={s.window} data-theme={theme}>
      <div className={s.titlebar} aria-hidden="true">
        <LogoMark size={14} />
        <span className={s.title}>{title}</span>
        <span className={s.controls}>
          <svg viewBox="0 0 10 10"><path d="M0 5h10" /></svg>
          <svg viewBox="0 0 10 10"><rect x="0.5" y="0.5" width="9" height="9" rx="1.5" /></svg>
          <svg viewBox="0 0 10 10"><path d="M0.5 0.5l9 9M9.5 0.5l-9 9" /></svg>
        </span>
      </div>
      {children}
    </div>
  );
}
