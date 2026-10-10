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
    title: 'Fluent in Polish',
    text: 'The whole interface speaks Polish as well as English, picked from Windows. Old charsets like Windows-1250 decode correctly, search ignores diacritics, and replies quote the Polish way: “Dnia 10.10.2026 o 06:12, Anna Kowalska napisał(a):”.',
  },
];

export const facts = ['Rust core', 'WebView2', 'SQLite + FTS5', 'IMAP + SMTP over TLS', 'WCAG 2.2 AA', 'No Electron', 'No telemetry'];

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
    text: 'R opens the reply right in the reading pane, quoted and signed. Compose is plain text in a mono face, the way mail between people reads best.',
    points: [
      'Reply, reply to all and forward, each one key',
      'A From picker with all your identities; the signature follows',
      'Ctrl+Enter sends, Esc saves a draft and closes',
      'E reopens a draft or a message still in the Outbox',
    ],
  },
  {
    id: 'setup',
    screen: 'account',
    title: 'Accounts that set themselves up',
    text: 'Type your name, address and password. Telex finds the IMAP and SMTP servers itself and logs in to both before it saves anything. If it can’t, the settings form opens filled in with what it found, and why it stopped.',
    points: [
      'A built-in list of common providers, checked against their live servers',
      'Then your domain’s own setup file and its DNS records (SRV and MX)',
      'Then the usual names: imap., mail. and smtp. your domain',
      'Tips when a provider needs an app password or IMAP turned on',
      'Asks only DNS and your domain’s servers, never a lookup service',
    ],
  },
  {
    id: 'sending',
    screen: 'outbox',
    title: 'Sending that tells the truth',
    text: 'Mail goes out over SMTP with SSL/TLS or STARTTLS. Telex says “Sent” only after your server has accepted the message, and keeps it in the Outbox until then, with the reason.',
    points: [
      'SMTP on port 465 (SSL/TLS) or 587 (STARTTLS), same login as IMAP',
      'An Outbox that retries at every check when the connection fails',
      'A copy of every sent message lands in your server’s Sent folder',
      'Forwarded attachments travel with the forward',
    ],
  },
  {
    id: 'identities',
    screen: 'identities',
    title: 'One account, several identities',
    text: 'Write as yourself, as the studio or as billing from the same mailbox. Each identity has its own signature and its own reply and forward headers, and a reply goes out from the address the message was sent to.',
    points: [
      'Any number of identities per account, one of them the default',
      'Header templates with {name}, {email}, {date}, {time} and {subject}',
      'The Polish reply header out of the box: “Dnia … napisał(a):”',
      'Switching identity in compose swaps the signature too',
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
      'Ignores diacritics: “krakow” finds “Kraków”',
      'Esc clears the filter and puts you back in the list',
    ],
  },
  {
    id: 'privacy',
    screen: 'images',
    title: 'Private by default',
    text: 'Telex talks to your mail servers, to image servers only when you load a message’s images, and to GitHub for updates only if you allow it. There is no account to create, no telemetry, and no Telex server between you and your mail.',
    points: [
      'HTML mail in a sandbox where no script can run',
      'Remote images blocked, so senders can’t see when you read',
      'Your password stays in Windows Credential Manager',
      'Your mail base is one file on your disk',
    ],
  },
  {
    id: 'rules',
    screen: 'rules',
    title: 'Rules that explain themselves',
    text: 'Telex’s answer to The Bat!’s Sorting Office. Rules run top to bottom on new mail, and when one moves or flags a message, the message says which rule did it.',
    points: [
      'Conditions on From, To or Cc, subject, text or anywhere',
      'Move, mark read, flag, move to Trash or answer automatically',
      'Run on arrival, or on any folder when you ask',
      'Auto-replies skip mailing lists and robots, and answer each sender once in 4 days',
    ],
  },
  {
    id: 'settings',
    screen: 'settings',
    title: 'Your colors, your language',
    text: 'Eight themes, from plain light to a green phosphor screen, each measured against WCAG 2.2. The interface speaks English and Polish. Settings save as you change them, and travel to another PC in one file.',
    points: [
      'Every theme meets AA contrast; the two high-contrast ones meet AAA',
      'English or Polish, as in Windows or as you pick',
      'Text from 90% to 200% with Ctrl+= and Ctrl+−',
      'Export settings, rules, identities and servers to a file, never passwords',
      'Import shows what would change, part by part, before anything does',
    ],
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
  group: 'Move' | 'Write' | 'Organize' | 'Find' | 'View';
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
  { id: 'settings', group: 'View', keys: [['Ctrl', ',']], action: 'Settings', match: [{ key: ',', ctrl: true }] },
  { id: 'larger', group: 'View', keys: [['Ctrl', '='], ['Ctrl', '−']], action: 'Larger / smaller text', match: [{ key: '=', ctrl: true }, { key: '+', ctrl: true }, { key: '-', ctrl: true }] },
  { id: 'normal', group: 'View', keys: [['Ctrl', '0']], action: 'Normal text size', match: [{ key: '0', ctrl: true }] },
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
  title: 'Inbox · sam@lindenstudio.com – Telex',
  caption: 'The main window: folders, message list and preview, with a demo mailbox.',
};

