import { faqGroups, features, roadmap, versions } from '@/lib/content';
import { knownRelease } from '@/lib/known-release';
import { pages } from '@/lib/pages';
import { site } from '@/lib/site';
import { compareVersions } from '@/lib/versions';

// /llms.txt (llmstxt.org): a plain summary of the site for AI assistants, from the same data as
// the pages. Prerendered at build time like a page.
export function GET() {
  const url = (path: string) => `${site.url}${path === '/' ? '/' : path}`;
  // Written at build time without asking GitHub: what is built for a version newer than the last
  // known release (lib/known-release.ts) is listed as next.
  const out = (v: string) => !!knownRelease && compareVersions(v, knownRelease.version) <= 0;
  const isDone = (s: (typeof roadmap)[number]) => s.status === 'done' && (!s.since || out(s.since));
  const done = roadmap.filter(isDone).map((s) => s.title);
  const next = roadmap.filter((s) => !isDone(s)).map((s) => s.title);
  const body = [
    `# ${site.name}`,
    '',
    `> ${site.description}`,
    '',
    'Telex is a desktop email client for Windows 10 and 11 (64-bit) and Linux (AppImage, .deb), written in Rust with a Tauri window (WebView2, WebKitGTK).',
    'It keeps mail in a local SQLite mail base with full-text search, syncs with IMAP servers both ways and sends over SMTP.',
    'It has no account, no telemetry and no server of its own. It is an early preview, released often.',
    `A live demo at ${url('/demo')} runs its real interface in the browser with a made-up mailbox.`,
    '',
    '## Highlights',
    '',
    ...features.map((f) => `- ${f.title}: ${f.text}`),
    '',
    `Works today: ${done.join(', ')}. Planned, in order: ${next.join(', ')}.`,
    '',
    '## Versions',
    '',
    ...versions.map((v) => `- ${v.version}${out(v.version) ? '' : ' (the next version, when this file was made)'}: ${v.title}. ${v.summary}${v.upgrade ? ` Upgrade: ${v.upgrade}` : ''}`),
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
