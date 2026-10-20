# Landing-site SEO, performance and security audit — 20 October 2026

Scope: the Next.js website in `site/`, its 16 public landing routes and their assets.
Measurements below are local production-build checks. Deployed-domain verification is separate.

## Evidence reviewed

- `telexmail.app-Coverage-Drilldown-2026-10-20.zip`: all three Search Console CSVs.
- `seomaster-telexmail-app-pelny-eksport-2026-10-20.zip`: export metadata, 53 findings,
  16 inventory URLs, content/trust/hosting evidence, 588 link records, 271 asset records,
  scan coverage and occurrence evidence, monitoring, alerts, ten repair tasks, search
  slices, link-counter reconciliation, and the three retained reports from 10–13 October.
- Current source, built HTML, production-server responses, Chrome browser behavior,
  axe checks, dependency audit and Lighthouse 13.5.0.

The newest retained report measured performance 99, accessibility 100, best practices 100
and SEO 100 on 13 October. Older open findings and repair tasks sometimes retain evidence
from 10–12 October despite newer passing measurements. Their status alone is insufficient
to establish a current defect. The export also warns that some occurrence maps are incomplete.
Search-performance slices overlap; their clicks/impressions were not added together.

## Fixes implemented

1. **Accessibility HTML exceeded the crawler limit.** The current build originally produced
   1,728,642 bytes and over 36,000 tags for `/accessibility`. All 2,170 contrast rows were sent
   in HTML and serialized server-component output, even inside closed disclosures. The page
   now sends summaries and loads one theme's measurements when opened. Its initial HTML is
   155,937 bytes, about **91% smaller**. All measurements remain available through ordinary
   JSON links without JavaScript. Fourteen JSON responses are prerendered, explicitly
   `noindex`, and omitted from the page sitemap. Loading, failure and retry states are accessible.
2. **JSON-LD escaping was ineffective.** The old replacement string evaluated to a literal
   `<`, allowing a future value containing `</script>` to escape its container. The replacement
   now emits a JSON Unicode escape. A regression passes hostile HTML through the actual
   component and verifies that it remains one script and round-trips to the original data.
3. **Canonical origin depended on hosting configuration.** Builds could publish localhost or
   a Vercel alias if domain variables were missing. The default is now `https://telexmail.app`,
   with an explicit `NEXT_PUBLIC_SITE_URL` override. Vercel aliases receive `noindex` whenever
   the canonical origin is a custom domain, including with no Vercel production-domain variable.
4. **Old screenshot URLs failed after PNG-to-WebP migration.** An internal rewrite serves
   existing WebP assets for old `/screens/*.png` requests, including requests inside the image
   optimizer. This preserves cached/indexed image URLs without duplicating image files or
   redirecting the optimizer to an empty response. Four representative legacy URLs, including
   both light and dark, were verified at the reported `w=3840&q=90` settings.
5. **Font loading and local analytics errors.** Phones below 900px use device-native fonts;
   all three final mobile runs fetched zero font files. Desktop keeps self-hosted Plex with
   optional display and no unconditional preloads. The sans family uses one variable-weight
   declaration; extended glyphs remain available on demand. Optional display avoids late
   text swaps. Analytics renders on Vercel only, so local/self-hosted production
   builds do not request a nonexistent Vercel endpoint. Actual Vercel Analytics still requires
   the hosting service to be enabled, as documented in `site/README.md`.
6. **Accessible names and link visibility.** Highlight links and screenshot buttons now include
   their visible text in their accessible names. Timeline source links are underlined rather
   than distinguished only by color. Theme names/notes and the accessibility snippet now
   match fourteen themes and nine languages.
7. **CSP hardening.** `script-src-attr 'none'` blocks injected inline event handlers. Existing
   static-page caching, same-origin resources, HSTS, nosniff, framing restrictions, referrer
   policy and permissions policy remain enforced. Per-request nonces were not introduced:
   Next.js requires dynamic rendering for those, while these pages are prerendered.
8. **Build guard.** `npm run check:seo` checks every public route's unique metadata, one H1,
   indexability, self canonical, sitemap membership, parseable JSON-LD, original image files
   and the audit crawler's 600,000-byte HTML budget.
9. **Mobile loading.** Responsive, fingerprinted AVIF/WebP hero derivatives remove the cold
   image-transform request. At 800px, the light/dark AVIFs are about 20 KB each; original
   gallery quality is unchanged. Prebuild checks the screenshot hashes and all derivatives.
   Only the matching theme/format downloads. The live demo module and iframe start on click.
10. **Less initial work.** Client components receive only their required content rather than
    importing complete page registries and screenshot collections. Links prefetch on hover,
    focus or touch; full-page prefetching replaces partial prefetching for these static routes.
    Offscreen home sections and the footer skip layout/paint with `content-visibility: auto`,
    while their content remains in HTML, searchable, accessible and printable. SVG patterns
    and paths replace hundreds of dot nodes. The tape builds its full program only when seen
    and pauses offscreen. Expensive paragraph wrapping is limited to long-form prose.
