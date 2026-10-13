import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {dirname,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
const root=resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const read=path=>readFileSync(resolve(root,path),'utf8');

test('a release cannot build or create its draft before the reusable security gate', () => {
  const release = read('.github/workflows/release.yml');
  assert.match(release, /security:\s*\n\s+name: Release security gate\s*\n\s+uses: \.\/\.github\/workflows\/security\.yml/);
  assert.match(release, /draft:\s*\n\s+name: Draft release\s*\n\s+needs: security/);
  assert.match(release, /release:\s*\n\s+name:[^\n]+\n\s+needs: draft/);
  assert.doesNotMatch(release, /continue-on-error:\s*true|^    if:\s*always\(\)/m);
  const checks = read('.github/workflows/security.yml');
  assert.match(checks, /workflow_call:/);
  assert.match(checks, /run-webview\.ps1/);
  assert.match(checks, /run-webview\.sh/);
  assert.match(checks, /cargo fuzz run mime_html/);
  assert.match(checks, /cargo fuzz run pst_import/);
  const lock=read('Cargo.lock');
  const ammonia=lock.split('[[package]]').find(p=>/^\s*name = "ammonia"$/m.test(p));
  const version=ammonia.match(/^version = "(\d+)\.(\d+)\.(\d+)"/m).slice(1).map(Number);
  assert.ok(version[0]>4 || version[0]===4 && (version[1]>2 || version[1]===2 && version[2]>=3),'The sanitizer must include the CSS-injection fix');
});
