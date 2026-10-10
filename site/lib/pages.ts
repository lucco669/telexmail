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
      'What Telex does: a dense list, one key per action, conversations, attachments, accounts that set themselves up, IMAP sync, rules and offline search.',
    ogTitle: 'Everything Telex does today.',
    ogScreen: 'inbox',
  },
  demo: {
    path: '/demo',
    label: 'Live demo',
    title: 'Live demo: try Telex in your browser',
    description:
      'The real Telex interface in your browser, with a demo mailbox: read, reply, filter, move and send with the keyboard. Nothing you do leaves the page.',
    ogTitle: 'Try it in your browser.',
    ogScreen: 'thread',
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
      'Telex in light and dark: the main window, reply, conversations, moving many messages, attachments, the quick filter, HTML mail, rules and settings.',
    ogTitle: 'A closer look.',
    ogScreen: 'filter',
  },
  accessibility: {
    path: '/accessibility',
    label: 'Accessibility',
    title: 'Accessibility: eight themes, all WCAG 2.2',
    description:
      'Eight themes, each measured against WCAG 2.2: AA contrast everywhere, AAA in high contrast. Text up to 200%, keyboard access, English and Polish.',
    ogTitle: 'Readable in every theme.',
    ogScreen: 'settings',
  },
  download: {
    path: '/download',
    label: 'Download',
    title: 'Download for Windows and Linux',
    description:
      'Download Telex for Windows 10 and 11, or Linux as an AppImage or .deb. System requirements, installation steps, and how Telex sets up your account.',
    ogTitle: 'Get Telex for Windows.',
    ogScreen: 'account',
  },
  changelog: {
    path: '/changelog',
    label: 'Changelog',
    title: 'Changelog: what’s new in each version',
    description:
      'Every Telex release and what it brought, from the first self-updating build to attachments, conversations and undo send, with its downloads.',
    ogTitle: 'What’s new in Telex.',
    ogScreen: 'select',
  },
  roadmap: {
    path: '/roadmap',
    label: 'Roadmap',
    title: 'Roadmap: what’s done and what’s next',
    description:
      'Where Telex stands: what works today, what the next version brings, and what follows, from Gmail sign-in to backups and local folders.',
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

export const nav: PageInfo[] = [pages.features, pages.demo, pages.keyboard, pages.accessibility, pages.changelog, pages.roadmap, pages.faq];

export type FooterLink = { path: string; label: string };

export const footerGroups: { title: string; items: FooterLink[] }[] = [
  { title: 'Product', items: [pages.features, pages.demo, pages.keyboard, pages.accessibility, pages.screenshots, pages.download] },
  { title: 'Project', items: [pages.changelog, pages.roadmap, pages.faq, pages.about] },
  { title: 'Legal', items: [pages.privacy, { path: '/privacy#cookies', label: 'Cookies' }] },
];
