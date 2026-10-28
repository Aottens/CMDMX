# Website checklist (WEB-03, 10..12, 20..22) — gate 3

Setup: the owner's Mac and an iPhone (or Safari's responsive mode). The site is built by
`.github/workflows/website.yml`; `subprojects/website/README.md` says how to build it locally.

## Once, before the first deploy (owner, GitHub settings)
- [ ] Settings › Pages › Build and deployment › Source: **GitHub Actions** (WEB-20)
- [ ] Settings › Environments › `github-pages` › Deployment branches and tags: add the tag rule `v*`
  beside `main`, so the run that `release.yml` starts on a tag may deploy (WEB-21)

## First deploy (WEB-20, 22)
- [ ] Actions › Website › Run workflow (branch `main`): build and deploy are green
- [ ] `https://aottens.github.io/CMDMX/` opens and shows the lockup, the download block with the latest
  version, date, size and SHA-256, the screenshots and the install steps (WEB-10..12)
- [ ] Download: the button downloads the DMG of the latest release; `shasum -a 256` of the file equals
  the SHA-256 on the page; the DMG opens as `tests/hardware/release.md` describes (WEB-11, REL-15)
- [ ] Release notes goes to the GitHub release page; repo and issues links open on GitHub (WEB-10)
- [ ] On the iPhone (or at 320 px wide): no sideways scrolling, the SHA-256 wraps (WEB-03)
- [ ] Dark mode and light mode both readable; Tab shows a visible focus ring on every link (WEB-03)
- [ ] Safari › Develop › Disable JavaScript, reload: the page looks the same (WEB-03)
- [ ] Web Inspector › Network: every request goes to `aottens.github.io` (or the own domain) only (WEB-02)

## After the next `v*` tag (WEB-21)
- [ ] Without touching anything: a few minutes after the Release run is green, its *Website* job is
  green too and the page shows the new version and the `CMDMX-<version>.dmg` link

## Own domain, when the owner has registered it (WEB-20)
- [ ] The domain is the only line in `subprojects/website/CNAME` (a one-line PR), Settings › Pages ›
  Custom domain is set, the registrar's DNS records point at GitHub Pages, *Enforce HTTPS* is on
- [ ] `https://<domain>/` shows the page; `https://aottens.github.io/CMDMX/` redirects to it

## Open points
- none
