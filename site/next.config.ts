import type { NextConfig } from 'next';

// With a custom domain attached, the project's *.vercel.app address serves the same page; keep
// search engines on the custom domain. (Vercel already hides preview deployments from them.)
const productionHost = process.env.VERCEL_PROJECT_PRODUCTION_URL ?? '';
const hasCustomDomain = productionHost !== '' && !productionHost.endsWith('.vercel.app');

// Content Security Policy: everything from this site only. 'unsafe-inline' scripts are needed for
// the inline data Next.js streams into static pages and for the theme script (nonces would make
// every page dynamic). Preview deployments also allow Vercel's comment toolbar.
const preview = process.env.VERCEL_ENV === 'preview';
const live = preview ? ' https://vercel.live' : '';
const csp = [
  "default-src 'self'",
  `script-src 'self' 'unsafe-inline'${live}`,
  `style-src 'self' 'unsafe-inline'${live}`,
  `img-src 'self' data: blob:${preview ? ' https://vercel.live https://vercel.com' : ''}`,
  `font-src 'self'${preview ? ' https://vercel.live https://assets.vercel.com' : ''}`,
  `connect-src 'self'${preview ? ' https://vercel.live wss://ws-us3.pusher.com' : ''}`,
  // Frames from this site only: the live demo on /demo, and the HTML mail frame inside it.
  `frame-src 'self'${live}`,
  "frame-ancestors 'none'",
  "object-src 'none'",
  "base-uri 'self'",
  "form-action 'self'",
  'upgrade-insecure-requests',
].join('; ');

// The live demo (public/demo-app, made by `npm run demo`) may be framed by this site's /demo page.
const demoCsp = csp.replace("frame-ancestors 'none'", "frame-ancestors 'self'");

const securityHeaders = [
  { key: 'Content-Security-Policy', value: csp },
  // .app is HSTS-preloaded as a whole; this says the same for any other domain.
  { key: 'Strict-Transport-Security', value: 'max-age=63072000; includeSubDomains; preload' },
  { key: 'X-Content-Type-Options', value: 'nosniff' },
  { key: 'X-Frame-Options', value: 'DENY' },
  { key: 'Referrer-Policy', value: 'strict-origin-when-cross-origin' },
  { key: 'Permissions-Policy', value: 'camera=(), microphone=(), geolocation=(), payment=(), usb=(), browsing-topics=()' },
  { key: 'Cross-Origin-Opener-Policy', value: 'same-origin' },
];

const nextConfig: NextConfig = {
  poweredByHeader: false,
  // Next 16 caching model: the release lookup in lib/release.ts is a `use cache` function.
  cacheComponents: true,
  partialPrefetching: true,
  images: {
    // Screenshots are mostly text, so they get a higher quality than the default 75.
    qualities: [75, 90],
    // Optimized screenshots change only with a deploy, and each deploy has its own image URLs.
    minimumCacheTTL: 60 * 60 * 24 * 31,
  },
  // The repo root has its own package-lock.json (the Tauri CLI); this site is a separate project.
  turbopack: { root: __dirname },
  // Used for the footer year and security.txt's expiry.
  env: { BUILD_DATE: new Date().toISOString() },
  async headers() {
    const rules = [];
    // Not in development: the dev server needs eval for fast refresh.
    if (process.env.NODE_ENV === 'production') {
      rules.push({ source: '/:path*', headers: securityHeaders });
      // Later rules win for the same header.
      rules.push({
        source: '/demo-app/:path*',
        headers: [
          { key: 'Content-Security-Policy', value: demoCsp },
          { key: 'X-Frame-Options', value: 'SAMEORIGIN' },
        ],
      });
    }
    // The demo's own page is the app's index.html; /demo is the page to find.
    rules.push({ source: '/demo-app/:path*', headers: [{ key: 'X-Robots-Tag', value: 'noindex' }] });
    // Screenshots and icons in public/ aren't fingerprinted: cache a day, refresh in the background.
    rules.push({
      source: '/:file(screens/.*|icon-.*\\.png)',
      headers: [{ key: 'Cache-Control', value: 'public, max-age=86400, stale-while-revalidate=604800' }],
    });
    if (hasCustomDomain) {
      rules.push({
        source: '/:path*',
        has: [{ type: 'host' as const, value: '.*\\.vercel\\.app' }],
        headers: [{ key: 'X-Robots-Tag', value: 'noindex' }],
      });
    }
    return rules;
  },
};

export default nextConfig;
