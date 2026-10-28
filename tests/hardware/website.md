# Website checklist (WEB-03, 10..12, 20..23, 30..32) — gate 3

Setup: the owner's Mac and an iPhone (or Safari's responsive mode). The site is built by
`.github/workflows/website.yml`; `subprojects/website/README.md` says how to build it locally.

## Once, before the first deploy (owner, GitHub settings)
- [x] Settings › Pages › Build and deployment › Source: **GitHub Actions** (WEB-20)
- [x] Settings › Environments › `github-pages` › Deployment branches and tags: add the tag rule `v*`
  beside `main`, so the run that `release.yml` starts on a tag may deploy (WEB-21)

## First deploy (WEB-20, 22)
- [x] Actions › Website › Run workflow (branch `main`): build and deploy are green
- [x] `https://aottens.github.io/CMDMX/` opens and shows the lockup, the download block with the latest
  version, date, size and SHA-256, the screenshots and the install steps (WEB-10..12)
- [x] Download: the button downloads the DMG of the latest release; `shasum -a 256` of the file equals
  the SHA-256 on the page; the DMG opens as `tests/hardware/release.md` describes (WEB-11, REL-15)
- [x] Release notes goes to the GitHub release page; repo and issues links open on GitHub (WEB-10)
- [x] On the iPhone (or at 320 px wide): no sideways scrolling, the SHA-256 wraps (WEB-03)
- [x] Dark mode and light mode both readable; Tab shows a visible focus ring on every link (WEB-03)
- [x] Safari › Develop › Disable JavaScript, reload: the page looks the same (WEB-03)
- [x] Web Inspector › Network: every request goes to `aottens.github.io` (or the own domain) only (WEB-02)

## The manual (VS8-04: WEB-23, 30, 31)
- [x] *Links › Read the user manual* opens `…/CMDMX/manual/`; the header shows the mark and *CMDMX*,
  which lead back to the landing page (WEB-30)
- [x] While the latest release has no manual (v0.1.0): every page shows the orange *Preview: written
  ahead of the next release* strip and the front page says it is for the next release (WEB-23); not seen: v0.2.0 with the manual was out before the first check, so the next line covers it
- [x] Search (magnifier or `S`) finds "Art-Net"; the printer icon shows the whole manual on one page
  and Safari › File › Export as PDF saves it (WEB-30)
- [x] Dark and light theme (brush icon) both readable; the sidebar shows sections 1–3 and 13, the
  rest greyed out (WEB-30, 31); since VS8-07 all 13 sections are written
- [x] Read Introduction, Getting started and Concepts: nothing described that the app cannot do
  (WEB-32); note anything wrong under Open points
- [x] After the first `v*` tag that contains `subprojects/website/manual/`: the banner is gone and the
  front page says *This manual is for CMDMX <version>* (WEB-23)

## After the next `v*` tag (WEB-21)
- [x] Without touching anything: a few minutes after the Release run is green, its *Website* job is
  green too and the page shows the new version and the `CMDMX-<version>.dmg` link

## Own domain, when the owner has registered it (WEB-20)
- [ ] The domain is the only line in `subprojects/website/CNAME` (a one-line PR), Settings › Pages ›
  Custom domain is set, the registrar's DNS records point at GitHub Pages, *Enforce HTTPS* is on
- [ ] `https://<domain>/` shows the page; `https://aottens.github.io/CMDMX/` redirects to it

## Open points
- none
