// Page copy that changes as the app grows: features, keys, screenshots, roadmap, questions.
// Keep claims true to the current build (see CLAUDE.md, "Current state").

/** The six points on the home page. */
export const features = [
  {
    title: 'Dense by default',
    text: 'Twenty-pixel rows, a condensed face for the list and mono for dates and sizes, so a screen holds a lot of mail. Want more air? Switch to Comfortable.',
  },
  {
    title: 'One key per action',
    text: 'Reply, forward, flag, delete, undo, filter, move through the list: each is a single key. The mouse is optional.',
  },
  {
    title: 'Your mail, on your disk',
    text: 'Messages are kept in full in a local mail base, so reading and searching work offline. There is no Telex server for your mail to pass through.',
  },
  {
    title: 'Private by default',
    text: 'No account and no telemetry. Remote images stay blocked until you allow them, and your password stays in Windows Credential Manager.',
  },
  {
    title: 'Light on your PC',
    text: 'A Rust core in the WebView2 window that Windows already has. No Electron, no bundled browser, a small installer.',
  },
  {
    title: 'Fluent in Polish mail',
    text: 'Old charsets like Windows-1250 and ISO-8859-2 decode correctly, search ignores diacritics, and replies quote the Polish way: “Dnia 10.10.2026 o 06:12, Anna Kowalska napisał(a):”.',
  },
];

export const facts = ['Rust core', 'WebView2', 'SQLite + FTS5', 'IMAP + SMTP over TLS', 'No Electron', 'No telemetry'];

/** The features page: one row per area, each with a screenshot. */
export type FeatureRow = { id: string; screen: string; title: string; text: string; points: string[]; soon?: boolean };

export const featureRows: FeatureRow[] = [
  {
    id: 'reading',
    screen: 'inbox',
    title: 'A message list built for volume',
    text: 'Folders on the left, the list on top, the message below. Rows are 20 pixels tall, so a normal screen shows thirty messages, not twelve.',
    points: [
      'Sort by status, sender, subject, date or size',
      'Unread in bold, flags and attachments at a glance',
      'Dates and sizes in mono, so columns line up',
      'Resizable panes, and a Comfortable setting for more room',
    ],
  },
  {
    id: 'writing',
    screen: 'reply',
    title: 'Plain text, written fast',
    text: 'R opens the reply right in the reading pane, quoted and ready. Compose is plain text in a mono face, the way mail between people reads best.',
    points: [
      'Reply, reply to all and forward, each one key',
      'Polish quote header: “Dnia … napisał(a):”',
      'Ctrl+Enter sends, Esc saves a draft and closes',
      'E reopens a draft or a message still in the Outbox',
    ],
  },
  {
    id: 'sending',
    screen: 'account',
    title: 'Sending that tells the truth',
    text: 'Mail goes out over SMTP with SSL/TLS or STARTTLS. Telex says “Sent” only after your server has accepted the message, and keeps it in the Outbox until then.',
    points: [
      'SMTP on port 465 (SSL/TLS) or 587 (STARTTLS), same login as IMAP',
      'An Outbox that retries at every check when the connection fails',
      'A copy of every sent message lands in your server’s Sent folder',
      'Forwarded attachments travel with the forward',
    ],
  },
  {
    id: 'sync',
    screen: 'comfortable',
    title: 'Two-way sync with your server',
    text: 'Reading, flagging, moving and deleting happen on your PC at once and reach the server right after. Changes wait in a queue that survives restarts and lost connections.',
    points: [
      'IMAP over SSL/TLS, folders straight from the server',
      'Sync on start, on Get mail and every 10 minutes',
      'Inbox, Sent, Trash and others found by their flags or by English and Polish names',
      'The first sync takes the newest 300 messages of each folder',
    ],
  },
  {
    id: 'search',
    screen: 'filter',
    title: 'Search that works offline',
    text: 'Press / and type. Senders and subjects match as you type; the text of every message matches through full-text search in the local mail base.',
    points: [
      'No server round trip: results come from your disk',
      'Ignores diacritics: “wrzesien” finds “wrzesień”',
      'Esc clears the filter and puts you back in the list',
    ],
  },
  {
    id: 'privacy',
    screen: 'images',
    title: 'Private by default',
    text: 'Telex talks to your mail servers, to image servers only when you load a message’s images, and to GitHub for updates only if you allow it. There is no account to create, no telemetry, and no Telex server between you and your mail.',
    points: [
      'Remote images blocked, so senders can’t see when you read',
      'Your password stays in Windows Credential Manager',
      'Your mail base is one file on your disk',
    ],
  },
  {
    id: 'rules',
    screen: 'spam',
    title: 'Rules that explain themselves',
    text: 'When a rule moves or flags a message, the message says which rule did it and why. The rule editor is Telex’s answer to The Bat!’s Sorting Office.',
    points: ['Sort, flag, move and auto-reply', 'Run on arrival or on demand', 'A note on every message a rule touched'],
  },
];

