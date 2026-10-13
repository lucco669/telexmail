// Every app and site icon, made from the two logo files in ../brand:
//   telex-mark.svg        the full mark, used from 48 px up
//   telex-mark-small.svg  simplified for 32 px and below, where the faint dots turn to noise
// Run after changing the logo:  npm run icons
// Uses sharp, which Next.js already installs.

import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import sharp from 'sharp';

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, '../..');
const site = resolve(here, '..');
const full = readFileSync(join(repo, 'brand/telex-mark.svg'));
const small = readFileSync(join(repo, 'brand/telex-mark-small.svg'));
const BRAND = '#2453a6';

const svgFor = (size) => (size <= 32 ? small : full);

// Rasterize the 64-unit SVG straight at the target size, so edges stay sharp.
const png = (size, svg = svgFor(size)) =>
  sharp(svg, { density: (72 * size) / 64 }).resize(size, size).png({ compressionLevel: 9 }).toBuffer();

// Square without transparent corners (iOS and Android cut their own shape). `scale` < 1 keeps the
// mark inside the safe zone of maskable icons.
async function fullBleed(size, scale = 1) {
  const inner = Math.round(size * scale);
  const mark = await png(inner, full);
  const at = Math.round((size - inner) / 2);
  return sharp({ create: { width: size, height: size, channels: 4, background: BRAND } })
    .composite([{ input: mark, left: at, top: at }])
    .png({ compressionLevel: 9 })
    .toBuffer();
}

// .ico with PNG-compressed entries (Windows Vista and later, and every browser).
async function ico(sizes) {
  const images = await Promise.all(sizes.map((s) => png(s)));
  const header = Buffer.alloc(6 + 16 * sizes.length);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(sizes.length, 4);
  let offset = header.length;
  sizes.forEach((s, i) => {
    const e = 6 + 16 * i;
    header.writeUInt8(s >= 256 ? 0 : s, e);
    header.writeUInt8(s >= 256 ? 0 : s, e + 1);
    header.writeUInt16LE(1, e + 4); // planes
    header.writeUInt16LE(32, e + 6); // bits per pixel
    header.writeUInt32LE(images[i].length, e + 8);
    header.writeUInt32LE(offset, e + 12);
    offset += images[i].length;
  });
  return Buffer.concat([header, ...images]);
}

const out = [
  // Desktop app: the files tauri.conf.json lists under bundle.icon.
  ['src-tauri/icons/32x32.png', () => png(32)],
  ['src-tauri/icons/128x128.png', () => png(128)],
  ['src-tauri/icons/128x128@2x.png', () => png(256)],
  ['src-tauri/icons/icon.png', () => png(512)],
  ['src-tauri/icons/icon.ico', () => ico([16, 24, 32, 48, 64, 128, 256])],
  // Website
  ['site/app/favicon.ico', () => ico([16, 32, 48])],
  ['site/app/apple-icon.png', () => fullBleed(180)],
  ['site/public/icon-192.png', () => png(192)],
  ['site/public/icon-512.png', () => png(512)],
  ['site/public/icon-maskable-512.png', () => fullBleed(512, 0.8)],
];

for (const [file, make] of out) {
  writeFileSync(join(repo, file), await make());
  console.log(file);
}
// Browsers that take SVG favicons draw them at 16–32 px, so they get the small mark.
copyFileSync(join(repo, 'brand/telex-mark-small.svg'), join(site, 'app/icon.svg'));
console.log('site/app/icon.svg');
copyFileSync(join(repo, 'brand/telex-mark.svg'), join(repo, 'ui/telex-mark.svg'));
console.log('ui/telex-mark.svg');

// Native installer artwork uses the app's light palette and the same mark. Windows
// installer engines expect uncompressed 24-bit BMPs, which sharp doesn't encode.
function bmp(width, height, rgb) {
  const stride = (width * 3 + 3) & ~3;
  const bytes = Buffer.alloc(54 + stride * height);
  bytes.write('BM');
  bytes.writeUInt32LE(bytes.length, 2);
  bytes.writeUInt32LE(54, 10);
  bytes.writeUInt32LE(40, 14);
  bytes.writeInt32LE(width, 18);
  bytes.writeInt32LE(height, 22);
  bytes.writeUInt16LE(1, 26);
  bytes.writeUInt16LE(24, 28);
  bytes.writeUInt32LE(stride * height, 34);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const from = (y * width + x) * 3, to = 54 + (height - y - 1) * stride + x * 3;
      bytes[to] = rgb[from + 2];
      bytes[to + 1] = rgb[from + 1];
      bytes[to + 2] = rgb[from];
    }
  }
  return bytes;
}

async function installerArt(width, height, { sidebar = false, wix = false } = {}) {
  // NSIS scales bitmaps to the dialog's physical pixels. Render at 3x so normal
  // Windows display scaling downsamples the mark instead of enlarging coarse pixels.
  const scale = wix ? 1 : 3;
  const size = sidebar ? 72 : 40;
  const left = sidebar ? Math.round((164 - size) / 2) : width - size - 20;
  const top = sidebar ? 38 : Math.round((height - size) / 2);
  const art = `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}">
    <rect width="${width}" height="${height}" fill="#ffffff"/>
    ${sidebar ? `<rect width="164" height="${height}" fill="#f3f5f8"/>
      <rect x="163" width="1" height="${height}" fill="#d2d7de"/>
      <rect y="${height - 4}" width="164" height="4" fill="${BRAND}"/>` : ''}
  </svg>`;
  const rgb = await sharp(Buffer.from(art), { density: 72 * scale })
    .composite([{ input: await png(size * scale), left: left * scale, top: top * scale }])
    .removeAlpha().raw().toBuffer();
  return bmp(width * scale, height * scale, rgb);
}

mkdirSync(join(repo, 'src-tauri/installer'), { recursive: true });
for (const [name, width, height, options] of [
  ['header', 150, 57, {}],
  // Segoe UI 9's welcome image control is 109 x 193 dialog units. Its physical
  // aspect ratio is about 164:364; matching it keeps the square mark square.
  ['sidebar', 164, 364, { sidebar: true }],
  ['wix-banner', 493, 58, { wix: true }],
  ['wix-dialog', 493, 312, { sidebar: true, wix: true }],
]) {
  const file = `src-tauri/installer/${name}.bmp`;
  writeFileSync(join(repo, file), await installerArt(width, height, options));
  console.log(file);
}
