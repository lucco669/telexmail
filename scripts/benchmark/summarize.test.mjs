import test from 'node:test';
import assert from 'node:assert/strict';
import {distribution, resources, assertComparable, assertMatchingWindows} from './summarize.mjs';

test('medians handle even samples and p95 uses nearest rank', () => {
  assert.deepEqual(distribution([4, 1, 3, 2]), {samples:4, min:1, median:2.5, p95:4, max:4});
  assert.throws(() => distribution([]));
  assert.throws(() => distribution([NaN]));
});
test('CPU uses elapsed time and excludes the first sample', () => {
  const sample = (time, cpu) => ({elapsedSeconds:time, cpuSeconds:cpu, privateRamBytes:1048576, privateCommitBytes:2097152,
    errors:[], processes:[{name:'telex.exe'}, {name:'msedgewebview2.exe'}], visible:true, minimized:false, foreground:true});
  const result = resources([{logicalProcessors:4, samples:[sample(1, null), sample(3, 1), sample(5, 1)]}]);
  assert.equal(result.cpuPercent, 12.5);
  assert.equal(result.privateRamMiB.median, 1);
  assert.equal(result.visibleFraction, 1);
  assert.equal(result.processNames.length, 2);
});
test('different datasets and machines cannot become a published comparison', () => {
  const hardware = {cpu:'test CPU', os:'Windows', build:'1', ramBytes:1};
  const telex = {fixtureId:'fixture', messages:100000, scenario:'idle', runs:[{resources:{hardware}}]};
  const other = {fixtureId:'fixture', messages:100000, scenario:'idle', runs:[{hardware, label:'idle', durationSeconds:30}]};
  assert.doesNotThrow(() => assertComparable(telex, other));
  assert.throws(() => assertComparable(telex, {...other, messages:300}));
  assert.throws(() => assertComparable(telex, {...other, runs:[{...other.runs[0], hardware:{...hardware, cpu:'other CPU'}}]}));
});
test('foreground resource summaries do not count CPU or duration across background gaps', () => {
  const make = (elapsedSeconds, foreground, cpuSeconds) => ({elapsedSeconds, foreground, cpuSeconds,
    privateRamBytes:1048576, privateCommitBytes:2097152, errors:[], processes:[{name:'app.exe'}], visible:true, minimized:false, windowWidth:1280, windowHeight:800});
  const samples = Array.from({length:22}, (_, i) => make(i + 1, i !== 10, i ? 1 : null));
  const result = resources([{logicalProcessors:2, samples}], {foregroundOnly:true});
  assert.equal(result.cpuSeconds, 19);
  assert.equal(result.durationSeconds, 19);
  assert.equal(result.foregroundFraction, 1);
  assert.equal(result.privateRamMiB.samples, 21);
  assert.equal(result.totalSamples, 22);
  assert.throws(() => resources([{logicalProcessors:2, samples:samples.slice(0,10)}], {foregroundOnly:true}));
});

test('window dimensions must match and remain stable for a comparison', () => {
  assert.doesNotThrow(() => assertMatchingWindows({windowSizes:['1280x800']}, {windowSizes:['1282x800']}));
  assert.throws(() => assertMatchingWindows({windowSizes:['1280x800']}, {windowSizes:['1600x1000']}));
  assert.throws(() => assertMatchingWindows({windowSizes:['1280x800', '1280x900']}, {windowSizes:['1280x800']}));
});

test('background summaries match visibility and omit intervals crossing a focus change', () => {
  const samples = Array.from({length:24}, (_, i) => ({elapsedSeconds:i+1, foreground:i===10, cpuSeconds:i?1:null,
    privateRamBytes:1048576, privateCommitBytes:2097152, errors:[], processes:[{name:'app.exe'}], visible:true, minimized:false, windowWidth:1280, windowHeight:800}));
  const result = resources([{logicalProcessors:2, samples}], {backgroundOnly:true});
  assert.equal(result.privateRamMiB.samples, 23);
  assert.equal(result.foregroundFraction, 0);
  assert.equal(result.cpuSeconds, 21);
  assert.equal(result.durationSeconds, 21);
  assert.throws(() => resources([{logicalProcessors:2, samples}], {foregroundOnly:true, backgroundOnly:true}));
});
