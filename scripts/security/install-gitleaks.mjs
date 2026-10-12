// Pinned upstream binaries. Verify the archive before extracting the one executable.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const version = '8.30.1';
const archives = {
  'linux-x64': ['tar.gz', '551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb'],
  'win32-x64': ['zip', 'd29144deff3a68aa93ced33dddf84b7fdc26070add4aa0f4513094c8332afc4e'],
};
const selected = archives[`${process.platform}-${process.arch}`];
if (!selected) throw new Error('Automatic Gitleaks installation supports Windows/Linux x64. Install Gitleaks manually on other systems.');
const [extension, expected] = selected;
const platform = process.platform === 'win32' ? 'windows' : 'linux';
const name = `gitleaks_${version}_${platform}_x64.${extension}`;
const executable = process.platform === 'win32' ? 'gitleaks.exe' : 'gitleaks';
const response = await fetch(`https://github.com/gitleaks/gitleaks/releases/download/v${version}/${name}`);
if (!response.ok) throw new Error(`Gitleaks download failed: HTTP ${response.status}`);
const bytes = Buffer.from(await response.arrayBuffer());
if (createHash('sha256').update(bytes).digest('hex') !== expected) throw new Error('Gitleaks archive checksum mismatch');
const tools = join(root, '.bench/security-tools');
const bin = join(tools, 'bin');
mkdirSync(bin, { recursive: true });
const archive = join(tools, name);
writeFileSync(archive, bytes);
execFileSync('tar', ['-xf', archive, '-C', bin, executable]);
const installed = execFileSync(join(bin, executable), ['version'], { encoding: 'utf8' }).trim();
if (installed !== version) throw new Error('Unexpected Gitleaks version');
console.log(`Installed Gitleaks ${installed}; verified archive SHA-256 ${expected}`);
