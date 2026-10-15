import { PageLink as Link } from '@/components/PageNavigation';
import { PageHeader } from '@/components/PageHeader';
import { pages } from '@/lib/pages';
import { pageMetadata } from '@/lib/seo';
import { site } from '@/lib/site';
import s from './page.module.css';

export const metadata = pageMetadata(pages.security);

const repo = `https://github.com/${site.sourceRepo}`;
const source = (path: string) => `${repo}/blob/main/${path}`;
const workflow = `${repo}/actions/workflows/security.yml`;

const protections = [
  {
    title: 'Passwords in the system credential store',
    text: 'Telex saves account passwords through Windows Credential Manager or a Secret Service keyring on Linux. Settings exports omit passwords.',
    evidence: [
      { label: 'Credential storage code', path: 'src-tauri/src/secrets.rs' },
      { label: 'Settings export code and tests', path: 'crates/telex-core/src/settings.rs' },
    ],
  },
  {
    title: 'Encrypted connections to your mail provider',
    text: 'IMAP and SMTP use TLS or require STARTTLS, with certificate validation. This protects the connection to your provider; it does not encrypt stored mail or make email end-to-end encrypted.',
    evidence: [
      { label: 'IMAP connection code', path: 'crates/telex-core/src/imap.rs' },
      { label: 'SMTP connection code', path: 'crates/telex-core/src/smtp.rs' },
    ],
  },
  {
    title: 'HTML mail with restricted capabilities',
    text: 'The renderer removes active content and adds a policy that restricts resource loads. The interface displays mail in a sandboxed frame without script permission. Tests cover scripts, forms, tracking images and private-network image addresses.',
    evidence: [
      { label: 'Renderer and regression tests', path: 'crates/telex-core/src/html.rs' },
      { label: 'Mail frame code', path: 'ui/app.js' },
      { label: 'Desktop security policy', path: 'src-tauri/tauri.conf.json' },
    ],
  },
  {
    title: 'Remote images blocked by default',
    text: 'Images hosted outside a message are fetched after you choose to load them or trust a sender. Trusted senders’ images can then load automatically, except in Spam. Loading an image can reveal your IP address and the time to its server.',
    evidence: [
      { label: 'Image fetching code and tests', path: 'crates/telex-core/src/html.rs' },
      { label: 'Sender trust rules', path: 'crates/telex-core/src/store.rs' },
    ],
  },
  {
    title: 'Attachment precautions',
    text: 'Attachment filenames are constrained to their destination folder. Files recognized as able to run code, and attachments in Spam, require confirmation before opening. On Windows, saved attachments are marked as coming from the internet. These precautions do not certify an attachment as safe.',
    evidence: [
      { label: 'Attachment handling and tests', path: 'crates/telex-core/src/attachments.rs' },
      { label: 'Opening and confirmation code', path: 'src-tauri/src/files.rs' },
    ],
  },
  {
    title: 'Link destinations you can inspect',
    text: 'Hover or keyboard focus reveals the actual destination, with hidden characters made visible. Its menu copies the address even in Spam, where opening links stays blocked. Displaying an address does not visit it or resolve redirects.',
    evidence: [
      { label: 'URL inspection code', path: 'ui/links.js' },
      { label: 'Message link interface', path: 'ui/app.js' },
      { label: 'Misleading-link regressions', path: 'scripts/security/guards.test.mjs' },
    ],
  },
  {
    title: 'Scam warning signs checked offline',
    text: 'When you open a message, the Rust engine checks supported sender, brand and link warning signs and explains each contributing reason. It uploads no mail, follows no links and does not change message flags. Scores are advisory points, not probabilities or a safety verdict.',
    evidence: [
      { label: 'Risk engine and regression tests', path: 'crates/telex-core/src/risk.rs' },
      { label: 'Scanner scope and limits', path: 'docs/mail-risk.md' },
      { label: 'Opened-message boundary', path: 'crates/telex-core/src/store.rs' },
    ],
  },
  {
    title: 'Previews and safeguards for mail rules',
    text: 'Rule previews only inspect current matches. Copy actions preserve the original and record duplicate protection; automatic forwards and replies run only on new arrivals, with duplicate and loop safeguards. Configuring a forward shares that mail with the recipient through your SMTP server.',
    evidence: [
      { label: 'Rules, previews and forwarding tests', path: 'crates/telex-core/src/filters.rs' },
      { label: 'Rule-action storage', path: 'crates/telex-core/src/store.rs' },
    ],
  },
  {
    title: 'Update signature verification',
    text: 'Release builds with the updater enabled verify the update signature and signed version before installation. Automatic checks require your choice in Settings; a manual check also contacts GitHub. Installation requires your click. Updater signatures are separate from Windows publisher signing.',
    evidence: [
      { label: 'Updater code', path: 'src-tauri/src/updates.rs' },
      { label: 'Release workflow', path: '.github/workflows/release.yml' },
      { label: 'Update request documentation', path: 'docs/releasing.md' },
    ],
  },
];

