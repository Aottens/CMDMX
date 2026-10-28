# Website

Plain HTML + CSS landing page, built by `build.sh` into `_site/` (git-ignored) and published to
GitHub Pages by `.github/workflows/website.yml`.

- Build: `subprojects/website/build.sh` (needs `jq` and `gh` logged in, or `GH_TOKEN` and `GH_REPO`).
  The download block is filled from the latest GitHub release.
- Preview: `python3 -m http.server -d subprojects/website/_site 8000`
- Test: `subprojects/website/test-build.sh` (runs the build against a fake `gh`).
- Custom domain: put the domain as the only line in `subprojects/website/CNAME`, then set it in
  Settings > Pages and at the registrar.
- The logo in `brand/` is a placeholder until VS8-02.

## Manual

mdBook sources in `manual/` (chapters in `manual/src/`), built by `build.sh` into `_site/manual/`.
Locally: `brew install mdbook` (v0.5.4, same as `install-mdbook.sh` for CI).

- The page header (mark, link back to the landing page, preview banner) is `manual/theme/header.hbs`.
- `website.yml` builds the manual of the latest release tag (`MANUAL_FROM_TAG=1`: no banner, front page
  names the version). Without that, the manual of `main` is built with the "Preview" banner.
- `build.sh` fails on a broken internal link or image, or a root-absolute link (WEB-34).
