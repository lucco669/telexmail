import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { PageLink as Link } from '@/components/PageNavigation';
import { PageHeader } from '@/components/PageHeader';
import { pages } from '@/lib/pages';
import { pageMetadata } from '@/lib/seo';
import { site } from '@/lib/site';
import s from '../privacy/page.module.css';

export const metadata = pageMetadata(pages.license);

// The agreement as the installers show it: public/eula.txt is a copy of the repository's EULA.txt,
// kept in step by scripts/third-party-notices.mjs (checked in CI). Its title and version line
// come first, then numbered sections, each a title line and indented text.
type Section = { title: string; text: string[] };
function agreement(): { version: string; intro: string[]; sections: Section[] } {
  const lines = readFileSync(join(process.cwd(), 'public/eula.txt'), 'utf8').replace(/\r\n/g, '\n').split('\n');
  const version = lines[1].trim();
  const paragraphs = (block: string[]) => block.join('\n').split(/\n\s*\n/).map((p) => p.replace(/\s+/g, ' ').trim()).filter(Boolean);
  const sections: Section[] = [];
  const intro: string[] = [];
  let current: { title: string; lines: string[] } | null = null;
  const done = () => { if (current) sections.push({ title: current.title, text: paragraphs(current.lines) }); };
  // Lines before the first section are its introduction; an unindented line after the sections
  // (the contact address) ends them: the page says that in its own words.
  let ended = false;
  for (const line of lines.slice(2)) {
    if (ended) continue;
    const head = line.match(/^(\d+)\.\s+(.+)$/);
    if (head) { done(); current = { title: `${head[1]}. ${head[2]}`, lines: [] }; }
    else if (!current) intro.push(line);
    else if (!line.trim() || /^\s/.test(line)) current.lines.push(line);
    else { done(); current = null; ended = true; }
  }
  done();
  return { version, intro: paragraphs(intro), sections };
}

export default function LicensePage() {
  const eula = agreement();
  return (
    <>
      <PageHeader
        page={pages.license}
        title="License."
        lead="Telex is free to use, at home and at work. Its code is public to read and review; all rights are reserved."
      >
        <a className="btn" href="/eula.txt">The agreement as text</a>
        <a className="btn" href="/third-party-notices.txt">Third-party notices</a>
      </PageHeader>
      <div className={`container ${s.layout}`}>
        <aside className={s.summary}>
          <p className={s.updated}>{eula.version}</p>
          <ul>
            <li>Free for personal and business use, on any number of PCs</li>
            <li>Not open source: the code is public so anyone can check it, not to reuse it</li>
            <li>No reselling, rebranding or modified copies</li>
            <li>Future versions may come under other terms; the version you have stays usable</li>
            <li>Your mail stays on your PC: the author receives none of your data</li>
            <li>Open-source components keep their own licenses</li>
          </ul>
        </aside>
        <article className="prose">
          <h2>The Telex app</h2>
          <p>
            The installers show this agreement before Telex is installed, and put it next to the app as{' '}
            <code>EULA.txt</code>.
          </p>
          {eula.intro.map((p) => <p key={p}>{p}</p>)}
          {eula.sections.map((section) => (
            <section key={section.title}>
              <h3>{section.title}</h3>
              {section.text.map((p) => <p key={p}>{p}</p>)}
            </section>
          ))}

          <h2>The source code</h2>
          <p>
            The <a href={`https://github.com/${site.releasesRepo}`}>repository on GitHub</a> is public so that anyone can
            read how Telex handles mail and privacy, and check its security: the{' '}
            <Link href={pages.security.path}>Privacy & security</Link> page links to the code and to every automated check.
            Publishing it grants no license: copyright © 2026 Łukasz Szramuk, all rights reserved. You may read it and
            fork it on GitHub as GitHub’s terms allow, but not copy, change or redistribute it, or build on it, without
            written permission.
          </p>
          <p>Bug reports and ideas are welcome in GitHub Issues. Code contributions can’t be accepted.</p>

          <h2>Third-party components</h2>
          <p>
            Telex is built on open-source libraries and fonts: Rust crates for mail, encryption, parsing and the window,
            and the IBM Plex, Inter, JetBrains Mono, Atkinson Hyperlegible and Source families. Each keeps its own license,
            and the <a href="/third-party-notices.txt">third-party notices</a> list every one with its license text. The
            installers carry the same file as <code>THIRD-PARTY-NOTICES.txt</code>.
          </p>

          {site.contact && (
            <>
              <h2>Questions</h2>
              <p>
                Write to <a href={`mailto:${site.contact}`}>{site.contact}</a> about the license or to ask permission for
                something it doesn’t allow.
              </p>
            </>
          )}
        </article>
      </div>
    </>
  );
}