const connections = [
  { purpose: 'Read and send mail', when: 'When you add, check or use an account, including background sync and Inbox watching.',
    destination: 'Your configured IMAP and SMTP servers. The desktop app connects directly to them.', path: 'crates/telex-core/src/imap.rs' },
  { purpose: 'Find account settings', when: 'During automatic account setup.',
    destination: 'DNS and provider setup URLs. One URL includes your email address; HTTPS redirects can lead to another server.', path: 'crates/telex-core/src/autoconfig.rs' },
  { purpose: 'Load remote images', when: 'After you allow images in a message or trust a sender.',
    destination: 'The public HTTP or HTTPS addresses in that message, fetched by the mail engine.', path: 'crates/telex-core/src/html.rs' },
  { purpose: 'Check or install updates', when: 'When permitted by your update setting, or when you check manually; downloads follow your install click.',
    destination: 'GitHub Releases and its asset servers. Requests disclose your IP address and request time.', path: 'src-tauri/src/updates.rs' },
  { purpose: 'Download spelling dictionaries', when: 'When you choose to download a language’s dictionary.',
    destination: 'LibreOffice dictionary files on GitHub, at a fixed commit. Each file must match a recorded SHA-256 hash.', path: 'crates/telex-core/src/spell.rs' },
  { purpose: 'Set a server-side away reply', when: 'When you configure an away reply and your provider supports it.',
    destination: 'Your provider’s ManageSieve service, with STARTTLS before login.', path: 'crates/telex-core/src/sieve.rs' },
];

