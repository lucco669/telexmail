import Link from 'next/link';
import { faq } from '@/lib/content';
import { pages } from '@/lib/pages';
import { megabytes, type Release } from '@/lib/release';
import { DownloadButton } from './DownloadButton';
import s from './Download.module.css';

const dateFmt = new Intl.DateTimeFormat('en-GB', { day: 'numeric', month: 'long', year: 'numeric', timeZone: 'UTC' });

export function Download({ release }: { release: Release | null }) {
  const file = release?.exe ?? release?.msi;
  return (
    <section id="download" className="section">
      <div className={`container ${s.layout}`}>
        <div className={s.card}>
          <p className="eyebrow">
            <b>07</b> · Download
          </p>
          <h2 className="h2">Get Telex for Windows.</h2>
          {release && file ? (
            <p className={s.text}>
              Version {release.version}
              {release.date && `, released ${dateFmt.format(new Date(release.date))}`}. Enter your address and password: Telex
              finds your servers and downloads your newest mail into a local, searchable mail base.
            </p>
          ) : (
            <p className={s.text}>
              The first preview build is being finished. The installer will appear right here: this page checks for new
              releases every hour.
            </p>
          )}
          <div className={s.actions}>
            <DownloadButton release={release} />
            {release?.exe && release.msi && (
              <a className={s.alt} href={release.msi.url}>
                .msi package ({megabytes(release.msi.size)})
              </a>
            )}
          </div>
          {(release?.appimage || release?.deb) && (
            <p className={s.linux}>
              On Linux:{' '}
              {release.appimage && <a href={release.appimage.url}>AppImage ({megabytes(release.appimage.size)})</a>}
              {release.appimage && release.deb && ' or '}
              {release.deb && <a href={release.deb.url}>.deb ({megabytes(release.deb.size)})</a>}, 64-bit.
            </p>
          )}

          <dl className={s.req}>
            <dt>System</dt>
            <dd>Windows 10 or 11, 64-bit; or a 64-bit Linux such as Ubuntu 22.04 and newer</dd>
            <dt>Runtime</dt>
            <dd>WebView2. Built into Windows 11; on Windows 10 the installer fetches it if needed.</dd>
            <dt>Mail</dt>
            <dd>An IMAP account (SSL/TLS on 993, or STARTTLS on 143 from 0.5) and SMTP (465 or 587), with a password.</dd>
          </dl>
          <p className={s.note}>
            The installer isn’t code-signed yet, so Windows SmartScreen may warn you. Choose <b>More info</b>, then{' '}
            <b>Run anyway</b>.
          </p>
          <p className={s.links}>
            <Link className="more" href={pages.download.path}>
              Installation steps and all versions
            </Link>
            <Link className="more" href={pages.demo.path}>
              Try it in your browser first
            </Link>
          </p>
        </div>

        <div>
          <h3 className={s.faqTitle}>Questions</h3>
          {faq.map((item) => (
            <details key={item.q} className={s.item} name="faq">
              <summary className={s.q}>{item.q}</summary>
              <p className={s.a}>{item.a}</p>
            </details>
          ))}
          <p className={s.faqMore}>
            <Link className="more" href={pages.faq.path}>
              All questions
            </Link>
          </p>
        </div>
      </div>
    </section>
  );
}
