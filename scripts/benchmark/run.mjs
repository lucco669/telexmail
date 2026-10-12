// One command for a complete, repeatable Telex run. Other clients are verified separately.
import { spawnSync } from 'node:child_process';
import { cpSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
if (process.platform !== 'win32') throw new Error('This desktop/resource benchmark currently targets Windows. The core scale test also runs on Linux.');
const work = join(repo, '.bench');
mkdirSync(work, {recursive:true});
const run = (command, args, options = {}) => {
  const result = spawnSync(command, args, {cwd:repo, stdio:'inherit', ...options});
  if (result.error || result.status !== 0) throw result.error ?? new Error(`${command} failed with exit ${result.status}`);
  return result;
};
const core = run('cargo', ['test', '-p', 'telex-core', '--release', '--test', 'scale', '--', '--ignored', '--nocapture'], {
  env:{...process.env, TELEX_SCALE_DB:join(work, 'telex.db'), TELEX_SCALE_EXPORT:join(work, 'mailbox')},
  stdio:['inherit','pipe','inherit'], encoding:'utf8', maxBuffer:1024 * 1024,
});
writeFileSync(join(work, 'core.txt'), core.stdout);
console.log(core.stdout);
run(process.execPath, ['scripts/benchmark/prepare.mjs']);
const tauri = join(repo, 'node_modules/@tauri-apps/cli/tauri.js');
try {
  run(process.execPath, [tauri, 'build', '--config', join(work, 'tauri.conf.json'), '--no-bundle']);
  cpSync(join(repo, 'target/release/telex.exe'), join(work, 'telex-benchmark.exe'));
  run('pwsh', ['-NoProfile', '-File', join(repo, 'scripts/benchmark/run-telex.ps1'), '-Runs', '5']);
} finally {
  // Restore the ordinary binary even if measurement fails; the benchmark copy stays in .bench.
  run(process.execPath, [tauri, 'build', '--no-bundle']);
}
console.log('Telex measurements saved. Verify and measure other clients, then run npm run benchmark:publish.');
