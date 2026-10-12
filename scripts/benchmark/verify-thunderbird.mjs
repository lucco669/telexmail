// Read only the isolated profile's index. Does not read a user's real Thunderbird profile.
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { DatabaseSync } from 'node:sqlite';

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const work = join(repo, '.bench');
const fixture = JSON.parse(readFileSync(join(work, 'mailbox/fixture.json'), 'utf8'));
const index = join(work, 'thunderbird/global-messages-db.sqlite');
if (!existsSync(index)) throw new Error('Thunderbird has not created its search index yet');
const db = new DatabaseSync(index, {readOnly:true});
// Deleted / moved rows can remain in Gloda. Only live rows in the fixture folders count.
const folderColumns = db.prepare('PRAGMA table_info(folderLocations)').all().map(column => column.name);
if (!folderColumns.includes('folderURI')) throw new Error('Unknown Thunderbird index schema');
const folders = db.prepare('SELECT id, folderURI, name, indexingPriority FROM folderLocations').all();
const verified = [], pending = [];
for (const folder of fixture.folders) {
  const expected = folders.find(row => decodeURIComponent(row.folderURI).endsWith('/' + folder.key));
  const count = expected ? db.prepare('SELECT COUNT(*) AS count FROM messages WHERE folderID = ? AND deleted = 0').get(expected.id).count : 0;
  if (count !== folder.messages) pending.push(`${folder.key}: ${count}/${folder.messages}`);
  verified.push({key:folder.key, messages:count});
}
db.close();
if (pending.length) {
  console.error(`Indexing incomplete: ${verified.reduce((n, folder) => n + folder.messages, 0)}/${fixture.messages} messages. ${pending.join('; ')}`);
  process.exit(1);
}
const report = {fixtureId:fixture.id, messages:verified.reduce((n, folder) => n + folder.messages, 0),
  processId:Number(readFileSync(join(work, 'thunderbird.pid'), 'utf8').trim()),
  indexingComplete:true, verifiedAt:new Date().toISOString(), method:'Live rows in the isolated Thunderbird Gloda index, checked per fixture folder', folders:verified};
writeFileSync(join(work, 'thunderbird-verified.json'), JSON.stringify(report, null, 2));
console.log('Verified: all 100,000 messages are searchable in the 12 expected folders.');
