// IndexNow (indexnow.org): tells Bing, Yandex, Seznam, Naver and the others that pages changed,
// so they crawl them now instead of on their own schedule. Google doesn't take part.
//
//   node scripts/indexnow.mjs                  every page in the live sitemap
//   node scripts/indexnow.mjs /features /faq   only these pages
//   node scripts/indexnow.mjs --dry-run        say what would be sent, send nothing
//
// Coolify runs it after each deploy (Post-deployment command; site/README.md). It reads the
// sitemap and the key file from the live site, so the new deploy must already be serving.
// A failure is reported but exits 0, so it never marks a good deploy as failed.

import { readFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';

// Public by design: the engines check that the site serves it at /<KEY>.txt (public/).
export const KEY = '89fd0334c1bccf295bfb4753a02be364';
const ENDPOINT = 'https://api.indexnow.org/indexnow';
const origin = (process.env.NEXT_PUBLIC_SITE_URL ?? 'https://telexmail.app').replace(/\/$/, '');

/** The page addresses of a sitemap, without its image entries. */
export function sitemapUrls(xml) {
  return [...xml.matchAll(/<url>\s*<loc>([^<]+)<\/loc>/g)].map((m) => m[1].trim());
}

async function get(url) {
  const res = await fetch(url, { headers: { 'User-Agent': 'Telex site IndexNow' }, signal: AbortSignal.timeout(15_000) });
  if (!res.ok) throw new Error(`${url} answered ${res.status}`);
  return res.text();
}

async function main(args) {
  const dry = args.includes('--dry-run');
  const paths = args.filter((a) => !a.startsWith('--'));
  if (paths.some((p) => !p.startsWith('/'))) {
    console.error('Pages are paths on the site, such as /features.');
    process.exit(1);
  }

  const local = (await readFile(new URL(`../public/${KEY}.txt`, import.meta.url), 'utf8')).trim();
  if (local !== KEY) throw new Error(`public/${KEY}.txt doesn't hold the key`);
  const served = (await get(`${origin}/${KEY}.txt`)).trim();
  if (served !== KEY) throw new Error(`${origin}/${KEY}.txt doesn't serve the key yet (is this deploy live?)`);

  const urlList = paths.length ? paths.map((p) => origin + p) : sitemapUrls(await get(`${origin}/sitemap.xml`));
  if (!urlList.length) throw new Error('the sitemap lists no pages');
  const body = { host: new URL(origin).host, key: KEY, keyLocation: `${origin}/${KEY}.txt`, urlList };
  if (dry) {
    console.log(`IndexNow: would send ${urlList.length} pages:\n${urlList.join('\n')}`);
    return;
  }

  const res = await fetch(ENDPOINT, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json; charset=utf-8' },
    body: JSON.stringify(body),
    signal: AbortSignal.timeout(15_000),
  });
  // 200: taken. 202: taken, the key still being checked. 403: key not valid. 422: URLs not of this
  // host. 429: too many submissions.
  if (res.status !== 200 && res.status !== 202) {
    throw new Error(`${ENDPOINT} answered ${res.status} ${(await res.text()).slice(0, 300)}`);
  }
  console.log(`IndexNow: ${urlList.length} pages sent (${res.status}).`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main(process.argv.slice(2)).catch((e) => {
    console.error(`IndexNow: nothing sent: ${e.message}`);
  });
}
