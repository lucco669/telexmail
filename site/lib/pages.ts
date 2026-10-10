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
  /** Screenshot on the link preview image (public/screens/<id>-light.png). */
  ogScreen: string;
};

export const pages = {
  home: {
    path: '/',
    label: 'Home',
    title: 'Telex – a fast, keyboard-driven email client for Windows',
    description:
      'A compact, fast email client for Windows. Dense lists, one key per action, and your mail on your own disk, searchable offline. No Electron, no telemetry.',
    ogTitle: 'Email at the speed of your keyboard.',
    ogScreen: 'inbox',
  },
  features: {
    path: '/features',
    label: 'Features',
    title: 'Features of a fast, keyboard-driven email client',
    description:
      'What Telex does today: a dense list, one key per action, accounts that set themselves up, IMAP sync, SMTP sending, rules and offline search.',
    ogTitle: 'Everything Telex does today.',
    ogScreen: 'inbox',
  },
  keyboard: {
    path: '/keyboard',
    label: 'Keyboard',
    title: 'Keyboard shortcuts',
    description:
      'Every Telex shortcut on one page: move, reply, forward, flag, delete, undo, filter and send without touching the mouse. Printable as a cheat sheet.',
    ogTitle: 'One key per action.',
    ogScreen: 'reply',
  },
  screenshots: {
    path: '/screenshots',
    label: 'Screenshots',
    title: 'Screenshots in light and dark',
    description:
      'Telex in light and dark: the main window, reply, quick filter, HTML mail, rules, identities, account setup, the Outbox and comfortable rows.',
    ogTitle: 'A closer look.',
    ogScreen: 'filter',
  },
  download: {
    path: '/download',
    label: 'Download',
    title: 'Download for Windows',
    description:
      'Download Telex for Windows 10 and 11. System requirements, installation steps, and how Telex sets up your IMAP and SMTP account.',
    ogTitle: 'Get Telex for Windows.',
    ogScreen: 'account',
  },
  roadmap: {
    path: '/roadmap',
    label: 'Roadmap',
    title: 'Roadmap: what’s done and what’s next',
    description:
      'Where Telex stands: what works today, what is being built, and what comes next, from HTML mail to filters and Gmail sign-in.',
    ogTitle: 'Early, and honest about it.',
    ogScreen: 'comfortable',
  },
  faq: {
    path: '/faq',
    label: 'FAQ',
    title: 'Questions and answers',
    description:
      'Answers about Telex: which mail providers work, sending, where your mail is stored, privacy, and installing on Windows.',
    ogTitle: 'Questions and answers.',
    ogScreen: 'images',
  },
  about: {
    path: '/about',
    label: 'About',
    title: 'About: a mail client built like a tool',
    description:
      'The ideas behind Telex: dense, keyboard-first, local and private, in the spirit of The Bat!. What it will never be, and what it is built with.',
    ogTitle: 'A mail client built like a tool.',
    ogScreen: 'reply',
  },
  privacy: {
    path: '/privacy',
    label: 'Privacy',
    title: 'Privacy: no account, no telemetry',
    description:
      'What the Telex app and this website store and send: no account, no telemetry, no cookies, no trackers. Your mail stays on your computer.',
    ogTitle: 'Nothing about you leaves your PC.',
    ogScreen: 'images',
  },
} satisfies Record<string, PageInfo>;

export const nav: PageInfo[] = [pages.features, pages.keyboard, pages.screenshots, pages.roadmap, pages.faq];

export type FooterLink = { path: string; label: string };

export const footerGroups: { title: string; items: FooterLink[] }[] = [
  { title: 'Product', items: [pages.features, pages.keyboard, pages.screenshots, pages.download] },
  { title: 'Project', items: [pages.roadmap, pages.faq, pages.about] },
  { title: 'Legal', items: [pages.privacy, { path: '/privacy#cookies', label: 'Cookies' }] },
];
