import { PageHeader } from '@/components/PageHeader';
import { pages } from '@/lib/pages';
import { pageMetadata } from '@/lib/seo';
import { site } from '@/lib/site';
import s from './page.module.css';

export const metadata = pageMetadata(pages.privacy);

// Change this date whenever the text below changes.
const UPDATED = '10 October 2026';

export default function PrivacyPage() {
  return (
    <>
      <PageHeader
        page={pages.privacy}
        title="Privacy."
        lead="The short version: the Telex app has no account, no telemetry and no Telex server, and this website has no cookies or trackers, only cookieless page counts."
      />
      <div className={`container ${s.layout}`}>
        <aside className={s.summary}>
          <p className={s.updated}>Last updated {UPDATED}</p>
          <ul>
            <li>Your mail stays on your PC and your mail server</li>
            <li>The app talks to your mail servers, and to image servers only when you ask</li>
            <li>No cookies or trackers on this site, only cookieless page counts</li>
          </ul>
        </aside>
        <article className="prose">
          <h2>The Telex app</h2>
          <h3>What it stores on your PC</h3>
          <p>
            The messages it downloads (in full, so you can read and search them offline), your folders and your account
            settings. All of it is in one file: <code>%APPDATA%\app.telexmail.desktop\telex.db</code>. Passwords are
            kept apart from it, in Windows Credential Manager under “Telex”.
          </p>
          <h3>Who it talks to</h3>
          <p>
            The IMAP and SMTP servers you set up, over encrypted connections (SSL/TLS, or STARTTLS for SMTP on port
            587). There is no Telex server: your mail never passes through us, and we never see it.
          </p>
          <p>
            And, only when you choose to load a message’s remote images (once, or always for a sender you trust, never
            automatically in Spam), the servers those images come from. Telex downloads them itself, without cookies or
            a referrer, and refuses addresses on your local network; the window that shows the message never goes
            online. Links you click open in your default browser.
          </p>
          <h3>What it doesn’t do</h3>
          <ul>
            <li>No usage statistics, crash reports or telemetry of any kind.</li>
            <li>No account to create, and no cloud service.</li>
            <li>
              No remote images until you choose to load them, so senders can’t use them to see when you read their
              mail.
            </li>
            <li>
              No update checks yet. When automatic updates arrive, this page will say exactly what they send, before
              they ship.
            </li>
          </ul>
          <h3>Removing your data</h3>
          <p>
            Removing an account in Telex deletes its local copies and its saved password; the mail on your server stays
            where it is. To remove everything, uninstall Telex and delete the <code>telex.db</code> file above.
          </p>

          <h2>This website</h2>
          <h3 id="cookies">No cookies, ads or trackers</h3>
          <p>
            This site sets no cookies and runs no ads, tracking pixels or third-party scripts, so there is no cookie
            banner to click away. Its fonts are served from this site, not from a font service.
          </p>
          <h3>Visitor statistics</h3>
          <p>
            To know which pages are read, the site uses Vercel Web Analytics. It counts page views, the page you came
            from, and your country, browser and device type. It sets no cookies, doesn’t follow you to other sites and
            doesn’t identify you; the script comes from this site itself. See{' '}
            <a href="https://vercel.com/docs/analytics/privacy-policy">how Vercel Web Analytics handles data</a>.
          </p>
          <h3>Hosting</h3>
          <p>
            The site is hosted by Vercel. Like any web server, Vercel processes your IP address and the details of each
            request to deliver the pages and keep the service secure. See{' '}
            <a href="https://vercel.com/legal/privacy-policy">Vercel’s privacy policy</a>.
          </p>
          <h3>Theme</h3>
          <p>
            If you pick light or dark, the choice is saved in your browser’s local storage on your own device. It is
            never sent anywhere.
          </p>
          <h3>Downloads</h3>
          <p>
            The site looks up the newest release on GitHub from its own server; your browser doesn’t contact GitHub
            until you click a download link. The installers are served by GitHub, under{' '}
            <a href="https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement">
              GitHub’s privacy statement
            </a>
            .
          </p>

          {site.contact && (
            <>
              <h2>Questions</h2>
              <p>
                Write to <a href={`mailto:${site.contact}`}>{site.contact}</a> with any question about this page or your
                data.
              </p>
            </>
          )}

          <h2>Changes</h2>
          <p>If anything here changes, this page changes first, with a new date at the top.</p>
        </article>
      </div>
    </>
  );
}
