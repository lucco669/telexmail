// Line icons in the app's style: 16px grid, 1.35 stroke.
const Svg = ({ children }: { children: React.ReactNode }) => (
  <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    {children}
  </svg>
);

export const DownloadIcon = () => (
  <Svg>
    <path d="M8 2v7.5M5 6.5l3 3 3-3" />
    <path d="M2 10v3.5h12V10" />
  </Svg>
);

export const ArrowIcon = () => (
  <Svg>
    <path d="M3 8h10M9 4l4 4-4 4" />
  </Svg>
);

export const CheckIcon = () => (
  <Svg>
    <path d="M3.5 8.5l3 3 6-7" />
  </Svg>
);
