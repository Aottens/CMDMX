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
