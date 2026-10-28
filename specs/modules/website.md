# Module spec: website (WEB)

Owner: `subprojects/website/` (landing page, manual, build script) and
`.github/workflows/website.yml`.
Status: **WEB-01..34 approved by owner 2026-10-27** (P1-21; answers under *Owner decisions*). Takes up IDEA-20
(the place to download CMDMX). Needs the rename (`brand.md`) first. Open questions at the end.
Builds on `release.md` (REL-06/09: the DMG on a `v*` tag, REL-15: install steps) and
`brand.md` (BRD-21..23 assets).

## Purpose
One public page where anyone finds out what CMDMX is, downloads the **latest release** without
having to know GitHub, and reads a **full user manual**. Static, free to host, rebuilt by CI, with
nothing to run or patch on a server.

## Definitions
- **Latest release:** the newest GitHub release of the repo that is not a draft or a pre-release
  (`gh release view` without a tag), with its `CMDMX-<version>.dmg` asset (BRD-04).
- **Site:** the built output: the landing page at `/`, the manual at `/manual/`.
- **Manual:** the user manual as Markdown in `subprojects/website/manual/src/`, built with mdBook.

## Acceptance criteria

### Layout and tools
- **WEB-01** `subprojects/website/` holds: `index.html` (the landing page template), `style.css`,
  `brand/` (BRD-21..23), `manual/` (`book.toml`, `src/SUMMARY.md`, chapters), `build.sh` and a
  `README.md` (how to build and preview locally). No Node, no npm, no framework: plain HTML and CSS,
  plus **mdBook** (a single Rust binary, pinned version) for the manual. `build.sh` writes the site to
  `subprojects/website/_site/` (git-ignored).
- **WEB-02** The site loads nothing from other hosts: fonts, icons and scripts are in the site;
  no analytics, no cookies, no trackers, so it needs no cookie banner. The only outbound links are
  to GitHub (release, repo, issues).
- **WEB-03** The landing page works without JavaScript, at phone width (no horizontal scroll from
  320 px up), in dark and light (`prefers-color-scheme`), with real headings, alt text on every
  image and a visible keyboard focus.

### Landing page
- **WEB-10** The page shows, top to bottom: the lockup (BRD-21); one sentence on what CMDMX is (a
  lighting console for macOS that works the way Titan operators expect: a descriptive mention, no
  Avolites logos or look-alike branding, IDEA-21); the **download block** (WEB-11); two or three
  screenshots from `docs/screenshots/` with captions; what works today (a short list kept in step
  with the README's *What works today* groups); the install steps (WEB-12); links to the manual,
  the repo and the issue tracker; a footer with `CMDMX by Colourmotion`, the licence line (WEB-14)
  and the year.
- **WEB-11** The download block shows the latest release's version, date, DMG size, "macOS 14 or
  later, Apple Silicon and Intel" (REL-04), and the SHA-256 of the DMG; its button links straight
  to the DMG asset's download URL, and a "Release notes" link goes to the release page. When there
  is no latest release, the block says "No release yet" and links to the repo instead; the build
  does not fail.
- **WEB-12** The install steps are REL-15's (drag to Applications; first open right-click › Open,
  or *Open Anyway* on macOS 15+; the Local Network and firewall prompts), as long as the DMG is
  not notarised. When notarisation lands (VS8-08), they shrink to "drag to Applications".
  Per REL-22 as amended by VS8-09, the install steps stay the not-notarised ones until the first
  notarised tag (VS8-10), whose PR shrinks them to drag to Applications.
- **WEB-13** Only `build.sh` fills the version: it reads the latest release with
  `gh release view --json tagName,publishedAt,assets,url` and replaces the `{{…}}` placeholders
  in `index.html`. It fails when any `{{` is left in the output. Nothing on the page is fetched
  at view time.
- **WEB-14** The licence line says what the `LICENSE` file says today and links to it. No price,
  no purchase page and no "free forever" promise until IDEA-21 is decided; a pricing section is
  its own criterion then.

### Hosting and deploy
- **WEB-20** The site is served by **GitHub Pages** from the Actions artifact (no `gh-pages`
  branch): `website.yml` builds with `build.sh`, uploads `_site/` with
  `actions/upload-pages-artifact` and deploys with `actions/deploy-pages`. Permissions only
  `contents: read`, `pages: write`, `id-token: write`. The site has its own
  domain from the start (owner decision 2): a `CNAME` file in the site, the domain in the Pages
  settings and the DNS records at the registrar (owner), HTTPS enforced;
  `https://aottens.github.io/CMDMX/` redirects to it.
- **WEB-21** The site is rebuilt and deployed: on a push to `main` that touches
  `subprojects/website/**`; on `workflow_dispatch`; and **after every release**. Because a release
  made with `GITHUB_TOKEN` does not start other workflows, `website.yml` is also a reusable workflow
  (`workflow_call`) and `release.yml` calls it as a job that runs after the release job succeeds.
  So the download button shows a new version minutes after its tag, with no manual step.
