import { cpSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const work = join(repo, '.bench');
const fixture = JSON.parse(readFileSync(join(work, 'mailbox/fixture.json'), 'utf8'));
const profile = join(work, 'thunderbird');
const mail = join(profile, 'Mail/Local Folders');
mkdirSync(mail, { recursive: true });
for (const folder of fixture.folders) {
  const [account, name] = folder.key.split('/');
  const parent = join(mail, `${account}.sbd`);
  mkdirSync(parent, { recursive: true });
  writeFileSync(join(mail, account), '');
  cpSync(join(work, 'mailbox', folder.file), join(parent, name));
}
const prefs = {
  'mail.accountmanager.accounts': 'account1',
  'mail.accountmanager.defaultaccount': 'account1',
  'mail.accountmanager.localfoldersserver': 'server1',
  'mail.account.account1.server': 'server1',
  'mail.server.server1.type': 'none',
  'mail.server.server1.hostname': 'Local Folders',
  'mail.server.server1.name': 'Benchmark · 100,000 messages',
  'mail.server.server1.userName': 'nobody',
  'mail.server.server1.directory': mail,
  'mail.server.server1.directory-rel': '[ProfD]Mail/Local Folders',
  'mail.server.server1.check_new_mail': false,
  'mail.server.server1.login_at_startup': false,
  'mail.shell.checkDefaultClient': false,
  'mailnews.start_page.enabled': false,
  'mailnews.start_page.url': 'about:blank',
  'mail.provider.enabled': false,
  'mail.rights.override': true,
  'mail.startup.enabledMailCheckOnce': false,
  'mailnews.mark_message_read.auto': false,
  'mailnews.display.prefer_plaintext': true,
  'mailnews.display.html_as': 1,
  'mailnews.default_sort_order': 2,
  'mailnews.default_view_flags': 0,
  'mail.threadpane.listview': 1,
};
writeFileSync(join(profile, 'user.js'), Object.entries(prefs).map(([key, value]) => `user_pref(${JSON.stringify(key)}, ${JSON.stringify(value)});`).join('\n'));
writeFileSync(join(profile, 'xulstore.json'), JSON.stringify({
  'chrome://messenger/content/messenger.xhtml': { messengerWindow: {width:'1280', height:'800', sizemode:'normal'} },
}));
writeFileSync(join(profile, 'benchmark-fixture.json'), JSON.stringify(fixture, null, 2));
console.log('Prepared .bench/thunderbird with the same 100,000 full messages.');
console.log('Launch thunderbird.exe -no-remote -profile <absolute .bench/thunderbird path> and allow indexing to finish.');
console.log('After verification, restart with -offline before recording idle resources.');
