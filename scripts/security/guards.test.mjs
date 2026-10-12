import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const read = path => readFileSync(resolve(root, path), 'utf8');

test('desktop CSP keeps scripts and webview network access local', () => {
  const config = JSON.parse(read('src-tauri/tauri.conf.json'));
  const policy = Object.fromEntries(config.app.security.csp.split(';').filter(s => s.trim()).map(s => {
    const [name, ...values] = s.trim().split(/\s+/);
    return [name, values];
  }));
  assert.deepEqual(policy['default-src'], ["'self'"]);
  assert.deepEqual(policy['script-src'], ["'self'"]);
  assert.deepEqual(policy['connect-src'], ['ipc:', 'http://ipc.localhost']);
  assert.deepEqual(policy['img-src'], ["'self'", 'data:']);
  assert.deepEqual(policy['frame-src'], ["'self'"]);
  for (const directive of ['object-src', 'base-uri', 'form-action']) {
    assert.deepEqual(policy[directive], ["'none'"]);
  }
});

test('mail frames cannot run scripts, submit forms or navigate the top window', () => {
  const frames = read('ui/app.js').match(/<iframe\b[^>]*>/g) ?? [];
  assert.ok(frames.length > 0, 'No mail frame found; review its security boundary');
  for (const frame of frames) {
    const sandbox = frame.match(/\bsandbox="([^"]*)"/);
    assert.ok(sandbox, 'Every mail frame needs a sandbox');
    const tokens = sandbox[1].trim().split(/\s+/).filter(Boolean);
    assert.ok(tokens.every(t => t === 'allow-same-origin'), `Unexpected sandbox permission: ${tokens}`);
    assert.match(frame, /\breferrerpolicy="no-referrer"/);
  }
});

test('Tauri plugin permissions do not grant shell, filesystem or remote-origin access', () => {
  const capabilities = JSON.parse(read('src-tauri/capabilities/default.json'));
  const allowed = new Set(['core:event:default', 'core:window:allow-set-title', 'core:window:allow-close',
    'core:window:allow-destroy', 'core:webview:allow-set-webview-zoom']);
  assert.equal(capabilities.remote, undefined);
  assert.ok(capabilities.permissions.every(p => allowed.has(p)), 'Review any new native permission');
  assert.deepEqual(capabilities.windows, ['main', 'message-*']);
});
