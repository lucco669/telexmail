# Telex website

The landing page for Telex: Next.js 16 (App Router), plain CSS with the app's color tokens, no
other dependencies. One page, prerendered, with a download button that finds the newest installer
on its own.

```
npm install
npm run dev        # http://localhost:3000
npm run build      # what Vercel runs
npm run screens    # retake the screenshots from ../ui (needs Edge or Chrome)
```

## Where things are

- `lib/site.ts`: name, title, description, site URL, the GitHub repo that holds releases.
- `lib/content.ts`: features, keyboard table, screenshot captions, roadmap, FAQ. Update the roadmap
  here when a step lands.
- `lib/release.ts`: reads the newest GitHub release with an `.exe` or `.msi` (checked hourly).
- `components/`: one file per section, each with its CSS module. Tokens are in `app/globals.css`.
- `public/screens/`: screenshots, `<scene>-light.png` and `<scene>-dark.png`, 1280×800 at 2×.
- `app/opengraph-image.png`: the link preview, also made by `npm run screens`.

## Screenshots

`npm run screens` opens `../ui/index.html` in headless Edge with a stand-in for Tauri that answers
from the sample mailbox (`crates/telex-core/src/sample.json`), plays a short scene for each shot
(open a reply, type in the quick filter, …) and saves light and dark versions. The clock is fixed
at 10.10.2026 09:30 so list dates stay the same. Scenes are at the top of `scripts/screens.mjs`.
`npm run screens reply og` retakes only those.

## Deploying on Vercel

1. Import the GitHub repo in Vercel and set **Root Directory** to `site`. Everything else is
   detected (Next.js, `npm install`, `npm run build`).
2. Add the domain under **Settings → Domains**. The site picks up the production domain on its own
   (`VERCEL_PROJECT_PRODUCTION_URL`) for canonical links, the sitemap and link previews.
3. Pushes to `main` that don't touch `site/` are skipped (`vercel.json`, `ignoreCommand`), so app
   commits don't rebuild the website.

Optional environment variables:

| Variable               | Default               | Use                                           |
|------------------------|-----------------------|-----------------------------------------------|
| `NEXT_PUBLIC_SITE_URL` | Vercel's production URL | Force the canonical URL                     |
| `TELEX_RELEASES_REPO`  | `lucco669/telexmail`  | Repo whose GitHub Releases hold the installers |
| `GITHUB_TOKEN`         | –                     | Only raises GitHub's API rate limit           |

## Making the download button live

Until a release exists the button says "Coming soon". It switches to a direct download link as
soon as the releases repo has a published release with a Tauri installer attached
(`Telex_<version>_x64-setup.exe`, plus the `.msi` as a second link), within an hour, with no
redeploy. The repo has to be **public**: visitors can't download assets from a private repo's
releases. If `telexmail` stays private, make a public repo just for installers (for example
`telex-releases`) and set `TELEX_RELEASES_REPO` to it.
