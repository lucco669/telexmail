import type { MetadataRoute } from 'next';
import { hero, screens } from '@/lib/content';
import { pages } from '@/lib/pages';
import { site } from '@/lib/site';

const shots = [hero, ...screens].map((s) => `${site.url}/screens/${s.id}-light.webp`);
const priority: Record<string, number> = { '/': 1, '/download': 0.9, '/features': 0.8 };

// Every page. Pages that show screenshots list them, so image search can find them.
// Omit lastModified until we track actual content changes; a deploy date isn't a page update.
export default function sitemap(): MetadataRoute.Sitemap {
  return Object.values(pages).map((p) => ({
    url: `${site.url}${p.path === '/' ? '/' : p.path}`,
    changeFrequency: p.path === '/privacy' ? 'yearly' : 'weekly',
    priority: priority[p.path] ?? 0.6,
    ...(['/', '/features', '/screenshots'].includes(p.path) && { images: shots }),
    ...(p.path === pages.history.path && { images: [`${site.url}/history/teleprinter.webp`] }),
  }));
}
