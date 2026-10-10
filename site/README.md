# Telex website

The website for Telex: Next.js 16 (App Router), plain CSS with the app's color tokens, no other
dependencies. Nine prerendered pages (home, features, keyboard, screenshots, download, roadmap,
FAQ, about, privacy), light and dark, with a download button that finds the newest installer on
its own.

```
npm install
npm run dev        # http://localhost:3000
npm run build      # what Vercel runs
npm run screens    # retake the screenshots from ../ui (needs Edge or Chrome)
npm run icons      # rebuild every app and site icon from ../brand
```

## Where things are

- `lib/pages.ts`: every page's address, menu label, title, description and link-preview line.
  The header, footer, sitemap, breadcrumbs and preview images all read it. Add a page here first.
- `lib/site.ts`: name, keywords, site URL, contact address, the GitHub repo that holds releases.
- `lib/content.ts`: features, feature rows, shortcuts and tips, screenshot captions, roadmap, FAQ,
  and the lines the telex tape punches. Keep it true to the app; update the roadmap when a step lands.
- `lib/release.ts`: reads the GitHub releases that have an `.exe` or `.msi` (checked hourly).
- `lib/seo.ts`: per-page metadata and the structured data (breadcrumbs, app, website, FAQ).
- `lib/theme.ts`: the light/dark choice. It is `data-theme` on `<html>`, saved in localStorage
  and set before the first paint by the script in `app/layout.tsx`; tokens are in `app/globals.css`.
- `app/<page>/page.tsx`: the subpages. Shared pieces are in `components/`, each with a CSS module.
- `public/screens/`: screenshots, `<scene>-light.png` and `<scene>-dark.png`, 1280×800 at 2×.
- `app/**/opengraph-image.png`: one link preview per page, made by `npm run screens`.

## The footer

`components/TelexTicker.tsx` is the live telex bar: it punches each line of `ticker` (in
`lib/content.ts`) into paper tape in real ITA2 code (`lib/ita2.ts`, with the figure/letter shift
columns) at 50 baud, the speed of a telex line, and prints it underneath. It runs only while it is
on screen, has a Pause button, and stands still for people who ask for reduced motion. Below it,
`components/DotMatrix.tsx` draws TELEX as teleprinter dots; pointing at dots lights them up.

## Logo and icons

The logo lives in `../brand`: `telex-mark.svg` for 48 px and up, `telex-mark-small.svg` (no faint
dot grid, bigger dots) for 32 px and below. `npm run icons` renders both with sharp into:

- the desktop app: `src-tauri/icons/` (PNG sizes and a 16–256 px `icon.ico`),
- the site: `app/favicon.ico`, `app/icon.svg`, `app/apple-icon.png` (full-bleed, iOS rounds it),
  `public/icon-192.png`, `public/icon-512.png`, `public/icon-maskable-512.png` (for Android).

The header mark in `components/Logo.tsx` draws the small variant inline; keep it in step.

## SEO

Each page exports `pageMetadata(pages.x)` (`lib/seo.ts`): title, description, canonical URL, Open
Graph and Twitter cards; its preview image is the `opengraph-image.png` next to it. Subpages get
a visible breadcrumb and a BreadcrumbList; the home and download pages describe the app
(SoftwareApplication), the FAQ page its questions (FAQPage). Next.js generates `robots.txt`,
`sitemap.xml` (every page, screenshots as image entries) and `manifest.webmanifest` from `app/`. With
a custom domain attached, the `*.vercel.app` address answers with `X-Robots-Tag: noindex`, so
only your domain gets indexed.

## Security, analytics and plain-text files

- `next.config.ts` sends security headers in production: a Content Security Policy (this site
  only; inline scripts allowed because static Next.js pages need them), HSTS, nosniff, DENY
  framing, a strict referrer policy, Permissions-Policy and COOP. Previews also allow Vercel's
  toolbar. If you add a third-party script or font, add its origin to the CSP there.
- Vercel Web Analytics (`<Analytics />` in `app/layout.tsx`) counts page views without cookies;
  its script comes from the site itself (`/_vercel/insights`). Turn it on in the Vercel project
  under **Analytics**, or the script answers 404. The privacy page describes it.
- `app/llms.txt/route.ts` and `app/.well-known/security.txt/route.ts` are generated from the
  same data as the pages at build time. security.txt expires a year after the build, so deploy
  at least once a year.
- Text colors are checked against WCAG AA (4.5:1). `--faint` is the lightest text allowed;
  don't make it lighter.

## Screenshots

`npm run screens` opens `../ui/index.html` in headless Edge with a stand-in for Tauri that answers
from the sample mailbox (`crates/telex-core/src/sample.json`), plays a short scene for each shot
(open a reply, type in the quick filter, …) and saves light and dark versions. The clock is fixed
at 10.10.2026 09:30 so list dates stay the same. Scenes are at the top of `scripts/screens.mjs`.
`npm run screens reply og` retakes only those; `og` makes the preview image of every page in
`lib/pages.ts` (its `ogTitle` next to its `ogScreen`).

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
| `NEXT_PUBLIC_CONTACT_EMAIL` | `admin@telexmail.app` | Overrides the contact address on the privacy page |

## Making the download button live

Until a release exists the button says "Coming soon". It switches to a direct download link as
soon as the releases repo has a published release with a Tauri installer attached
(`Telex_<version>_x64-setup.exe`, plus the `.msi` as a second link), within an hour, with no
redeploy. The repo has to be **public**: visitors can't download assets from a private repo's
releases. If `telexmail` stays private, make a public repo just for installers (for example
`telex-releases`) and set `TELEX_RELEASES_REPO` to it.
