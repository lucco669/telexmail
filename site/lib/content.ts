// Page copy that changes as the app grows: features, keys, screenshots, versions, roadmap, questions.
// Keep claims true to the current build (see CLAUDE.md, "Current state"). No imports: scripts/screens.mjs
// loads this file directly.

// Keep a feature's introduction version fixed after release. The newest entry in `versions`
// drives the hero and What's new; lib/versions.ts compares it with the live release.
const V095 = '0.9.5';
const V098 = '0.9.8';
const V099 = '0.9.9';
const V0910 = '0.9.10';
const V0911 = '0.9.11';
const V0912 = '0.9.12';

/** The points on the home page: short, the features page has the detail. */
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
    text: 'Messages are kept whole, attachments too. Search offline across your accounts, and keep useful queries as saved searches in the folder list.',
  },
  {
    title: 'Rules you can preview',
    text: 'Match senders, text, named headers and attachments. Preview the matches, then file, flag, copy or forward new mail, with a note explaining each change.',
  },
  {
    title: 'Conversations your way',
    text: 'T brings a conversation together across folders. Prefer a shorter list? Turn on expandable thread rows, with counts and arrow-key controls.',
  },
  {
    title: 'Private by default',
    text: 'Remote images stay blocked and passwords use your system’s store. The spam filter learns on your PC, and links and scam warning signs are checked there too. No Telex account, and nothing counted unless you say yes.',
  },
  {
    title: 'Readable your way',
    text: 'Eight measured themes, six fonts, text up to 200%, and five toolbar layouts. Keep labels, use larger icons, or choose a toolbar made entirely of text.',
  },
  {
    title: 'Light on your PC',
    text: 'A small Windows download and a Rust mail core. The list loads a page at a time, keeping large mailboxes responsive. Startup, search and whole-app memory are measured with 100,000 messages.',
  },
  {
    title: 'Fluent in Polish',
    text: 'The whole interface in Polish or English. Windows-1250 mail decodes correctly, search ignores diacritics, replies quote “Dnia … napisał(a):”.',
  },
];

