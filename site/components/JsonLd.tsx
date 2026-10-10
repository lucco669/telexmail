import { faq, features, hero, screens } from '@/lib/content';
import type { Release } from '@/lib/release';
import { site } from '@/lib/site';

/**
 * Structured data for search engines: the app (SoftwareApplication), the site and the FAQ.
 * No price or rating: there is no pricing yet, and ratings must come from real reviews.
 */
export function JsonLd({ release }: { release: Release | null }) {
  const file = release?.exe ?? release?.msi;
  const data = {
    '@context': 'https://schema.org',
    '@graph': [
      {
        '@type': 'WebSite',
        '@id': `${site.url}/#website`,
        url: `${site.url}/`,
        name: site.name,
        description: site.description,
        inLanguage: 'en',
      },
      {
        '@type': 'SoftwareApplication',
        '@id': `${site.url}/#app`,
        name: site.name,
        description: site.description,
        url: `${site.url}/`,
        applicationCategory: 'CommunicationApplication',
        operatingSystem: 'Windows 10, Windows 11',
        image: `${site.url}/icon-512.png`,
        screenshot: [hero, ...screens].map((s) => `${site.url}/screens/${s.id}-light.png`),
        featureList: features.map((f) => f.title),
        ...(release && file
          ? {
              softwareVersion: release.version,
              downloadUrl: file.url,
              fileSize: `${Math.round(file.size / 1024 / 1024)}MB`,
              ...(release.date && { datePublished: release.date.slice(0, 10) }),
            }
          : {}),
      },
      {
        '@type': 'FAQPage',
        '@id': `${site.url}/#faq`,
        mainEntity: faq.map((item) => ({
          '@type': 'Question',
          name: item.q,
          acceptedAnswer: { '@type': 'Answer', text: item.a },
        })),
      },
    ],
  };
  return (
    <script
      type="application/ld+json"
      // Escaping "<" keeps the JSON from closing the script tag early.
      dangerouslySetInnerHTML={{ __html: JSON.stringify(data).replace(/</g, '\u003c') }}
    />
  );
}