/** Honest gaps, shown on the features page. */
export const notYet = [
  'HTML mail keeps its inline styling, but style sheets inside a message (<style> blocks) are dropped for now.',
  'Gmail and Outlook.com need OAuth2 sign-in, which isn’t built yet.',
  'IMAP connects over SSL/TLS on port 993 only; STARTTLS for IMAP is planned.',
  'You can forward attachments, but not attach new files or open them yet.',
  'Only the newest 300 messages per folder come down on the first sync.',
];

// `match` lists the KeyboardEvent combinations that light the row up in the keyboard table.
export type Shortcut = {
  id: string;
  group: 'Move' | 'Write' | 'Organize' | 'Find';
  keys: string[][];
  action: string;
  match: { key: string; shift?: boolean; ctrl?: boolean }[];
};

export const shortcuts: Shortcut[] = [
  { id: 'move', group: 'Move', keys: [['↑'], ['↓']], action: 'Previous / next message', match: [{ key: 'ArrowUp' }, { key: 'ArrowDown' }] },
  { id: 'ends', group: 'Move', keys: [['Home'], ['End']], action: 'First / last message', match: [{ key: 'Home' }, { key: 'End' }] },
  { id: 'new', group: 'Write', keys: [['N']], action: 'New message', match: [{ key: 'n' }] },
  { id: 'reply', group: 'Write', keys: [['R']], action: 'Reply', match: [{ key: 'r' }] },
  { id: 'replyall', group: 'Write', keys: [['Shift', 'R']], action: 'Reply to all', match: [{ key: 'R', shift: true }] },
  { id: 'forward', group: 'Write', keys: [['Shift', 'F']], action: 'Forward', match: [{ key: 'F', shift: true }] },
  { id: 'edit', group: 'Write', keys: [['E']], action: 'Edit a draft or an unsent message', match: [{ key: 'e' }] },
  { id: 'send', group: 'Write', keys: [['Ctrl', 'Enter']], action: 'Send from compose', match: [{ key: 'Enter', ctrl: true }] },
  { id: 'flag', group: 'Organize', keys: [['F']], action: 'Flag / unflag', match: [{ key: 'f' }] },
  { id: 'unread', group: 'Organize', keys: [['U']], action: 'Toggle unread', match: [{ key: 'u' }] },
  { id: 'delete', group: 'Organize', keys: [['Del']], action: 'Move to Trash', match: [{ key: 'Delete' }] },
  { id: 'undo', group: 'Organize', keys: [['Z']], action: 'Undo last delete', match: [{ key: 'z' }] },
  { id: 'filter', group: 'Find', keys: [['/']], action: 'Quick filter', match: [{ key: '/' }] },
  { id: 'view', group: 'Find', keys: [['H']], action: 'HTML / plain text view', match: [{ key: 'h' }] },
  { id: 'esc', group: 'Find', keys: [['Esc']], action: 'Clear filter · save draft and close', match: [{ key: 'Escape' }] },
];

