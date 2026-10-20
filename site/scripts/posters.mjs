// Precompute the first-paint screenshot so a cold visit never waits for image transformation.
// Full-size screenshots keep using Next's image optimizer at their existing quality.
import { access, mkdir, readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import sharp from 'sharp';

const root = new URL('../', import.meta.url);
const out = new URL('public/posters/', root);
await mkdir(out, { recursive: true });
const inputs = Object.fromEntries(await Promise.all(['light', 'dark'].map(async (theme) => [theme, await readFile(new URL(`public/screens/inbox-${theme}.webp`, root))])));
const sourceHashes = Object.fromEntries(Object.entries(inputs).map(([theme, input]) => [theme, createHash('sha256').update(input).digest('hex')]));
const manifest = new URL('lib/hero-posters.json', root);
try {
  const existing = JSON.parse(await readFile(manifest, 'utf8'));
  if (existing.version === 2 && JSON.stringify(existing.sourceHashes) === JSON.stringify(sourceHashes)) {
    await Promise.all(['light', 'dark'].flatMap((theme) => existing[theme].flatMap(({ src, avifSrc }) => [src, avifSrc].map((path) => access(new URL(`public${path}`, root))))));
    console.log('Responsive hero posters are up to date.');
    process.exit(0);
  }
} catch { /* New source or missing derivatives: rebuild before Next compiles. */ }
const posters = { version: 2, id: 'inbox', sourceHashes };
for (const theme of ['light', 'dark']) {
  const input = inputs[theme];
  posters[theme] = [];
  for (const width of [480, 800, 1280, 1920, 2560]) {
    const buffer = await sharp(input).resize({ width, withoutEnlargement: true }).webp({ quality: 70, effort: 6 }).toBuffer();
    const hash = createHash('sha256').update(buffer).digest('hex').slice(0, 12);
    const file = `inbox-${theme}-${width}.${hash}.webp`;
    await writeFile(new URL(file, out), buffer);
    const avif = await sharp(input).resize({ width, withoutEnlargement: true }).avif({ quality: 50, effort: 5 }).toBuffer();
    const avifHash = createHash('sha256').update(avif).digest('hex').slice(0, 12);
    const avifFile = `inbox-${theme}-${width}.${avifHash}.avif`;
    await writeFile(new URL(avifFile, out), avif);
    posters[theme].push({ width, src: `/posters/${file}`, avifSrc: `/posters/${avifFile}` });
  }
}
await writeFile(manifest, `${JSON.stringify(posters, null, 2)}\n`);
console.log('Built responsive AVIF/WebP hero posters (480–2560 px).');
