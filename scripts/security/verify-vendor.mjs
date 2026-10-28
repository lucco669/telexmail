// Path overrides are checked byte for byte. An additional registry-identity lock lets
// RustSec still report advisories against these upstream Rust packages, without ignores.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFileSync,readdirSync,lstatSync,mkdirSync,writeFileSync} from 'node:fs';
import {resolve,relative,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'../..');
const manifest=JSON.parse(readFileSync(resolve(root,'scripts/security/vendor-sources.json'),'utf8'));
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
function files(dir){return readdirSync(dir).flatMap(name=>{
 const path=resolve(dir,name),info=lstatSync(path);assert.ok(!info.isSymbolicLink(),'Vendor sources must not contain symlinks');
 return info.isDirectory()?files(path):[relative(root,path).replaceAll('\\','/')];
}).sort();}
const directories=[...manifest.packages.map(p=>p.directory),'vendor/native-licenses'];
assert.deepEqual(readdirSync(resolve(root,'vendor')).sort(),['glib',...directories.map(d=>d.split('/').at(-1))].sort(),'Unexpected vendor directory');
const actual=directories.flatMap(d=>files(resolve(root,d))).sort();
assert.deepEqual(actual,manifest.files.map(file=>file.path).sort(),'Vendor source inventory changed');
for(const {path,sha256} of manifest.files) assert.equal(hash(readFileSync(resolve(root,path))),sha256,`Vendor source changed: ${path}`);
for(const archive of manifest.nativeArchives) assert.equal(hash(readFileSync(resolve(root,archive.path))),archive.sha256,`Native source changed: ${archive.path}`);
if(process.argv.includes('--audit-lock')) {
 let lock=readFileSync(resolve(root,'Cargo.lock'),'utf8');
 lock=lock.split('[[package]]').map(block=>{
  const name=block.match(/^name = "([^"]+)"/m)?.[1],version=block.match(/^version = "([^"]+)"/m)?.[1];
  const pkg=manifest.packages.find(p=>p.name===name&&p.version===version);
  if(!pkg)return block;
  assert.doesNotMatch(block,/^source =/m,`${name} must use the verified source override`);
  return block.replace(/^version = "[^"]+"/m, match=>`${match}\nsource = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "${pkg.archiveSha256}"`);
 }).join('[[package]]');
 mkdirSync(resolve(root,'.bench/security'),{recursive:true});
 writeFileSync(resolve(root,'.bench/security/vendor-reference.lock'),lock);
}
console.log(`Verified ${manifest.packages.length} source overrides and ${manifest.nativeArchives.length} native archives; upstream registry identities retained for advisory scanning.`);
