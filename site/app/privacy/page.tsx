import { PageLink as Link } from '@/components/PageNavigation';
import { PageHeader } from '@/components/PageHeader';
import { pages } from '@/lib/pages';
import { pageMetadata } from '@/lib/seo';
import { site } from '@/lib/site';
import s from './page.module.css';

export const metadata = pageMetadata(pages.privacy);

// Change this date whenever the text below changes.
const UPDATED = '13 October 2026';

export default function PrivacyPage() {
  return (
    <>
      <PageHeader
        page={pages.privacy}
        title="Privacy."
        lead="Where Telex keeps your mail and passwords, when the app goes online, and what this website counts."
      >
        <Link className="btn" href={pages.security.path}>Privacy & security evidence</Link>
      </PageHeader>
      <div className={`container ${s.layout}`}>
        <aside className={s.summary}>
          <p className={s.updated}>Last updated {UPDATED}</p>
          <ul>
            <li>Your mail stays on your PC and your mail server</li>
            <li>Mail, account discovery, images, dictionaries and updates have different network uses</li>
            <li>Update checks contact GitHub only if allowed by your setting or started manually</li>
            <li>Saved searches and scam warning checks stay local; configured forwards send mail to their recipients</li>
            <li>This site uses cookieless Vercel Web Analytics for page counts</li>
            <li>The live demo runs in your browser: nothing you do there is sent</li>
          </ul>
        </aside>
        <article className="prose">
          <h2>The Telex app</h2>
          <h3>What it stores on your PC</h3>
          <p>
            The messages it downloads (in full, so you can read and search them offline), your folders, accounts, rules,
            identities, saved search names and queries, and rule-action history used to prevent duplicate copies and forwards.
            These are in <code>%APPDATA%\app.telexmail.desktop\telex.db</code>. Your choices in
            Settings (theme, language and the like) are in <code>settings.json</code> next to it. Passwords are kept
            apart from both, in Windows Credential Manager under “Telex”, or a Secret Service keyring on Linux.
          </p>
          <p>
            Telex does not encrypt the local mail database or its backups. Settings, exports and saved attachments
            are also ordinary files, protected by your operating system and storage. On Linux the app’s data is in{' '}
            <code>~/.local/share/app.telexmail.desktop/</code>. See the{' '}
            <Link href={`${pages.security.path}#limits`}>current security limits</Link>.
          </p>
          <p>
            From version 0.5, an attachment you open is written to a Telex folder among your temporary files, marked
            as coming from the internet so Windows treats it with care, and that folder is emptied each time Telex
            starts. Attachments you save go where you choose. The addresses Telex suggests while you write come from
            your own mail base; nothing is looked up anywhere.
          </p>
          <p>
            Exporting your settings writes a file wherever you choose, with your settings, rules, identities and
            signatures, and your accounts’ server names and user names. It never holds passwords. Nothing is sent
            anywhere: what you do with the file is up to you.
          </p>
          <h3 id="local-checks">Saved searches and local mail inspection</h3>
          <p>
            A saved search stores a query, scope and list options in your local mail base. Opening it searches downloaded
            mail on this PC, without sending the query to a server or making extra message copies. Deleting a saved
            search removes that view, not the mail it finds. Mail-base backups include saved searches.
          </p>
          <p>
            Link inspection displays addresses already present in a message. Hovering, focusing or copying them does
            not visit the destination. The scam checker also runs locally, when you open a message, with recent reports
            cached in memory. It uploads nothing, performs no DNS or reputation lookup, fetches no images and follows
            no redirects. It does not move, delete or mark mail read; your reading setting still controls read flags.
            Its limited, advisory score is explained on the <Link href={`${pages.security.path}#limits`}>security page</Link>.
          </p>
          <h3>Who it talks to</h3>
          <p>
            The IMAP and SMTP servers you set up, over encrypted connections (SSL/TLS, or STARTTLS on port 587 and, from
            version 0.5, 143), never unencrypted. There is no Telex server: your mail never passes through us, and we
            never see it. From version 0.5, to show new mail at once, Telex keeps one connection open to each
            account’s IMAP server, watching the Inbox (IMAP IDLE); you can turn that off in Settings › Mail.
          </p>
          <p>
            Rules can copy mail to a server folder or forward newly arriving mail to addresses you configure.
            A forward sends the message through your SMTP server to those recipients; automatic and away replies also
            send through your account. These actions need no Telex relay. Previewing a rule checks local matches and
            sends nothing; running rules manually does not forward old messages or send automatic replies.
          </p>
          <p>
            When you add an account, Telex finds its servers itself. It looks in its own list of common providers
            first; for other addresses it asks your domain’s web server for a setup file (at{' '}
            <code>autoconfig.</code> and <code>/.well-known/</code> addresses, with your email address so a provider
            can answer for it), reads your domain’s DNS records, and tries the usual server names such as{' '}
            <code>mail.</code> and <code>imap.</code> your domain. All of that goes to DNS and to servers of your own
            domain or its mail provider; HTTPS redirects from a setup URL may lead to another server.
          </p>
          <p>
            And, only when you choose to load a message’s remote images (once, or always for a sender you trust, never
            automatically in Spam), the servers those images come from. Telex downloads them itself, without cookies or
            a referrer, and refuses addresses on your local network; the window that shows the message never goes
            online. Links you click open in your default browser.
          </p>
          <p>
            Downloading a spelling dictionary, when you choose to do so, requests LibreOffice’s dictionary files
            from GitHub at a fixed commit. Telex checks each file against its recorded SHA-256 hash. Setting a
            server-side away reply contacts your provider’s ManageSieve service with STARTTLS before login.
          </p>
          <h3 id="updates">Update checks</h3>
          <p>
            Telex looks for a new version only if you allow it. From the first start it asks whether to check when it
            starts; yes means a check a few seconds after each start and once a day while it stays open (Settings ›
            Updates can make it once a day only). If you say no, it never checks on its own. Clicking the version number
            in the status bar checks right away. A check is a single request to GitHub, where Telex is published:
          </p>
          <ul>
            <li>
              It asks for{' '}
              <code>https://github.com/lucco669/telexmail/releases/latest/download/latest.json</code>, a small file that
              is the same for everyone. It names the newest version, what’s new in it and where its installers are.
              GitHub sends the request on to its file servers.
            </li>
            <li>
              Besides the address, the request carries two headers, <code>User-Agent: Telex</code> and{' '}
              <code>Accept: application/json</code>. No cookies, no identifier, not even your Telex version, and nothing
              about you or your mail.
            </li>
            <li>
              Like any request on the internet, it shows GitHub your IP address and the time. GitHub handles that under
              its{' '}
              <a href="https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement">
                privacy statement
              </a>
              .
            </li>
          </ul>
          <p>
            Nothing installs without your click. When you choose <b>Install and restart</b>, Telex downloads the
            installer from GitHub the same way (with <code>Accept: application/octet-stream</code>), checks that it is
            signed with Telex’s release key, and only then runs it. A file without that signature is refused. Builds
            you make yourself from the source can’t update at all.
          </p>
          <h3>What it doesn’t do</h3>
          <ul>
            <li>The desktop app has no usage-analytics or crash-report upload feature.</li>
            <li>No account to create, and no cloud service.</li>
            <li>
              No remote images until you choose to load them, so senders can’t use them to see when you read their
              mail.
            </li>
            <li>No update checks unless you allow them, and no update installed without your click.</li>
          </ul>
          <h3>Removing your data</h3>
          <p>
            Removing an account in Telex deletes its local copies and requests deletion of its saved password; the mail on your server stays
            where it is. Backups and files you exported or saved separately remain. After uninstalling, remove the
            app’s data folder, any separate backups and exports, and remaining “Telex” credentials from your system’s
            credential store if you want those copies gone. File deletion is not a promise of secure erasure.
          </p>

          <h2>This website</h2>
          <h3 id="cookies">Cookies and ads</h3>
          <p>
            This site has no advertising integration. Its visitor statistics use cookieless Vercel Web Analytics,
            described below. Its fonts are served from this site, not from a font service.
          </p>
          <h3>Visitor statistics</h3>
          <p>
            To know which pages are read, the site uses Vercel Web Analytics. It counts page views, the page you came
            from, and your country, browser and device type. It uses no cookies; the analytics script is served
            through this site. See{' '}
            <a href="https://vercel.com/docs/analytics/privacy-policy">how Vercel Web Analytics handles data</a>.
          </p>
          <h3>Hosting</h3>
          <p>
            The site is hosted by Vercel. Like any web server, Vercel processes your IP address and the details of each
            request to deliver the pages and keep the service secure. See{' '}
            <a href="https://vercel.com/legal/privacy-notice">Vercel’s privacy policy</a>.
          </p>
          <h3>Theme</h3>
          <p>
            If you pick light or dark, the choice is saved in your browser’s local storage on your own device. It is
            never sent anywhere.
          </p>
          <h3 id="demo">The live demo</h3>
          <p>
            The <Link href={pages.demo.path}>live demo</Link> is Telex’s own interface with a made-up mailbox, and it runs
            entirely in your browser. What you type, “send” or attach there stays in the page and is gone when you
            reload it; files you attach are never read, only their names and sizes are shown. Like the app, it keeps
            its theme, language, text size and pane sizes in your browser’s local storage. Links in the demo mailbox
            open in a new tab, as they would in your browser from the app.
          </p>
          <p>
            Demo saved searches and rule edits stay in memory and reset on reload, as do mail changes. The scam report
            is a fixed illustrative fixture; the desktop Rust scanner does not run in your browser. The demo cannot
            send automatic forwards or real replies to anyone.
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
