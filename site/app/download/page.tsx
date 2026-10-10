import Link from 'next/link';
import { CtaBand } from '@/components/CtaBand';
import { DownloadButton } from '@/components/DownloadButton';
import { PageHeader } from '@/components/PageHeader';
import { Shot } from '@/components/Shot';
import { WindowFrame } from '@/components/WindowFrame';
import { screens } from '@/lib/content';
import { pages } from '@/lib/pages';
import { getReleases, megabytes } from '@/lib/release';
import { appLd, pageMetadata } from '@/lib/seo';
import s from './page.module.css';

export const metadata = pageMetadata(pages.download);

const dateFmt = new Intl.DateTimeFormat('en-GB', { day: 'numeric', month: 'long', year: 'numeric', timeZone: 'UTC' });
const account = screens.find((x) => x.id === 'account')!;

export default async function DownloadPage() {
  const releases = await getReleases();
  const latest = releases[0] ?? null;
  const file = latest?.exe ?? latest?.msi;
  return (
    <>
      <PageHeader
        page={pages.download}
        title="Get Telex for Windows."
        lead={
          latest && file
            ? `Version ${latest.version}${latest.date ? `, released ${dateFmt.format(new Date(latest.date))}` : ''}. A ${megabytes(file.size)} installer for Windows 10 and 11.`
            : 'The first preview build is being finished. The installer will appear right here: this page checks for new releases every hour.'
        }
        extraLd={[appLd(latest)]}
      >
        <DownloadButton release={latest} />
        {latest?.exe && latest.msi && (
          <a className={s.alt} href={latest.msi.url}>
            .msi package ({megabytes(latest.msi.size)})
          </a>
        )}
      </PageHeader>

      <div className={`container ${s.layout}`}>
        <section aria-labelledby="install">
          <h2 id="install" className={s.h2}>
            Install in four steps
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
                Click <b>Add your account</b> under the folders and enter your email, IMAP server and password. The SMTP
                server defaults to the IMAP one. Telex logs in to both before it saves anything.
              </p>
            </li>
            <li>
              <h3>Let the first sync run</h3>
              <p>
                Telex downloads the newest 300 messages of each folder into its local mail base, then checks for new
                mail every 10 minutes. Until you add an account, it shows a sample mailbox to try things on.
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

      <div className={`container ${s.facts}`}>
        <section aria-labelledby="requirements">
          <h2 id="requirements" className={s.h2}>
            Requirements
          </h2>
          <dl className={s.req}>
            <dt>System</dt>
            <dd>Windows 10 or 11, 64-bit</dd>
            <dt>Runtime</dt>
            <dd>WebView2, built into Windows 11 and fetched by the installer on Windows 10</dd>
            <dt>Incoming</dt>
            <dd>IMAP over SSL/TLS, port 993, with a password</dd>
            <dt>Outgoing</dt>
            <dd>SMTP over SSL/TLS (465) or STARTTLS (587), same login as IMAP</dd>
            <dt>Not yet</dt>
            <dd>
              Gmail and Outlook.com, which need OAuth2 sign-in (<Link href={pages.roadmap.path}>roadmap</Link>)
            </dd>
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
        </section>
      </div>

      {releases.length > 0 && (
        <div className={`container ${s.versions}`}>
          <h2 className={s.h2}>All versions</h2>
          <table className={s.table}>
            <thead>
              <tr>
                <th scope="col">Version</th>
                <th scope="col">Released</th>
                <th scope="col">Installer</th>
                <th scope="col">Notes</th>
              </tr>
            </thead>
            <tbody>
              {releases.map((r) => (
                <tr key={r.version}>
                  <td className={s.mono}>{r.version}</td>
                  <td>{r.date ? dateFmt.format(new Date(r.date)) : '–'}</td>
                  <td>
                    {r.exe && <a href={r.exe.url}>.exe ({megabytes(r.exe.size)})</a>}
                    {r.exe && r.msi && ' · '}
                    {r.msi && <a href={r.msi.url}>.msi</a>}
                  </td>
                  <td>
                    <a href={r.pageUrl}>Release notes</a>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <CtaBand title="Questions before you install?" secondary={pages.faq} />
    </>
  );
}
