// Everything that names or places the product lives here, so a rename or a new domain is one edit.

function siteUrl(): string {
  if (process.env.NEXT_PUBLIC_SITE_URL) return process.env.NEXT_PUBLIC_SITE_URL;
  // Set by Vercel: the production domain, e.g. your custom domain once it is attached.
  if (process.env.VERCEL_PROJECT_PRODUCTION_URL) return `https://${process.env.VERCEL_PROJECT_PRODUCTION_URL}`;
  return 'http://localhost:3000';
}

export const site = {
  name: 'Telex',
  title: 'Telex – fast, keyboard-driven email for Windows',
  description:
    'A compact, fast mail client for Windows. Dense message lists, one key for every action, and your mail on your own disk, searchable offline. No Electron, no account, no telemetry.',
  url: siteUrl(),
  // GitHub repository whose releases hold the installers. It must be public for visitors to download.
  releasesRepo: process.env.TELEX_RELEASES_REPO ?? 'lucco669/telexmail',
};
