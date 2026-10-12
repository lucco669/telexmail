import { Analytics } from '@vercel/analytics/next';
import type { Metadata, Viewport } from 'next';
import { IBM_Plex_Mono, IBM_Plex_Sans, IBM_Plex_Sans_Condensed } from 'next/font/google';
import { Footer } from '@/components/Footer';
import { Header } from '@/components/Header';
import { PageNavigation } from '@/components/PageNavigation';
import { site } from '@/lib/site';
import { THEME_SCRIPT } from '@/lib/theme-script';
import './globals.css';

// next/font downloads the fonts at build time and serves them from this site: no requests to Google.
const sans = IBM_Plex_Sans({
  subsets: ['latin', 'latin-ext'],
  weight: ['400', '500', '600'],
  variable: '--font-plex-sans',
});
const cond = IBM_Plex_Sans_Condensed({
  subsets: ['latin', 'latin-ext'],
  weight: ['600'],
  variable: '--font-plex-cond',
});
// Only small labels use mono, so it isn't worth preloading.
const mono = IBM_Plex_Mono({
  subsets: ['latin', 'latin-ext'],
  weight: ['400', '500'],
  variable: '--font-plex-mono',
  preload: false,
});

// Defaults for every page; each page adds its own title, description, canonical URL and social
// cards with pageMetadata() from lib/seo.ts. Icons (favicon.ico, icon.svg, apple-icon.png), the
// manifest, Open Graph images, robots.txt and sitemap.xml are files in app/; Next.js adds their tags.
export const metadata: Metadata = {
  metadataBase: new URL(site.url),
  title: { default: site.title, template: `%s – ${site.name}` },
  description: site.description,
  keywords: site.keywords,
  applicationName: site.name,
  category: 'technology',
  alternates: { canonical: '/' },
  robots: {
    index: true,
    follow: true,
    googleBot: { index: true, follow: true, 'max-image-preview': 'large', 'max-snippet': -1 },
  },
  openGraph: {
    type: 'website',
    url: '/',
    siteName: site.name,
    locale: 'en_US',
    title: site.title,
    description: site.description,
  },
  twitter: { card: 'summary_large_image', title: site.title, description: site.description },
  appleWebApp: { title: site.name },
  formatDetection: { telephone: false, email: false, address: false },
};

export const viewport: Viewport = {
  colorScheme: 'light dark',
  themeColor: [
    { media: '(prefers-color-scheme: light)', color: '#ffffff' },
    { media: '(prefers-color-scheme: dark)', color: '#111418' },
  ],
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    // suppressHydrationWarning: the theme script may set data-theme before React loads.
    <html lang="en" data-scroll-behavior="smooth" className={`${sans.variable} ${cond.variable} ${mono.variable}`} suppressHydrationWarning>
      <head>
        <script dangerouslySetInnerHTML={{ __html: THEME_SCRIPT }} />
      </head>
      <body id="top">
        <a className="skip" href="#content">
          Skip to content
        </a>
        <PageNavigation>
          <Header />
          <main id="content">{children}</main>
          <Footer />
        </PageNavigation>
        {/* Vercel Web Analytics: page views without cookies (see the privacy page). */}
        <Analytics />
      </body>
    </html>
  );
}
