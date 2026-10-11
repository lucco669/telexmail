// Page copy that changes as the app grows: features, keys, screenshots, versions, roadmap, questions.
// Keep claims true to the current build (see CLAUDE.md, "Current state"). No imports: scripts/screens.mjs
// loads this file directly.

/**
 * The version being finished. What is marked `since: '0.5.0'` is built but not released yet: the site
 * says "Coming in 0.5" until GitHub has a release this new, then "New in 0.5" (lib/versions.ts), with
 * no redeploy. After the release, set NEXT to the version after it.
 */
export const NEXT = '0.9.0';

/** What NEXT brings, in a few words: the hero's pill and the footer's tape (capitals there). */
export const nextTeaser = 'Move in from Thunderbird and Outlook, spelling, away replies.';

/** The six points on the home page: short, the features page has the detail. */
export const features = [
  {
    title: 'Dense by default',
    text: 'Twenty-pixel rows and a condensed face: a screen holds thirty messages, not twelve. Want more air? Switch to Comfortable.',
  },
  {
    title: 'One key per action',
    text: 'Reply, flag, move, delete, undo, search, read on to the next unread: one key each, for one message or fifty. The mouse is optional.',
  },
  {
    title: 'Your mail, on your disk',
    text: 'Messages are kept whole in a local mail base, so reading, search and attachments work offline. No Telex server in between.',
  },
  {
    title: 'Private by default',
    text: 'No account and no telemetry. Remote images stay blocked until you allow them, and passwords stay in your system’s password store.',
  },
  {
    title: 'Light on your PC',
    text: 'A Rust core in the web view your system already has. No Electron, no bundled browser, an installer of about 4 MB.',
  },
  {
    title: 'Fluent in Polish',
    text: 'The whole interface in Polish or English. Windows-1250 mail decodes correctly, search ignores diacritics, replies quote “Dnia … napisał(a):”.',
  },
];

/** The strip under the hero's screenshot: what you get, in a few words each. */
export const facts = ['Offline search', 'Several accounts', 'Undo send', 'Rules with notes', 'New mail at once', 'English and Polish', 'No Electron', 'No telemetry'];

/**
 * Which mail works, beside the download buttons. Keep it true to crates/telex-core/src/autoconfig.rs:
 * Gmail is on its provider list with the app-password tip; Outlook.com and Microsoft 365 are
 * `unsupported: MicrosoftSignIn` until OAuth2 sign-in lands (roadmap step 5).
 */
export const compat = {
  works: 'Works with any IMAP/SMTP mail, Gmail included (with an app password).',
  later: 'Outlook.com and Microsoft 365: coming with Microsoft sign-in.',
};

/** The download page's "Which mail works" lists. */
export type ProviderGroup = { title: string; text: string; link?: { href: string; label: string } };
export const providers: Record<'works' | 'appPassword' | 'later', ProviderGroup> = {
  works: {
    title: 'Works today',
    text: 'Any provider with IMAP and SMTP over SSL/TLS or STARTTLS and a password: your own domain or hosting mailbox, Zoho, GMX, WEB.DE, mail.com, OVHcloud, WP, o2, Onet, Interia and many more. For some of them you turn on IMAP in their settings first; Telex says so.',
  },
  appPassword: {
    title: 'Works with an app password',
    text: 'Gmail and Google Workspace, iCloud, Yahoo, AOL, Fastmail, Yandex, Mail.ru. These take an app password made for mail programs instead of your usual one, and Telex tells you when you add the account. Google offers app passwords once 2-Step Verification is on, but not with Advanced Protection, and some work or school accounts have them turned off.',
    link: { href: 'https://support.google.com/accounts/answer/185833', label: 'Google: sign in with app passwords' },
  },
  later: {
    title: 'Not yet',
    text: 'Outlook.com, Hotmail, Live and Microsoft 365 let other mail apps in only with Microsoft sign-in (OAuth2). That is the next step on the roadmap, together with signing in with Google, so Gmail won’t need an app password. Proton Mail works with other apps only through its Bridge, which Telex can’t use yet.',
  },
};

/** Beside the download buttons: what Telex does for you, true to the current build. */
export const benefits = [
  { title: 'Search all your mail offline', text: 'Every message is kept whole on your PC, attachments too.' },
  { title: 'Several accounts, one window', text: 'Each in its own color, one search across all of them.' },
  { title: 'Take it back', text: 'Z undoes a delete, a move or a discarded message, and holds sent mail for five seconds.' },
  { title: 'Nothing to sign up for', text: 'No account, no telemetry, no Telex server in between.' },
];

/** A line in a list that may belong to a version not released yet. */
export type Point = string | { text: string; since: string };

/** The features page: one row per area, each with a screenshot. */
export type FeatureRow = { id: string; screen: string; title: string; text: string; points: Point[]; since?: string };

