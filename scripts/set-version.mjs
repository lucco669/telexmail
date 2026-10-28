// Sets Telex's version everywhere it is written down, or checks that they all agree.
//
//   node scripts/set-version.mjs 0.2.0         set it (npm run set-version -- 0.2.0)
//   node scripts/set-version.mjs --check       check that every file has the same version
//   node scripts/set-version.mjs --check v0.2.0   ... and that it is this one (the release workflow)
//
// The files: package.json and package-lock.json, src-tauri/tauri.conf.json (the version the app
// reports and the updater compares), the workspace Cargo.toml and both Cargo.lock files,
// including the translation macro crate. Only version text changes; formatting stays as it is.

import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

// Each regex captures (text before)(the version)(text after).
const places = [
  { file: 'package.json', re: /^(\s*"version":\s*")([^"]*)(")/m },
  { file: 'package-lock.json', re: /^(\s*"version":\s*")([^"]*)(")/m },
  { file: 'package-lock.json', re: /("packages":\s*\{\s*"":\s*\{\s*"name":\s*"[^"]*",\s*"version":\s*")([^"]*)(")/ },
  { file: 'src-tauri/tauri.conf.json', re: /^(\s*"version":\s*")([^"]*)(")/m },
  { file: 'Cargo.toml', re: /(\[workspace\.package\][^[]*?^version\s*=\s*")([^"]*)(")/m },
  { file: 'Cargo.lock', re: /(^name = "telex"\r?\nversion = ")([^"]*)(")/m },
  { file: 'Cargo.lock', re: /(^name = "telex-core"\r?\nversion = ")([^"]*)(")/m },
  { file: 'Cargo.lock', re: /(^name = "telex-i18n"\r?\nversion = ")([^"]*)(")/m },
  { file: 'fuzz/Cargo.lock', re: /(^name = "telex-core"\r?\nversion = ")([^"]*)(")/m },
  { file: 'fuzz/Cargo.lock', re: /(^name = "telex-i18n"\r?\nversion = ")([^"]*)(")/m },
];

const read = (file) => readFileSync(join(root, file), 'utf8');
function current() {
  return places.map(({ file, re }) => {
    const m = read(file).match(re);
    if (!m) fail(`Can't find the version in ${file}. Has its layout changed?`);
    return { file, version: m[2] };
  });
}
function fail(message) {
  console.error(message);
  process.exit(1);
}

// MAJOR.MINOR.PATCH only. The MSI installer allows at most 255.255.65535 and no text
// pre-release, and GitHub's "latest release", which the updater asks for, skips pre-releases.
function parse(v) {
  const m = /^(\d+)\.(\d+)\.(\d+)$/.exec(v);
  if (!m || +m[1] > 255 || +m[2] > 255 || +m[3] > 65535 || m.slice(1).some((n) => n.length > 1 && n[0] === '0')) {
    fail(`"${v}" isn't a version Telex can use. Write it as MAJOR.MINOR.PATCH, e.g. 0.2.0 (at most 255.255.65535).`);
  }
  return m.slice(1).map(Number);
}
// Whether version a (as numbers) comes before b.
function older(a, b) {
  for (let i = 0; i < 3; i++) if (a[i] !== b[i]) return a[i] < b[i];
  return false;
}

const args = process.argv.slice(2);
if (args[0] === '--check') {
  const found = current();
  const wanted = args[1]?.replace(/^v/, '') ?? found[0].version;
  const wrong = found.filter((p) => p.version !== wanted);
  if (wrong.length) {
    fail(
      `The version should be ${wanted} everywhere, but:\n` +
        [...new Set(wrong.map((p) => `  ${p.file} says ${p.version}`))].join('\n') +
        `\nRun: npm run set-version -- ${wanted}, commit, and tag that commit.`,
    );
  }
  parse(wanted);
  console.log(`Version ${wanted} in every file.`);
} else if (args.length === 1 && !args[0].startsWith('-')) {
  const next = args[0].replace(/^v/, '');
  const nextParts = parse(next);
  const before = current();
  const now = before.find((p) => p.file === 'src-tauri/tauri.conf.json').version;
  if (/^\d+\.\d+\.\d+$/.test(now) && older(nextParts, now.split('.').map(Number))) {
    fail(`${next} is older than the current ${now}. The updater only installs newer versions.`);
  }
  if (before.every((p) => p.version === next)) {
    console.log(`Already ${next} in every file.`);
    process.exit(0);
  }
  const files = [...new Set(places.map((p) => p.file))];
  for (const file of files) {
    let text = read(file);
    for (const { re } of places.filter((p) => p.file === file)) text = text.replace(re, `$1${next}$3`);
    if (file.endsWith('.json')) JSON.parse(text); // still valid
    writeFileSync(join(root, file), text);
  }
  const changed = current();
  if (changed.some((p) => p.version !== next)) fail('Some files didn\'t take the new version. Check them by hand.');
  console.log(`Version ${now} -> ${next} in ${files.join(', ')}.`);
  console.log(`Next: commit ("Release ${next}"), push, then tag and push the tag:`);
  console.log(`  git tag -a v${next} -m "What's new in ${next}"`);
  console.log(`  git push origin v${next}`);
} else {
  fail('Usage: node scripts/set-version.mjs 0.2.0  |  node scripts/set-version.mjs --check [v0.2.0]');
}