11. **Smaller blocking CSS.** Next 16.4's documented experimental graph chunker cuts the home's
    initial CSS from 66,658 to 44,144 uncompressed bytes (about 34%). All 16 routes were checked
    after changing it. Keep the framework pinned and repeat the styling/loading checks when
    upgrading. Global CSS inlining was rejected because it increased HTML without improving
    performance. Mobile typography and the shorter hero introduction were visually checked
    at 320px and 390px; desktop retains the two-column hero.

## Disposition of the exported open findings

| Finding | Evidence and disposition |
| --- | --- |
| Missing H1: one URL | The affected URL is `/demo-app/index.html?theme=light`, a deliberately unindexed app frame. All 16 public landing routes have one H1. Keep the frame excluded. |
| SoftwareApplication review/rating missing: two pages | Real reviews were not supplied. Keep truthful app metadata and the zero-price offer. Google requires a real rating/review for this particular rich result; ordinary indexing does not require eligibility for it. Do not fabricate reviews. |
| Broken images: 25 | Historical optimizer requests target `.png` files. Current markup uses existing WebP files; compatibility rewrites also restore old paths. The build verifies 112 referenced images, including 20 new hero derivatives. |
| Render console/resource errors | The 12 October sample lost its title, H1 and almost all content; the 13 October sample had no differences or errors. Current local browser tests find no errors and identical source/rendered SEO on all 16 routes. Live edge access remains a separate issue below. |
| Noindex / links to noindex | Intentional demo-frame exclusion. New contrast JSON evidence is also deliberately excluded. All sitemap page URLs remain indexable. |
| Server fingerprint | `X-Powered-By` is absent in current production responses. Vercel's own infrastructure headers are controlled by Vercel; no inaccurate proxy header masking was added. |
| JavaScript minification | The historical test sampled two unspecified resources. Production Next.js landing bundles are minified. The separately loaded demo contains the desktop UI's original scripts; its bundling is outside these landing-bundle changes. |
| Content/trust inventory | Informational collection result, not a failing test. Contact, legal pages, organization data and CTAs already exist. |
| External links reported broken | Recorded failures are access denials/rate limits, not confirmed 404s. Both museum pages, Vodafone, Signal, Slack and the Smithsonian reference were independently found at their publishers. Preserve these primary references. |
| External redirects: 47 | The samples are GitHub release downloads. Keep GitHub's stable URLs; their signed redirect destinations expire. |
| Partial inventory: one URL | The crawler stopped reading `/accessibility` at 600,000 bytes. Fixed by reducing initial HTML; the largest current public page is `/features` at 295,192 bytes. |

