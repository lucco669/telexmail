import { site } from '@/lib/site';

// /.well-known/security.txt (RFC 9116): where to report a security problem. The required expiry
// is a year after the build, so it stays valid as long as the site is deployed at least yearly.
export function GET() {
  const built = new Date(process.env.BUILD_DATE ?? '2026-10-10T00:00:00Z');
  const expires = new Date(built.getTime() + 365 * 24 * 60 * 60 * 1000).toISOString();
  const body = [
    `Contact: mailto:${site.contact}`,
    `Expires: ${expires}`,
    'Preferred-Languages: en, pl',
    `Canonical: ${site.url}/.well-known/security.txt`,
    '',
  ].join('\n');
  return new Response(body, { headers: { 'Content-Type': 'text/plain; charset=utf-8' } });
}
