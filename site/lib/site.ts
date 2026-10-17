// Everything that names or places the product lives here, so a rename or a new domain is one edit.

function siteUrl(): string {
  if (process.env.NEXT_PUBLIC_SITE_URL) return process.env.NEXT_PUBLIC_SITE_URL.replace(/\/$/, '');
  // Set by Vercel: the production domain, e.g. your custom domain once it is attached.
  if (process.env.VERCEL_PROJECT_PRODUCTION_URL) return `https://${process.env.VERCEL_PROJECT_PRODUCTION_URL}`;
  return 'http://localhost:3000';
}

export const site = {
  name: 'Telex',
  // Search results show about 60 characters of the title and 155 of the description.
  title: 'Telex – fast email for Windows and Linux',
  description:
    'Fast email for Windows and Linux: offline saved searches, powerful rule previews, optional threads and local scam checks. No Electron or tracking.',
  keywords: [
    'email client for Windows',
    'email client for Linux',
    'IMAP email client',
    'keyboard-driven email',
    'offline email search',
    'saved email searches',
    'email sorting rules',
    'offline scam checks',
    'lightweight email client',
    'The Bat! alternative',
  ],
  url: siteUrl(),
  // Shown on the privacy page as the address for questions about personal data.
  contact: process.env.NEXT_PUBLIC_CONTACT_EMAIL ?? 'admin@telexmail.app',
  // Security evidence always points to the source repository, even if downloads move elsewhere.
  sourceRepo: 'lucco669/telexmail',
  // GitHub repository whose releases hold the installers. It must be public for visitors to download.
  releasesRepo: process.env.TELEX_RELEASES_REPO ?? 'lucco669/telexmail',
};