Google's requirements for software-app rich results are documented in its
[SoftwareApplication guide](https://developers.google.com/search/docs/appearance/structured-data/software-app).
The referenced historical sources remain available, including the
[Baudot museum entry](https://collection.sciencemuseumgroup.org.uk/objects/co33386/baudot-type-printing-multiplex-telegraph-complete-with-keyboard-sender),
[Vodafone article](https://www.vodafone.com/news/newsroom/technology/25-anniversary-text-message),
[Signal support page](https://support.signal.org/hc/en-us/articles/360007320391-Is-it-private-Can-I-trust-it),
[Slack help page](https://slack.com/help/articles/115000769927-Use-threads-to-organize-discussions)
and [Smithsonian collection](https://www.si.edu/spotlight/electricityandmagnetism).

## Verification

- `npm run build`: passes; all 16 public pages and 14 evidence responses are prerendered.
- `npm run typecheck`: passes.
- `node --test scripts/seo.test.mjs scripts/count.test.mjs scripts/night-shift.test.mjs`:
  **29 passing tests**.
- `npm run check:seo`: all 16 pages pass; 112 referenced images exist;
  every page is below 600 KB.
- `npm audit --json`: **zero reported vulnerabilities**, including development dependencies.
- Chrome/Playwright: all 16 routes return 200, preserve source/rendered SEO, and load without
  console errors or failed resources. Checked 55 image responses, 34 internal targets and
  their page anchors, production security headers, primary-domain indexability and alias noindex.
- axe WCAG A/AA structural checks pass on all landing pages. Additional contrast and visible-name
  checks pass in light/dark on home, accessibility, screenshots and demo. The embedded desktop
  app frame was excluded from landing accessibility assertions; its splitters and compact hit
  targets still have app-specific accessibility findings. No full app compliance claim is made.
- Five mobile layouts have no horizontal overflow and were visually inspected. The evidence
  disclosure works without JavaScript, loads 155 rows only after opening, reuses loaded data,
  recovers after a failed request, and rejects unknown theme IDs with 404/noindex.
- Screenshot dialog opening, next-image navigation and Escape work. A browser injection check
  confirms that the CSP prevents an inline event handler from executing.
- Additional interaction checks pass: saved color choice, mobile popover navigation, home
  shortcut highlighting/counts, screenshot tabs/arrow keys/theme, hash targets through skipped
  sections, navigation scroll reset/back, desktop demo activation/theme updates, the mobile demo
  link, tape animation/pause/play/offscreen pause, and 320px/390px layouts. A mobile cold browser
  fetches one theme-matching AVIF and no font files. Desktop, mobile and footer screenshots were
  inspected after the performance changes.

Lighthouse 13.5.0, Chrome 155, cold browser storage. Final mobile runs use the unmodified
default mobile simulation (412×823, DPR 1.75, 4× CPU slowdown, 150ms RTT and 1,638.4 kbps).
No resources, analytics endpoints or audits were blocked by the runner. Vercel Analytics is
absent from non-Vercel builds by the hosting condition above, so its live cost needs a deployed
measurement. No mobile-specific user-agent behavior was introduced.

The production build was measured over a loopback HTTPS HTTP/2 proxy with streaming Brotli
quality 5, matching the transport types used by Vercel. It forwards the same Next response
body/status/security headers without caching or content changes. The audit Chrome alone ignored
the temporary local certificate; no certificate was trusted on the computer. This removes the
HTTP/1.1/gzip development-server transport mismatch; it does not emulate Vercel latency, cache,
firewall or production analytics. Direct `next start` was also measured for comparison.

| Configuration | Performance | Accessibility | Best practices | SEO | LCP | TBT | CLS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Original, mobile, HTTP/1.1 + gzip | 91 | 100 | 96 | 100 | 3.4 s | 20 ms | 0.002 |
| Final, mobile run 1, HTTP/2 + Brotli | 100 | 100 | 100 | 100 | 1.9 s | 40 ms | 0 |
| Final, mobile run 2, HTTP/2 + Brotli | 100 | 100 | 100 | 100 | 1.9 s | 30 ms | 0 |
| Final, mobile run 3, HTTP/2 + Brotli | 100 | 100 | 100 | 100 | 1.9 s | 40 ms | 0 |
| Final, desktop, HTTP/2 + Brotli | 100 | 100 | 100 | 100 | 0.5 s | 0 ms | 0 |
| Final, mobile, direct HTTP/1.1 + gzip | 98 | 100 | 100 | 100 | 2.5 s | 40 ms | 0 |

Final HTTP/2 runs transfer about 199 KB in 16 requests. Mobile FCP and Speed Index are about
1.0s, with zero CLS. Machine-readable results, exact settings and source-report hashes are in
[`site-lighthouse-2026-10-20.json`](site-lighthouse-2026-10-20.json). Raw local reports remain in
`C:/Temp/telexmail-seo-audit-tools/`.

These are lab measurements, not field Core Web Vitals or a guarantee that every future run will
score 100. Shared computer load caused material variability during tuning. Earlier actual DevTools
throttling checks are a different experiment; they are not substituted for these standard final
runs. Recheck PageSpeed on Vercel after deployment. Vercel's compression behavior is described in
[Next's hosting documentation](https://nextjs.org/docs/14/app/api-reference/next-config-js/compress).

## Remaining hosting and search steps

1. **Verify deployment of the landing commit.** Check the commit's Vercel status and deployed
   assets before treating local measurements as production results.
2. **Resolve audit access at Vercel's edge.** On 20 October, direct requests to the live home,
   robots and sitemap endpoints received HTTP 429 with `X-Vercel-Mitigated: challenge`, before
   the application responded. This is not proof that verified Googlebot is blocked. Inspect
   Firewall events during a SEOMaster crawl and renderer test. Add a narrowly scoped rule for
   confirmed scanner/renderer outbound IPs/CIDRs, the exact hostname, and GET/HEAD only. Those
   outbound ranges are absent from the supplied export. Follow the existing detailed workflow
   in `site/README.md` and [Vercel's custom-rule documentation](https://vercel.com/docs/vercel-firewall/vercel-waf/custom-rules).
   Do not authenticate a scanner merely by a public header/user-agent or disable protection globally.
3. **Run Search Console live inspection** for `/about`, `/demo` and `/faq`, then request indexing
   after confirming Google receives the real 200 page and submit/refresh the sitemap. The coverage
   export says “Discovered — currently not indexed”; `1970-01-01` does not establish a real crawl
   on that date. All three URLs already have crawlable internal links, complete server HTML and
   sitemap entries. Google controls crawl scheduling and indexing; code cannot force either.
   See [Google's Page indexing report guidance](https://support.google.com/webmasters/answer/7440203).
4. **Rerun SEOMaster and PageSpeed on the deployed domain**, with crawler access established,
   and compare current evidence rather than keeping stale findings open. Monitor real mobile
   LCP/INP/CLS as traffic becomes sufficient; the export's 30 impressions and zero clicks are
   not enough evidence to promise rankings, conversions or field-performance improvements.