export const keyboardTips = [
  {
    title: 'Arrow past without reading',
    text: 'A message counts as read after it has been on screen for a moment, so moving through the list with ↑ ↓ doesn’t mark everything you pass.',
  },
  {
    title: 'Delete without fear',
    text: 'Del moves a message to Trash and Z brings it back. In Trash, Del asks for a second press before it deletes for good.',
  },
  {
    title: 'Never lose a draft',
    text: 'Esc in compose saves a draft and closes it. E opens it again, and also reopens a message that is still waiting in the Outbox.',
  },
  {
    title: 'Filter in two keys',
    text: 'Press / and type a few letters; the list narrows as you type. Esc clears the filter and puts you back in the list.',
  },
];

// Images come from `npm run screens` (public/screens/<id>-light.png and <id>-dark.png).
export type Screen = { id: string; label: string; title: string; caption: string };

export const hero: Screen = {
  id: 'inbox',
  label: 'Inbox',
  title: 'Inbox · biuro@twojastudio.pl – Telex',
  caption: 'The main window: folders, message list and preview, with the sample mailbox.',
};

export const screens: Screen[] = [
  {
    id: 'reply',
    label: 'Reply',
    title: 'Inbox · biuro@twojastudio.pl – Telex',
    caption: 'R opens the reply right in the reading pane: plain text in a mono face, quoted with a Polish header.',
  },
  {
    id: 'filter',
    label: 'Quick filter',
    title: 'Inbox · biuro@twojastudio.pl – Telex',
    caption: 'Press / and type. Senders and subjects match as you type, message text through full-text search: “wrzesien” finds “wrzesień”.',
  },
  {
    id: 'images',
    label: 'Blocked images',
    title: 'Inbox · biuro@twojastudio.pl – Telex',
    caption: 'Remote images stay off until you load them, so a sender can’t tell when you opened the message.',
  },
  {
    id: 'spam',
    label: 'Rule notes',
    title: 'Spam · biuro@twojastudio.pl – Telex',
    caption: 'When a rule moves a message, the message says which rule and why.',
  },
  {
    id: 'comfortable',
    label: 'Comfortable',
    title: 'Inbox · twoje.imie@gmail.com – Telex',
    caption: 'Compact is the default. One click on the toolbar gives every row more room.',
  },
  {
    id: 'account',
    label: 'Account',
    title: 'Inbox · biuro@twojastudio.pl – Telex',
    caption: 'Adding an account: IMAP and SMTP in one dialog. Telex logs in to both before it saves anything.',
  },
];

export type Step = { title: string; text: string; status: 'done' | 'now' | 'next' | 'later'; notYet?: string };

export const roadmap: Step[] = [
  {
    title: 'Real mail over IMAP',
    text: 'SSL/TLS with password login, folders from the server, sync on start and every 10 minutes, offline search.',
    status: 'done',
    notYet: 'STARTTLS for IMAP, downloading mail older than the first 300 per folder, instant push (IDLE).',
  },
  {
    title: 'Write back',
    text: 'Reading, flagging, moving and deleting reach the server. Changes wait in a queue that survives restarts and lost connections.',
    status: 'done',
  },
  {
    title: 'Sending',
    text: 'SMTP over SSL/TLS or STARTTLS, an Outbox that retries, and a copy of every sent message in Sent on the server.',
    status: 'done',
    notYet: 'Attaching new files, Bcc, drafts on the server.',
  },
  {
    title: 'HTML mail',
    text: 'Rendered in a sandbox where no script can run. Remote images stay blocked until you allow them, per message or per sender, and Telex fetches them itself.',
    status: 'done',
    notYet: 'Style sheets inside messages, opening attachments.',
  },
  {
    title: 'Filters',
    text: 'A Sorting Office: rules that sort, flag, move and auto-reply, run on arrival or on demand, each leaving a note on the message.',
    status: 'done',
    notYet: 'Regular expressions, conditions on headers and attachments, rules that forward.',
  },
  { title: 'Gmail and Outlook.com', text: 'OAuth2 sign-in, since both refuse plain passwords over IMAP.', status: 'next' },
  {
    title: 'Templates and identities',
    text: 'Several identities per account, each with its own signature and reply and forward templates. A reply is from the address it was sent to.',
    status: 'done',
    notYet: 'Greeting templates for new messages, HTML signatures.',
  },
  {
    title: 'Updates',
    text: 'Telex checks GitHub for a new version once a day if you allow it, or when you click its version number, and installs it with one click and a restart. Updates must carry Telex’s signature.',
    status: 'done',
    notYet: 'A code-signed installer, so Windows SmartScreen stops warning.',
  },
];

