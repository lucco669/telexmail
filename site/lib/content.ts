// Page copy that changes as the app grows: features, keys, screenshots, roadmap, questions.
// Keep claims true to the current build (see CLAUDE.md, "Current state").

export const features = [
  {
    title: 'Dense by default',
    text: 'Twenty-pixel rows, a condensed face for the list and mono for dates and sizes, so a screen holds a lot of mail. Want more air? Switch to Comfortable.',
  },
  {
    title: 'One key per action',
    text: 'Reply, flag, delete, undo, filter, move through the list: each is a single key. The mouse is optional.',
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

export const facts = ['Rust core', 'WebView2', 'SQLite + FTS5', 'IMAP over TLS', 'No Electron', 'No telemetry'];

// `match` lists the KeyboardEvent combinations that light the row up in the keyboard section.
export type Shortcut = { id: string; keys: string[][]; action: string; match: { key: string; shift?: boolean; ctrl?: boolean }[] };

export const shortcuts: Shortcut[] = [
  { id: 'move', keys: [['↑'], ['↓']], action: 'Previous / next message', match: [{ key: 'ArrowUp' }, { key: 'ArrowDown' }] },
  { id: 'ends', keys: [['Home'], ['End']], action: 'First / last message', match: [{ key: 'Home' }, { key: 'End' }] },
  { id: 'reply', keys: [['R']], action: 'Reply', match: [{ key: 'r' }] },
  { id: 'replyall', keys: [['Shift', 'R']], action: 'Reply to all', match: [{ key: 'R', shift: true }] },
  { id: 'forward', keys: [['Shift', 'F']], action: 'Forward', match: [{ key: 'F', shift: true }] },
  { id: 'new', keys: [['N']], action: 'New message', match: [{ key: 'n' }] },
  { id: 'edit', keys: [['E']], action: 'Edit a draft or unsent message', match: [{ key: 'e' }] },
  { id: 'flag', keys: [['F']], action: 'Flag / unflag', match: [{ key: 'f' }] },
  { id: 'unread', keys: [['U']], action: 'Toggle unread', match: [{ key: 'u' }] },
  { id: 'delete', keys: [['Del']], action: 'Move to Trash · in Trash, twice to delete', match: [{ key: 'Delete' }] },
  { id: 'undo', keys: [['Z']], action: 'Undo last delete', match: [{ key: 'z' }] },
  { id: 'filter', keys: [['/']], action: 'Quick filter', match: [{ key: '/' }] },
  { id: 'esc', keys: [['Esc']], action: 'Clear filter · save draft and close', match: [{ key: 'Escape' }] },
  { id: 'send', keys: [['Ctrl', 'Enter']], action: 'Send from compose', match: [{ key: 'Enter', ctrl: true }] },
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
    caption: 'When a rule moves a message, the message says which rule and why. The rule editor is on the roadmap.',
  },
  {
    id: 'comfortable',
    label: 'Comfortable',
    title: 'Inbox · twoje.imie@gmail.com – Telex',
    caption: 'Compact is the default. One click on the toolbar gives every row more room.',
  },
];

export type Step = { title: string; text: string; status: 'done' | 'now' | 'next' | 'later' };

export const roadmap: Step[] = [
  {
    title: 'Real mail over IMAP',
    text: 'SSL/TLS with password login, folders from the server, sync on start and every 10 minutes, offline search.',
    status: 'done',
  },
  {
    title: 'Write back',
    text: 'Reading, flagging, moving and deleting reach the server. Changes wait in a queue that survives restarts and lost connections.',
    status: 'done',
  },
  {
    title: 'Sending',
    text: 'SMTP over SSL/TLS or STARTTLS, an Outbox that retries, a copy in Sent on the server, and E to fix and resend.',
    status: 'done',
  },
  {
    title: 'HTML mail',
    text: 'Rendered in a sandbox without scripts. Remote content stays blocked until you allow it, per message or per sender.',
    status: 'next',
  },
  {
    title: 'Filters',
    text: 'A Sorting Office: rules that sort, flag, move and auto-reply, run on arrival or on demand, each leaving a note on the message.',
    status: 'later',
  },
  { title: 'Gmail and Outlook.com', text: 'OAuth2 sign-in, since both refuse plain passwords over IMAP.', status: 'later' },
  { title: 'Templates and identities', text: 'Quote templates, signatures and several identities per account.', status: 'later' },
  { title: 'Updates and signing', text: 'Automatic updates and a code-signed installer.', status: 'later' },
];

export const faq = [
  {
    q: 'Can Telex send mail?',
    a: 'Yes, over SMTP with SSL/TLS or STARTTLS, using the same login as IMAP. Mail goes through an Outbox: if the connection drops, it waits and goes out at the next check, and a copy is saved in Sent on the server. A message only counts as sent once your server has accepted it.',
  },
  {
    q: 'Does it work with Gmail or Outlook.com?',
    a: 'Not yet. Both require OAuth2 sign-in instead of a password, and that is on the roadmap. Most other providers and hosting mailboxes work today over IMAP with SSL/TLS on port 993.',
  },
  {
    q: 'Does Telex send anything about me anywhere?',
    a: 'No. There is no account, no telemetry and no Telex server. The app talks to your mail server and nothing else.',
  },
  {
    q: 'Where is my mail kept?',
    a: 'In one file on your disk: %APPDATA%\\app.telexmail.desktop\\telex.db. Passwords are stored separately, in Windows Credential Manager.',
  },
  {
    q: 'Is it like The Bat!?',
    a: 'That is the inspiration: a fast, dense, keyboard-driven client with serious filters. Telex is a new app written from scratch, and not affiliated with Ritlabs.',
  },
];