export const featureRows: FeatureRow[] = [
  {
    id: 'reading',
    screen: 'inbox',
    title: 'A message list built for volume',
    text: 'Folders on the left, the list on top, the message below. Rows are 20 pixels tall, so a normal screen shows thirty messages, not twelve, and the list stays quick at a hundred thousand.',
    points: [
      'Sort by status, sender, subject, date or size',
      'Unread in bold, flags and attachments at a glance',
      'Dates and sizes in mono, so columns line up',
      'Resizable panes, and a Comfortable setting for more room',
      { text: 'Double-click or Enter opens a message in a window of its own, to read side by side', since: '0.6.0' },
      { text: 'Columns fit the window, so a date is never cut; drag From wider, double-click to fit the names, right-click the headings for full dates', since: '0.7.1' },
      { text: 'Click the dot to mark a message read or unread, the flag to flag it; right-click a message for everything else', since: '0.7.1' },
      { text: 'Unread at the top of the folders: the unread mail of every folder and account in one list; what you read stays until you leave', since: '0.8.1' },
      { text: 'O, or the dot above the list, shows only the unread mail of a folder', since: '0.8.1' },
      { text: 'In lists that mix accounts, every row carries its account’s color', since: '0.8.1' },
    ],
  },
  {
    id: 'read',
    screen: 'menu',
    title: 'Read on your terms',
    text: 'Space reads on: it pages down the message, then opens the next unread one, and after the last it takes you to the next folder with new mail. Mail counts as read when you say so, and everything you can do with a message is one right-click away.',
    since: '0.7.1',
    points: [
      'Settings › Reading: read at once, after a moment you choose, only when opened in its own window, or only when you mark it',
      'Click the dot at the start of a row for read or unread, the flag next to it to flag; the envelope above a message does the same',
      'Right-click a message (or Shift+F10) for reply, move, flag, read, the conversation and a rule, each with its key',
      'Shift+Space pages back, then goes to the previous unread',
    ],
  },
  {
    id: 'many',
    screen: 'select',
    title: 'Fifty messages, one key',
    text: 'Select with Shift and the arrows, Shift-click a range, Ctrl-click one more, or Ctrl+A for the whole list. Then F, U, Del and M work on all of them, and Z undoes the move.',
    since: '0.5.0',
    points: [
      'M moves the selection: type part of a folder’s name, diacritics ignored',
      'G goes to a folder the same way; folders you used last come first',
      'Shift+U marks a whole folder read',
      'The server gets one command per change, not one per message',
      { text: 'Or drag them onto a folder: it fills up before it takes them, so nothing moves by accident', since: '0.6.0' },
    ],
  },
  {
    id: 'conversations',
    screen: 'thread',
    title: 'The whole conversation, one key away',
    text: 'Press T on any message to see everything it answers and everything that answers it: your replies in Sent, the client’s mail you filed in Clients, the draft you haven’t finished. T or Esc goes back.',
    since: '0.5.0',
    points: [
      'Followed by Message-ID, In-Reply-To and References, however deep',
      'Across the account’s folders, leaving out Spam and Trash',
      'Enter opens a message in its own folder',
    ],
  },
  {
    id: 'local',
    screen: 'local',
    title: 'Mail that lives on your PC',
    text: 'Folders on this PC keep mail only on your disk, off the server and out of its quota: drag old mail there, press M, or let a rule file it. Bring in mail from Thunderbird or The Bat! as mbox or .eml, and save any folder as mbox.',
    since: '0.6.0',
    points: [
      'Their own group under your accounts, with Archive and Trash; make, rename and remove your own folders right in the tree',
      'Moving mail there removes it from the server; moving it back uploads it, read state and flag included',
      'Searchable offline like everything else, and part of conversations (T)',
      'Rules can file mail from any account there',
      'Drop .eml and mbox files on a folder to bring the mail in; Thunderbird’s read and starred marks come along',
      { text: 'Move in from Thunderbird or Outlook: their local folders and .pst files come over folder by folder, with their accounts and filters', since: NEXT },
    ],
  },
  {
    id: 'backups',
    screen: 'backups',
    title: 'Backups you can count on',
    text: 'Mail on your PC deserves a copy elsewhere. Back up the whole mail base in one file while you work, by hand or every day or week, to a folder you choose: another disk, a USB stick, OneDrive.',
    since: '0.6.0',
    points: [
      'One consistent file with all mail, folders, rules and identities; never passwords',
      'Keeps the newest 3 to 20 backups and touches no other file',
      'Restoring keeps the mail base as it was next to it, so nothing is lost',
      'After a restore the servers’ state wins, and mail that was waiting in the Outbox moves to Drafts instead of going out twice',
      'Compact gives the space of removed mail back to the disk',
    ],
  },
  {
    id: 'writing',
    screen: 'writing',
    title: 'Plain text, or HTML when it helps',
    text: 'R opens the reply right in the reading pane, quoted and signed. Compose is plain text in a mono face, the way mail between people reads best; switch a message to HTML for bold, lists and links, and it still goes out with a plain-text copy.',
    points: [
      'Reply, reply to all and forward, each one key',
      'Plain text or HTML, as a default or per message; replies can follow the original',
      'Pasted web pages lose their styles and scripts: only formatting goes in',
      'A From picker with all your identities; the signature follows',
      'Ctrl+Enter sends, Esc saves a draft and closes',
      'E reopens a draft or a message still in the Outbox',
      { text: 'Drafts save themselves while you write; New or Reply in the middle of a message puts it in Drafts first', since: '0.7.2' },
      { text: 'Discard can be undone: Z or the Undo button brings the message back', since: '0.7.2' },
      { text: 'Spelling in your languages, Polish and English together: right-click a word or press F7 for suggestions', since: NEXT },
    ],
  },
  {
    id: 'attachments',
    screen: 'attach',
    title: 'Attachments, Bcc and the right address',
    text: 'Click an attachment to open it in its app, or save one or all of them. Attach files with Attach… or drop them on the window. Telex suggests addresses from your own mail as you type.',
    since: '0.5.0',
    points: [
      'Programs, scripts, shortcuts and macro documents ask before they open; anything in Spam too',
      'Opened files carry the Mark of the Web, so Windows treats them with care',
      'Bcc behind one click; blind copies never show in what goes out',
      'Suggestions: people you write to first, then those who write most; no-reply robots left out',
      'A warning before you send more than 20 MB',
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
      'Tips when a provider needs an app password or IMAP turned on: Gmail, iCloud and Yahoo work with an app password',
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
      { text: 'Undo send: mail waits 5 seconds (or up to 30), and Z takes it back to change it', since: '0.5.0' },
      { text: 'An Undo button in the status bar counts down the seconds left', since: '0.7.2' },
      'An Outbox that retries at every check when the connection fails',
      'A copy of every sent message lands in your server’s Sent folder',
      'Forwarded attachments travel with the forward',
      { text: 'Drafts and the Outbox of every account wait at the end of the folder list, only while they hold something; what didn’t go shows in red', since: '0.8.0' },
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
    title: 'Two-way sync, and new mail at once',
    text: 'Reading, flagging, moving and deleting happen on your PC at once and reach the server right after. Changes wait in a queue that survives restarts and lost connections.',
    points: [
      { text: 'New mail shows within seconds: one IMAP IDLE connection per Inbox', since: '0.5.0' },
      { text: 'A notification names the sender while Telex is in the background', since: '0.5.0' },
      'IMAP over SSL/TLS, folders straight from the server',
      { text: 'IMAP over STARTTLS on port 143 too, never unencrypted', since: '0.5.0' },
      'Inbox, Sent, Trash and others found by their flags or by English and Polish names',
      { text: 'Sent is the folder that holds your sent mail, even next to a server’s own empty one; or choose it, and Drafts, Archive, Spam and Trash, in Settings', since: '0.8.0' },
      'The first sync takes the newest 300 messages of each folder',
      { text: 'Older mail on request, newest first, a thousand at a time', since: '0.5.0' },
      { text: 'Every account checked at the same time, so several take about as long as one', since: '0.7.2' },
      { text: 'An account whose last check failed says so in the folder tree and the status bar; one click tries again', since: '0.7.2' },
    ],
  },
  {
    id: 'search',
    screen: 'suggest',
    title: 'Search that works offline',
    text: 'Press / and type. Senders and subjects match as you type; the text of every message matches through full-text search in the local mail base. Tab looks in all your mail at once.',
    points: [
      'No server round trip: results come from your disk',
      'Ignores diacritics: “krakow” finds “Kraków”',
      {
        text: 'Operators: from: to: subject: is:unread is:flagged has:file after:2026-09 before: larger:5m smaller:',
        since: '0.5.0',
      },
      'Esc clears the filter and puts you back in the list',
      { text: 'Suggestions as you type: people from your own mail, operators, your recent searches; Enter goes to the results', since: '0.7.1' },
      { text: 'What matched is marked in the list', since: '0.7.1' },
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
      'Your password stays in your system’s password store',
      'Your mail base is one file on your disk',
      'In a dark theme, HTML mail takes the theme’s colors; one click shows it on white',
    ],
  },
  {
    id: 'usage',
    screen: 'usage',
    title: 'See where your mail goes',
    text: 'A quiet bar in the status bar shows how full the open account is on the server. Settings › Usage shows each account the way a phone shows its storage: what takes the space, and how mail comes and goes.',
    points: [
      'The server’s storage, when it reports a quota, with a warning before it fills up',
      { text: 'The status bar’s storage bar fills in the account’s own color and says how much room is left', since: '0.6.0' },
      'This PC by kind of folder: Inbox, Sent, your folders, Spam and Trash',
      'Mail received, sent and caught as spam, day by day for 30 days',
      'Who sends the most, and the largest messages, one click away',
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
      { text: 'Start a rule from the selected message: “New rule for mail from …”', since: '0.5.0' },
      'Auto-replies skip mailing lists and robots, and answer each sender once in 4 days',
      { text: 'An away reply per account, kept on your server where it can be, so it answers while Telex is closed', since: NEXT },
    ],
  },
  {
    id: 'settings',
    screen: 'settings',
    title: 'Your colors, your language',
    text: 'Eight themes, from plain light to a green phosphor screen, each measured against WCAG 2.2. The interface speaks English and Polish. Settings save as you change them, and travel to another PC in one file.',
    points: [
      'Every theme meets AA contrast; the two high-contrast ones meet AAA',
      { text: 'Updates checked when Telex starts, once a day, or only when you ask; never before you say yes', since: '0.7.1' },
      'English or Polish, as in Windows or as you pick, down to the error messages',
      'Accounts, identities and servers in the same window',
      { text: 'Each account’s color as a line down its folders, a dot, its initials or a tint', since: '0.8.0' },
      { text: 'Five fonts: IBM Plex, your system’s own, Classic (Tahoma), Inter, and Atkinson Hyperlegible for low vision', since: '0.8.1' },
      { text: 'Put your accounts in the order you like, and keep any folder in view', since: '0.7.1' },
      { text: 'Choose when mail counts as read: at once, after the time you set, only in its own window, or only when you mark it', since: '0.7.1' },
      'Text from 90% to 200% with Ctrl+= and Ctrl+−',
      'Export settings, rules, identities and servers to a file, never passwords',
      'Import shows what would change, part by part, before anything does',
    ],
  },
];

/** Honest gaps, shown on the features page. */
export const notYet = [
  'Outlook.com, Hotmail and Microsoft 365 accept only Microsoft sign-in (OAuth2), which isn’t built yet. Gmail works today with an app password; signing in with Google comes in the same step.',
  'HTML mail keeps its inline styling, but style sheets inside a message (<style> blocks) are dropped for now.',
  'Drafts stay on your PC; they aren’t saved to the server’s Drafts folder yet.',
  'HTML compose has no images or colors yet, and signatures are plain text.',
  'There is no Mac version, and the Windows installer isn’t code-signed yet, so SmartScreen warns once.',
];

// `match` lists the KeyboardEvent combinations that light the row up in the keyboard table. `shift`
// set to false means the row doesn't light up with Shift held (so Shift+↓ finds "select", not "move").
export type Shortcut = {
  id: string;
  group: 'Move' | 'Select' | 'Write' | 'Organize' | 'Find' | 'View';
  keys: string[][];
  action: string;
  match: { key: string; shift?: boolean; ctrl?: boolean }[];
  since?: string;
  /** Shown in the short table on the home page. */
  home?: boolean;
};

export const shortcuts: Shortcut[] = [
  { id: 'move', group: 'Move', keys: [['↑'], ['↓']], action: 'Previous / next message', match: [{ key: 'ArrowUp', shift: false }, { key: 'ArrowDown', shift: false }], home: true },
  { id: 'space', group: 'Move', keys: [['Space']], action: 'Read on: page down, then the next unread message (Shift: back)', match: [{ key: ' ' }], since: '0.7.1', home: true },
  { id: 'ends', group: 'Move', keys: [['Home'], ['End']], action: 'First / last message', match: [{ key: 'Home', shift: false }, { key: 'End', shift: false }] },
  { id: 'thread', group: 'Move', keys: [['T']], action: 'The message’s conversation (T or Esc goes back)', match: [{ key: 't' }], since: '0.5.0', home: true },
  { id: 'goto', group: 'Move', keys: [['G']], action: 'Go to a folder: type part of its name', match: [{ key: 'g' }], since: '0.5.0' },
  { id: 'window', group: 'Move', keys: [['Enter']], action: 'The message in a window of its own (or double-click it); Esc closes it', match: [{ key: 'Enter' }], since: '0.6.0' },
  { id: 'open', group: 'Move', keys: [['Enter']], action: 'Open the message’s folder (in all-mail results and conversations)', match: [{ key: 'Enter' }], since: '0.5.0' },
  { id: 'select', group: 'Select', keys: [['Shift', '↑'], ['↓']], action: 'Select more (or fewer) messages', match: [{ key: 'ArrowUp', shift: true }, { key: 'ArrowDown', shift: true }], since: '0.5.0' },
  { id: 'selectends', group: 'Select', keys: [['Shift', 'Home'], ['End']], action: 'Select to the top / bottom', match: [{ key: 'Home', shift: true }, { key: 'End', shift: true }], since: '0.5.0' },
  { id: 'selectall', group: 'Select', keys: [['Ctrl', 'A']], action: 'Select every message in the list', match: [{ key: 'a', ctrl: true }], since: '0.5.0' },
  { id: 'new', group: 'Write', keys: [['N']], action: 'New message', match: [{ key: 'n' }] },
  { id: 'reply', group: 'Write', keys: [['R']], action: 'Reply', match: [{ key: 'r' }], home: true },
  { id: 'replyall', group: 'Write', keys: [['Shift', 'R']], action: 'Reply to all', match: [{ key: 'R', shift: true }] },
  { id: 'forward', group: 'Write', keys: [['Shift', 'F']], action: 'Forward', match: [{ key: 'F', shift: true }] },
  { id: 'edit', group: 'Write', keys: [['E']], action: 'Edit a draft or an unsent message', match: [{ key: 'e' }] },
  { id: 'send', group: 'Write', keys: [['Ctrl', 'Enter']], action: 'Send from compose', match: [{ key: 'Enter', ctrl: true }] },
  { id: 'spell', group: 'Write', keys: [['F7']], action: 'The next spelling mistake, with suggestions (compose)', match: [{ key: 'F7' }], since: NEXT },
  { id: 'format', group: 'Write', keys: [['Ctrl', 'B'], ['I'], ['U'], ['K']], action: 'Bold / italic / underline / link (HTML compose)', match: [{ key: 'b', ctrl: true }, { key: 'i', ctrl: true }, { key: 'u', ctrl: true }, { key: 'k', ctrl: true }] },
  { id: 'flag', group: 'Organize', keys: [['F']], action: 'Flag / unflag (the whole selection)', match: [{ key: 'f' }] },
  { id: 'unread', group: 'Organize', keys: [['U']], action: 'Toggle unread (or click the dot in the row)', match: [{ key: 'u' }] },
  { id: 'folderread', group: 'Organize', keys: [['Shift', 'U']], action: 'Mark everything in the folder read', match: [{ key: 'U', shift: true }], since: '0.5.0' },
  { id: 'delete', group: 'Organize', keys: [['Del']], action: 'Move to Trash (in Trash: two separate presses delete for good)', match: [{ key: 'Delete' }], home: true },
  { id: 'moveto', group: 'Organize', keys: [['M']], action: 'Move to a folder: type part of its name', match: [{ key: 'm' }], since: '0.5.0', home: true },
  { id: 'undo', group: 'Organize', keys: [['Z']], action: 'Undo the last delete, move or discard · take back what you just sent', match: [{ key: 'z' }], home: true },
  { id: 'filter', group: 'Find', keys: [['/']], action: 'Search, with from: is:unread has:file …', match: [{ key: '/' }], home: true },
  { id: 'onlyunread', group: 'Find', keys: [['O']], action: 'Only the unread mail of the list (O again: all of it)', match: [{ key: 'o' }], since: '0.8.1' },
  { id: 'scope', group: 'Find', keys: [['Tab']], action: 'In the filter: this folder / all mail', match: [{ key: 'Tab', shift: false }] },
  { id: 'view', group: 'Find', keys: [['H']], action: 'HTML / plain text view', match: [{ key: 'h' }] },
  { id: 'esc', group: 'Find', keys: [['Esc']], action: 'Clear the filter, the selection or the conversation · save draft and close', match: [{ key: 'Escape' }] },
  { id: 'settings', group: 'View', keys: [['Ctrl', ',']], action: 'Settings', match: [{ key: ',', ctrl: true }] },
  { id: 'larger', group: 'View', keys: [['Ctrl', '='], ['−']], action: 'Larger / smaller text', match: [{ key: '=', ctrl: true }, { key: '+', ctrl: true }, { key: '-', ctrl: true }] },
  { id: 'normal', group: 'View', keys: [['Ctrl', '0']], action: 'Normal text size', match: [{ key: '0', ctrl: true }] },
  { id: 'menu', group: 'View', keys: [['Shift', 'F10']], action: 'The menu of the selected message (or right-click it)', match: [{ key: 'F10', shift: true }], since: '0.7.1' },
  { id: 'help', group: 'View', keys: [['?'], ['F1']], action: 'Every key, and the tips that aren’t obvious (or hover the ? in the status bar)', match: [{ key: '?' }, { key: 'F1' }], since: '0.6.0' },
];

export type Tip = { title: string; text: string; since?: string };

export const keyboardTips: Tip[] = [
  {
    title: 'Arrow past without reading',
    text: 'A message counts as read after it has been on screen for a moment, so moving through the list with ↑ ↓ doesn’t mark everything you pass. Settings › Reading says how long, or leaves it to you.',
  },
  {
    title: 'Read without the mouse',
    text: 'Space pages down the message and, at its end, opens the next unread one. After the last, it names the next folder with new mail; Space again goes there.',
    since: '0.7.1',
  },
  {
    title: 'Only what’s new',
    text: 'Unread at the top of the folders gathers the unread mail of every account, wherever rules filed it; O shows a folder’s alone. What you read stays in the list until you leave it.',
    since: '0.8.1',
  },
  {
    title: 'Unread until you say so',
    text: 'Settings › Reading can leave mail unread until you mark it: U, the dot at the start of a row, or the envelope above the message.',
    since: '0.7.1',
  },
  {
    title: 'Clear a pile in four keys',
    text: 'Shift+↓ a few times, then M, three letters of the folder’s name and Enter. F, U and Del work on the selection the same way.',
    since: '0.5.0',
  },
  {
    title: 'Delete without fear',
    text: 'Del moves a message to Trash and Z brings it back; Z undoes a move too. In Trash, Del asks for a second press before it deletes for good.',
  },
  {
    title: 'Take it back',
    text: 'After Ctrl+Enter, a message waits five seconds before it goes. Z in that moment brings it back into compose, untouched.',
    since: '0.5.0',
  },
  {
    title: 'Drag without accidents',
    text: 'Drag messages onto a folder and rest there a moment: the folder fills up, then takes them. Let go early, or press Esc, and nothing moves; Z undoes the rest.',
    since: '0.6.0',
  },
  {
    title: 'Never lose a draft',
    text: 'Esc in compose saves a draft and closes it. E opens it again, and also reopens a message that is still waiting in the Outbox.',
  },
  {
    title: 'Search in two keys',
    text: 'Press / and type a few letters; the list narrows as you type and marks what matched. Pick a person or an operator from under the box, Enter for the results, Tab for all mail, Esc to go back.',
  },
];

// Images come from `npm run screens` (public/screens/<id>-light.webp and <id>-dark.webp).
export type Screen = { id: string; label: string; title: string; caption: string; since?: string };

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
    id: 'menu',
    label: 'Message menu',
    title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'Right-click a message for everything you can do with it, each with its key. The dot and the flag at the start of a row switch read and flag with a click.',
    since: '0.7.1',
  },
  {
    id: 'unread',
    label: 'Unread',
    title: 'Unread · All accounts – Telex',
    caption: 'Unread at the top of the folders: new mail from every folder and account in one list, each row with its account’s line.',
    since: '0.8.1',
  },
  {
    id: 'thread',
    label: 'Conversation',
    title: 'Conversation · sam@lindenstudio.com – Telex',
    caption: 'T shows the whole conversation: the enquiry filed in Clients, your estimate in Sent, the reply in the Inbox, the answer still in the Outbox.',
    since: '0.5.0',
  },
  {
    id: 'select',
    label: 'Move many',
    title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'Three messages selected with Shift+↓, then M: type part of a folder’s name and press Enter.',
    since: '0.5.0',
  },
  {
    id: 'attach',
    label: 'Attachments',
    title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'A new message with two files attached and Bcc open. Addresses are suggested from your own mail as you type.',
    since: '0.5.0',
  },
  {
    id: 'local',
    label: 'On this PC',
    title: 'Clients 2025 · On this PC – Telex',
    caption: 'Folders on this PC keep mail off the server for good. Right-click one to rename it, save it as mbox or bring mail in from files.',
    since: '0.6.0',
  },
  {
    id: 'backups',
    label: 'Backups',
    title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'Settings › Backups: the mail base in one file, by hand or every day or week, and a restore that keeps what was there.',
    since: '0.6.0',
  },
  {
    id: 'writing',
    label: 'HTML compose',
    title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'A message in HTML: bold, lists, quotes and links from a small bar or the usual keys. It goes out with a plain-text copy.',
  },
  {
    id: 'suggest',
    label: 'Search',
    title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'Search keeps your recent searches and offers the operators; as you type, it suggests people from your own mail. Enter goes to the results.',
    since: '0.7.1',
  },
  {
    id: 'filter',
    label: 'Found',
    title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'Senders and subjects match as you type, message text through full-text search, and what matched is marked: “krakow” finds “Kraków”.',
  },
  {
    id: 'operators',
    label: 'Operators',
    title: 'All mail · sam@lindenstudio.com – Telex',
    caption: 'Operators in search, across all mail: every message with a file since September, with the folder it is in.',
    since: '0.5.0',
  },
  {
    id: 'images',
    label: 'HTML mail',
    title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'HTML mail in a sandbox. Remote images stay off until you load them, so a sender can’t tell when you opened the message.',
  },
  {
    id: 'reading',
    label: 'Reading',
    title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'Settings › Reading: mail counts as read at once, after the moment you choose, only in its own window, or only when you mark it.',
    since: '0.7.1',
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
    id: 'accounts',
    label: 'Accounts',
    title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'Every account in Settings: its servers, how its last check went, its identities.',
  },
  {
    id: 'usage',
    label: 'Usage',
    title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'Settings › Usage: the server’s storage, what takes the space on this PC, and 30 days of mail in and out.',
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

/**
 * Three things to try first: above the live demo, and on the home page's demo band. The `keys` come
 * first, then `text`, where [Tab] is drawn as a key. Keep them true to scripts/demo-mailbox.json and
 * demo/backend.js.
 */
export const demoFirst: { keys: string[]; text: string }[] = [
  { keys: ['/'], text: 'and type from:anna: the list narrows as you type, and [Tab] looks in all her mail.' },
  { keys: ['T'], text: 'on Daniel Price’s “Re: Online shop estimate”: the whole conversation, from four folders.' },
  { keys: ['Del'], text: 'moves a message to Trash, and [Z] brings it back.' },
];

/** The cards under the live demo. Keep them true to scripts/demo-mailbox.json and demo/backend.js. */
export const demoTries: { keys: string[]; title: string; text: string }[] = [
  {
    keys: ['↓'],
    title: 'Read your way down',
    text: 'Arrow through the Inbox. A message counts as read after a moment on screen, not as you pass it.',
  },
  {
    keys: ['Space'],
    title: 'Read on with Space',
    text: 'Space pages down a message, then opens the next unread. After the Inbox’s last one, Space again goes to the next folder with new mail.',
  },
  {
    keys: ['Right-click'],
    title: 'Everything on one menu',
    text: 'Right-click a message for reply, move, flag, read or unread and more. Right-click the column headings for full dates; drag the edge before Subject.',
  },
  {
    keys: ['O'],
    title: 'Only what’s new',
    text: 'O shows only the Inbox’s unread mail; Unread at the top of the folders has both accounts’. Read one: it stays until you leave.',
  },
  {
    keys: ['T'],
    title: 'Follow a conversation',
    text: 'On Daniel Price’s “Re: Online shop estimate”: his enquiry filed in Clients, your estimate in Sent, his reply. Esc goes back.',
  },
  {
    keys: ['Shift+↓', 'M'],
    title: 'Move a pile',
    text: 'Select a few messages, press M, type “arch” and Enter: they go to the Archive. Z puts them back.',
  },
  {
    keys: ['Drag'],
    title: 'Drag to a folder',
    text: 'Drag a message onto Clients 2025 under On this PC and rest a moment: the folder fills, then takes it. Z puts it back.',
  },
  {
    keys: ['?'],
    title: 'Every key at hand',
    text: 'Hover the ? at the right of the status bar, or press ?, for every key and the tricks that aren’t obvious.',
  },
  {
    keys: ['/'],
    title: 'Search with suggestions',
    text: 'Type “ko” and pick Anna from the list, or has:file after:2026-09. What matched is marked; Tab looks in all mail, Esc goes back.',
  },
  {
    keys: ['R', 'Ctrl+Enter'],
    title: 'Send, and take it back',
    text: 'Write a line and send. It waits five seconds in the Outbox: Z in that moment brings it back into compose.',
  },
  {
    keys: ['Get mail'],
    title: 'Get new mail',
    text: 'Each press brings a message: a reply that joins its conversation, an invoice the Invoices rule files away with a note. Wait a minute and mail arrives by itself, as with IMAP IDLE.',
  },
  {
    keys: ['H'],
    title: 'HTML or plain text',
    text: 'On the Frontend Weekly newsletter: H switches views, Load images fills in the pictures (stand-ins here), the sun shows it on white.',
  },
  {
    keys: ['Ctrl+,'],
    title: 'Make it yours',
    text: 'Eight themes from Paper to Phosphor, five fonts, Polish, bigger text, comfortable rows. Rules and identities can be edited too.',
  },
];

/** The tabs on the home page: a tour, not the whole gallery. */
export const homeScreens = ['unread', 'menu', 'reply', 'thread', 'select', 'suggest', 'local', 'attach', 'images', 'rules', 'settings', 'polish'].map(
  (id) => screens.find((s) => s.id === id)!,
);

/** Every release, newest first, as its tag message tells it. Dates and files come from GitHub. */
export type Version = {
  version: string;
  title: string;
  date: string;
  summary: string;
  points: string[];
  upgrade?: string;
  /** The cards on the home page while this is the newest version. */
  highlights?: { title: string; text: string; keys?: string }[];
};

export const versions: Version[] = [
  {
    version: NEXT,
    title: 'Move in from Thunderbird and Outlook, spelling, away replies',
    date: '2026-10-11',
    summary: 'Everything from your old mail program in one go, spelling in the languages you write, and an automatic reply while you’re away.',
    points: [
      'Move in from another mail program: Telex finds Thunderbird (and Betterbird, SeaMonkey, Postbox) and Outlook on your PC by itself. Thunderbird brings its accounts with their identities and signatures, the mail it keeps on your PC and its filters, as rules where Telex can do the same; Outlook brings its IMAP accounts and its .pst data files, folder by folder. You type each password once; mail on your servers is downloaded from them, not copied. Nothing comes twice if you do it again.',
      'Spelling in the languages you write: Windows’ own dictionaries, and for a language Windows lacks, LibreOffice’s, fetched only when you click Download and checked before use. Wavy lines in plain text and HTML; right-click a word or press F7 for suggestions, Add to dictionary or Ignore. Quotes, links and addresses are left alone.',
      'An away reply for each account (Settings › Accounts › Auto-reply): on your server where it can hold one (ManageSieve), so it answers while Telex is closed and your own server filters stay; elsewhere Telex answers when it checks. Mailing lists, robots and Spam never get it, and each person gets it once in the days you choose.',
      'Mail files: Outlook’s .msg and Apple Mail’s .emlx come in too, and Maildir and MH folders; messages Thunderbird deleted but hadn’t compacted stay out.',
      'Safer and quicker underneath: images you load come only from public addresses, checked where Telex connects, four at a time; one Telex runs at a time, so mail can’t go out twice; the window never waits on mail work; and if the mail base can’t be opened, Telex says so instead of starting empty.',
    ],
    upgrade: 'The mail base gets two new tables (the away replies); older versions can’t open it afterwards.',
    highlights: [
      { title: 'Move in', text: 'Thunderbird’s and Outlook’s accounts, local mail and filters in one go.' },
      { title: 'Spelling', keys: 'F7', text: 'Windows’ dictionaries or LibreOffice’s, with suggestions on a right-click.' },
      { title: 'Away replies', text: 'On your server where it can be, so it answers while Telex is closed.' },
    ],
  },
  {
    version: '0.8.1',
    title: 'Unread at hand, accounts on every row, five fonts',
    date: '2026-10-11',
    summary: 'One list of everything unread, one key for only the unread mail of a folder, each account’s color on its rows, and fonts to choose from.',
    points: [
      'Unread, at the top of the folder list: the unread mail of every folder and account in one list, wherever rules filed it. Mail you read there stays until you leave, so nothing jumps away under you.',
      'O, the dot above the list’s dots, or “3 unread” in the status bar: only the unread mail of the open folder. O again, or Esc, shows everything. G offers Unread too.',
      'Lists that mix accounts (Unread, all mail, every account’s drafts) mark each row with its account the way the folder list does: a line at its start, a tint, a dot or its initials.',
      'Settings › Appearance › Font: IBM Plex, your system’s own (Segoe UI and Cascadia Mono), Classic (Tahoma and Consolas), Inter with JetBrains Mono, or Atkinson Hyperlegible, made for low vision. Each sets the window, the list, and the mono face for dates and writing.',
    ],
    highlights: [
      { title: 'Unread at hand', keys: 'O', text: 'Everything unread from every folder in one list, or just a folder’s with one key.' },
      { title: 'Accounts on every row', text: 'Unread and all mail show each message’s account as a line, a tint, a dot or initials.' },
      { title: 'Five fonts', text: 'IBM Plex, your system’s, Classic Tahoma, Inter, or Atkinson Hyperlegible.' },
    ],
  },
  {
    version: '0.8.0',
    title: 'Sent where your sent mail is, a calmer folder list',
    date: '2026-10-11',
    summary: 'Telex finds the folder that really holds your sent mail, keeps Drafts and the Outbox out of the way until they matter, and lets you choose how accounts are told apart.',
    points: [
      'Sent is the folder that holds your sent mail: next to a server’s own empty “Sent”, the “Wysłane” another program filled wins, and stays Sent from then on.',
      'Settings › Accounts › Folders: choose which server folder is Sent, Drafts, Archive, Spam and Trash; right-click any folder for “Use for sent mail”. Your choice stays.',
      'Drafts and the Outbox of every account wait at the end of the folder list as one list each, and only while they hold something; mail that didn’t go shows its count in red, also in the status bar.',
      'Sent is quieter than the Inbox in the folder list, and folders on this PC show Archive and Trash only when they hold mail.',
      'Each account’s color as a line down its folders, a dot, its initials or a tint, or not at all: Settings › Appearance.',
      'Slim scrollbars in each theme’s colors, here and on this site.',
    ],
    upgrade: 'The mail base gets one new column (which folder you chose for what); older versions can’t open it afterwards.',
    highlights: [
      { title: 'The right Sent', text: 'The folder that holds your sent mail, whatever the server calls it; or the one you choose.' },
      { title: 'Unsent at the end', text: 'Drafts and the Outbox of every account, one list each, only while they hold something.' },
      { title: 'Accounts in color', text: 'A line down each account’s folders, or a dot, its initials, a tint: your pick.' },
    ],
  },
  {
    version: '0.7.2',
    title: 'Faster checks, drafts that keep, an Undo you can see',
    date: '2026-10-11',
    summary: 'Mail checks take seconds, nothing you write gets lost, and a message opens in its own window every time.',
    points: [
      'Opening a message in a window of its own (double-click or Enter) could freeze Telex on Windows; it opens at once now.',
      'Checking mail is much faster: every account is checked at the same time, and server commands no longer wait on the network between them. Four accounts on a server across the Atlantic went from about 19 seconds to about 2.',
      'Compose saves the message as a draft every few seconds while you write, so a crash loses little; a draft you are editing stays as it was until you save or send.',
      'New, Reply or E while you are writing puts that message in Drafts first, instead of dropping it.',
      'Discard can be undone: Z, or the Undo button, brings the message back.',
      'An Undo button in the status bar whenever Z can undo something; for sent mail it counts down the seconds left.',
      'An account whose last check failed shows a mark in the folder tree and a chip in the status bar with the reason; click it to try again.',
      'Deleting for good in Trash takes two separate presses of Del: holding the key or a double-click no longer counts as both.',
    ],
    highlights: [
      { title: 'Checks in seconds', text: 'Every account at once, and no waiting between commands: four accounts in about two seconds.' },
      { title: 'Drafts that keep', text: 'Saved as you write; New or Reply mid-message files it in Drafts, and Z undoes Discard.' },
      { title: 'Undo you can see', keys: 'Z', text: 'A button in the status bar, counting down while sent mail waits to go.' },
    ],
  },
  {
    version: '0.7.1',
    title: 'Space for the next unread, a list that fits, search that suggests',
    date: '2026-10-11',
    summary: 'Read on with one key, decide when mail counts as read, find things faster, and put the window in your order.',
    points: [
      'Space reads on: it pages down the message, then opens the next unread one, and after the last offers the next folder with new mail. Shift+Space goes back.',
      'Settings › Reading: mail counts as read at once, after a moment you choose (up to 10 seconds), only in its own window, or only when you mark it.',
      'Click the dot at the start of a row to mark it read or unread, the flag next to it to flag; the envelope above a message does the same.',
      'Right-click a message (or Shift+F10) for everything you can do with it, each with its key.',
      'The list’s columns fit the window, so a date is never cut. Drag the edge before Subject to set From’s width, double-click it to fit the names; right-click the headings for full dates or no Size column.',
      'Search suggests as you type: people from your own mail, operators, your recent searches. Enter goes to the results, and what matched is marked.',
      'Put your accounts in your order (right-click › Move up, Alt+↑ ↓, or Settings › Accounts), and keep any folder in view outside “more”.',
      'Updates can be checked each time Telex starts, as well as once a day; still never before you say yes.',
      'The status bar keeps its keys in the middle, and the list scrolls with less work.',
    ],
    highlights: [
      { title: 'Next unread', keys: 'Space', text: 'Read on with one key: down the message, then to the next unread, folder after folder.' },
      { title: 'Read when you say', text: 'At once, after the moment you pick, only in its own window, or only when you mark it.' },
      { title: 'One right-click', text: 'Everything you can do with a message, each with its key; the dot and the flag switch with a click.' },
      { title: 'Columns that fit', text: 'Dates never cut, From as wide as you drag it, full dates on request.' },
      { title: 'Search that suggests', keys: '/', text: 'People, operators and recent searches under the box; what matched is marked.' },
      { title: 'Your order', keys: 'Alt+↑', text: 'Accounts in the order you like, and the folders you use kept in view.' },
    ],
  },
  {
    version: '0.6.0',
    title: 'Folders on this PC, backups and drag and drop',
    date: '2026-10-11',
    summary: 'Keep mail for good on your own disk, back it all up in one file, and move mail with the mouse as safely as with M.',
    points: [
      'Folders on this PC: their own group under the accounts, with Archive and Trash. Mail moved there leaves the server and its quota; moved back, it is uploaded with its flags.',
      'Make, rename and remove those folders right in the tree; rules can file mail from any account there.',
      'Save any folder as an mbox file; bring in .eml and mbox files from Thunderbird or The Bat!, from Open or dropped on a folder.',
      'Settings › Backups: the mail base in one file while you work, by hand or every day or week, keeping the newest few. Restore keeps the old mail base next to it, and Compact gives space back.',
      'Drag messages onto a folder: it fills up before it takes them, Esc stops, Z undoes. A closed account or “more” opens when you rest on it.',
      'Double-click or Enter opens a message in a window of its own.',
      'The ? in the status bar shows every key and the tips that aren’t obvious.',
      'A cleaner message header in one face, and a storage bar in each account’s color with the room left.',
    ],
    upgrade: 'This version upgrades the mail base when it first starts; Telex 0.5 can’t open it afterwards.',
    highlights: [
      { title: 'On this PC', text: 'Folders kept only on your disk: archive old mail and free the server’s quota.' },
      { title: 'Backups', text: 'The whole mail base in one file, every day or week, and a restore that keeps what was there.' },
      { title: 'Drag and drop', text: 'Drag messages onto a folder; it fills before it takes them, so nothing moves by accident.' },
      { title: 'mbox and .eml', text: 'Bring mail in from Thunderbird or The Bat!, and save any folder as mbox.' },
      { title: 'Its own window', keys: 'Enter', text: 'Double-click a message to read it in a window of its own.' },
      { title: 'Every key', keys: '?', text: 'Hover the ? in the status bar for every key and tip.' },
    ],
  },
  {
    version: '0.5.0',
    title: 'Attachments, conversations and mail at once',
    date: '2026-10-10',
    summary: 'The everyday tools: files in and out, many messages at a time, whole conversations, and new mail the moment it lands.',
    points: [
      'Attachments open in their app with a click; programs, scripts and anything in Spam ask first. Save one, or all of them into a folder.',
      'Attach files with Attach… or by dropping them on the window. Bcc, and address suggestions in To, Cc and Bcc from your own mail.',
      'Several messages at once: Shift+arrows, Shift+click, Ctrl+click, Ctrl+A. F, U, Del, M and Z work on all of them.',
      'M moves the selection to a folder and G goes to one: type part of the name. Shift+U marks a folder read.',
      'T shows a message’s whole conversation, across the account’s folders.',
      'Undo send: sent mail waits 5 seconds in the Outbox, and Z takes it back.',
      'New mail at once over IMAP IDLE, with a notification while Telex is in the background.',
      'Older mail on request: the status bar says how many messages are still on the server and downloads them.',
      'Quick-filter operators: from: to: subject: is:unread is:flagged has:file after: before: larger: smaller:',
      'A rule from the selected message, and IMAP over STARTTLS on port 143.',
    ],
    upgrade: 'This version upgrades the mail base when it first starts; Telex 0.4 can’t open it afterwards.',
    highlights: [
      { title: 'Attachments', text: 'Click to open one in its app, save one or all, attach files by dropping them on the window.' },
      { title: 'Conversations', keys: 'T', text: 'Everything a message answers and everything that answers it, from every folder.' },
      { title: 'Many at once', keys: 'Shift+↓', text: 'Select with Shift, Ctrl or Ctrl+A; F, U, Del and M act on all of them.' },
      { title: 'Undo send', keys: 'Z', text: 'Sent mail waits five seconds in the Outbox. Changed your mind? Z brings it back.' },
      { title: 'New mail at once', text: 'IMAP IDLE on every Inbox, and a notification while Telex is in the background.' },
      { title: 'Filter operators', keys: '/', text: 'from: to: subject: is:unread has:file after: before: larger: smaller:' },
    ],
  },
  {
    version: '0.4.0',
    title: 'HTML compose, dark mail, usage and Linux',
    date: '2026-10-10',
    summary: 'Write in HTML when it helps, read HTML mail in a dark theme, see where your storage goes, and run Telex on Linux.',
    points: [
      'The Text | HTML switch in a message: bold, italics, lists, quotes and links (Ctrl+B, Ctrl+I, Ctrl+U, Ctrl+K). Every HTML message goes out with a plain-text copy.',
      'HTML mail follows dark themes: its colors are turned around to match, pictures stay as they are. The sun button shows a message on white.',
      'Settings › Usage: each account’s storage on the server, what takes the space on this PC, 30 days of mail in and out, who sends the most.',
      'Accounts, servers and identities in Settings › Accounts; Reading and Writing tabs.',
      'Polish down to the error messages and notes, and special folders with Polish names.',
      'Linux: an AppImage that updates itself, and a .deb. Passwords go to GNOME Keyring or KWallet.',
    ],
    upgrade: 'This version upgrades the mail base when it first starts; Telex 0.3 can’t open it afterwards.',
  },
  {
    version: '0.3.1',
    title: 'Settings, eight themes and Polish',
    date: '2026-10-10',
    summary: 'A settings window, eight themes that all meet WCAG 2.2, and the whole interface in Polish.',
    points: [
      'A Settings window (Ctrl+,): theme, language, row height, text size up to 200%, how often to check mail, when a message counts as read.',
      'Eight themes, from Light to Phosphor. Every one meets WCAG 2.2 AA contrast, the two high-contrast ones AAA.',
      'Telex speaks Polish, following the Windows display language or the one you pick.',
      'Settings, rules, identities and servers exported to one file, never passwords; importing shows what would change first.',
      'Big mailboxes stay fast: the list loads a page at a time, and the quick filter can look in all mail (Tab).',
    ],
    upgrade: 'This version upgrades the mail base when it first starts; older versions can’t open it afterwards.',
  },
  {
    version: '0.2.1',
    title: 'Easier account setup',
    date: '2026-10-10',
    summary: 'Name, address and password: Telex finds the servers itself.',
    points: [
      'Adding an account asks only for your name, email address and password. Telex finds the servers from its provider list, your domain’s setup file and DNS records, or the usual names.',
      'It logs in to IMAP and SMTP before saving; if it can’t, the full form opens filled in, with the reason.',
      'Telex starts empty and opens the setup on first start, instead of a demo mailbox.',
    ],
  },
  {
    version: '0.2.0',
    title: 'The first release that updates itself',
    date: '2026-10-10',
    summary: 'Real mail: IMAP sync both ways, SMTP sending, safe HTML mail, rules and identities.',
    points: [
      'Real IMAP accounts with two-way sync: reading, flagging, moving and deleting reach the server.',
      'Sending over SMTP, with an Outbox that retries and a copy in Sent.',
      'HTML mail in a sandbox, remote images only on request.',
      'Rules that sort, flag and answer mail, and say why on each message.',
      'Identities with their own signatures and reply headers.',
      'Updates from GitHub, signed, and only when you allow them.',
    ],
  },
];

export type Step = {
  title: string;
  text: string;
  status: 'done' | 'now' | 'next' | 'later';
  notYet?: string;
  /** Built for this version: "now" until it is released, then "done". */
  since?: string;
};

export const roadmap: Step[] = [
  {
    title: 'Real mail over IMAP',
    text: 'SSL/TLS with password login, servers found from your address, folders from the server, sync on start and every 10 minutes, offline search.',
    status: 'done',
    notYet: 'Faster flag sync for folders with hundreds of thousands of messages (CONDSTORE).',
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
    notYet: 'Drafts on the server’s Drafts folder.',
  },
  {
    title: 'HTML mail',
    text: 'Rendered in a sandbox where no script can run. Remote images stay blocked until you allow them, per message or per sender, and Telex fetches them itself.',
    status: 'done',
    notYet: 'Style sheets inside messages.',
  },
  {
    title: 'Filters',
    text: 'A Sorting Office: rules that sort, flag, move and auto-reply, run on arrival or on demand, each leaving a note on the message.',
    status: 'done',
    notYet: 'Regular expressions, conditions on headers and attachments, rules that copy, forward or run on a schedule.',
  },
  {
    title: 'Templates and identities',
    text: 'Several identities per account, each with its own signature and reply and forward templates. A reply is from the address it was sent to.',
    status: 'done',
    notYet: 'Greeting templates for new messages, HTML signatures.',
  },
  {
    title: 'Updates',
    text: 'Telex checks GitHub for a new version when it starts and once a day, if you allow it, or when you click its version number, and installs it with one click and a restart. Updates must carry Telex’s signature.',
    status: 'done',
    notYet: 'A code-signed installer, so Windows SmartScreen stops warning.',
  },
  {
    title: 'Settings, themes and Polish',
    text: 'A settings window, eight themes that all meet WCAG 2.2 AA contrast (two at AAA), text up to 200%, the whole interface in Polish, and settings, rules and identities exported to one file and imported with a preview.',
    status: 'done',
    notYet: 'More languages.',
  },
  {
    title: 'HTML compose, usage and Linux',
    text: 'Write in plain text or HTML, with a plain-text copy always going along. HTML mail takes a dark theme’s colors. A Usage page for each account. An AppImage that updates itself, and a .deb.',
    status: 'done',
    notYet: 'Images and colors in HTML compose, HTML signatures, a Mac version.',
  },
  {
    title: 'Attachments and mail at speed',
    text: 'Open, save and attach files, Bcc and address suggestions, many messages at once with M and G, conversations with T, undo send, new mail at once over IMAP IDLE, older mail on request, STARTTLS.',
    status: 'done',
    since: '0.5.0',
    notYet: 'Rules on attachments.',
  },
  {
    title: 'Archive and backup',
    text: 'Folders kept only on your PC, backups by hand or every day or week that you can restore, mbox export and .eml and mbox import, drag and drop, a message in its own window.',
    status: 'done',
    since: '0.6.0',
    notYet: 'POP3 accounts, and moving the mail base to another folder or disk.',
  },
  {
    title: 'Reading and finding',
    text: 'Space for the next unread, read rules of your own, a right-click menu, columns that fit the window, search with suggestions, accounts in your order.',
    status: 'done',
    since: '0.7.1',
    notYet: 'Saved searches as folders of their own.',
  },
  {
    title: 'Sign in with Microsoft and Google',
    text: 'OAuth2 sign-in. Outlook.com, Hotmail and Microsoft 365 let other mail apps in no other way, and Gmail, which works today with an app password, won’t need one.',
    status: 'next',
  },
];

export type Question = { id?: string; q: string; a: string };

export const faqGroups: { title: string; items: Question[] }[] = [
  {
    title: 'Accounts and mail',
    items: [
      {
        id: 'providers',
        q: 'Which mail providers work?',
        a: 'Any provider that offers IMAP (SSL/TLS on port 993, or STARTTLS on 143) and SMTP (SSL/TLS on 465, or STARTTLS on 587) with a password: your own domain or hosting mailbox, and providers such as Gmail, iCloud, Yahoo, Fastmail, Zoho, GMX, WP, Onet and Interia. Telex finds the settings from your address and tells you when a provider wants an app password (Gmail, iCloud, Yahoo) or IMAP turned on. Not yet: Outlook.com, Hotmail and Microsoft 365, which accept only Microsoft sign-in (planned), and Proton Mail, which needs its Bridge.',
      },
      {
        q: 'Can I bring my mail from Thunderbird or Outlook?',
        a: 'From version 0.9, yes. Settings › Import and export finds Thunderbird (and Betterbird, SeaMonkey, Postbox) and Outlook on your PC, and the add-account window offers it too. Accounts come with their servers, identities and signatures, and you type each password once; mail on your servers is downloaded from them. Mail kept only on your PC (Thunderbird’s Local Folders, Outlook’s .pst files) comes into folders on this PC, and Thunderbird’s filters become rules where Telex can do the same. Before 0.9, mbox and .eml files could be brought in by hand.',
      },
      {
        id: 'setup',
        q: 'Do I need to know my server settings?',
        a: 'Usually not. Enter your name, address and password, and Telex finds the IMAP and SMTP servers: from its list of common providers, your domain’s setup file or DNS records, or by trying the usual names such as mail.yourdomain. It logs in to both before saving. If nothing works, the settings form opens with what it found, so you only fill in the rest.',
      },
      {
        q: 'Can Telex send mail?',
        a: 'Yes. Mail goes out over SMTP, waits in the Outbox if the connection fails, and is marked Sent only once your server accepts it. A copy goes to the Sent folder on your server.',
      },
      {
        q: 'Can I take back a message I just sent?',
        a: 'From version 0.5, yes: sent mail waits five seconds in the Outbox before it goes (up to 30, in Settings › Writing), and Z in that moment brings it back into compose. Once it has gone, no mail client can recall it.',
      },
      {
        q: 'Can I attach files and open attachments?',
        a: 'From version 0.5. Click an attachment to open it in its app, or save one or all of them. Programs, scripts, shortcuts, macro documents and anything in Spam ask before they open. Attach files with Attach… or by dropping them on the window; Telex warns above 20 MB. Before 0.5, attachments could only be forwarded.',
      },
      {
        q: 'Does new mail show up right away?',
        a: 'From version 0.5, yes. Telex keeps one IMAP IDLE connection open to each account’s Inbox, so new mail appears within seconds, with a notification while Telex is in the background. Servers without IDLE, and the other folders, are checked every 10 minutes, or as often as Settings › Mail says.',
      },
      {
        q: 'Does Telex download all my old mail?',
        a: 'The first sync takes the newest 300 messages of each folder, so you can start at once. From version 0.5 the status bar says how many older ones are still on the server, and downloads them when you ask, newest first.',
      },
      {
        id: 'gmail',
        q: 'Does it work with Gmail or Outlook.com?',
        a: 'Gmail, yes, with an app password: turn on 2-Step Verification in your Google account, make an app password (myaccount.google.com/apppasswords) and give Telex that instead of your usual password; Telex reminds you when you add the account. Google doesn’t offer app passwords with Advanced Protection, and some work or school accounts have them turned off. Outlook.com, Hotmail and Microsoft 365, not yet: Microsoft lets other mail apps in only with Microsoft sign-in (OAuth2). That is next on the roadmap, together with signing in with Google.',
      },
      {
        q: 'Do changes I make reach the server?',
        a: 'Yes. Reading, flagging, moving and deleting happen locally at once and then on the server. If you are offline, they wait in a queue and go out at the next sync.',
      },
      {
        q: 'Can I keep mail unread until I decide?',
        a: 'From version 0.7, yes. Settings › Reading says when a message counts as read: as soon as you select it, after it has been on screen for a moment you choose, only when you open it in its own window, or only when you mark it yourself, with U, the dot at the start of its row, or the envelope above it. Space goes to the next unread message either way.',
      },
      {
        q: 'Can it read HTML newsletters?',
        a: 'Yes. HTML mail is shown in a sandbox where no script can run. Remote images stay blocked until you load them for that message, or for everything from a sender you trust (never automatically in Spam). H switches to plain text. In a dark theme the message takes the theme’s colors, or stays white if you prefer.',
      },
      {
        q: 'Can I write in HTML?',
        a: 'Yes. Plain text is the default, and Settings › Writing or the Text | HTML switch in a message turns on bold, italics, lists, quotes and links. Every HTML message goes out with a plain-text copy, and pasted web pages lose their styles and scripts.',
      },
    ],
  },
  {
    title: 'Privacy and data',
    items: [
      {
        id: 'privacy',
        q: 'Does Telex send anything about me anywhere?',
        a: 'No. There is no account, no telemetry and no Telex server. The app talks to your mail servers, to the servers that host a message’s images only when you choose to load them, and to GitHub to look for a new version only if you allow it. That check sends nothing about you.',
      },
      {
        q: 'Where is my mail kept?',
        a: 'In one file on your disk: %APPDATA%\\app.telexmail.desktop\\telex.db on Windows, ~/.local/share/app.telexmail.desktop/telex.db on Linux. Your settings sit next to it in settings.json. Passwords are stored separately, in Windows Credential Manager or your Linux keyring.',
      },
      {
        q: 'Can I keep mail only on my PC?',
        a: 'From version 0.6, yes: folders on this PC keep mail on your disk only, off the server and out of its quota. Drag mail there, press M, or let a rule file it; moving it back to a server folder uploads it again. You can also bring in mail from other programs as mbox or .eml files, and save any folder as mbox.',
      },
      {
        q: 'How do I back up my mail?',
        a: 'From version 0.6, in Settings › Backups: back up now, or every day or week by itself, into a folder you pick, ideally on another disk or in OneDrive. A backup is one file with all your mail, folders, rules and identities, never passwords. Restoring keeps the mail base as it was next to it, lets the servers’ state win at the next check, and moves mail waiting to be sent to Drafts.',
      },
      {
        q: 'Can I move Telex to another PC?',
        a: 'Yes. Settings › Import and export saves your settings, rules, identities and signatures, and your accounts’ server settings to one file. It never holds passwords. Importing it shows what would change, part by part, before anything does; accounts that aren’t set up yet open the setup form filled in, so you only type the password.',
      },
      {
        q: 'Can I search without an internet connection?',
        a: 'Yes. Every downloaded message is stored in full and indexed on your PC, so search, full-text included, works offline, and so does opening attachments.',
      },
    ],
  },
  {
    title: 'Installing',
    items: [
      {
        id: 'demo',
        q: 'Can I try Telex without installing it?',
        a: 'Yes. The live demo on this site runs the real Telex interface in your browser, with a made-up mailbox: every key, the settings and the eight themes work. It can’t reach real mail servers, and nothing you do there leaves your browser.',
      },
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
        a: 'Click the version number at the right end of the status bar. Telex asks GitHub for the newest version and, with your click, installs it and restarts. On first start it also asks whether to check by itself each time it starts (or, in Settings › Updates, once a day). Installing a newer version over the old one works too; your mail and settings stay.',
      },
      {
        q: 'Is Telex available in Polish?',
        a: 'Yes. The interface speaks English and Polish and follows the Windows display language; if Windows is in English but set to Poland, Telex offers Polish on first start. You can switch any time in Settings › Language.',
      },
      {
        q: 'Is there a Mac or Linux version?',
        a: 'For Linux there is an AppImage that updates itself and a .deb package, both for 64-bit PCs, next to the Windows installer on the download page. There is no Mac version yet. Telex is made for Windows first.',
      },
      {
        q: 'Is it like The Bat!?',
        a: 'That is the inspiration: a fast, dense, keyboard-driven client with serious filters. Telex is a new app written from scratch, and not affiliated with Ritlabs.',
      },
    ],
  },
];

/** The questions on the home page. */
export const faq: Question[] = ['providers', 'gmail', 'privacy'].map(
  (id) => faqGroups.flatMap((g) => g.items).find((q) => q.id === id)!,
);

/** Lines the telex tape above the footer punches, one after another (letters, figures and . , : ? ' / + = - ( ) only). */
export const ticker = [
  'ZCZC TELEX FOR WINDOWS AND LINUX',
  'FAST KEYBOARD MAIL. LOCAL MAIL BASE. NO TELEMETRY.',
  'TRY IT IN YOUR BROWSER: SEE THE LIVE DEMO',
  'TYPE YOUR ADDRESS. TELEX FINDS THE SERVERS.',
  'GMAIL: WORKS WITH AN APP PASSWORD. NEXT: MICROSOFT SIGN-IN',
  'EIGHT THEMES, ALL WCAG 2.2. ENGLISH AND POLISH.',
  'TRY: SPACE = NEXT UNREAD, R = REPLY, T = CONVERSATION, / = SEARCH',
  'NNNN',
];