/** The strip under the hero's screenshot: what you get, in a few words each. */
export const facts = ['Spam filter that learns', 'Saved searches', 'Rule previews', 'Copy as Markdown', 'Undo send', 'Windows and Linux', 'No Electron', 'No tracking'];

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
  { title: 'Keep searches close', text: 'Search offline across your accounts, then save the query in your folder list.' },
  { title: 'Junk out of the way', text: 'A spam filter that learns from your own mail on your PC, and says why it moved each message.' },
  { title: 'Preview your rules', text: 'Match headers and attachments, and inspect results before filing or forwarding new mail.' },
  { title: 'Several accounts, one window', text: 'Each in its own color, one search across all of them.' },
  { title: 'Take it back', text: 'Z undoes a delete, a move or a discarded message, and holds sent mail for five seconds.' },
  { title: 'Nothing to sign up for', text: 'No account, no tracking, no Telex server between you and your mail.' },
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
      { text: 'Unread mail stands out: read mail goes quieter, or unread takes the accent color', since: '0.9.1' },
      { text: 'With several accounts every list wears its account’s color, and all-mail search names the account next to the folder', since: '0.9.1' },
      { text: 'Dates, the Size column and fitting the columns in Settings › Appearance too', since: '0.9.1' },
      { text: 'Quoted mail reads as blocks: who wrote it on top, a colored bar for each level, in the normal text color; long quotes fold, and Q opens them all', since: V0910 },
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
      { text: 'Copy as Markdown: a message, a selection or a whole conversation, oldest first, ready for notes or an AI chat (Ctrl+C, the menu, or the button above a message)', since: V0911 },
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
    id: 'threads',
    screen: 'threads',
    title: 'A shorter list, with expandable threads',
    text: 'Keep one row per conversation in a folder, then expand the replies you want to read. Settings › Reading turns threads on; individual message rows remain the default.',
    since: V095,
    points: [
      'Message counts and unread counts show what is inside a thread',
      'The chevron or → expands; ← collapses the selected thread',
      'Space still reaches unread messages hidden inside a collapsed thread',
      'Search and unread-only lists show individual messages',
      'Actions apply to the selected message; use T for the conversation across folders',
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
      { text: 'Move in from Thunderbird or Outlook: their local folders and .pst files come over folder by folder, with their accounts and filters', since: '0.9.0' },
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
    title: 'HTML when it helps, plain text when you want it',
    text: 'R opens the reply right in the reading pane, quoted and signed. New messages and replies start in HTML, for bold, lists and links, and always go out with a plain-text copy; switch a message, or the default, to plain text in a mono face.',
    points: [
      'Reply, reply to all and forward, each one key',
      { text: 'Reply | Reply all at the top of every reply: switch any time, your text stays; with Reply, a line names who else was on the original, one click from being added', since: V0910 },
      { text: 'A reply to all that goes to 8 people or more asks once before Send', since: V0910 },
      { text: 'Mention an attachment without attaching one, and Send asks first: “attached”, “enclosed”, “w załączniku”; quotes and your signature don’t count', since: V0910 },
      { text: 'A reply offers the original’s files: one click and they go along', since: V0910 },
      'HTML or plain text, as a default or per message; replies can follow the original',
      'Pasted web pages lose their styles and scripts: only formatting goes in',
      'A From picker with all your identities; the signature follows',
      'Ctrl+Enter sends, Esc saves a draft and closes',
      'E reopens a draft or a message still in the Outbox',
      { text: 'Drafts save themselves while you write; New or Reply in the middle of a message puts it in Drafts first', since: '0.7.2' },
      { text: 'Discard can be undone: Z or the Undo button brings the message back', since: '0.7.2' },
      { text: 'Spelling in your languages, Polish and English together: right-click a word or press F7 for suggestions', since: '0.9.0' },
      { text: 'The languages spelling checks show at the end of the compose bar, and a click there changes them', since: '0.9.1' },
      { text: 'Click an underlined word for its suggestions, ranked across your languages: “Wycene” offers “Wycenę” first', since: V099 },
      { text: 'Lines follow the text as you type, and the word you are typing waits until you finish it; signatures and the away reply are checked too', since: V099 },
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
      { text: 'Every signature in one place, Settings › Writing, saved as you type; replies and forwards signed or not', since: V098 },
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
      { text: 'New mail brings the list to its top, where it lands; the message you were on stays selected', since: V099 },
      'IMAP over SSL/TLS, folders straight from the server',
      { text: 'IMAP over STARTTLS on port 143 too, never unencrypted', since: '0.5.0' },
      'Inbox, Sent, Trash and others found by their flags or by English and Polish names',
      { text: 'Sent is the folder that holds your sent mail, even next to a server’s own empty one; or choose it, and Drafts, Archive, Spam and Trash, in Settings', since: '0.8.0' },
      'The first sync takes the newest 300 messages of each folder',
      { text: 'Older mail on request, newest first, a thousand at a time', since: '0.5.0' },
      { text: 'Every account checked at the same time, so several take about as long as one', since: '0.7.2' },
      { text: 'An account whose last check failed says so in the folder tree and the status bar; one click tries again', since: '0.7.2' },
      { text: 'Gentle with shared servers: at most 4 checks and 8 watched Inboxes at once per server, and a pause when one stops answering', since: V0911 },
      { text: 'Settings › Connections: every connection’s steps, timed, and where and why one failed; copy it all for whoever helps', since: V0911 },
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
    id: 'saved-searches',
    screen: 'saved-searches',
    title: 'Useful searches, always at hand',
    text: 'Keep “files to review”, unread client mail or flagged invoices beside your folders. A saved search runs its query on your current local mail each time you open it; it does not copy messages into another folder.',
    since: V095,
    points: [
      'Type a query, choose this folder or all mail, and use Save search beside the search box',
      'Keep the query, scope, sort order, body-search choice and unread-only setting together',
      'Open the results from the folder list, including while offline',
      'Use Saved searches to create, edit or delete a query without deleting its messages',
      'Results cover downloaded mail; older messages still on the server appear after you fetch them',
    ],
  },
  {
    id: 'privacy',
    screen: 'images',
    title: 'Private by default',
    text: 'Your mail connects directly to your provider. Images load only when you allow them or trust a sender; update checks follow your choice. Account discovery, optional dictionary downloads and server-side away replies have their own network uses, documented on the privacy page.',
    points: [
      'HTML mail in a sandbox where no script can run',
      'Remote images blocked, so senders can’t see when you read',
      'Your password stays in your system’s password store',
      'Your mail base is one file on your disk',
      'In a dark theme, HTML mail takes the theme’s colors; one click shows it on white',
      'No Telex account or mail relay; counted only if you say yes, without an ID',
      { text: 'Signed and encrypted mail (OpenPGP, as GnuPG and Thunderbird write it): signatures checked against the sender’s key, encrypted mail decrypted only while it is open and never kept or searched as text', since: V0912 },
      { text: 'Sign and Encrypt in compose, to everyone’s keys and yours; Telex says whose key is missing instead of sending in the clear', since: V0912 },
      { text: 'Keys in Settings › Keys: make one in a click, import or export, compare fingerprints; keys come with mail too (Autocrypt)', since: V0912 },
      { text: 'S/MIME too, as Outlook and Apple Mail write it: your certificate from a .p12 or .pfx file signs and decrypts, Windows says whether a sender’s certificate is from an authority it trusts, and people’s certificates come with their signed mail', since: V0912 },
    ],
  },
  {
    id: 'spam-filter',
    screen: 'spam-filter',
    title: 'A spam filter that learns from you',
    text: 'Telex learns what your spam looks like from your own Spam folder and the mail you keep, on your PC, and files new junk away before you see it. Each message it moves says why.',
    since: V098,
    points: [
      'A statistical filter (as SpamBayes): words, senders, link hosts and a few header facts, nothing uploaded',
      'The note on a message it moved gives the score and the words that told, and how to send it back',
      'Never moves mail from people you have written to, senders whose images you trust, or answers to your own mail',
      'Mark as spam and Not spam in the message menu teach it; moving mail into or out of Spam does too, and Z undoes both',
      'Careful, normal or strict in Settings › Mail, or off; it waits until it has seen 50 of each kind',
      'It runs before your rules, and mail it files away brings no notification',
      { text: 'Spam you throw in the Trash is marked read on the way, so it never counts as unread', since: V0911 },
    ],
  },
  {
    id: 'link-inspection',
    screen: 'links',
    title: 'Inspect before you follow',
    text: 'See where a link actually points, and why an opened message looks suspicious. Telex checks sender and link warning signs on your PC without opening the destination or uploading your mail.',
    since: V095,
    points: [
      { text: 'Point at a link or focus it: its real destination shows in one line at the foot of the message, never over it', since: V098 },
      'Right-click a link to copy its address; in Spam links stay blocked, copying still works',
      'Warning reasons include sender and brand lookalikes, disguised destinations and hidden characters; mail without warning signs shows nothing',
      'The score is advisory points out of 100, not a probability; no warning does not establish that mail is safe',
      'The check runs when you open a message; it does not move mail, follow redirects or verify attachments, reputation, SPF, DKIM or DMARC',
    ],
  },
  {
    id: 'usage',
    screen: 'usage',
    title: 'See where your mail goes',
    text: 'A quiet bar in the status bar shows how full the open account is on the server. Settings › Usage puts every account on one line, then shows the one you pick the way a phone shows its storage: what takes the space, and how mail comes and goes.',
    points: [
      'The server’s storage, when it reports a quota, with a warning before it fills up',
      { text: 'Every account on one line: the server, this PC and the last check side by side', since: V099 },
      { text: 'The status bar’s storage bar fills in the account’s own color and says how much room is left', since: '0.6.0' },
      'This PC by kind of folder: Inbox, Sent, your folders, Spam and Trash',
      'Mail received, sent and caught as spam, day by day for 30 days',
      'Who sends the most, and the largest messages, one click away',
    ],
  },
  {
    id: 'rules',
    screen: 'rule-preview',
    title: 'Rules that explain themselves',
    text: 'Telex’s answer to The Bat!’s Sorting Office. Combine conditions, preview their matches on the current list, and file new mail automatically. Notes on each affected message explain which rule changed it.',
    points: [
      'Conditions on From, To or Cc, subject, text or anywhere',
      'Move, mark read, flag, move to Trash or answer automatically',
      'Run on arrival, or on any folder when you ask',
      { text: 'Match any named header, attachment filename or MIME type, including presence and absence; combine all or any conditions', since: V095 },
      { text: 'Copy to another folder while keeping the original; forward newly arriving mail with duplicate and loop protection', since: V095 },
      { text: 'Preview an unsaved rule on the current folder or search: total matches, sample messages and each condition’s result, without changing or sending mail', since: V095 },
      'Automatic forwarding and replies run only on new arrivals; running rules by hand does not send old mail',
      { text: 'Start a rule from the selected message: “New rule for mail from …”', since: '0.5.0' },
      'Auto-replies skip mailing lists and robots, and answer each sender once in 4 days',
      { text: 'An away reply per account, kept on your server where it can be, so it answers while Telex is closed', since: '0.9.0' },
    ],
  },
  {
    id: 'toolbar-layouts',
    screen: 'toolbar-layouts',
    title: 'A toolbar that fits you',
    text: 'Keep familiar controls or give them more room. Settings › Appearance offers five layouts, applied immediately and remembered for the next start.',
    since: V095,
    points: [
      'Classic: compact icons beside their labels',
      'Icons only or Large icons when you recognize the controls by shape',
      'Labels below for bigger controls with their names underneath',
      'Text only for a toolbar without action icons',
      'Search and Settings remain reachable as the window gets narrower',
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
      { text: 'Six fonts: IBM Plex, your system’s own, Classic (Tahoma), Inter, Atkinson Hyperlegible for low vision, and the light Source Sans', since: '0.9.1' },
      { text: 'Unread mail your way: read mail quieter, unread in color, or bold only', since: '0.9.1' },
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
  'There is no Mac version, and the Windows installer isn’t code-signed yet, so SmartScreen may warn when you install.',
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
  { id: 'expandthread', group: 'Move', keys: [['→'], ['←']], action: 'Expand / collapse a thread (with threads enabled in Settings › Reading)', match: [{ key: 'ArrowRight', shift: false }, { key: 'ArrowLeft', shift: false }], since: V095 },
  { id: 'goto', group: 'Move', keys: [['G']], action: 'Go to a folder: type part of its name', match: [{ key: 'g' }], since: '0.5.0' },
  { id: 'window', group: 'Move', keys: [['Enter']], action: 'The message in a window of its own (or double-click it); Esc closes it', match: [{ key: 'Enter' }], since: '0.6.0' },
  { id: 'open', group: 'Move', keys: [['Enter']], action: 'Open the message’s folder (in all-mail results and conversations)', match: [{ key: 'Enter' }], since: '0.5.0' },
  { id: 'select', group: 'Select', keys: [['Shift', '↑'], ['↓']], action: 'Select more (or fewer) messages', match: [{ key: 'ArrowUp', shift: true }, { key: 'ArrowDown', shift: true }], since: '0.5.0' },
  { id: 'selectends', group: 'Select', keys: [['Shift', 'Home'], ['End']], action: 'Select to the top / bottom', match: [{ key: 'Home', shift: true }, { key: 'End', shift: true }], since: '0.5.0' },
  { id: 'selectall', group: 'Select', keys: [['Ctrl', 'A']], action: 'Select every message in the list', match: [{ key: 'a', ctrl: true }], since: '0.5.0' },
  { id: 'markdown', group: 'Select', keys: [['Ctrl', 'C']], action: 'Copy the selected messages as Markdown, for notes or an AI chat (with no text selected)', match: [{ key: 'c', ctrl: true }], since: V0911 },
  { id: 'new', group: 'Write', keys: [['N']], action: 'New message', match: [{ key: 'n' }] },
  { id: 'reply', group: 'Write', keys: [['R']], action: 'Reply', match: [{ key: 'r' }], home: true },
  { id: 'replyall', group: 'Write', keys: [['Shift', 'R']], action: 'Reply to all', match: [{ key: 'R', shift: true }] },
  { id: 'forward', group: 'Write', keys: [['Shift', 'F']], action: 'Forward', match: [{ key: 'F', shift: true }] },
  { id: 'edit', group: 'Write', keys: [['E']], action: 'Edit a draft or an unsent message', match: [{ key: 'e' }] },
  { id: 'send', group: 'Write', keys: [['Ctrl', 'Enter']], action: 'Send from compose', match: [{ key: 'Enter', ctrl: true }] },
  { id: 'quotes', group: 'View', keys: [['Q']], action: 'Open or fold the quoted mail of a message', match: [{ key: 'q' }], since: V0910 },
  { id: 'spell', group: 'Write', keys: [['F7']], action: 'The next spelling mistake, with suggestions (compose)', match: [{ key: 'F7' }], since: '0.9.0' },
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
  {
    title: 'Keep the query, not a copy',
    text: 'After / and a query, Tab chooses the folder or all mail. Save search beside the box keeps that query in the folder list. Opening it searches your current downloaded mail; deleting it leaves messages alone.',
    since: V095,
  },
  {
    title: 'Open just the replies you need',
    text: 'Turn on threads in Settings › Reading, then use → and ← in the message list to expand and collapse. Space finds unread replies even inside a closed thread. T still gathers the whole conversation across folders.',
    since: V095,
  },
  {
    title: 'Inspect a link with the keyboard',
    text: 'Tab to a message link: its real destination shows at the foot of the message. Shift+F10 or the menu key opens its menu, to open it or copy the address. This works in Spam too, where opening links stays blocked. Nothing visits the website.',
    since: V095,
  },
  {
    title: 'Teach the spam filter',
    text: 'Shift+F10 on junk that got through, then Mark as spam: it goes to Spam and the filter learns from it. In Spam, Not spam sends a message back and unlearns it. Z takes either back.',
    since: V098,
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
    id: 'markdown', label: 'Copy as Markdown', title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'Copy as Markdown, from the menu, Ctrl+C or the button above a message: the subject as a heading, who, when and which files, then the text. Select a conversation to copy it all, oldest first.', since: V0911,
  },
  {
    id: 'connections', label: 'Connections', title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'Settings › Connections: every server Telex reached and its last connections, each step timed. When a check fails, it shows where and why, and Copy details puts it all in a message for whoever helps.', since: V0911,
  },
  {
    id: 'reply-checks', label: 'Reply checks', title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'A reply that would have gone wrong: Telex names who else was on the original, offers its two files, and asks before sending “attached” with nothing attached.', since: V0910,
  },
  {
    id: 'quotes', label: 'Quoted mail', title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'Quoted mail as blocks: who wrote each part on top, a bar for each level, in the normal text color. Long quotes fold to their first lines; Q opens them all.', since: V0910,
  },
  {
    id: 'spelling', label: 'Spelling', title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'Click an underlined word: its suggestions sit right under it, closest first, with Add to dictionary and Ignore. The focus stays in the message, so you can keep typing.', since: V099,
  },
  {
    id: 'spam-filter', label: 'Spam filter', title: 'Spam · sam@lindenstudio.com – Telex',
    caption: 'Mail the spam filter moved says so, with its score and the words that told. Not spam in the message’s menu sends it back to the Inbox and teaches the filter.', since: V098,
  },
  {
    id: 'saved-searches', label: 'Saved searches', title: 'All mail – Telex',
    caption: 'A saved has:file query across all mail, opened from the folder list. Results use current downloaded mail; deleting a search leaves its messages alone.', since: V095,
  },
  {
    id: 'rule-preview', label: 'Rule preview', title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'Preview an attachment rule against the current list: total matches, sample messages and each condition’s result. Previewing changes and sends nothing.', since: V095,
  },
  {
    id: 'links', label: 'Link inspection', title: 'Spam · sam@lindenstudio.com – Telex',
    caption: 'A link’s real destination in one line at the foot of the message, under the local scam-risk reasons, in Spam. Opening links stays blocked; the advisory score is not a probability.', since: V095,
  },
  {
    id: 'threads', label: 'Threads', title: 'Sent · sam@lindenstudio.com – Telex',
    caption: 'Optional expandable conversation rows with message counts in Sent. Settings › Reading turns them on; search and unread-only lists keep individual messages.', since: V095,
  },
  {
    id: 'toolbar-layouts', label: 'Toolbar layouts', title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'Labels below is one of five toolbar layouts in Settings › Appearance, alongside Classic, Icons only, Large icons and Text only.', since: V095,
  },
  {
    id: 'reply',
    label: 'Reply',
    title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'R opens the reply right in the reading pane, quoted and signed, from the identity you pick. Reply | Reply all switches at any time, and the line under Cc names who else was on the original.',
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
    caption: 'Settings › Backups: the mail base in one file, by hand or every day or week, to a folder you choose, and a restore that keeps what was there.',
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
    caption: 'Operators in search, across all mail: every message with a file since September, with the folder and account it is in.',
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
    id: 'signatures',
    label: 'Signatures',
    title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'Settings › Writing: every address’s signature in one place, saved as you type and spell-checked, then spelling and its languages.',
    since: V098,
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
    id: 'appearance',
    label: 'Appearance',
    title: 'Inbox · sam@lindenstudio.com – Telex',
    caption: 'Settings › Appearance: six fonts, each card drawn in its own face, text size and rows, five toolbars, and how unread mail stands out.',
    since: '0.9.1',
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
    caption: 'Settings › Usage: every account on one line, then the one you pick: what takes the space on this PC, 30 days of mail in and out, who sends the most.',
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
    keys: ['/', 'Save search'],
    title: 'Keep a live search',
    text: 'Type has:file, choose All mail, then Save search beside the box. Name it “Files to review”, save and close the dialog. Open it from the folder list; Saved searches lets you edit or delete it.',
  },
  {
    keys: ['Rules', 'Preview'],
    title: 'Preview before filing',
    text: 'Open Rules and start a new rule named “PDF review”. Choose Attachment name, Ends with and .pdf; leave the Flag action selected, then click Preview matches. You see the current list’s match count and sample condition results without changing mail.',
  },
  {
    keys: ['Ctrl+,', '→'],
    title: 'Expand a thread',
    text: 'In Settings › Reading, turn on “Group related messages into threads”. Close Settings, open the studio’s Sent folder and select the Online shop estimate row. → expands it; ← collapses it.',
  },
  {
    keys: ['Spam', 'Not spam'],
    title: 'See why it’s spam',
    text: 'Open Spam under the studio account and select the €500 gift card: its note says the spam filter moved it, with the score and the words that told. Right-click it and choose Not spam to send it back to the Inbox; Z undoes that. The demo’s filter doesn’t learn; the desktop app’s does, on your PC.',
  },
  {
    keys: ['Spam', 'Hover'],
    title: 'Where a link really goes',
    text: 'In the same Spam folder, select “Your music subscription needs attention” and point at its button: the real destination shows in one line at the foot of the message. Right-click the link to copy it. The warning reasons are an illustrative fixture here; the desktop app computes them locally in Rust.',
  },
  {
    keys: ['Ctrl+,'],
    title: 'Choose your toolbar',
    text: 'Settings › Appearance has five toolbar layouts. Try Labels below or Text only: the controls change immediately. Change the font, unread style or theme in the same place.',
  },
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
    text: 'Eight themes from Paper to Phosphor, six fonts, how unread mail stands out, Polish, bigger text. Rules and identities can be edited too.',
  },
];

/** The tabs on the home page: a tour, not the whole gallery. */
export const homeScreens = ['markdown', 'reply-checks', 'quotes', 'spelling', 'spam-filter', 'links', 'signatures', 'saved-searches', 'rule-preview', 'connections', 'threads', 'unread', 'reply', 'thread', 'local', 'images', 'appearance', 'polish'].map(
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
  highlights?: { title: string; text: string; keys?: string; href?: string }[];
};

export const versions: Version[] = [
  {
    version: V0911,
    title: 'Copy mail as Markdown, gentler on shared servers, a log of every connection',
    date: '2026-10-17',
    summary: 'One key puts a message, or a whole conversation, on the clipboard as clean Markdown for your notes or an AI chat. Telex also goes easier on mail servers many accounts share, and shows every step of its connections when one fails.',
    points: [
      'Copy as Markdown: Ctrl+C in the list (when no text is selected), the message menu, or the new button above a message. The subject becomes a heading, From, To, Cc, the date and the attachments a short list, then the text, quotes and all. Select several messages, or a conversation with T and Ctrl+A, and they come oldest first, one after another. Mail written only in HTML keeps its links. Nothing is marked read.',
      'Spam you delete, or move to the Trash, is marked read on the way, so it no longer counts as unread there.',
      'Gentler on shared servers: Telex opens at most 4 connections for checks and 8 for watching Inboxes to one server at a time. A server that stops greeting gets a rest of 1 to 15 minutes, and the error says until when. Purelymail is on the provider list.',
      'Settings › Connections: every server reached since Telex started and the last 1,000 connections, each step timed (address, connecting, TLS, greeting, login), with where one failed and why. Filter by account or show only problems, and copy it all as text. It opens from Details next to a failed check.',
      'Asks once, after the update question, whether this copy may be counted: with a yes, one anonymous count a day, with no ID. Settings › Updates changes your answer.',
      'Fixed: the plain-text copy of HTML mail could run two words together after formatting (“big world” became “bigworld”).',
    ],
    highlights: [
      { title: 'Copy as Markdown', text: 'A message as clean Markdown, ready for notes or an AI chat.', keys: 'Ctrl+C', href: '/features#read' },
      { title: 'Whole conversations', text: 'Select a thread and copy it all, oldest first, one message after another.', keys: 'T', href: '/features#read' },
      { title: 'Spam out of sight', text: 'Spam you throw in the Trash is marked read on the way.', href: '/features#spam-filter' },
      { title: 'Gentle with servers', text: 'A few connections at a time per server, and a pause when one goes quiet.', href: '/features#sync' },
      { title: 'Every connection, explained', text: 'Settings › Connections times each step and says where a failed check stopped.', href: '/features#sync' },
      { title: 'Counted only with a yes', text: 'One anonymous count a day, no ID, and only if you agree.', href: '/privacy' },
    ],
  },
  {
    version: V0910,
    title: 'Replies that keep everyone, quoted mail you can read, no forgotten attachments',
    date: '2026-10-16',
    summary: 'The three classic reply mistakes, caught before they happen: leaving people out, forgetting the file, and losing track of who said what in a long quoted history.',
    points: [
      'Reply | Reply all sits at the top of every reply and switches at any time without touching your text. With Reply, a line under Cc names who else was on the original, one click from being added.',
      'A reply to all that would go to 8 people or more asks once before it goes, and offers to answer only the sender instead.',
      'Send asks when your text mentions an attachment (“attached”, “enclosed”, “załączam”, “w załączniku”) and nothing is attached. Quotes, a forwarded message and your signature don’t count, so a confidentiality footer won’t set it off.',
      'A reply offers the original’s files: “The original had 2 files · Attach them”.',
      'Quoted mail reads as blocks: who wrote it on top, a colored bar for each level, in the normal text color instead of grey. Long quotes fold to their first lines; in HTML mail, Gmail, Outlook, Apple Mail and Thunderbird quotes fold behind one line. Q opens or folds them all, and a reply’s own quote stays folded in the editor until you click it.',
      'Settings › Writing › Sending can turn either question off.',
    ],
    highlights: [
      { title: 'Nobody left out', text: 'Reply | Reply all at the top of every reply, and a line naming who else was on the original.', href: '/features#writing' },
      { title: 'Not to everyone by accident', text: 'A reply to all for 8 people or more asks once before it goes.', href: '/features#writing' },
      { title: 'The file you meant to attach', text: '“Attached” with nothing attached? Send asks first, in English and Polish.', href: '/features#writing' },
      { title: 'The original’s files', text: 'One click puts them in a reply, carried along like a forward’s.', href: '/features#writing' },
      { title: 'Quotes you can read', text: 'Blocks with who wrote them, a bar per level, folded when long.', keys: 'Q', href: '/features#reading' },
      { title: 'In every mail program’s style', text: 'Gmail, Outlook, Apple Mail and Thunderbird quotes fold the same way.', href: '/features#reading' },
    ],
  },
  {
    version: V099,
    title: 'Spelling you can click, new mail on top, tidier Settings',
    date: '2026-10-16',
    summary: 'A misspelt word shows its suggestions with one click, ranked across your languages; new mail brings the list to its top; and Settings says what it needs to in fewer words.',
    points: [
      'Click an underlined word: its suggestions appear right under it, with Add to dictionary and Ignore, and the focus stays in the message. Right-click and F7 still work.',
      'Suggestions from every language you write in are ranked by how close they are, so a missing Polish letter comes first: “Wycene” offers “Wycenę”, not English guesses.',
      'The wavy lines are red and clearer, follow the text as you type, and leave the word you are typing alone until you finish it. The subject’s line is no longer cut off.',
      'Signatures and the away reply are checked with Telex’s dictionaries too; the webview’s English-only checker is gone everywhere.',
      'New mail that lands in the open list brings it to its top (its end when sorted oldest first); the message you were on stays selected.',
      'Settings, tidied: shorter notes everywhere, Appearance opens with Theme and Font, the toolbar layouts in one row, Usage as one line per account with the details of the one you pick, Backups’ schedule, keep and folder side by side, and account actions in a row of their own.',
    ],
    highlights: [
      { title: 'Click to fix', text: 'An underlined word shows its suggestions right under it. Keep typing whenever you like.', keys: 'F7', href: '/features#writing' },
      { title: 'Polish first', text: 'Suggestions ranked across your languages: “Wycene” gives “Wycenę”, not English guesses.', href: '/features#writing' },
      { title: 'Lines that keep up', text: 'Red and clear, they move with the text and wait for the word you are typing.' },
      { title: 'New mail on top', text: 'The list goes to where new mail lands, and your selection stays.', href: '/features#sync' },
      { title: 'Usage at a glance', text: 'Every account on one line, the details of the one you pick below.', href: '/features#usage' },
      { title: 'Tidier Settings', text: 'Shorter notes and choices side by side, in every tab.' },
    ],
  },
  {
    version: V098,
    title: 'A spam filter of your own, quieter link checks, HTML by default',
    date: '2026-10-15',
    summary: 'Telex learns what spam looks like from your own mail and files it away, a link shows where it really goes in one line instead of a panel, and new messages and replies start in HTML with every signature in one place.',
    points: [
      'A spam filter that learns on your PC from your Spam folder and the mail you keep. New Inbox mail above its cutoff goes to Spam with a note saying why; it never touches people you have written to, and does nothing until it has seen enough of both kinds. Mark as spam and Not spam in the message menu teach it; Settings › Mail sets how careful it is, or turns it off.',
      'Point at a link or focus it: its real destination shows in one line at the foot of the message, never over it. Right-click to open it or copy the address. The link panel and the links bar are gone.',
      'Scam warnings appear only on mail that shows warning signs, and Settings › Reading can turn them off.',
      'New messages and replies start in HTML, still sent with a plain-text copy. Settings › Writing lists every signature to edit in place, saved as you type, and can leave the signature out of replies and forwards.',
      'Mark as read (or unread) is the first item of the message menu.',
      'Recipients written as their own address in angle brackets, or with an @ in the name, send instead of being refused.',
      'Thread rows and rule previews stay quick in very large mailboxes; a preview reads the first 2,000 messages of a list and says when there were more. One damaged saved search no longer hides the others.',
    ],
    upgrade: 'Version 0.9.8 upgrades the local mail base for the spam filter. Back up in Settings › Backups before updating; older Telex versions cannot reopen the upgraded mail base. Settings you already chose, such as plain-text compose, stay as they are.',
    highlights: [
      { title: 'Your own spam filter', text: 'Learns from your Spam folder on this PC and files new junk away, with the reason on each message.', href: '/features#spam-filter' },
      { title: 'Where a link really goes', text: 'One line at the foot of the message, never over it. Right-click to copy.', href: '/features#link-inspection' },
      { title: 'HTML by default', text: 'New messages and replies start formatted, and always go out with a plain-text copy.', href: '/features#writing' },
      { title: 'Signatures in one place', text: 'Every address you write from, edited in Settings › Writing and saved as you type.', href: '/features#identities' },
      { title: 'Mark as read, first', text: 'The first item of the message menu, where the pointer lands.' },
      { title: 'Quick at any size', text: 'Thread rows and rule previews stay fast in mailboxes of 100,000 messages.', href: '/performance' },
    ],
  },
  {
    version: V095,
    title: 'Saved searches, stronger rules and local scam checks',
    date: '2026-10-13',
    summary: 'Keep searches in the folder list, preview richer rules, group conversations when you want to, and inspect suspicious links with an offline scam-risk check.',
    points: [
      'Saved searches: save a query, its scope and list options under a name; open it from the folder list against your current mail. Edit or delete it from Saved searches.',
      'Rules can match named headers, attachment filenames and MIME types, including presence or absence. Copy keeps the original; automatic forwarding runs only on new mail and guards against duplicate sends and robot loops. Rule previews show the full match count and sample condition results without changing or sending mail. New rule actions travel with settings exports.',
      'Optional threads in Settings › Reading, off by default: compact rows with message counts, expandable with the chevron or ←/→. Space reaches hidden unread messages, and message bodies stay paged.',
      'Inspect real link destinations on hover or keyboard focus, list and copy them even in Spam. A local Rust check shows an advisory scam-risk score and explains sender mismatches, brand lookalikes, disguised links and other warning signs. It stays offline; the score is not a calibrated probability.',
      'Five toolbar layouts in Settings › Appearance: Classic, Icons only, Large icons, Labels below and Text only. They apply immediately and persist; Search and Settings stay accessible in narrow windows.',
      'The patched ammonia 4.2.3 sanitizer reaches released builds. Linux uses a checksum-verified GLib 0.18.5 backport of the upstream iterator fix, tested with compiler optimizations. Release builds now wait for dependency and secret scans, security regressions, hostile-email tests in Windows WebView2 and Linux WebKitGTK, and resource-limited MIME/HTML/PST fuzzing.',
    ],
    upgrade: 'Version 0.9.5 upgrades the local mail base. Back up in Settings › Backups before updating; older Telex versions cannot reopen the upgraded mail base. Your mail and settings stay in place.',
    highlights: [
      { title: 'Saved searches', text: 'Keep useful queries in the folder list. Results follow your current downloaded mail.', href: '/features#saved-searches' },
      { title: 'Preview stronger rules', text: 'Headers, attachments, copy and forward, with a preview before you run a rule.', href: '/features#rules' },
      { title: 'Inspect suspicious mail', text: 'Actual link destinations and offline scam-risk reasons, including in Spam.', href: '/features#link-inspection' },
      { title: 'Optional threads', text: 'Expandable conversation rows with counts. Arrow keys open replies; Space finds the unread ones.', keys: '→', href: '/features#threads' },
      { title: 'Five toolbar layouts', text: 'Compact icons, larger controls, labels underneath or text alone. Pick the layout that fits you.', href: '/features#toolbar-layouts' },
      { title: 'Checks before every release', text: 'Dependency and secret scans, native Windows and Linux mail tests, and bounded parser fuzzing gate release builds.', href: '/security#checks' },
    ],
  },
  {
    version: '0.9.1',
    title: 'Unread at a glance, account colors on every list, a sixth font',
    date: '2026-10-11',
    summary: 'New mail stands out from what you’ve read, every list wears its account’s color, search says whose mail it found, and the list’s options sit in Settings.',
    points: [
      'Unread mail stands out at a glance: read mail goes a quieter color, so the bold rows are the new ones. Settings › Appearance › Unread mail also offers unread in the accent color, or bold only as before.',
      'Account colors on every list: with more than one account, an account’s folder wears its color the way you chose for the folder list, a line down its rows or a tint, and a dot or its initials above it. Lists that mix accounts keep marking each row.',
      'Search in all mail names the account next to the folder (“Inbox · anna@studio.pl”), so you know which mailbox a message is in.',
      'Settings › Appearance › Message list columns: short or full dates, the Size column and fitting the columns again, besides the right-click on the headings.',
      'A sixth font: Source Sans Light with Source Code Pro, light and airy. The fonts now fill two rows of three.',
      'Spelling shows its languages at the end of the compose bar (“EN · PL”): every ticked one is checked at once, and a click there ticks others, turns spelling off, or opens the dictionaries.',
    ],
    highlights: [
      { title: 'Unread at a glance', text: 'Read mail quieter, unread bold; or unread in color. Your pick in Settings.' },
      { title: 'Colors on every list', text: 'Each account’s line or tint on its folders’ lists too, and its name in search results.' },
      { title: 'A sixth font', text: 'Source Sans Light, an airy set next to Plex, Inter, Atkinson and Windows’ own.' },
    ],
  },
  {
    version: '0.9.0',
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
    text: 'A Sorting Office: match text, named headers and attachments; preview matches, then move, flag or copy mail. Forwarding and auto-replies run on new arrivals, with notes and duplicate protection.',
    status: 'done',
    notYet: 'Regular expressions and rules that run on a schedule.',
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
    notYet: 'A code-signed installer with a verified publisher identity. SmartScreen reputation may still take time.',
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
    text: 'Space for the next unread, read rules of your own, a right-click menu, columns that fit the window, search with suggestions and saved queries in the folder list, accounts in your order.',
    status: 'done',
    since: '0.7.1',
  },
  {
    title: 'Moving in, spelling and away replies',
    text: 'Bring accounts, identities, supported filters and local mail from Thunderbird or Outlook, including .pst files. Check spelling in multiple languages and configure an away reply on the server where supported, or while Telex checks mail.',
    status: 'done',
    since: '0.9.0',
    notYet: 'Importing OAuth, Exchange or POP3 account connections; passwords must be entered again.',
  },
  {
    title: 'Unread at a glance and six fonts',
    text: 'Quieter read mail, accent-colored unread mail or bold alone; account colors on every list; column choices in Settings; six fonts and spelling-language controls in compose.',
    status: 'done',
    since: '0.9.1',
  },
  {
    title: 'Threads, saved searches and stronger rules',
    text: 'Named live searches, optional expandable threads, header and attachment conditions, rule previews, copy and forwarding, and five toolbar layouts.',
    status: 'done',
    since: V095,
  },
  {
    title: 'Local mail inspection and release security checks',
    text: 'Link destinations and offline scam warning reasons. Dependency and secret scans, hostile-mail tests in both native webviews and bounded MIME, HTML and PST fuzzing gate release builds.',
    status: 'done',
    since: V095,
    notYet: 'An independent security audit, Windows publisher signing, and verified sender authentication or link reputation.',
  },
  {
    title: 'A spam filter of your own, HTML by default',
    text: 'A statistical spam filter that learns from your mail on your PC and explains each move, link destinations in one quiet line, new messages and replies in HTML, and every signature in Settings › Writing.',
    status: 'done',
    since: V098,
    notYet: 'HTML signatures, and images and colors in HTML compose.',
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
        q: 'Does spelling check Polish and English together?',
        a: 'Yes, from version 0.9: every language you tick is checked at once, so one message can mix them; a word any of them knows is right. Telex picks them from your Windows languages and its own, uses Windows’ dictionaries, and fetches LibreOffice’s for a language Windows lacks when you click Download in Settings › Writing. From 0.9.1 the languages show at the end of the compose bar, and a click there changes them.',
      },
      {
        q: 'Can I write in HTML?',
        a: 'Yes. From version 0.9.8 new messages and replies start in HTML, with bold, italics, lists, quotes and links. Every HTML message goes out with a plain-text copy, and pasted web pages lose their styles and scripts. Prefer plain text in a mono face? Settings › Writing makes it the default, and the Text | HTML switch changes one message. If you chose plain text before 0.9.8, that choice stays.',
      },
    ],
  },
  {
    title: 'Searching, rules and appearance',
    items: [
      {
        id: 'saved-searches',
        q: 'How do saved searches work?',
        a: 'Type a query such as has:file is:unread, choose this folder or all mail, and click Save search beside the box. Give it a name and open it from the folder list. It keeps the query, scope, sort order and list options, then searches your current downloaded mail each time you open it. It works offline and creates no message copies. Use Saved searches to edit or delete it; deleting the search does not delete mail.',
      },
      {
        q: 'Can rules match headers and attachments?',
        a: 'Yes, in 0.9.5. Alongside sender, recipients, subject and text, conditions can inspect a named header, attachment filename or MIME type, including presence and absence. Combine all or any conditions. Rules can move, flag, mark read, copy, forward or reply, and leave notes explaining their changes. Regular expressions and scheduled rules are not built yet.',
      },
      {
        q: 'Does previewing a rule change anything?',
        a: 'No. In the rule editor, Preview checks the unsaved rule against the current folder or search results. It reports the full match count, sample messages and each condition’s result. It does not save the rule, move mail, mark messages read or send anything. You decide whether to save or run the rule afterward.',
      },
      {
        q: 'Can a rule copy or forward mail automatically?',
        a: 'Yes. Copy keeps the original and puts a copy in the chosen folder, with protection against repeating the same rule action. Forwarding uses the rule’s sending account and runs only on newly arriving mail, with duplicate and loop protection. Running rules manually does not forward old messages or send automatic replies. A queued forward is sent through your SMTP server; its recipient receives the mail.',
      },
      {
        q: 'Are conversations and thread rows the same thing?',
        a: 'T gathers a selected message’s conversation across the account’s folders. Optional thread rows instead group related mail inside one folder: turn on Group related messages into threads in Settings › Reading, then expand with → or the chevron and collapse with ←. Counts show messages and unread mail. Space reaches hidden unread replies. Search and unread-only lists remain individual messages, and actions apply to the selected message.',
      },
      {
        q: 'Can I change the toolbar layout?',
        a: 'Yes. Settings › Appearance offers Classic, Icons only, Large icons, Labels below and Text only. Your choice applies immediately and is saved. Search and Settings remain reachable in narrow windows. The same tab offers eight themes, six fonts, larger text, density and unread-mail styling.',
      },
    ],
  },
  {
    title: 'Privacy and data',
    items: [
      {
        id: 'license',
        q: 'Is Telex free? Is it open source?',
        a: 'Free, yes: for personal and business use, on any number of PCs, with no account and no ads. Open source, no: the code is public on GitHub so anyone can read how Telex handles mail and check its security, but all rights are reserved, so it may not be copied, changed or redistributed. Future versions may come under other terms; a version you have stays usable. The open-source libraries and fonts Telex is built on keep their own licenses, listed in the third-party notices. The License page has the full agreement.',
      },
      {
        id: 'privacy',
        q: 'Does Telex send anything about me anywhere?',
        a: 'The desktop app has no Telex account or mail relay. It connects directly to your mail servers. From 0.9.11 it asks once whether it may be counted: if you say yes, it sends this website its version, system, language and the month it was first counted once a day, with no ID. Account setup uses DNS and provider setup URLs; images load after your permission or sender trust; optional dictionaries and permitted or manual update checks contact GitHub; server-side away replies use your provider’s ManageSieve service. Requests reveal your IP address and time, and one setup URL includes your email address. Local search, link inspection and scam checks do not upload mail. The privacy page explains each connection.',
      },
      {
        id: 'spam-filter',
        q: 'Does Telex have its own spam filter?',
        a: 'From version 0.9.8, yes, besides whatever your provider filters. It learns on your PC from the mail in your Spam folders and the read mail you keep, and moves new Inbox mail it is sure about to Spam, with a note giving the score and the words that told. It never moves mail from people you have written to, senders whose images you trust, or answers to your own mail, and it does nothing until it has seen 50 messages of each kind. Mark as spam and Not spam in the message menu teach it. Settings › Mail sets it to careful (the default), normal or strict, or turns it off. Nothing is uploaded or looked up.',
      },
      {
        q: 'Does the scam check upload mail or open links?',
        a: 'No. The Rust mail engine checks warning signs locally when you open a message. It explains sender and brand lookalikes, misleading links and other supported signals without visiting websites, following redirects or uploading content. The score is advisory points out of 100, not a probability. Its brand catalogue and scan size are limited; no warning does not mean a message is safe. It does not verify attachments, link reputation, SPF, DKIM or DMARC, and does not move messages for you.',
      },
      {
        q: 'Can I inspect links in Spam?',
        a: 'Yes. Point at a link or focus it with Tab: its real destination shows in one line at the foot of the message, never over the mail. Right-click it (or Shift+F10) to copy the address. Hidden characters are made visible. Opening links remains blocked in Spam; move a message out of Spam before using them. Inspecting or copying an address does not contact its website.',
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
        a: 'Yes. The live demo runs the real Telex interface with a made-up mailbox: shortcuts, saved searches, rule previews, optional threads, toolbar layouts and themes work. It cannot reach mail servers or send real mail. Scam warning reasons and the spam filter’s note come from fixed examples; the desktop app’s Rust scanner and spam filter do not run in the browser. Your demo edits are reset when you reload.',
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
export const faq: Question[] = ['providers', 'gmail', 'spam-filter', 'privacy'].map(
  (id) => faqGroups.flatMap((g) => g.items).find((q) => q.id === id)!,
);

/** Lines the telex tape above the footer punches, one after another (letters, figures and . , : ? ' / + = - ( ) only). */
export const ticker = [
  'ZCZC TELEX FOR WINDOWS AND LINUX',
  'FAST KEYBOARD MAIL. LOCAL MAIL BASE. NO TRACKING.',
  'TRY IT IN YOUR BROWSER: SEE THE LIVE DEMO',
  'TYPE YOUR ADDRESS. TELEX FINDS THE SERVERS.',
  'SAVE A SEARCH. PREVIEW A RULE. EXPAND A THREAD.',
  'A SPAM FILTER THAT LEARNS ON YOUR PC. NOTHING UPLOADED.',
  'CLICK A MISSPELT WORD: SUGGESTIONS IN YOUR LANGUAGES.',
  'ATTACHED? TELEX ASKS BEFORE YOU SEND WITHOUT THE FILE.',
  'INSPECT LINKS. SCAM WARNING SIGNS ARE CHECKED ON YOUR PC.',
  'FIVE TOOLBAR LAYOUTS: PICK YOUR WAY TO WORK.',
  'GMAIL: WORKS WITH AN APP PASSWORD. NEXT: MICROSOFT SIGN-IN',
  'EIGHT THEMES, ALL WCAG 2.2. SIX FONTS. ENGLISH AND POLISH.',
  'TRY: SPACE = NEXT UNREAD, R = REPLY, T = CONVERSATION, / = SEARCH',
  'NNNN',
];
