import type { Metadata } from 'next';
import { features, hero, screens, type Question } from './content';
import { pages, type PageInfo } from './pages';
import type { Release } from './release';
import { site } from './site';

const abs = (path: string) => `${site.url}${path === '/' ? '/' : path}`;

/** Title, description, canonical URL and social cards for one page. Images come from the
 *  opengraph-image.png next to each page (made by `npm run screens`). */
export function pageMetadata(p: PageInfo): Metadata {
  const isHome = p.path === '/';
  const title = isHome ? p.title : `${p.title} – ${site.name}`;
  return {
    title: isHome ? { absolute: p.title } : p.title,
    description: p.description,
    alternates: { canonical: p.path },
    openGraph: { type: 'website', siteName: site.name, locale: 'en_US', url: p.path, title, description: p.description },
    twitter: { card: 'summary_large_image', title, description: p.description },
  };
}

// schema.org objects, combined into one @graph by <JsonLd>.

/** Who makes Telex. Only facts that are true: no address, phone or ratings we don't have. */
export const organizationLd = () => ({
  '@type': 'Organization',
  '@id': `${site.url}/#organization`,
  name: site.name,
  url: abs('/'),
  logo: abs('/icon-512.png'),
  ...(site.contact && {
    email: site.contact,
    contactPoint: { '@type': 'ContactPoint', contactType: 'customer support', email: site.contact, availableLanguage: ['en', 'pl'] },
  }),
});

export const websiteLd = () => ({
  '@type': 'WebSite',
  '@id': `${site.url}/#website`,
  url: abs('/'),
  name: site.name,
  description: site.description,
  inLanguage: 'en',
  publisher: { '@id': `${site.url}/#organization` },
});

/** The app. No price or rating: there is no pricing yet, and ratings must come from real reviews. */
export function appLd(release: Release | null) {
  const file = release?.exe ?? release?.msi;
  return {
    '@type': 'SoftwareApplication',
    '@id': `${site.url}/#app`,
    name: site.name,
    description: site.description,
    url: abs('/'),
    applicationCategory: 'CommunicationApplication',
    operatingSystem: release?.appimage || release?.deb ? 'Windows 10, Windows 11, Linux' : 'Windows 10, Windows 11',
    image: abs('/icon-512.png'),
    screenshot: [hero, ...screens].map((s) => abs(`/screens/${s.id}-light.webp`)),
    featureList: features.map((f) => f.title),
    publisher: { '@id': `${site.url}/#organization` },
    ...(release && file
      ? {
          softwareVersion: release.version,
          downloadUrl: file.url,
          fileSize: `${Math.round(file.size / 1024 / 1024)}MB`,
          ...(release.date && { datePublished: release.date.slice(0, 10) }),
        }
      : {}),
  };
}

export const faqLd = (items: Question[]) => ({
  '@type': 'FAQPage',
  mainEntity: items.map((item) => ({
    '@type': 'Question',
    name: item.q,
    acceptedAnswer: { '@type': 'Answer', text: item.a },
  })),
});

export const breadcrumbLd = (p: PageInfo) => ({
  '@type': 'BreadcrumbList',
  itemListElement: [pages.home, p].map((item, i) => ({
    '@type': 'ListItem',
    position: i + 1,
    name: item.path === '/' ? site.name : item.label,
    item: abs(item.path),
  })),
});
