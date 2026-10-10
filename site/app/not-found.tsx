import type { Metadata } from 'next';
import { Footer } from '@/components/Footer';
import { Header } from '@/components/Header';

export const metadata: Metadata = { title: 'Page not found', robots: { index: false } };

export default function NotFound() {
  return (
    <>
      <Header />
      <main className="section">
        <div className="container section-head">
          <p className="eyebrow">
            <b>404</b> · Not found
          </p>
          <h1 className="h2">No message at this address.</h1>
          <p className="lead">The page may have moved. Everything about Telex is on the home page.</p>
          <p>
            <a className="btn btn-primary" href="/">
              Go to the home page
            </a>
          </p>
        </div>
      </main>
      <Footer />
    </>
  );
}
