// Moves the files a release build left in a second draft with the same tag onto the published
// release, adds their platforms to its latest.json (pointing at the copies), and removes that
// draft. 0.8.0's and 0.8.1's Linux builds finished after the draft had been published, and
// tauri-action, finding no draft for the tag, made one of its own; release.yml now uploads to the
// release by its id, so this is for releases made before that.
//
//   node scripts/repair-release.mjs [v0.8.1] [--dry-run]
//
// With no tag, every published release that has such a draft. A draft whose tag isn't published
// yet is an ordinary release in the making and is left alone. Needs GITHUB_TOKEN with
// contents: write (drafts are invisible without it) and GITHUB_REPOSITORY; run by
// .github/workflows/repair-release.yml.

const API = 'https://api.github.com';
const repo = process.env.GITHUB_REPOSITORY || 'lucco669/telexmail';
const token = process.env.GITHUB_TOKEN;
const dry = process.argv.includes('--dry-run');
const tagArg = process.argv.slice(2).find((a) => !a.startsWith('--'));

async function call(method, url, { body, type, accept = 'application/vnd.github+json' } = {}) {
  const headers = { Accept: accept, 'X-GitHub-Api-Version': '2022-11-28', 'User-Agent': 'telex-repair-release' };
  if (token) headers.Authorization = `Bearer ${token}`;
  if (type) headers['Content-Type'] = type;
  // Downloads are redirected to GitHub's file servers; fetch drops the token on the way.
  const res = await fetch(url.startsWith('https://') ? url : API + url, { method, headers, body });
  if (!res.ok) throw new Error(`${method} ${url}: ${res.status} ${await res.text()}`);
  return res;
}

const getJson = async (url) => (await call('GET', url)).json();

async function allReleases() {
  const all = [];
  for (let page = 1; ; page++) {
    const batch = await getJson(`/repos/${repo}/releases?per_page=100&page=${page}`);
    all.push(...batch);
    if (batch.length < 100) return all;
  }
}

const assetsOf = (release) => getJson(`/repos/${repo}/releases/${release.id}/assets?per_page=100`);

async function download(asset) {
  const res = await call('GET', `/repos/${repo}/releases/assets/${asset.id}`, { accept: 'application/octet-stream' });
  return Buffer.from(await res.arrayBuffer());
}

// Named like tauri-action names them: the label repeats the name.
async function upload(release, name, data) {
  const q = encodeURIComponent(name);
  const res = await call('POST', `https://uploads.github.com/repos/${repo}/releases/${release.id}/assets?name=${q}&label=${q}`, {
    body: data,
    type: 'application/octet-stream',
  });
  return res.json();
}

const assetUrl = (id) => `${API}/repos/${repo}/releases/assets/${id}`;

async function repair(published, draft) {
  const tag = published.tag_name;
  console.log(`${tag}: draft ${draft.id} (${draft.assets.map((a) => a.name).join(', ')})`);
  const there = new Map((await assetsOf(published)).map((a) => [a.name, a]));

  // The files, each draft asset's id mapped to its copy's.
  const copies = new Map();
  for (const asset of draft.assets) {
    if (asset.name.startsWith('latest.json')) continue;
    let copy = there.get(asset.name);
    if (copy) {
      if (copy.size !== asset.size) {
        throw new Error(`${tag}: ${asset.name} is on the release already, ${copy.size} bytes against the draft's ${asset.size}`);
      }
      console.log(`  ${asset.name}: on the release already`);
    } else if (dry) {
      console.log(`  ${asset.name}: would copy (${asset.size} bytes)`);
    } else {
      copy = await upload(published, asset.name, await download(asset));
      console.log(`  ${asset.name}: copied (${copy.size} bytes)`);
    }
    copies.set(asset.id, copy?.id ?? 'new');
  }

  // latest.json: the draft's platforms that the release lacks, with the copies' addresses.
  const theirsAsset = draft.assets.find((a) => a.name === 'latest.json');
  const oursAsset = there.get('latest.json');
  if (theirsAsset) {
    const theirs = JSON.parse(await download(theirsAsset));
    const ours = oursAsset ? JSON.parse(await download(oursAsset)) : { ...theirs, platforms: {} };
    if (theirs.version !== ours.version) {
      throw new Error(`${tag}: the draft's latest.json is for ${theirs.version}, the release's for ${ours.version}`);
    }
    const added = [];
    for (const [platform, entry] of Object.entries(theirs.platforms ?? {})) {
      if (ours.platforms[platform]) continue;
      const id = Number(/\/releases\/assets\/(\d+)$/.exec(entry.url)?.[1]);
      if (!copies.has(id)) throw new Error(`${tag}: ${platform} points at ${entry.url}, not one of the draft's files`);
      ours.platforms[platform] = { signature: entry.signature, url: assetUrl(copies.get(id)) };
      added.push(platform);
    }
    if (!added.length) {
      console.log('  latest.json: has every platform already');
    } else if (dry) {
      console.log(`  latest.json: would add ${added.join(', ')}`);
    } else {
      // The new file goes up under another name first, so the release is never without one
      // for longer than a rename. (One left by a run that stopped there goes first.)
      const left = there.get('latest.json.new');
      if (left) await call('DELETE', `/repos/${repo}/releases/assets/${left.id}`);
      const fresh = await upload(published, 'latest.json.new', Buffer.from(JSON.stringify(ours, null, 2)));
      if (oursAsset) await call('DELETE', `/repos/${repo}/releases/assets/${oursAsset.id}`);
      const body = JSON.stringify({ name: 'latest.json', label: 'latest.json' });
      await call('PATCH', `/repos/${repo}/releases/assets/${fresh.id}`, { body, type: 'application/json' });
      console.log(`  latest.json: added ${added.join(', ')}`);
    }
  }

  if (dry) {
    console.log(`  would remove draft ${draft.id}`);
    return;
  }
  // The draft goes only when everything in it is on the release.
  const now = new Map((await assetsOf(published)).map((a) => [a.name, a]));
  const missing = draft.assets.filter((a) => !a.name.startsWith('latest.json') && now.get(a.name)?.size !== a.size);
  if (missing.length) throw new Error(`${tag}: not on the release after copying: ${missing.map((a) => a.name).join(', ')}`);
  await call('DELETE', `/repos/${repo}/releases/${draft.id}`);
  console.log(`  removed draft ${draft.id}`);
}

if (!token) console.warn('No GITHUB_TOKEN: drafts are invisible, so there is nothing to find.');
const releases = await allReleases();
const tags = tagArg ? [tagArg] : [...new Set(releases.filter((r) => r.draft).map((r) => r.tag_name))];
let repaired = 0;
for (const tag of tags) {
  const published = releases.find((r) => r.tag_name === tag && !r.draft);
  const drafts = releases.filter((r) => r.tag_name === tag && r.draft);
  if (!published) {
    if (tagArg) throw new Error(`${tag} has no published release.`);
    continue;
  }
  if (!drafts.length && tagArg) console.log(`${tag}: no draft with this tag, nothing to do.`);
  for (const draft of drafts) {
    await repair(published, draft);
    repaired++;
  }
}
if (!tagArg && !repaired) console.log('No published release has a draft with its tag; nothing to do.');
