// Local path dependencies are outside RustSec's registry matching. Verify our complete,
// checksum-recorded backport, rather than suppressing the advisory or changing its version.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFileSync, readdirSync, lstatSync} from 'node:fs';
import {resolve, relative, dirname} from 'node:path';
import {fileURLToPath} from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const directory = resolve(root, 'vendor/glib');
const manifest = JSON.parse(readFileSync(resolve(root, 'scripts/security/glib-backport.json')));
function files(dir){
  return readdirSync(dir).flatMap(name => {
    const path = resolve(dir, name), info = lstatSync(path);
    assert.ok(!info.isSymbolicLink(), 'The backport must not contain symlinks');
    return info.isDirectory() ? files(path) : [relative(directory, path).replaceAll('\\', '/')];
  }).sort();
}
assert.deepEqual(files(directory), Object.keys(manifest.files).sort(), 'GLib backport file inventory changed');
for (const [file, expected] of Object.entries(manifest.files)){
  const actual = createHash('sha256').update(readFileSync(resolve(directory, file))).digest('hex');
  assert.equal(actual, expected, `GLib backport changed: ${file}`);
}
const iterator = readFileSync(resolve(directory, 'src/variant_iter.rs'), 'utf8');
assert.match(iterator, /let mut p: \*mut libc::c_char = std::ptr::null_mut\(\);/);
assert.match(iterator, /s as \*const u8 as \*const _,\s*&mut p,/);
const cargo = readFileSync(resolve(root, 'Cargo.toml'), 'utf8');
assert.match(cargo, /\[patch\.crates-io\][\s\S]*?glib = \{ path = "vendor\/glib" \}/);
const lock = readFileSync(resolve(root, 'Cargo.lock'), 'utf8');
const glib = lock.split('[[package]]').filter(p => /^\s*name = "glib"$/m.test(p));
assert.equal(glib.length, 1, 'Expected one GLib dependency');
assert.match(glib[0], /version = "0\.18\.5"/);
assert.doesNotMatch(glib[0], /^source =/m, 'GLib must resolve to the verified local backport');
console.log('Verified GLib 0.18.5 backport of RUSTSEC-2024-0429 (full source hashes; unchanged API/version).');
