import type { NextConfig } from 'next';

// With a custom domain attached, the project's *.vercel.app address serves the same page; keep
// search engines on the custom domain. (Vercel already hides preview deployments from them.)
const productionHost = process.env.VERCEL_PROJECT_PRODUCTION_URL ?? '';
const hasCustomDomain = productionHost !== '' && !productionHost.endsWith('.vercel.app');

const nextConfig: NextConfig = {
  // Next 16 caching model: the release lookup in lib/release.ts is a `use cache` function.
  cacheComponents: true,
  partialPrefetching: true,
  images: {
    // Screenshots are mostly text, so they get a higher quality than the default 75.
    qualities: [75, 90],
  },
  // The repo root has its own package-lock.json (the Tauri CLI); this site is a separate project.
  turbopack: { root: __dirname },
  // Used as the sitemap's lastModified.
  env: { BUILD_DATE: new Date().toISOString() },
  async headers() {
    if (!hasCustomDomain) return [];
    return [
      {
        source: '/:path*',
        has: [{ type: 'host', value: '.*\\.vercel\\.app' }],
        headers: [{ key: 'X-Robots-Tag', value: 'noindex' }],
      },
    ];
  },
};

export default nextConfig;