export const screens: Screen[] = [
  {
    id: 'reply',
    label: 'Reply',
    title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'R opens the reply right in the reading pane: plain text in a mono face, quoted and signed, from the identity you pick.',
  },
  {
    id: 'filter',
    label: 'Quick filter',
    title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'Press / and type. Senders and subjects match as you type, message text through full-text search: “krakow” finds “Kraków”.',
  },
  {
    id: 'images',
    label: 'HTML mail',
    title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'HTML mail in a sandbox. Remote images stay off until you load them, so a sender can’t tell when you opened the message.',
  },
  {
    id: 'rules',
    label: 'Rules',
    title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'A rule: conditions on sender, recipients, subject or text, then move, mark read, flag, delete or answer.',
  },
  {
    id: 'spam',
    label: 'Rule notes',
    title: 'Spam · sam@lindenstudio.com – Telex',
    caption: 'When a rule moves a message, the message says which rule did it.',
  },
  {
    id: 'identities',
    label: 'Identities',
    title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'Several identities per account, each with its own signature and reply and forward headers.',
  },
  {
    id: 'account',
    label: 'Setup',
    title: 'Telex',
    caption: 'Adding an account: name, address and password. Telex finds the servers itself and logs in before it saves anything.',
  },
  {
    id: 'outbox',
    label: 'Outbox',
    title: 'Outbox · sam@lindenstudio.com – Telex',
    caption: 'Mail that couldn’t go out waits in the Outbox with the reason. “Sent” means your server accepted it.',
  },
  {
    id: 'settings',
    label: 'Themes',
    title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'Settings: eight themes, each card drawn in its own colors. Every one meets WCAG 2.2 AA contrast.',
  },
  {
    id: 'polish',
    label: 'Polski',
    title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'The same window in Polish. Telex follows the Windows display language, or the one you pick.',
  },
  {
    id: 'comfortable',
    label: 'Comfortable',
    title: 'Inbox · sam@carterfamily.net – Telex',
    caption: 'Compact is the default. One click on the toolbar gives every row more room.',
  },
];

export type Step = { title: string; text: string; status: 'done' | 'now' | 'next' | 'later'; notYet?: string };

export const roadmap: Step[] = [
  {
    title: 'Real mail over IMAP',
    text: 'SSL/TLS with password login, servers found from your address, folders from the server, sync on start and every 10 minutes, offline search.',
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
  {
    title: 'Settings, themes and Polish',
    text: 'A settings window, eight themes that all meet WCAG 2.2 AA contrast (two at AAA), text up to 200%, the whole interface in Polish, and settings, rules and identities exported to one file and imported with a preview.',
    status: 'done',
    notYet: 'More languages, and messages from the mail engine in Polish.',
  },
  {
    title: 'Archive and backup',
    text: 'Folders kept only on your PC, automatic backups you can restore, export and import as .eml or mbox, and POP3 accounts. Big mailboxes stay as fast as small ones.',
    status: 'later',
  },
];

export type Question = { q: string; a: string };

export const faqGroups: { title: string; items: Question[] }[] = [
  {
    title: 'Accounts and mail',
    items: [
      {
        q: 'Which mail providers work?',
        a: 'Any provider that offers IMAP over SSL/TLS (port 993) and SMTP over SSL/TLS (465) or STARTTLS (587) with a password: most hosting mailboxes and many providers. Telex finds their settings from your address. Gmail and Outlook.com need OAuth2 sign-in, which is on the roadmap.',
      },
      {
        q: 'Do I need to know my server settings?',
        a: 'Usually not. Enter your name, address and password, and Telex finds the IMAP and SMTP servers: from its list of common providers, your domain’s setup file or DNS records, or by trying the usual names such as mail.yourdomain. It logs in to both before saving. If nothing works, the settings form opens with what it found, so you only fill in the rest.',
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
        a: 'In one file on your disk: %APPDATA%\\app.telexmail.desktop\\telex.db. Your settings sit next to it in settings.json. Passwords are stored separately, in Windows Credential Manager.',
      },
      {
        q: 'Can I move Telex to another PC?',
        a: 'Yes. Settings › Import and export saves your settings, rules, identities and signatures, and your accounts’ server settings to one file. It never holds passwords. Importing it shows what would change, part by part, before anything does; accounts that aren’t set up yet open the setup form filled in, so you only type the password.',
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
        q: 'Is Telex available in Polish?',
        a: 'Yes. The interface speaks English and Polish and follows the Windows display language; if Windows is in English but set to Poland, Telex offers Polish on first start. You can switch any time in Settings › Language.',
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
  'TYPE YOUR ADDRESS. TELEX FINDS THE SERVERS.',
  'IMAP SYNC, SMTP SENDING, SAFE HTML MAIL AND RULES WORK TODAY',
  'NEXT: GMAIL AND OUTLOOK.COM SIGN-IN',
  'EIGHT THEMES, ALL WCAG 2.2. ENGLISH AND POLISH.',
  'TRY: R = REPLY, F = FLAG, / = FILTER',
  'NNNN',
];
