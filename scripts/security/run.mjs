// Run every check, preserve failures, and write evidence even when a tool fails.
// Run via `npm run security` so npm's own CLI path is available on Windows too.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { appendFileSync, copyFileSync, existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { npmResult, rustResult, secretsResult } from './reports.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const out = join(root, '.bench/security');
mkdirSync(out, { recursive: true });
const localBin = join(root, '.bench/security-tools/bin');
const localTool = name => {
  const path = join(localBin, name + (process.platform === 'win32' ? '.exe' : ''));
  return existsSync(path) ? path : name;
};
const invoke = (program, args, cwd = root) => spawnSync(program, args, {
  cwd, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024, timeout: 15 * 60 * 1000,
  env: { ...process.env, CARGO_TERM_COLOR: 'never', NO_COLOR: '1' },
});
const git = args => {
  const result = invoke('git', args);
  if (result.status !== 0) throw new Error('Could not identify the Git source being checked');
  return result.stdout;
};
const sourceFiles = () => git(['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split('\0').filter(Boolean).sort();
const sourceDigest = files => {
  const hash = createHash('sha256');
  for (const file of files) {
    const path = resolve(root, file);
    const inside = relative(root, path);
    if (inside.startsWith('..') || lstatSync(path).isSymbolicLink()) throw new Error('Source snapshot must contain only files inside the repository');
    hash.update(file + '\0').update(createHash('sha256').update(readFileSync(path)).digest()).update('\0');
  }
  return hash.digest('hex');
};
const files = sourceFiles();
const digest = sourceDigest(files);
const report = {
  schemaVersion: 1,
  startedAt: new Date().toISOString(),
  source: {
    commit: git(['rev-parse', 'HEAD']).trim(),
    workingTreeDirty: git(['status', '--porcelain=v1']).trim() !== '',
    snapshotSha256: digest,
    files: files.length,
    lockfiles: Object.fromEntries(['Cargo.lock', 'package-lock.json', 'site/package-lock.json'].map(file =>
      [file, createHash('sha256').update(readFileSync(join(root, file))).digest('hex')])),
  },
  platform: `${process.platform}-${process.arch}`,
  runUrl: process.env.GITHUB_RUN_ID
    ? `${process.env.GITHUB_SERVER_URL}/${process.env.GITHUB_REPOSITORY}/actions/runs/${process.env.GITHUB_RUN_ID}` : null,
  checks: [],
};
const version = (program, args) => {
  const result = invoke(program, args);
  return result.status === 0 ? result.stdout.trim() : 'unavailable';
};
report.tools = { node: process.version,
  npm: process.env.npm_execpath ? version(process.execPath, [process.env.npm_execpath, '--version']) : 'unavailable',
  rust: version('rustc', ['--version']),
  cargoAudit: version(localTool('cargo-audit'), ['--version']), gitleaks: version(localTool('gitleaks'), ['version']) };

function check(id, command, program, args, cwd, classify, log = false) {
  const startedAt = new Date().toISOString();
  console.log(`Checking ${id}...`);
  const result = invoke(program, args, cwd);
  let details;
  try {
    if (result.error) throw result.error;
    details = classify(result.stdout, result.status);
  } catch {
    details = { status: 'error', reason: 'The tool failed to produce a completed result. Run the listed command to inspect the error.' };
  }
  if (log) writeFileSync(join(out, `${id}.log`), (result.stdout ?? '') + (result.stderr ?? ''));
  const item = { id, command, startedAt, finishedAt: new Date().toISOString(), exitCode: result.status, ...details };
  report.checks.push(item);
  writeFileSync(join(out, `${id}.json`), JSON.stringify(item, null, 2) + '\n');
  console.log(`${id}: ${item.status}`);
}

check('rust-dependencies', 'cargo audit --deny unsound --json', localTool('cargo-audit'), ['audit', '--deny', 'unsound', '--json'], root, rustResult);
for (const [id, directory] of [['npm-app', root], ['npm-site', join(root, 'site')]]) {
  if (!process.env.npm_execpath) throw new Error('Run this script with npm run security');
  check(id, `npm audit --json${id === 'npm-site' ? ' (in site/)' : ''}`, process.execPath,
    [process.env.npm_execpath, 'audit', '--json'], directory, npmResult);
}

const leakArgs = ['--redact=100', '--no-banner', '--no-color', '--ignore-gitleaks-allow', '--report-format', 'json', '--report-path', '-', '--log-level', 'error'];
check('secrets-history', 'gitleaks git --log-opts=--all --redact=100', localTool('gitleaks'),
  ['git', '--log-opts=--all', ...leakArgs, root], root, secretsResult);

// Copy only Git-tracked and non-ignored source, including local edits; skip build/profile files.
// The isolated copy is retained in ignored .bench/ for local inspection, never uploaded by CI.
const snapshot = mkdtempSync(join(root, '.bench/security-source-'));
for (const file of files) {
  const destination = join(snapshot, file);
  mkdirSync(dirname(destination), { recursive: true });
  copyFileSync(join(root, file), destination);
}
check('secrets-source', 'gitleaks dir --redact=100 <source snapshot>', localTool('gitleaks'),
  ['dir', ...leakArgs, snapshot], root, secretsResult);

const testResult = (stdout, exitCode) => ({ status: exitCode === 0 ? 'passed' : 'failed' });
check('security-guards', 'node --test scripts/security/guards.test.mjs scripts/security/reports.test.mjs', process.execPath,
  ['--test', 'scripts/security/guards.test.mjs', 'scripts/security/reports.test.mjs'], root, testResult, true);
check('core-tests', 'cargo test -p telex-core --locked --lib', 'cargo',
  ['test', '-p', 'telex-core', '--locked', '--lib'], root, testResult, true);

try {
  if (sourceDigest(sourceFiles()) !== digest) throw new Error('Source changed');
} catch {
  report.checks.push({ id: 'source-stability', status: 'error',
    reason: 'Source changed while the checks were running; rerun against a stable snapshot.' });
}
report.finishedAt = new Date().toISOString();
report.status = report.checks.some(c => ['failed', 'error'].includes(c.status)) ? 'failed'
  : report.checks.some(c => c.status === 'passed_with_warnings') ? 'passed_with_warnings' : 'passed';
writeFileSync(join(out, 'report.json'), JSON.stringify(report, null, 2) + '\n');
const summary = [
  '# Telex security checks', '',
  `Result: **${report.status}**. Source: \`${report.source.commit}\`. Working tree dirty: ${report.source.workingTreeDirty}.`,
  `Started: ${report.startedAt}. Finished: ${report.finishedAt}. Platform: ${report.platform}.`, '',
  '| Check | Result |', '| --- | --- |', ...report.checks.map(c => `| ${c.id} | ${c.status} |`), '',
  ...report.checks.flatMap(c => (c.advisories ?? []).map(a => `- ${a.url ? `[${a.id}](${a.url}): ` : ''}${a.package} ${a.version} (${a.kind})`)), '',
  'Dependency scans use current advisory services. Secret scans use Gitleaks patterns; matched values and contextual source text are omitted.',
  'These checks do not establish installer provenance, absence of telemetry, antivirus clearance, or an independent audit.', '',
].join('\n');
writeFileSync(join(out, 'summary.md'), summary);
if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, summary);
console.log(`Evidence: ${join(out, 'report.json')}`);
process.exitCode = report.status === 'failed' ? 1 : 0;
