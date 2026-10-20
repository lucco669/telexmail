// Everything that names or places the product lives here, so a rename or a new domain is one edit.

function siteUrl(): string {
  if (process.env.NEXT_PUBLIC_SITE_URL) return process.env.NEXT_PUBLIC_SITE_URL.replace(/\/$/, '');
  // Canonicals must stay on the public domain in local builds, previews and other hosts too.
  // Vercel's production URL can be a *.vercel.app alias until a custom domain is configured.
  return 'https://telexmail.app';
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