export default function SecurityPage() {
  return (
    <>
      <PageHeader
        page={pages.security}
        title="Privacy & security."
        lead="What Telex protects, what it sends, and where you can inspect the evidence."
      >
        <a className="btn btn-primary" href={workflow}>See security check results</a>
        <Link className="btn" href={pages.privacy.path}>Read the privacy details</Link>
      </PageHeader>
      <div className={`container ${s.layout}`}>
        <aside className={s.summary}>
          <p className={s.updated}>Updated 13 October 2026</p>
          <p className={s.summaryTitle}>Evidence you can inspect.</p>
          <p>Source, tests and scan results describe specific protections. They do not establish that an app is safe in every situation.</p>
          <nav aria-label="On this page">
            <a href="#protections">Protections and source</a>
            <a href="#network">Network access</a>
            <a href="#checks">Automated checks</a>
            <a href="#limits">Current limits</a>
            <a href="#report">Report a problem</a>
          </nav>
        </aside>

        <div className={s.content}>
          <section id="protections" aria-labelledby="protections-h">
            <h2 id="protections-h">Protections and their source</h2>
            <p className={s.intro}>These links show the source on the main branch, which can include unreleased changes.
              A check result applies to the commit it tested, rather than to every downloadable installer.</p>
            <div className={s.protections}>
              {protections.map(item => (
                <article className={s.protection} key={item.title}>
                  <h3>{item.title}</h3>
                  <p>{item.text}</p>
                  <ul className={s.evidence} aria-label={`Evidence for ${item.title}`}>
                    {item.evidence.map(link => <li key={link.path}><a href={source(link.path)}>{link.label} ↗</a></li>)}
                  </ul>
                </article>
              ))}
            </div>
          </section>

          <section id="network" aria-labelledby="network-h">
            <h2 id="network-h">When the app goes online</h2>
            <p className={s.intro}>The desktop app has no Telex account or Telex mail relay. Its network functions are listed below.
              Links you open use your default browser, which has its own privacy settings.</p>
            <dl className={s.connections}>
              {connections.map(item => (
                <div key={item.purpose}>
                  <dt>{item.purpose}</dt>
                  <dd><p>{item.when}</p><p>{item.destination}</p><a href={source(item.path)}>Inspect the code ↗</a></dd>
                </div>
              ))}
            </dl>
            <p className={s.note}>This website uses Vercel Web Analytics for cookieless page counts.
              Website statistics are separate from the desktop app. <Link href={pages.privacy.path}>Details of storage, analytics and requests</Link>.</p>
          </section>

          <section id="checks" aria-labelledby="checks-h">
            <h2 id="checks-h">Automated checks, with their results</h2>
            <p className={s.intro}>The security workflow is configured for pushes to main, version tags, pull requests and a weekly advisory refresh,
              on Windows and Linux. Open a run to see its actual result, date and tested commit.</p>
            <ul className={s.checks}>
              <li><b>Rust dependencies:</b> cargo-audit checks the complete Cargo lockfile against RustSec. Vulnerabilities and soundness advisories fail the check; maintenance warnings remain visible.</li>
              <li><b>JavaScript dependencies:</b> npm audit checks both the desktop tooling and website lockfiles, including development dependencies. Reported vulnerabilities fail the check.</li>
              <li><b>Secrets:</b> Gitleaks scans fetched Git history and the current source for recognized secret patterns. Public evidence contains counts and rule names, with matched values and contextual source text omitted.</li>
              <li><b>Security regressions:</b> tests check desktop security settings, mail-frame restrictions and misleading link display, then run the mail engine’s unit tests, including HTML, image, attachment, local risk and password-export cases.</li>
              <li><b>Linux source verification:</b> the vendored GLib inventory and hashes are checked, then its affected iterator operations run in an optimized regression test.</li>
              <li><b>Native mail rendering:</b> isolated Tauri builds test hostile messages on light and dark paper in Windows WebView2 and Linux WebKitGTK, using the production CSP and IPC layer. A separate raw-script fixture exercises the sandbox.</li>
              <li><b>Parser fuzzing:</b> coverage-guided MIME/HTML and complete PST-import targets each run for 60 seconds, with per-input timeout, memory and input-size limits. Crashes, timeouts and memory-limit failures fail the gate and retain reproducers.</li>
            </ul>
            <p className={s.note}>The release workflow depends on the complete security workflow before creating a draft or signing installers.
              Failed scans, source verification, native webview tests or fuzzing stop that release build. This is a configured gate;
              use the run for the exact release commit to inspect the result.</p>
            <p className={s.note}>Each run retains a security-evidence artifact for 90 days with tool versions, timestamps, source and lockfile hashes,
              advisory details and test logs. GitHub may require sign-in to download artifacts. Missing tools or unavailable audit services produce errors.</p>
            <div className={s.actions}>
              <a href={workflow}>Open the security runs ↗</a>
              <a href={source('.github/workflows/security.yml')}>Inspect the workflow ↗</a>
              <a href={source('.github/workflows/release.yml')}>Inspect the release gate ↗</a>
              <a href={source('docs/security.md')}>Repeat the checks yourself ↗</a>
            </div>
            <p className={s.note}>This page does not display an assumed passing badge. No completed run means no completed result to claim.
              The checks do not prove that installers match the source or establish antivirus clearance.</p>
          </section>

          <section id="limits" aria-labelledby="limits-h">
            <h2 id="limits-h">Current limits and open findings</h2>
            <ul className={s.checks}>
              <li><b>Local scam checks have a bounded scope.</b> The initial brand catalogue is small and scans have size limits.
                The checker does not verify attachments, remote images, final redirects, site reputation, SPF, DKIM or DMARC.
                An incomplete check is reported; no warning does not establish safety. Scores are heuristic points out of 100, capped at 99.
                <span className={s.inlineEvidence}> <a href={source('docs/mail-risk.md')}>Scope, scoring and limits</a></span></li>
              <li><b>Local mail is not encrypted by Telex.</b> The mail database and its backups are ordinary SQLite files.
                Settings, exported mail and saved attachments also need protection from your operating system and storage.
                <span className={s.inlineEvidence}> <a href={source('crates/telex-core/src/store.rs')}>Storage code</a> · <a href={source('crates/telex-core/src/backup.rs')}>Backup code</a></span></li>
              <li><b>Windows publisher signing is not configured.</b> Updater signatures verify Telex updates, but do not give the installer a Windows-verified publisher identity.
                <span className={s.inlineEvidence}> <a href={source('src-tauri/tauri.conf.json')}>Installer configuration</a></span></li>
              <li><b>The Linux GLib iterator fix is backported.</b> Tauri’s GTK dependency family needs GLib 0.18, so Telex uses its original 0.18.5 source with the upstream fix for
                <a href="https://rustsec.org/advisories/RUSTSEC-2024-0429.html"> RUSTSEC-2024-0429</a> applied.
                Every vendored source file is checksum-verified and an optimized Linux test exercises the affected operations.
                Registry dependency scans exclude local path dependencies; these additional checks are required.
                <span className={s.inlineEvidence}> <a href={source('docs/security-backport.md')}>Backport evidence</a> · <a href={source('crates/telex-security/tests/glib_regression.rs')}>Regression test</a></span></li>
              <li><b>A dependency maintenance warning remains open.</b> The Linux dependency graph also includes the unmaintained proc-macro-error 1.0.4.
                <a href="https://rustsec.org/advisories/RUSTSEC-2024-0370.html"> Advisory and replacement guidance</a>.</li>
              <li><b>No independent security audit is published here.</b> Automated scans and tests have a limited scope and can miss problems.
                There is no antivirus certification, reproducible-build claim or guarantee of complete safety on this page.</li>
            </ul>
            <p className={s.note}>The dependency findings above were identified on 12 October 2026. Use the newest run for the latest dependency results.</p>
          </section>

          <section id="report" aria-labelledby="report-h">
            <h2 id="report-h">Report a security problem privately</h2>
            <p>Email <a href={`mailto:${site.contact}`}>{site.contact}</a> with the Telex version, operating system, reproduction steps and expected impact.
              Use a test mailbox and remove passwords and personal mail from any example. Please keep exploit details out of public issues until the report has been reviewed.</p>
            <p className={s.note}><a href={source('SECURITY.md')}>Read the reporting policy ↗</a></p>
          </section>
        </div>
      </div>
    </>
  );
}
