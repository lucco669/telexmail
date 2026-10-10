import type { Metadata } from 'next';
import Link from 'next/link';

export const metadata: Metadata = { title: 'Page not found', robots: { index: false }, alternates: { canonical: null } };

export default function NotFound() {
  return (
    <div className="section">
      <div className="container section-head">
        <p className="eyebrow">
          <b>404</b> · Not found
        </p>
        <h1 className="h1">No message at this address.</h1>
        <p className="lead">The page may have moved. Everything about Telex starts on the home page.</p>
        <p>
          <Link className="btn btn-primary" href="/">
            Go to the home page
          </Link>
        </p>
      </div>
    </div>
  );
}
