import type { Metadata, Viewport } from 'next';
import Script from 'next/script';
import { IBM_Plex_Mono, IBM_Plex_Sans, IBM_Plex_Sans_Condensed } from 'next/font/google';
import { Footer } from '@/components/Footer';
import { Header } from '@/components/Header';
import { PageNavigation } from '@/components/PageNavigation';
import { site } from '@/lib/site';
import { THEME_SCRIPT } from '@/lib/theme-script';
import './globals.css';

// Holds every Umami send until a first real pointer, touch, wheel or key event and drops
// automated browsers: Umami's own bot check is the user agent alone.
const UMAMI_GATE = `(function(){var human=false,wait=[],ev=['pointermove','pointerdown','touchstart','wheel','keydown'];function go(e){if(!e.isTrusted||human)return;human=true;ev.forEach(function(n){removeEventListener(n,go,true)});wait.splice(0).forEach(function(f){f()})}ev.forEach(function(n){addEventListener(n,go,{capture:true,passive:true})});window.umamiBeforeSend=function(type,payload){if(navigator.webdriver)return null;return human?payload:new Promise(function(r){wait.push(function(){r(payload)})})}})();`;

// next/font downloads the fonts at build time and serves them from this site: no requests to Google.
// Plex is optional on desktop; phones use native faces in globals.css. Avoid unconditional
// preloads so phones never fetch unused fonts, or swap text after it has painted.
const sans = IBM_Plex_Sans({
  subsets: ['latin'],
  weight: 'variable',
  display: 'optional',
  variable: '--font-plex-sans',
  preload: false,
});
const cond = IBM_Plex_Sans_Condensed({
  subsets: ['latin'],
  weight: ['600'],
  display: 'optional',
  variable: '--font-plex-cond',
  preload: false,
});
// Only small labels use mono, so it isn't worth preloading.
const mono = IBM_Plex_Mono({
  subsets: ['latin'],
  weight: ['400', '500'],
  display: 'optional',
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
        {/* Umami on the self-hosted stats.szram.uk: page views without cookies (see the privacy page). */}
        <script dangerouslySetInnerHTML={{ __html: UMAMI_GATE }} />
        <Script
          src="https://stats.szram.uk/script.js"
          data-website-id="2f758525-dfe0-4e8f-ac75-a69453171fcf"
          data-before-send="umamiBeforeSend"
          strategy="afterInteractive"
        />
      </body>
    </html>
  );
}
