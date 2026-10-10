import type { MetadataRoute } from 'next';
import { hero, screens } from '@/lib/content';
import { site } from '@/lib/site';

// One page. The screenshots are listed so image search can find them.
export default function sitemap(): MetadataRoute.Sitemap {
  return [
    {
      url: `${site.url}/`,
      lastModified: process.env.BUILD_DATE,
      changeFrequency: 'weekly',
      priority: 1,
      images: [hero, ...screens].map((s) => `${site.url}/screens/${s.id}-light.png`),
    },
  ];
}