- **WEB-22** PR CI builds the site (`build.sh` without deploying, with "No release yet" allowed)
  when a PR touches `subprojects/website/**`, so a broken manual or page fails the PR.
- **WEB-23** The manual on the site is the manual **of the latest release**: `website.yml` checks
  out the latest release tag for `subprojects/website/manual/` and `main` for the rest. If that tag
  has no manual yet, the manual is built from `main` with a banner on every page: "Preview: written
  ahead of the next release".

### The manual
- **WEB-30** The manual is mdBook with its built-in search, sidebar, print page (one page for the
  whole manual, so it can be saved as PDF) and dark/light themes; its header carries the mark
  (BRD-21) and a link back to the landing page. It states on its front page which version it is
  for (filled by `build.sh` like WEB-13).
- **WEB-31** Structure (`SUMMARY.md`), one Markdown file per section:
  1. **Introduction**: what CMDMX is, the building-blocks idea (program once, play back as cue
     lists, pages or later a timeline), what it does not do yet.
  2. **Getting started**: install (WEB-12), first launch, the engine and the app (the engine runs
     on when the app crashes), connect output (Art-Net, sACN, USB DMX with `--serial` for the
     Eurolite), your first show in ten minutes.
  3. **Concepts**: fixtures and patch, the programmer, attribute groups (I P C G B E S FX), HTP and
     LTP, priorities, palettes as references, playbacks and cue lists, pages.
  4. **Patching**: the fixture library (QLC+ set, GDTF import, user profiles), the profile editor,
     patching and multi-select, universes and output nodes.
  5. **Programming**: selecting (order, Fix, patterns), wheels, the colour picker (RGB/CMY/HSI),
     Fan, Locate and Clear, groups.
  6. **Palettes**: kinds (Global/Shared/Normal), recording and updating, quick palettes, fade and
     overlap.
  7. **Playbacks and cues**: recording, fader modes, release, chases (BPM, tap), cells.
  8. **Cue lists**: Go/Back, times per cue and per attribute group, link and follow, tracking modes.
  9. **Running a show**: pages, Grand Master and DBO, group masters, the BPM master, undo, what
     happens when something fails (output holds, autosave, the fallback).
  10. **Control surfaces**: MIDI Learn and mappings, Mackie Control (BCF2000, X-Touch), OSC and
      the TouchOSC layout, Stream Deck.
  11. **Shows and files**: where shows live, autosave, backup, moving a show to another Mac.
  12. **Reference**: keyboard shortcuts, the `/cmdmx` OSC addresses, daemon flags, file locations,
      troubleshooting (no output, port in use, Gatekeeper), glossary.
  13. **Release notes**: one entry per release, newest first, linking to the GitHub release.
- **WEB-32** The manual describes only what works in the version it is for; it is written from the
  app and the specs, in its own words: no text, screenshots or diagrams copied from the Avolites
  manual (IDEA-21). Screenshots come from the real app (`docs/screenshots/`, or new ones under
  `manual/src/images/`), each with alt text.
- **WEB-33** From the manual's first PR on, every PR that changes user-visible behaviour updates
  the manual section it touches in the same PR (CLAUDE.md, step 5, gets this line).
  `scripts/check-tracking.py` warns (not fails) when a PR changes `subprojects/app/Sources` without
  touching `subprojects/website/manual/`.
- **WEB-34** `build.sh` fails on a broken internal link or a missing image in the manual (mdBook's
  link check, or a small check in `build.sh`).

## Tests
- WEB-13, 11 (no release), 34: `subprojects/website/test-build.sh` runs `build.sh` against a fake
  `gh` (a release JSON and an empty one) and checks the version, URL, SHA-256 and "No release yet"
  in `_site/index.html`, no `{{` left, and that a broken link fails the build. In PR CI (WEB-22).
- WEB-02, 03: the same script fails on `http(s)://` in `src`/`href` of scripts, styles and fonts
  outside GitHub links; the owner checks phone width and dark/light once in the PR (preview).
- WEB-20, 21, 23: `tests/hardware/website.md` (owner, gate 3 style): after the first deploy the page
  is live, after the next `v*` tag the button shows it without a manual step, the DMG downloads
  and opens as REL-15.
- WEB-31, 32: reviewed by the owner per manual PR.

## Out of scope
A purchase or licence page (IDEA-21); auto-update (Sparkle); notarisation itself (VS8-08); other
languages (owner decision 1); a blog or news; comments or a forum (GitHub issues
are the contact); hosting the hardware build plans (IDEA-23, later, as its own section).

## Owner decisions (P1-21, 2026-10-27)
1. Site and manual in English; Dutch later, as an idea if wanted.
2. An own domain now, not github.io first; the owner registers it and names it. Amended 2026-10-28
   (DECISIONS): VS8-03 ships on `aottens.github.io/CMDMX` with a placeholder logo; the domain is one
   line in `subprojects/website/CNAME` once named.
3. The site may go live as a *preview* with the DMG not notarised and REL-15's right-click steps
   (WEB-12); notarisation is its own task (VS8-08), done before the site is announced anywhere.
4. No price, no purchase page and no licence promise until IDEA-21 is decided (WEB-14).
