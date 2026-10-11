import Link from 'next/link';
import { CtaBand } from '@/components/CtaBand';
import { Compat, DownloadChoice, LookupNote, ReleaseStatus } from '@/components/DownloadChoice';
import { PageHeader } from '@/components/PageHeader';
import { Shot } from '@/components/Shot';
import { WindowFrame } from '@/components/WindowFrame';
import { providers, screens } from '@/lib/content';
import { pages } from '@/lib/pages';
import { allReleasesPage, getReleaseInfo, megabytes } from '@/lib/release';
import { appLd, pageMetadata } from '@/lib/seo';
import { versionAnchor } from '@/lib/versions';
import s from './page.module.css';

export const metadata = pageMetadata(pages.download);

const dateFmt = new Intl.DateTimeFormat('en-GB', { day: 'numeric', month: 'long', year: 'numeric', timeZone: 'UTC' });
const account = screens.find((x) => x.id === 'account')!;

export default async function DownloadPage() {
  const { list: releases, live } = await getReleaseInfo();
  const latest = releases[0] ?? null;
  // GitHub couldn't be asked and no release is known: link to GitHub rather than say "coming soon".
  const unknown = !live && !latest;
  const linux = latest?.appimage || latest?.deb;
  return (
    <>
      <PageHeader
        page={pages.download}
        title="Get Telex for Windows and Linux."
        lead={
          latest ? (
            <>
              Version {latest.version}
              {latest.date ? `, released ${dateFmt.format(new Date(latest.date))}` : ''}. Free, and a preview: Telex is at
              0.x, gets new versions often and updates itself.{' '}
              <Link className={s.leadLink} href={`${pages.changelog.path}#${versionAnchor(latest.version)}`}>
                What’s new in {latest.version}
              </Link>
            </>
          ) : unknown ? (
            'Free, and a preview: Telex is at 0.x, gets new versions often and updates itself. Every release is on GitHub.'
          ) : (
            'The first preview build is being finished. The installer will appear right here: this page checks for new releases every hour.'
          )
        }
        extraLd={[appLd(latest)]}
      />

      <section className={`container ${s.get}`} aria-label="Downloads">
        {!live && <LookupNote release={latest} />}
        <DownloadChoice release={latest} unknown={unknown} />
        <ReleaseStatus release={latest} unknown={unknown} />
        <Compat />
        <p className={s.alts}>
          <Link className="more" href={pages.demo.path}>
            Try it in your browser first
          </Link>
        </p>
      </section>

      <div className={`container ${s.layout}`}>
        <section aria-labelledby="install">
          <h2 id="install" className={s.h2}>
            Install on Windows in four steps
          </h2>
          <ol className={s.steps}>
            <li>
              <h3>Download the installer</h3>
              <p>
                The <code>.exe</code> is the one to pick. The <code>.msi</code> package is for installing on many PCs
                at once.
              </p>
            </li>
            <li>
              <h3>Run it</h3>
              <p>
                The installer isn’t code-signed yet, so Windows SmartScreen may say it doesn’t recognize it. Choose{' '}
                <b>More info</b>, then <b>Run anyway</b>. On Windows 10 without WebView2, the installer fetches it.
              </p>
            </li>
            <li>
              <h3>Add your account</h3>
              <p>
                Telex opens the setup on first start. Enter your name, email and password: it finds the IMAP and SMTP
                servers itself and logs in to both before it saves anything. If your provider needs an app password
                (Gmail, iCloud, Yahoo), it says so. Your own settings? Choose <b>Set up manually</b>.
              </p>
            </li>
            <li>
              <h3>Let the first sync run</h3>
              <p>
                Telex downloads the newest 300 messages of each folder into its local mail base, then checks for new
                mail every 10 minutes. New Inbox mail shows the moment it arrives, and older mail comes down when you
                ask for it.
              </p>
            </li>
          </ol>
        </section>
        <div className={s.shot}>
          <WindowFrame title={account.title}>
            <Shot id={account.id} alt={`Telex: ${account.caption}`} sizes="(max-width: 960px) calc(100vw - 32px), 600px" />
          </WindowFrame>
          <p className={s.shotCaption}>{account.caption}</p>
        </div>
      </div>

      {linux && (
        <section id="linux" className={`container ${s.linux}`} aria-labelledby="linux-h">
          <h2 id="linux-h" className={s.h2}>
            On Linux
          </h2>
          <div className={s.linuxGrid}>
            {latest.appimage && (
              <div className={s.pkg}>
                <h3>AppImage</h3>
                <p className={s.p}>
                  One file that runs on most 64-bit distributions and updates itself, like the Windows version. Make it
                  runnable and start it:
                </p>
                <pre className={s.cmd}>
                  <code>
                    chmod +x {latest.appimage.name}
                    {'\n'}./{latest.appimage.name}
                  </code>
                </pre>
                <a className="btn" href={latest.appimage.url}>
                  Download the AppImage ({megabytes(latest.appimage.size)})
                </a>
              </div>
            )}
            {latest.deb && (
              <div className={s.pkg}>
                <h3>.deb package</h3>
                <p className={s.p}>
                  For Debian, Ubuntu and their relatives. Smaller, installed like any package; updates come as a new
                  .deb from this page.
                </p>
                <pre className={s.cmd}>
                  <code>sudo apt install ./{latest.deb.name}</code>
                </pre>
                <a className="btn" href={latest.deb.url}>
                  Download the .deb ({megabytes(latest.deb.size)})
                </a>
              </div>
            )}
          </div>
          <p className={s.p}>
            Both need a 64-bit PC with Ubuntu 22.04 or newer (or a system of the same age), and a keyring for the
            passwords: GNOME Keyring or KWallet.
          </p>
        </section>
      )}

      <section id="mail" className={`container ${s.mail}`} aria-labelledby="mail-h">
        <h2 id="mail-h" className={s.h2}>
          Which mail works
        </h2>
        <div className={s.mailGrid}>
          {[providers.works, providers.appPassword, providers.later].map((g, i) => (
            <div key={g.title} className={s.group} data-kind={i === 2 ? 'later' : 'works'}>
              <h3>{g.title}</h3>
              <p className={s.p}>{g.text}</p>
              {g.link && (
                <p className={s.p}>
                  <a href={g.link.href}>{g.link.label}</a>
                </p>
              )}
              {i === 2 && (
                <p className={s.p}>
                  <Link href={pages.roadmap.path}>The roadmap</Link>
                </p>
              )}
            </div>
          ))}
        </div>
      </section>

      <div className={`container ${s.facts}`}>
        <section aria-labelledby="requirements">
          <h2 id="requirements" className={s.h2}>
            Requirements
          </h2>
          <dl className={s.req}>
            <dt>System</dt>
            <dd>Windows 10 or 11, 64-bit; or 64-bit Linux, Ubuntu 22.04 or newer</dd>
            <dt>Runtime</dt>
            <dd>WebView2, built into Windows 11 and fetched by the installer on Windows 10</dd>
            <dt>Incoming</dt>
            <dd>IMAP over SSL/TLS (port 993) or STARTTLS (port 143), with a password or an app password</dd>
            <dt>Outgoing</dt>
            <dd>SMTP over SSL/TLS (465) or STARTTLS (587), same login as IMAP</dd>
          </dl>
          <p className={s.p} style={{ marginTop: 16 }}>
            Not sure about your provider? The <Link href={pages.faq.path}>questions and answers</Link> cover the usual
            cases.
          </p>
        </section>
        <section aria-labelledby="uninstall">
          <h2 id="uninstall" className={s.h2}>
            Uninstalling
          </h2>
          <p className={s.p}>
            Remove Telex in <b>Settings → Apps</b>. Your local mail base is the file{' '}
            <code>%APPDATA%\app.telexmail.desktop\telex.db</code>; delete it too if you want the downloaded mail gone.
            Saved passwords are under “Telex” in Windows Credential Manager. Mail on your server is never touched.
          </p>
          <p className={s.p} style={{ marginTop: 16 }}>
            On Linux, delete the AppImage or remove the package (<code>sudo apt remove telex</code>). The mail base is in{' '}
            <code>~/.local/share/app.telexmail.desktop/</code>, and the passwords are in your keyring under “Telex”.
          </p>
        </section>
      </div>

      {releases.length > 0 && (
        <div className={`container ${s.versions}`}>
          <h2 className={s.h2}>All versions</h2>
          <div className={s.tableWrap}>
            <table className={s.table}>
              <thead>
                <tr>
                  <th scope="col">Version</th>
                  <th scope="col">Released</th>
                  <th scope="col">Windows</th>
                  <th scope="col">Linux</th>
                  <th scope="col">Notes</th>
                </tr>
              </thead>
              <tbody>
                {releases.map((r) => (
                  <tr key={r.version}>
                    <td className={s.mono}>
                      <Link href={`${pages.changelog.path}#${versionAnchor(r.version)}`}>{r.version}</Link>
                    </td>
                    <td>{r.date ? dateFmt.format(new Date(r.date)) : '–'}</td>
                    <td>
                      {r.exe && <a href={r.exe.url}>.exe ({megabytes(r.exe.size)})</a>}
                      {r.exe && r.msi && ' · '}
                      {r.msi && <a href={r.msi.url}>.msi</a>}
                    </td>
                    <td>
                      {r.appimage && <a href={r.appimage.url}>AppImage</a>}
                      {r.appimage && r.deb && ' · '}
                      {r.deb && <a href={r.deb.url}>.deb</a>}
                      {!r.appimage && !r.deb && '–'}
                    </td>
                    <td>
                      <a href={r.pageUrl}>On GitHub</a>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {!live && (
            <p className={s.p} style={{ marginTop: 16 }}>
              Older versions: <a href={allReleasesPage}>every release on GitHub</a>.
            </p>
          )}
        </div>
      )}
      {releases.length === 0 && unknown && (
        <div className={`container ${s.versions}`}>
          <h2 className={s.h2}>All versions</h2>
          <p className={s.p}>
            <a href={allReleasesPage}>Every release is on GitHub</a>, with its installers and notes.
          </p>
        </div>
      )}
      <CtaBand title="Questions before you install?" secondary={pages.faq} />
    </>
  );
}
