import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { KEY, sitemapUrls } from './indexnow.mjs';

test('the IndexNow key file in public/ holds the key the script sends', async () => {
  assert.match(KEY, /^[0-9a-f]{32}$/);
  assert.equal(await readFile(new URL(`../public/${KEY}.txt`, import.meta.url), 'utf8'), KEY);
});

test('IndexNow sends the sitemap pages, not their image entries', () => {
  const xml = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:image="http://www.google.com/schemas/sitemap-image/1.1">
<url>
<loc>https://telexmail.app/</loc>
<image:image><image:loc>https://telexmail.app/screens/inbox-light.webp</image:loc></image:image>
</url>
<url>
<loc>https://telexmail.app/features</loc>
</url>
</urlset>`;
  assert.deepEqual(sitemapUrls(xml), ['https://telexmail.app/', 'https://telexmail.app/features']);
});
