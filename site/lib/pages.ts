// Every page of the site: its address, menu label and search snippet. The header, footer, sitemap,
// breadcrumbs and link previews all read from here. (No imports: scripts/screens.mjs loads it too.)

export type PageInfo = {
  path: string;
  label: string;
  /** <title>, before " – Telex". About 50 characters at most. */
  title: string;
  /** Meta description, at most about 155 characters. */
  description: string;
  /** Big line on the page's link preview image. */
  ogTitle: string;
  /** Screenshot on the link preview image (public/screens/<id>-light.webp). */
  ogScreen: string;
  /** Optional editorial image in public/, in place of an app screenshot. */
  ogImage?: string;
};

export const pages = {
  home: {
    path: '/',
    label: 'Home',
    title: 'Telex – fast email for Windows and Linux',
    description:
      'Fast email for Windows and Linux: a spam filter that learns on your PC, saved searches, rule previews and optional threads. No Electron or telemetry.',
    ogTitle: 'Email at the speed of your keyboard.',
    ogScreen: 'inbox',
  },
  features: {
    path: '/features',
    label: 'Features',
    title: 'Features of a fast, keyboard-driven email client',
    description:
      'Explore Telex: a learning spam filter, saved searches, header and attachment rules with previews, optional threads, link checks and HTML compose.',
    ogTitle: 'Everything Telex does today.',
    ogScreen: 'rule-preview',
  },
  demo: {
    path: '/demo',
    label: 'Live demo',
    title: 'Live demo: try Telex in your browser',
    description:
      'Try Telex’s real interface: saved searches, rule previews, threads, spam notes and toolbar layouts in a demo mailbox. Your edits stay in the tab.',
    ogTitle: 'Try it in your browser.',
    ogScreen: 'thread',
  },
  keyboard: {
    path: '/keyboard',
    label: 'Keyboard',
    title: 'Keyboard shortcuts',
    description:
      'Every Telex shortcut: read, reply, select, move, search, expand threads and inspect links from the keyboard. Print a cheat sheet or try the demo.',
    ogTitle: 'One key per action.',
    ogScreen: 'reply',
  },
  performance: {
    path: '/performance',
    label: 'Performance',
    title: 'Performance: small footprint, measured speed',
    description: 'Telex with 100,000 messages: real desktop startup, search, navigation and whole-app memory measurements, with comparison data and a repeatable method.',
    ogTitle: 'Light on your PC. Quick through your inbox.',
    ogScreen: 'filter',
  },
  screenshots: {
    path: '/screenshots',
    label: 'Screenshots',
    title: 'Screenshots in light and dark',
    description:
      'See Telex in light and dark: saved searches, rule previews, link inspection, expandable threads, five toolbar layouts, compose and settings.',
    ogTitle: 'A closer look.',
    ogScreen: 'toolbar-layouts',
  },
  accessibility: {
    path: '/accessibility',
    label: 'Accessibility',
    title: 'Accessibility: eight themes, all WCAG 2.2',
    description:
      'Eight measured themes, six fonts, five toolbar layouts and text up to 200%. AA contrast, two AAA themes, keyboard access, English and Polish.',
    ogTitle: 'Readable in every theme.',
    ogScreen: 'settings',
  },
  download: {
    path: '/download',
    label: 'Download',
    title: 'Download for Windows and Linux',
    description:
      'Download Telex for Windows or Linux. Get saved searches, stronger rules and local scam checks, with installation, migration and upgrade guidance.',
    ogTitle: 'Get Telex for Windows and Linux.',
    ogScreen: 'account',
  },
  changelog: {
    path: '/changelog',
    label: 'Changelog',
    title: 'Changelog: what’s new in each version',
    description:
      'Every Telex release, including 0.9.5: saved searches, stronger rules, threads, toolbar layouts and local scam checks. Notes, upgrades and downloads.',
    ogTitle: 'What’s new in Telex.',
    ogScreen: 'saved-searches',
  },
  roadmap: {
    path: '/roadmap',
    label: 'Roadmap',
    title: 'Roadmap: what’s done and what’s next',
    description:
      'Telex’s shipped features and current limits: saved searches, richer rules, local mail inspection and security gates. Next: Microsoft and Google sign-in.',
    ogTitle: 'Early, and honest about it.',
    ogScreen: 'comfortable',
  },
  faq: {
    path: '/faq',
    label: 'FAQ',
    title: 'Questions and answers',
    description:
      'Telex answers: providers, saved searches, rule previews, copy and forwarding, threads, toolbar layouts, local scam checks, privacy and upgrades.',
    ogTitle: 'Questions and answers.',
    ogScreen: 'images',
  },
  about: {
    path: '/about',
    label: 'About',
    title: 'About: a mail client built like a tool',
    description:
      'Why Telex is dense, keyboard-first and local: reusable searches, rules you can preview, readable layouts and offline mail inspection. Built in Rust.',
    ogTitle: 'A mail client built like a tool.',
    ogScreen: 'reply',
  },
  history: {
    path: '/history',
    label: 'Telex & email history',
    title: 'Telex, email and the history of written messages',
    description:
      'Explore telex and email history, try real paper-tape encoding, and play Night Shift: a telegram dispatch game in an illustrated nighttime office.',
    ogTitle: 'Before the inbox, there was telex.',
    ogScreen: 'inbox',
    ogImage: '/history/teleprinter.webp',
  },
  security: {
    path: '/security',
    label: 'Privacy & security',
    title: 'Privacy & security: protections and evidence',
    description:
      'Inspect Telex’s local scam checks, link protections and release security gates: scans, native webview tests, parser fuzzing, source and current limits.',
    ogTitle: 'Privacy & security. Evidence you can inspect.',
    ogScreen: 'links',
  },
  privacy: {
    path: '/privacy',
    label: 'Privacy',
    title: 'Privacy: what the app and website store and send',
    description:
      'Where Telex stores mail, saved searches and passwords; how offline scam checks work; when it goes online; and what the website and demo store.',
    ogTitle: 'What stays on your PC. What goes online.',
    ogScreen: 'images',
  },
} satisfies Record<string, PageInfo>;

export const nav: PageInfo[] = [pages.features, pages.demo, pages.performance, pages.keyboard, pages.accessibility, pages.changelog, pages.roadmap, pages.faq, { ...pages.history, label: 'History' }];

export type FooterLink = { path: string; label: string };

export const footerGroups: { title: string; items: FooterLink[] }[] = [
  { title: 'Product', items: [pages.features, pages.demo, pages.performance, pages.keyboard, pages.accessibility, pages.screenshots, pages.download] },
  { title: 'Project', items: [pages.changelog, pages.roadmap, pages.faq, pages.security, pages.about, pages.history] },
  { title: 'Legal', items: [pages.privacy, { path: '/privacy#cookies', label: 'Cookies' }] },
];
