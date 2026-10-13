import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import vm from 'node:vm';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const read = path => readFileSync(resolve(root, path), 'utf8');
const linkWindow = {};
vm.runInNewContext(read('ui/links.js'), {window:linkWindow, URL});
const {inspect, visible} = linkWindow.TelexLinks;

test('link preview reveals the actual host of misleading login and subdomain URLs', () => {
  assert.equal(inspect('https://spotify.com@billing.example/login').destination, 'billing.example');
  assert.equal(inspect('https://spotify.com@billing.example/login').credentials, true);
  assert.equal(inspect('https://spotify.com.billing.example/login').destination, 'spotify.com.billing.example');
  assert.equal(inspect('https://billing.example/?next=https://spotify.com').destination, 'billing.example');
  assert.equal(inspect('https://billing.example:8443/login').destination, 'billing.example:8443');
});

test('international hostnames are displayed in their ASCII form without a safety verdict', () => {
  const info = inspect('https://sp\u043etify.com/login'); // Cyrillic o
  assert.equal(info.destination, 'xn--sptify-xqf.com');
  assert.equal(inspect('https://żółw.pl').destination, 'xn--w-uga1v8h.pl');
});

test('invisible and bidirectional characters cannot conceal a displayed destination', () => {
  const href = 'https://billing.example/\u202ecom.yfitops\u200b?token=abc';
  const info = inspect(href);
  assert.equal(info.href, href, 'copy preserves the exact original address');
  assert.equal(info.destination, 'billing.example');
  assert.equal(info.display, 'https://billing.example/\\u202ecom.yfitops\\u200b?token=abc');
  assert.equal(info.hidden, true);
  assert.equal(visible('Spot\u200bify\u2066'), 'Spot\\u200bify\\u2066');
});

test('link inspection excludes executable, relative and malformed URLs', () => {
  for (const href of ['javascript:alert(1)', 'data:text/html,hi', 'file:///C:/x.exe', 'ftp://example.com', '/login', '//example.com', 'https://', 'https://%zz']) {
    assert.equal(inspect(href), null, href);
  }
  assert.equal(inspect('mailto:user@example.com?subject=Hello').destination, 'user@example.com');
});

test('copy keeps tracking and signed parameters intact', () => {
  const href = 'https://mail.example/click?token=a%2Fb%2Bcd&target=https%3A%2F%2Fexample.com#frag';
  assert.equal(inspect(href).href, href);
  assert.equal(inspect(href).destination, 'mail.example');
});

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
