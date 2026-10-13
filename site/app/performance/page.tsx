import { PageLink as Link } from '@/components/PageNavigation';
import { CtaBand } from '@/components/CtaBand';
import { PageHeader } from '@/components/PageHeader';
import { Performance } from '@/components/Performance';
import { benchmarkDate, cpuPercent, mib, milliseconds, performance, seconds } from '@/lib/performance';
import { pages } from '@/lib/pages';
import { getLatestRelease } from '@/lib/release';
import { pageMetadata } from '@/lib/seo';
import s from './page.module.css';

export const metadata = pageMetadata(pages.performance);
const actionNames: Record<string, string> = {
  'folder-switch': 'Switch folders: list and message visible',
  'open-message': 'Open a message, fetched from the mail base',
  'jump-to-end': 'Jump to the end of the 40,000-message inbox',
};

export default async function PerformancePage() {
  const release = await getLatestRelease();
  const clients = [
    {application:'Telex', version:performance.version, resources:performance.resources},
    ...performance.comparisons,
  ];
  const maxRam = Math.max(...clients.map(client => client.resources.privateRamMiB.median));
  const thunderbird = clients.find(client => client.application === 'Thunderbird');
  const ramSaving = thunderbird && thunderbird.resources.privateRamMiB.median > performance.resources.privateRamMiB.median
    ? Math.round((1 - performance.resources.privateRamMiB.median / thunderbird.resources.privateRamMiB.median) * 100) : null;
  return (
    <>
      <PageHeader page={pages.performance} title="Small footprint. Measured speed." compact
        lead="100,000 complete synthetic messages in a repeatable test mailbox. The full desktop app, including its window. Every figure below has a date, a workload and a method." />
      <Performance release={release} compact />
      {release && release.version !== performance.version && <p className={`container ${s.note}`}>
        The current download is Telex {release.version}; the recorded desktop benchmark below is version {performance.version}.
        Newer features have not been assigned these timings without another measurement.
      </p>}
      <section className={`container ${s.section}`} aria-labelledby="search-ready">
        <h2 className="h2" id="search-ready">Searchable as mail is saved.</h2>
        <p className={s.intro}>Telex updates its full-text index while it stores each batch of messages. Once a message is saved,
          it is ready to search. Normal sync and import need no separate background indexing phase afterward.</p>
        <div className={s.design}>
          <div><h3>One storage step</h3><p>Messages and their search entries are saved together. Search keeps up with stored mail, including message bodies.</p></div>
          <div><h3>Small lists, at any depth</h3><p>Folder views load a page of summaries. Optional threads keep message bodies paged too. Opening a 40,000-message inbox does not load 40,000 message bodies into the window.</p></div>
          <div><h3>An index without extra body copies</h3><p>The search index stores tokens and message IDs. It does not keep another full copy of each message’s text.</p></div>
        </div>
        <p className={s.note}>Saved searches reuse the same local query engine; they store queries and list options rather than another copy of the mailbox.
          The local scam check runs when a message is opened, with recent opened messages cached. Listing, searching and syncing do not run that scanner.
          {' '}<Link href={`${pages.features.path}#saved-searches`}>Saved searches</Link> · <Link href={`${pages.features.path}#link-inspection`}>Local mail inspection</Link>.</p>
        {performance.coreBuild && <p className={s.note}>In the core benchmark, generating and storing 100,000 synthetic messages, including their search index,
          took {performance.coreBuild.seconds.toFixed(1)} seconds, in batches of {performance.coreBuild.batchMessages.toLocaleString('en-GB')}.
          This excludes network transfer and MIME parsing, and is not a desktop import or a speed comparison with another app.</p>}
        <p className={s.note}>Thunderbird uses a separate global search database, Gloda, whose background indexer deliberately throttles work to preserve responsiveness.
          These are different designs; the indexing wait we saw during setup does not establish an import-speed ratio.
          {' '}<a href="https://support.mozilla.org/en-US/kb/global-search">Thunderbird’s global search</a> ·
          {' '}<a href="https://searchfox.org/comm-central/source/mailnews/db/gloda/modules/GlodaIndexer.sys.mjs">Gloda’s scheduling design</a></p>
      </section>
      <section className={`container ${s.section}`} aria-labelledby="desktop-results">
        <h2 className="h2" id="desktop-results">100,000 messages. Ready to work.</h2>
        <p className={s.intro}>Telex {performance.version}, built in release mode with an isolated benchmark profile. Measured on {benchmarkDate}.
          These timings include the Rust core, the desktop window and two animation frames for rendering.</p>
        <div className={s.scroll}>
          <table className={s.table}>
            <caption>Desktop interaction timings on the shared synthetic mailbox</caption>
            <thead><tr><th scope="col">Action</th><th scope="col">Median</th><th scope="col">Range</th><th scope="col">Samples</th></tr></thead>
            <tbody>
              <tr><th scope="row">Repeat launch → inbox and selected message ready</th><td>{seconds(performance.warmStartupMs.median)}</td><td>{seconds(performance.warmStartupMs.min)}–{seconds(performance.warmStartupMs.max)}</td><td>{performance.warmStartupMs.samples}</td></tr>
              {Object.entries(performance.metrics).map(([name, result]) => (
                <tr key={name}><th scope="row">{actionNames[name] ?? `Full-text search: “${name.slice(7)}”`}
                  {'matches' in result && <small>{result.matches.toLocaleString('en-GB')} matches across all mail</small>}</th>
                  <td>{milliseconds(result.median)}</td><td>{milliseconds(result.min)}–{milliseconds(result.max)}</td><td>{result.samples}</td></tr>
              ))}
            </tbody>
          </table>
        </div>
        <p className={s.note}>The first recorded launch took {seconds(performance.firstLaunchMs)}. Repeat launches use Windows’ existing caches.
          A cold boot was not measured. Full-text search includes Telex’s 150 ms delay after typing; its result cache is invalidated before each query.</p>
      </section>
      <section className={`container ${s.section}`} aria-labelledby="footprint">
        <h2 className="h2" id="footprint">What the whole app uses.</h2>
        {ramSaving !== null && <p className={s.comparison}><strong>{ramSaving}% less idle RAM than Thunderbird.</strong>
          <span>Median private RAM in this offline test of 100,000 messages on one Windows PC.</span></p>}
        <p className={s.intro}>The same 100,000 full messages, with the 40,000-message inbox open. RAM includes every descendant process,
          including Telex’s WebView2 browser, renderer and GPU processes. Windows private working set, in MiB; lower uses less private RAM.</p>
        <ul className={s.bars} aria-label="Median private RAM by app">
          {clients.map((client, i) => (
            <li key={client.application}>
              <div className={s.barLabel}><span><b>{client.application}</b> <small>{client.version}</small></span><strong>{mib(client.resources.privateRamMiB.median)}</strong></div>
              <div className={s.track} aria-hidden="true"><div className={i === 0 ? s.telexBar : s.otherBar} style={{width:`${client.resources.privateRamMiB.median / maxRam * 100}%`}} /></div>
            </li>
          ))}
        </ul>
        <div className={s.scroll}>
          <table className={s.table}>
            <caption>Offline idle resources after startup and indexing have settled</caption>
            <thead><tr><th scope="col">App</th><th scope="col">RAM median</th><th scope="col">RAM range</th><th scope="col">Idle CPU</th><th scope="col">Sampling windows</th></tr></thead>
            <tbody>{clients.map(client => (
              <tr key={client.application}><th scope="row">{client.application} {client.version}</th><td>{mib(client.resources.privateRamMiB.median)}</td>
                <td>{mib(client.resources.privateRamMiB.min)}–{mib(client.resources.privateRamMiB.max)}</td><td>{cpuPercent(client.resources.cpuPercent)}</td><td>{client.resources.runs}</td></tr>
            ))}</tbody>
          </table>
        </div>
        <p className={s.note}>This compares one local, offline mail workload, rather than every feature of these apps. CPU is a share of all logical processors.
          Results vary with hardware, message content, app settings and other work on the PC.</p>
        <p className={s.note}>Telex was relaunched for each resource window. Thunderbird’s windows come from one fresh offline launch after indexing finished.
          Both use an unthreaded list sorted newest first and show the plain-text body of the same newest inbox message.
          Only visible, unminimized background samples are included; the JSON records included and total sample counts.</p>
        <p className={s.note}>Classic Outlook and new Outlook have no comparable result in this dataset yet. Their entries will appear only after the same mailbox and measurement procedure have been verified.</p>
      </section>
      <section className={`container ${s.section}`} id="method" aria-labelledby="method-heading">
        <h2 className="h2" id="method-heading">How we measured.</h2>
        <dl className={s.method}>
          <div><dt>Machine</dt><dd>{performance.hardware.os}, build {performance.hardware.build}; {performance.hardware.cpu}; {(performance.hardware.ramBytes / 1073741824).toFixed(1)} GiB usable RAM.</dd></div>
          <div><dt>Mailbox</dt><dd>100,000 deterministic synthetic messages, 12 folders in three groups, five years of dates, plain text and HTML, and about 10% with attachments. Every client gets the full messages from the same fixture.</dd></div>
          <div><dt>Desktop timings</dt><dd>Five launches of an optimized release build. The first is reported separately. Folder changes, uncached message reads, deep navigation and three full-text queries are repeated. Timings end after the UI work and two animation frames; they do not measure physical display scanout.</dd></div>
          <div><dt>Resources</dt><dd>Five seconds to settle, then 30 seconds sampled at roughly one-second intervals per run. Public figures use only visible, unminimized background samples for every app; CPU intervals crossing a focus change are excluded. We sum private working sets across the process tree, avoiding double counting shared pages. Private committed memory is recorded separately and is not presented as RAM.</dd></div>
          <div><dt>Comparison</dt><dd>The same machine, corpus, sampling period and offline inbox scenario; visible, unminimized windows in the background, with matching recorded dimensions. Thunderbird’s fixture is in Local Folders, with the three groups as branches. Telex uses three synthetic accounts with network checks disabled. Mail sync and sending are outside this test.</dd></div>
          <div><dt>Download size</dt><dd>The Windows installer’s byte count comes from the displayed GitHub release, which may differ from the benchmark build. WebView2 is shared with Windows and is not bundled; downloading or provisioning that runtime is outside the installer size. Mailbox data is excluded.</dd></div>
        </dl>
        <div className={s.sources}>
          <a className="more" href="/benchmarks/windows-100k.json" download>Download measurement data (JSON)</a>
          <a className="more" href="https://github.com/lucco669/telexmail/blob/main/docs/performance.md">Reproduce the benchmark</a>
          <a href="https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/process-model">WebView2 process model</a>
          <a href="https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-process_memory_counters_ex2">Windows memory counters</a>
        </div>
        <p className={s.note}>Measured by the Telex developer. Independent measurements are welcome; <Link href={pages.about.path}>read about the project</Link>.</p>
      </section>
      <CtaBand secondary={pages.demo} />
    </>
  );
}
