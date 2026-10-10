/** Structured data for search engines, as one schema.org graph. Builders are in lib/seo.ts. */
export function JsonLd({ items }: { items: object[] }) {
  const data = { '@context': 'https://schema.org', '@graph': items };
  return (
    <script
      type="application/ld+json"
      // Escaping "<" keeps the JSON from closing the script tag early.
      dangerouslySetInnerHTML={{ __html: JSON.stringify(data).replace(/</g, '\u003c') }}
    />
  );
}
