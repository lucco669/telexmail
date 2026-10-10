import type { NextConfig } from 'next';

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
};

export default nextConfig;
