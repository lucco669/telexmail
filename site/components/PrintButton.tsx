'use client';

/** Prints the page; the print styles turn it into a one-page cheat sheet. */
export function PrintButton({ label }: { label: string }) {
  return (
    <button type="button" className="btn no-print" onClick={() => window.print()}>
      <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinejoin="round" aria-hidden="true">
        <path d="M4.5 6V2h7v4M4.5 11.5h-2V6h11v5.5h-2" />
        <rect x="4.5" y="9.5" width="7" height="4.5" />
      </svg>
      {label}
    </button>
  );
}
