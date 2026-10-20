import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { pathToFileURL } from 'node:url';
import { renderToStaticMarkup } from 'react-dom/server';
import ts from 'typescript';

const require = createRequire(import.meta.url);
let loaded = 0;
async function load(path) {
  const source = await readFile(new URL(path, import.meta.url), 'utf8');
  const { outputText } = ts.transpileModule(source, {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext, jsx: ts.JsxEmit.ReactJSX },
  });
  const code = outputText.replace(/from ["']react\/jsx-runtime["']/g, `from ${JSON.stringify(pathToFileURL(require.resolve('react/jsx-runtime')).href)}`);
  return import(`data:text/javascript;base64,${Buffer.from(code).toString('base64')}#${loaded++}`);
}

test('JSON-LD cannot close its script element, even with HTML in a release or FAQ value', async () => {
  const { JsonLd } = await load('../components/JsonLd.tsx');
  const payload = { '@type': 'Thing', name: '</script><script>alert("xss")</script>', description: '<!-- <tag> & Unicode: Łódź' };
  const html = renderToStaticMarkup(JsonLd({ items: [payload] }));
  assert.equal((html.match(/<script/g) ?? []).length, 1);
  assert.equal((html.match(/<\/script>/g) ?? []).length, 1);
  const json = html.slice(html.indexOf('>') + 1, html.lastIndexOf('</script>'));
  assert.ok(!json.includes('<'));
  assert.deepEqual(JSON.parse(json), { '@context': 'https://schema.org', '@graph': [payload] });
});

test('canonical URLs stay on the public domain without deployment-specific configuration', async () => {
  const keys = ['NEXT_PUBLIC_SITE_URL', 'VERCEL_PROJECT_PRODUCTION_URL'];
  const previous = keys.map((key) => process.env[key]);
  try {
    delete process.env.NEXT_PUBLIC_SITE_URL;
    delete process.env.VERCEL_PROJECT_PRODUCTION_URL;
    assert.equal((await load('../lib/site.ts')).site.url, 'https://telexmail.app');
    process.env.VERCEL_PROJECT_PRODUCTION_URL = 'telex-preview.vercel.app';
    assert.equal((await load('../lib/site.ts')).site.url, 'https://telexmail.app');
    process.env.NEXT_PUBLIC_SITE_URL = 'https://example.com/';
    assert.equal((await load('../lib/site.ts')).site.url, 'https://example.com');
  } finally {
    keys.forEach((key, i) => { if (previous[i] === undefined) delete process.env[key]; else process.env[key] = previous[i]; });
  }
});
