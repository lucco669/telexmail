// Included INSIDE the real UI's closure by prepare.mjs; never served in the normal app.
// The title is a local measurement channel, read by run-telex.ps1. No debug port or network.
async function runDesktopBenchmark(){
  const window = globalThis.__TAURI__.window.getCurrentWindow();
  const paint = () => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
  const metrics = [];
  const measure = async (name, repetitions, action) => {
    const samples = [];
    for (let i = 0; i < repetitions; i++){
      const start = performance.now();
      await action(i);
      await paint();
      samples.push(performance.now() - start);
    }
    metrics.push({name, unit:'ms', samples});
  };
  await boot();
  await renderPreview();
  await paint();
  if (Object.values(COUNTS).reduce((n, c) => n + c.total, 0) !== 100000) throw new Error('Expected exactly 100,000 messages');
  if (state.folder !== 'twojastudio/inbox' || !selRow || list.total !== 40000) throw new Error('Expected the 40,000-message inbox with a selected message');
  const readyAt = Date.now();
  await window.setTitle(`TELEX_BENCHMARK_READY:${readyAt}`);
  // Allow the runner to measure 30 seconds of idle resources after startup settles.
  await new Promise(resolve => setTimeout(resolve, 45000));
  await window.setTitle('TELEX_BENCHMARK_ACTIVE');
  const inbox = 'twojastudio/inbox';
  await measure('folder-switch', 21, async i => {
    await setFolder(i % 2 ? inbox : 'twojastudio/sent');
    await renderPreview();
  });
  await setFolder(inbox);
  const ids = list.rows.slice(1, 22).map(m => m.id);
  await measure('open-message', ids.length, async i => {
    bodies.delete(ids[i]); // Each message is fetched, rather than read from the UI's body cache.
    await select(ids[i], true);
  });
  // Keep reset work outside timed samples for the deep-scroll result.
  const jumps = [];
  for (let i = 0; i < 11; i++){
    await moveBy(-Infinity); await paint();
    const start = performance.now();
    await moveBy(Infinity); await paint();
    jumps.push(performance.now() - start);
  }
  metrics.push({name:'jump-to-end', unit:'ms', samples:jumps});
  await setFolder(inbox);
  state.scope = 'all';
  paintScope();
  for (const query of ['kowalska', 'żółwia', 'faktura wrzesień']){
    const samples = [], matches = [];
    for (let i = 0; i < 7; i++){
      // Invalidate cached result ids without changing the searchable text.
      await invoke('set_flagged', {id:ids[0], flagged:i % 2 === 0});
      const done = new Promise((resolve, reject) => {
        const timeout = setTimeout(() => { globalThis.removeEventListener('telex-benchmark-view', handler); reject(new Error(`Search timed out: ${query}`)); }, 15000);
        function handler(e){
          if (e.detail.query === query && e.detail.bodies){
            clearTimeout(timeout); globalThis.removeEventListener('telex-benchmark-view', handler);
            resolve(e.detail.total);
          }
        }
        globalThis.addEventListener('telex-benchmark-view', handler);
      });
      const start = performance.now();
      qEl.value = query;
      qEl.dispatchEvent(new Event('input', {bubbles:true}));
      matches.push(await done);
      await paint();
      samples.push(performance.now() - start);
    }
    if (!matches.every(n => n === matches[0]) || matches[0] === 0) throw new Error(`Inconsistent search results: ${query}`);
    metrics.push({name:`search:${query}`, unit:'ms', samples, matches:matches[0]});
  }
  await setFolder(inbox);
  await renderPreview(); await paint();
  await window.setTitle('TELEX_BENCHMARK_RESULT:' + JSON.stringify({schema:1, readyAt, messages:100000, metrics}));
}
runDesktopBenchmark().catch(async error => {
  await globalThis.__TAURI__.window.getCurrentWindow().setTitle('TELEX_BENCHMARK_ERROR:' + String(error));
});