export type Question = { q: string; a: string };

export const faqGroups: { title: string; items: Question[] }[] = [
  {
    title: 'Accounts and mail',
    items: [
      {
        q: 'Which mail providers work?',
        a: 'Any provider that offers IMAP over SSL/TLS (port 993) and SMTP over SSL/TLS (465) or STARTTLS (587) with a password: most hosting mailboxes and many providers. Gmail and Outlook.com need OAuth2 sign-in, which is on the roadmap.',
      },
      {
        q: 'Can Telex send mail?',
        a: 'Yes. Mail goes out over SMTP, waits in the Outbox if the connection fails, and is marked Sent only once your server accepts it. A copy goes to the Sent folder on your server.',
      },
      {
        q: 'Does it work with Gmail or Outlook.com?',
        a: 'Not yet. Both require OAuth2 sign-in instead of a password, and that is on the roadmap.',
      },
      {
        q: 'Do changes I make reach the server?',
        a: 'Yes. Reading, flagging, moving and deleting happen locally at once and then on the server. If you are offline, they wait in a queue and go out at the next sync.',
      },
      {
        q: 'Can it read HTML newsletters?',
        a: 'Yes. HTML mail is shown in a sandbox where no script can run. Remote images stay blocked until you load them for that message, or for everything from a sender you trust (never automatically in Spam). H switches to plain text.',
      },
    ],
  },
  {
    title: 'Privacy and data',
    items: [
      {
        q: 'Does Telex send anything about me anywhere?',
        a: 'No. There is no account, no telemetry and no Telex server. The app talks to your mail servers, to the servers that host a message’s images only when you choose to load them, and to GitHub to look for a new version only if you allow it. That check sends nothing about you.',
      },
      {
        q: 'Where is my mail kept?',
        a: 'In one file on your disk: %APPDATA%\\app.telexmail.desktop\\telex.db. Passwords are stored separately, in Windows Credential Manager.',
      },
      {
        q: 'Can I search without an internet connection?',
        a: 'Yes. Every downloaded message is stored in full and indexed on your PC, so the quick filter and full-text search work offline.',
      },
    ],
  },
  {
    title: 'Installing',
    items: [
      {
        q: 'Which Windows versions are supported?',
        a: 'Windows 10 and 11, 64-bit. Telex uses WebView2, which Windows 11 already has; on Windows 10 the installer fetches it if it is missing.',
      },
      {
        q: 'Why does Windows warn me when I run the installer?',
        a: 'The installer isn’t code-signed yet, so SmartScreen doesn’t know it. Choose More info, then Run anyway. Signing is on the roadmap.',
      },
      {
        q: 'How do I update Telex?',
        a: 'Click the version number at the right end of the status bar. Telex asks GitHub for the newest version and, with your click, installs it and restarts. On first start it also asks whether to check once a day by itself. Installing a newer version over the old one works too; your mail and settings stay.',
      },
      {
        q: 'Is there a Mac or Linux version?',
        a: 'No. Telex is made for Windows first.',
      },
      {
        q: 'Is it like The Bat!?',
        a: 'That is the inspiration: a fast, dense, keyboard-driven client with serious filters. Telex is a new app written from scratch, and not affiliated with Ritlabs.',
      },
    ],
  },
];

/** The four questions on the home page. */
export const faq: Question[] = [faqGroups[0].items[0], faqGroups[0].items[1], faqGroups[1].items[0], faqGroups[2].items[3]];

/** Lines the telex tape above the footer punches, one after another. */
export const ticker = [
  'ZCZC TELEX FOR WINDOWS',
  'FAST KEYBOARD MAIL. LOCAL MAIL BASE. NO TELEMETRY.',
  'IMAP SYNC, SMTP SENDING, SAFE HTML MAIL AND RULES WORK TODAY',
  'NEXT: GMAIL AND OUTLOOK.COM SIGN-IN',
  'TRY: R = REPLY, F = FLAG, / = FILTER',
  'NNNN',
];
