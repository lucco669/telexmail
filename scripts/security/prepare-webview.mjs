// Build a separate app identity with the production CSP, sandbox and Rust IPC commands.
// Test scripts and synthetic mail are kept in .bench and never bundled in a release.
import {spawnSync} from 'node:child_process';
import {readFileSync, writeFileSync, mkdirSync, cpSync, mkdtempSync} from 'node:fs';
import {basename, dirname, join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const work = join(root, '.bench');
mkdirSync(work, {recursive:true});
const fixture = mkdtempSync(join(work, 'webview-fixtures-'));
const generated = spawnSync('cargo', ['run', '-p', 'telex-security', '--locked', '--', 'fixture', fixture], {cwd:root, stdio:'inherit'});
if (generated.status !== 0) throw new Error('Security fixture generation failed');
const identifier = 'app.telexmail.securitycheck.' + basename(fixture).split('-').at(-1).toLowerCase();
const data = process.platform === 'win32' ? process.env.APPDATA : process.env.XDG_DATA_HOME || join(process.env.HOME, '.local/share');
const profile = join(data, identifier);
mkdirSync(profile, {recursive:true});
cpSync(join(fixture, 'telex.db'), join(profile, 'telex.db'));
writeFileSync(join(profile, 'settings.json'), JSON.stringify({language:'en', theme:'light', mark_read:'manual', check_every:0, push_mail:false, notify:false, updates:'never', backup_every:'off'}));
const uiName = 'security-ui-' + process.platform;
const ui = join(work, uiName);
mkdirSync(ui, {recursive:true});
cpSync(join(root, 'ui'), ui, {recursive:true});
cpSync(join(root, 'scripts/security/webview.js'), join(ui, 'security-test.js'));
writeFileSync(join(ui, 'security-corpus.js'), `window.TELEX_SECURITY_CORPUS=${readFileSync(join(fixture, 'corpus.json'), 'utf8')};\n`);
let index = readFileSync(join(ui, 'index.html'), 'utf8');
index = index.replace(/<script\b[^>]*src="app\.js"[^>]*><\/script>/, '<script src="security-corpus.js"></script><script src="security-test.js"></script>');
if (!index.includes('security-test.js')) throw new Error('Security test UI hook no longer matches');
writeFileSync(join(ui, 'index.html'), index);
writeFileSync(join(work, 'security-tauri-' + process.platform + '.conf.json'), JSON.stringify({identifier, productName:'Telex Security Check', build:{frontendDist:'../.bench/' + uiName}, app:{windows:[{label:'main', title:'Telex Security Check', width:900, height:650, visible:process.platform !== 'win32'}]}}, null, 2));
console.log('Prepared an isolated securitycheck profile and webview harness.');
