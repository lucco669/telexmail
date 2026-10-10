import { faqGroups, features, roadmap } from '@/lib/content';
import { pages } from '@/lib/pages';
import { site } from '@/lib/site';

// /llms.txt (llmstxt.org): a plain summary of the site for AI assistants, from the same data as
// the pages. Prerendered at build time like a page.
export function GET() {
  const url = (path: string) => `${site.url}${path === '/' ? '/' : path}`;
  const done = roadmap.filter((s) => s.status === 'done').map((s) => s.title);
  const next = roadmap.filter((s) => s.status !== 'done').map((s) => s.title);
  const body = [
    `# ${site.name}`,
    '',
    `> ${site.description}`,
    '',
    'Telex is a desktop email client for Windows 10 and 11 (64-bit), written in Rust with a Tauri/WebView2 window.',
    'It keeps mail in a local SQLite mail base with full-text search, syncs with IMAP servers both ways and sends over SMTP.',
    'It has no account, no telemetry and no server of its own. It is an early preview.',
    '',
    '## Highlights',
    '',
    ...features.map((f) => `- ${f.title}: ${f.text}`),
    '',
    `Works today: ${done.join(', ')}. Planned, in order: ${next.join(', ')}.`,
    '',
    '## Pages',
    '',
    ...Object.values(pages)
      .filter((p) => p.path !== '/privacy')
      .map((p) => `- [${p.label}](${url(p.path)}): ${p.description}`),
    '',
    '## Questions',
    '',
    ...faqGroups.flatMap((g) => g.items).map((q) => `- ${q.q} ${q.a}`),
    '',
    '## Optional',
    '',
    `- [${pages.privacy.label}](${url(pages.privacy.path)}): ${pages.privacy.description}`,
    '',
  ].join('\n');
  return new Response(body, { headers: { 'Content-Type': 'text/plain; charset=utf-8' } });
}
