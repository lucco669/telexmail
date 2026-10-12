import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export function distribution(values) {
  if (!values.length || values.some(value => !Number.isFinite(value) || value < 0)) throw new Error('Missing or invalid measurements');
  const sorted = [...values].sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return {samples:sorted.length, min:sorted[0], median:sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2,
    p95:sorted[Math.ceil(sorted.length * .95) - 1], max:sorted.at(-1)};
}

export function resources(runs, {foregroundOnly = false, backgroundOnly = false} = {}) {
  if (foregroundOnly && backgroundOnly) throw new Error('Choose one window state');
  const matched = foregroundOnly || backgroundOnly;
  if (!runs.length) throw new Error('No resource runs');
  const allSamples = runs.flatMap(run => run.samples);
  if (allSamples.some(sample => sample.errors.length || !sample.processes.length)) throw new Error('Incomplete process tree');
  const eligible = sample => !matched || (sample.visible && !sample.minimized && (backgroundOnly ? !sample.foreground : sample.foreground));
  const samples = allSamples.filter(eligible);
  if (samples.length < 2 || (matched && samples.length < 20)) throw new Error('Not enough matched window-state samples');
  if (matched && samples.some(sample => !(sample.windowWidth > 0 && sample.windowHeight > 0))) throw new Error('Missing window dimensions; rerun with the current collector');
  let cpuSeconds = 0, duration = 0;
  for (const run of runs) {
    for (let i = 1; i < run.samples.length; i++) {
      const previous = run.samples[i - 1], current = run.samples[i];
      // Do not count background gaps or intervals crossing a foreground change.
      if (!eligible(previous) || !eligible(current)) continue;
      cpuSeconds += current.cpuSeconds ?? 0;
      duration += current.elapsedSeconds - previous.elapsedSeconds;
    }
  }
  if (duration <= 0) throw new Error('No eligible CPU sampling intervals');
  return {privateRamMiB:distribution(samples.map(sample => sample.privateRamBytes / 1048576)),
    privateCommitMiB:distribution(samples.map(sample => sample.privateCommitBytes / 1048576)),
    cpuPercent:100 * cpuSeconds / duration / runs[0].logicalProcessors,
    cpuSeconds, durationSeconds:duration, processNames:[...new Set(samples.flatMap(sample => sample.processes.map(process => process.name)))],
    foregroundFraction:samples.filter(sample => sample.foreground).length / samples.length,
    visibleFraction:samples.filter(sample => sample.visible && !sample.minimized).length / samples.length,
    runs:runs.filter(run => run.samples.some(eligible)).length,
    totalSamples:allSamples.length,
    windowSizes:[...new Set(samples.map(sample => `${sample.windowWidth}x${sample.windowHeight}`))],
    sampleSelection:backgroundOnly ? 'visible, unminimized background samples only' : foregroundOnly ? 'visible, unminimized foreground samples only' : 'all recorded window states'};
}

export function assertMatchingWindows(telex, other) {
  if (telex.windowSizes.length !== 1 || other.windowSizes.length !== 1) throw new Error('Window resized during measurement');
  const dimensions = value => value.split('x').map(Number);
  const a = dimensions(telex.windowSizes[0]), b = dimensions(other.windowSizes[0]);
  // Allow a two-pixel frame rounding difference between window toolkits.
  if (a.some((value, i) => !Number.isFinite(value) || !Number.isFinite(b[i]) || Math.abs(value - b[i]) > 2)) throw new Error('Comparison used different window dimensions');
}

export function assertComparable(telex, other) {
  if (other.runs.length !== telex.runs.length) throw new Error('Comparison used a different number of sampling windows');
  for (const key of ['fixtureId', 'messages', 'scenario']) {
    if (telex[key] !== other[key]) throw new Error(`Comparison mismatch: ${key}`);
  }
  const hardware = telex.runs[0].resources.hardware;
  if (JSON.stringify(hardware) !== JSON.stringify(other.runs[0].hardware)) throw new Error('Comparison used different hardware');
  for (const run of other.runs) {
    if (run.label !== telex.scenario || run.durationSeconds < 29) throw new Error('Comparison used a different or incomplete sampling window');
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
  const work = join(repo, '.bench');
  const telex = JSON.parse(readFileSync(join(work, 'telex-desktop.json'), 'utf8').replace(/^\uFEFF/, ''));
  if (telex.runs.length !== 5) throw new Error('This public benchmark requires five completed Telex runs');
  const metrics = {};
  for (const metric of telex.runs[0].desktop.metrics) {
    const runs = telex.runs.map(run => run.desktop.metrics.find(m => m.name === metric.name));
    if (runs.some(run => !run || run.matches !== metric.matches)) throw new Error(`Inconsistent metric: ${metric.name}`);
    metrics[metric.name] = {...distribution(runs.flatMap(run => run.samples)), ...(metric.matches ? {matches:metric.matches} : {})};
  }
  const output = {schema:1, measuredAt:telex.runs[0].resources.measuredAt,
    application:telex.application, version:telex.version, build:telex.build,
    fixtureId:telex.fixtureId, messages:telex.messages, scenario:telex.scenario,
    hardware:telex.runs[0].resources.hardware,
    firstLaunchMs:telex.runs[0].startupMs,
    warmStartupMs:distribution(telex.runs.slice(1).map(run => run.startupMs)),
    metrics, resources:resources(telex.runs.map(run => run.resources), {backgroundOnly:true}), comparisons:[]};
  const core = readFileSync(join(work, 'core.txt'), 'utf8').match(/built 100,000 messages in ([\d.]+) s/);
  if (core) output.coreBuild = {messages:100000, seconds:Number(core[1]), batchMessages:1000,
    includes:'synthetic message generation, storing full messages and maintaining the full-text index',
    excludes:'network transfer and MIME parsing; not a desktop import or a competitor indexing comparison'};
  for (const name of ['thunderbird', 'outlook-classic', 'outlook-new']) {
    const file = join(work, `${name}.json`);
    if (!existsSync(file)) continue;
    const other = JSON.parse(readFileSync(file, 'utf8').replace(/^\uFEFF/, ''));
    assertComparable(telex, other);
    const measured = resources(other.runs, {backgroundOnly:true});
    assertMatchingWindows(output.resources, measured);
    // A minimized window or mismatched foreground state makes RAM comparisons misleading.
    if (measured.visibleFraction !== 1 || output.resources.visibleFraction !== 1 || Math.abs(measured.foregroundFraction - output.resources.foregroundFraction) > .1) {
      throw new Error(`${name}: window visibility or foreground state differs`);
    }
    output.comparisons.push({application:other.application, version:other.version, measuredAt:other.runs[0].measuredAt, notes:other.notes, resources:measured});
  }
  const dest = join(repo, 'site/public/benchmarks');
  mkdirSync(dest, {recursive:true});
  writeFileSync(join(dest, 'windows-100k.json'), JSON.stringify(output, null, 2) + '\n');
  console.log('Saved verified public data to site/public/benchmarks/windows-100k.json');
}
