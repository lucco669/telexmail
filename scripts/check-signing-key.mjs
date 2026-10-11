// Checks that the release signing key and the public key Telex is built with belong together.
// The release workflow runs it before building: the bundler only warns about a mismatch, and
// every installed Telex refuses an update signed with another key.
//
//   node scripts/check-signing-key.mjs
//
// Reads TAURI_SIGNING_PRIVATE_KEY (the key, or a file holding it, as the bundler takes it),
// TAURI_SIGNING_PRIVATE_KEY_PASSWORD and TELEX_UPDATER_PUBKEY,
// signs a scratch file with `tauri signer sign` and verifies the signature with the public key
// the way the updater does (minisign: Ed25519 over a BLAKE2b-512 hash of the file).

import { execFileSync } from 'node:child_process';
import { createHash, createPublicKey, verify } from 'node:crypto';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = join(dirname(fileURLToPath(import.meta.url)), '../node_modules/@tauri-apps/cli/tauri.js');

// On GitHub Actions the reason also becomes an annotation, shown on the run's page without
// opening the log.
function fail(message) {
  if (process.env.GITHUB_ACTIONS) {
    console.log(`::error title=Signing key check::${message.replace(/%/g, '%25').replace(/\r/g, '%0D').replace(/\n/g, '%0A')}`);
  } else {
    console.error(message);
  }
  process.exit(1);
}
// Tauri keeps minisign's text files base64-encoded. Their second line is the key or the
// signature, base64 again.
const minisignLine = (text) =>
  Buffer.from(Buffer.from(text.trim(), 'base64').toString('utf8').split(/\r?\n/)[1] ?? '', 'base64');

const where = 'under Settings > Secrets and variables > Actions';
const pub = process.env.TELEX_UPDATER_PUBKEY?.trim();
if (!pub) {
  fail(`TELEX_UPDATER_PUBKEY is empty. Add it ${where} (Variables tab, or as a secret), with the whole contents of the .key.pub file.`);
}
const given = process.env.TAURI_SIGNING_PRIVATE_KEY ?? '';
// The release workflow passes a file, so the key isn't in every step's environment.
const secret = existsSync(given) ? readFileSync(given, 'utf8') : given;
if (!secret.trim()) fail(`TAURI_SIGNING_PRIVATE_KEY is empty. Add it as a secret ${where}, with the whole contents of the .key file.`);
// Tauri's signer can't read the key with a line break around it, which copying often adds.
if (secret !== secret.trim()) {
  fail(`TAURI_SIGNING_PRIVATE_KEY starts or ends with a line break or space. Update the secret ${where} with the key alone (open the .key file in Notepad, Ctrl+A, Ctrl+C).`);
}
const pk = minisignLine(pub); // "Ed", key id (8 bytes), Ed25519 key (32)
if (pk.length !== 42) fail("TELEX_UPDATER_PUBKEY isn't a Tauri public key. Use the whole contents of the .key.pub file.");

const dir = mkdtempSync(join(tmpdir(), 'telex-keycheck-'));
try {
  const file = join(dir, 'check.txt');
  writeFileSync(file, `Telex signing key check, ${new Date().toISOString()}\n`);
  try {
    // `signer sign` takes only the key itself.
    execFileSync(process.execPath, [cli, 'signer', 'sign', file], {
      stdio: ['ignore', 'ignore', 'pipe'],
      env: { ...process.env, TAURI_SIGNING_PRIVATE_KEY: secret },
    });
  } catch (e) {
    fail(`Couldn't sign with TAURI_SIGNING_PRIVATE_KEY: the key or its password (TAURI_SIGNING_PRIVATE_KEY_PASSWORD) is wrong.\n${e.stderr ?? e.message}`);
  }
  const sig = minisignLine(readFileSync(`${file}.sig`, 'utf8')); // "ED", key id (8), signature (64)
  const data = readFileSync(file);
  const key = createPublicKey({ key: { kty: 'OKP', crv: 'Ed25519', x: pk.subarray(10).toString('base64url') }, format: 'jwk' });
  const signed = sig.subarray(0, 2).toString() === 'ED' ? createHash('blake2b512').update(data).digest() : data;
  const pair = sig.length === 74 && sig.subarray(2, 10).equals(pk.subarray(2, 10)) && verify(null, signed, key, sig.subarray(10));
  const id = (b) => Buffer.from(b.subarray(2, 10)).reverse().toString('hex').toUpperCase();
  if (!pair) {
    fail(
      `The private key (key id ${id(sig)}) and TELEX_UPDATER_PUBKEY (key id ${id(pk)}) aren't a pair. ` +
        'Installed copies of Telex would refuse this release. Use the .key and .key.pub files from the same `tauri signer generate`.',
    );
  }
  console.log(`The signing key and TELEX_UPDATER_PUBKEY are a pair (key id ${id(pk)}).`);
} finally {
  rmSync(dir, { recursive: true, force: true });
}
