import data from '@/lib/themes.json';

export function generateStaticParams() {
  return data.themes.map(({ id }) => ({ theme: `${id}.json` }));
}

// Public, static evidence, separate from the indexable page. No credentials or external fetches.
export async function GET(_request: Request, { params }: { params: Promise<{ theme: string }> }) {
  const { theme } = await params;
  const measured = data.themes.find(({ id }) => `${id}.json` === theme);
  const headers = { 'X-Robots-Tag': 'noindex', 'Cache-Control': 'public, max-age=0, must-revalidate' };
  if (!measured) return Response.json({ error: 'Unknown theme.' }, { status: 404, headers });
  return Response.json({ tokens: measured.tokens, pairs: measured.pairs }, { headers });
}
