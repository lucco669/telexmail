import assert from 'node:assert/strict';
import { readFile, access } from 'node:fs/promises';
import ts from 'typescript';

const root = new URL('../', import.meta.url);
const source = await readFile(new URL('lib/pages.ts', root), 'utf8');
const { outputText } = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext } });
const { pages } = await import(`data:text/javascript;base64,${Buffer.from(outputText).toString('base64')}`);
const sitemap = await readFile(new URL('.next/server/app/sitemap.xml.body', root), 'utf8');
const attributes = (tag) => Object.fromEntries([...tag.matchAll(/([\w:-]+)="([^"]*)"/g)].map((m) => [m[1], m[2]]));
const titles = new Set();
const descriptions = new Set();
let origin;
let largest = { path: '', bytes: 0 };
const images = new Set();

for (const p of Object.values(pages)) {
  const name = p.path === '/' ? 'index' : p.path.slice(1);
  const html = await readFile(new URL(`.next/server/app/${name}.html`, root), 'utf8');
  const bytes = Buffer.byteLength(html);
  assert.ok(bytes < 600000, `${p.path}: ${bytes} HTML bytes exceed the audit crawler's 600 KB limit`);
  if (bytes > largest.bytes) largest = { path: p.path, bytes };
  assert.equal([...html.matchAll(/<h1(?:\s|>)/g)].length, 1, `${p.path}: one H1`);
  const title = [...html.matchAll(/<title>(.*?)<\/title>/g)];
  assert.equal(title.length, 1, `${p.path}: one title`);
  assert.ok(title[0][1] && !titles.has(title[0][1]), `${p.path}: unique nonempty title`);
  titles.add(title[0][1]);
  const tags = [...html.matchAll(/<(?:meta|link)\b[^>]*>/g)].map(([tag]) => attributes(tag));
  const desc = tags.filter((tag) => tag.name === 'description');
  assert.equal(desc.length, 1, `${p.path}: one description`);
  assert.ok(desc[0].content && !descriptions.has(desc[0].content), `${p.path}: unique description`);
  descriptions.add(desc[0].content);
  assert.ok(!tags.some((tag) => ['robots', 'googlebot'].includes(tag.name) && /\bnoindex\b/i.test(tag.content)), `${p.path}: indexable`);
  const canonical = tags.filter((tag) => tag.rel === 'canonical');
  assert.equal(canonical.length, 1, `${p.path}: one canonical`);
  const url = new URL(canonical[0].href);
  origin ??= url.origin;
  assert.equal(url.origin, origin, `${p.path}: consistent origin`);
  assert.equal(url.pathname, p.path, `${p.path}: self canonical`);
  assert.ok(sitemap.includes(`<loc>${url.href}</loc>`), `${p.path}: in sitemap`);
  const jsonLd = [...html.matchAll(/<script type="application\/ld\+json">(.*?)<\/script>/gs)];
  assert.ok(jsonLd.length, `${p.path}: structured data`);
  for (const [, json] of jsonLd) JSON.parse(json);
  for (const [tag] of html.matchAll(/<(?:img|source)\b[^>]*>/g)) {
    const attr = attributes(tag);
    const urls = [attr.src, ...(attr.srcSet ?? attr.srcset ?? '').split(',').map((part) => part.trim().split(' ')[0])].filter(Boolean);
    for (const src of urls) {
      const image = new URL(src.replaceAll('&amp;', '&'), origin);
      const path = image.pathname === '/_next/image' ? image.searchParams.get('url') : image.pathname;
      if (['/screens/', '/history/', '/posters/'].some((prefix) => path?.startsWith(prefix))) images.add(path);
    }
  }
}

for (const path of images) await access(new URL(`public${path}`, root));
console.log(`${titles.size} public pages: unique metadata, one H1, self canonicals, sitemap, indexability and JSON-LD verified.`);
console.log(`${images.size} original images exist. Largest HTML: ${largest.path}, ${largest.bytes} bytes (limit 600000).`);
