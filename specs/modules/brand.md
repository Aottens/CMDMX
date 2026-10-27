# Module spec: brand (BRD)

Owner: the product name wherever a user sees it (app bundle, DMG, release, sACN/MIDI names,
data folders, docs) and the brand assets in `subprojects/website/brand/`.
Status: **draft, waiting for owner approval (gate 1)** (P1-21, 2026-10-27). Takes up IDEA-19
(brand "CMDMX by Colourmotion", owner 2026-10-22). Open questions at the end.
Builds on `release.md` (REL-02..09, the DMG and release job), `output-protocols.md` (sACN source
name), `showfile.md` (show folder), `control-surfaces.md` (control folder, CTL-23).

## Purpose
One name everywhere a user looks: **CMDMX**, made by **Colourmotion**. Done before there are users,
so nothing has to be renamed twice, and before the website (`website.md`) is built on it.

## Definitions
- **Product name:** `CMDMX`, always in capitals, never "CmDmx", "cmdmx" or "CM-DMX" in prose.
  Lower case `cmdmx` stays where it is an identifier (crates `cmdmx-*`, the binary `cmdmx-daemon`,
  the OSC schema `/cmdmx`).
- **Brand line:** `CMDMX by Colourmotion`, for the website, the About panel and the README title.
- **Old name:** `ColourmotionDMX`, in any capitalisation.
- **Living file:** a file that describes the product as it is now: code, scripts, workflows,
  README, specs, checklists in `tests/hardware/` and `tests/manual/` (not their `runs/`), touchosc,
  the website. **Not** living: dated records (`tests/*/runs/`, `DECISIONS.md`, `IDEAS.md` history
  sections, `docs/research/`), which keep the name they were written with.

## Acceptance criteria

### The rename
- **BRD-01** No living file contains the old name, except where a criterion below keeps it on
  purpose (BRD-08 migration, BRD-10 repo redirect note). `scripts/check-brand.sh` greps the tree
  case-insensitively for `colourmotiondmx`, skips the not-living paths of *Definitions* and the
  allowlist lines marked `brand: old-name`, and fails naming each hit; it runs in CI's engine job.
- **BRD-02** The app: SwiftPM product and target `CMDMX` (`Sources/CMDMX/`, `CMDMXApp`), window
  title `CMDMX`, `CFBundleName`/`CFBundleDisplayName`/`CFBundleExecutable` `CMDMX`, menu
  `CMDMX › About CMDMX`, `NSHumanReadableCopyright` `© 2026 Colourmotion`. The About panel shows
  `CMDMX`, the REL-02 version line and the copyright line.
- **BRD-03** The bundle id stays `nl.colourmotion.dmx`, so macOS keeps the Local Network and
  firewall answers (REL-15) on Macs that already run v0.1.0.
- **BRD-04** Packaging (amends REL-04, 06, 08, 09): the bundle is `CMDMX.app` with
  `Contents/MacOS/CMDMX`; the DMG is `CMDMX-<version>.dmg` with volume name `CMDMX <version>`,
  holding `CMDMX.app` and `Applications`; the GitHub release is named `CMDMX <version>`.
  `make-dmg.sh`, `check-dmg.sh`, `bundle-app.sh` and `release.yml` follow; `check-dmg.sh` checks
  the new names.
- **BRD-05** Names on the wire: the sACN source name is `CMDMX` (amends the OUT sACN criterion; byte
  test updated); the Art-Net poll reply short name and long name, where sent, are `CMDMX` and
  `CMDMX by Colourmotion`; the Bonjour names in `touchosc/serve.sh` and the layout's page name are
  `CMDMX`; the layout file is `touchosc/CMDMX.touchosc`. The OSC schema `/cmdmx` does not change.
- **BRD-06** Messages: every user-visible string that named the product says `CMDMX` (e.g. the
  SHW schema error "… was made by a newer CMDMX …"); tests that assert those strings follow.
- **BRD-07** Data folders move to the new name: `~/Documents/CMDMX/Shows`,
  `~/Library/Application Support/CMDMX/` (`profiles/`, `control/`, `state.json`, `daemon.pid`) and
  `~/Library/Logs/CMDMX/`. The daemon's defaults (`--show-dir`, `--profiles-dir`, state and control
  paths) and the app's launcher (REL-10..12) use them.
- **BRD-08** Migration, once, in the daemon at start before it reads any of them: for each of
  `Documents/…/Shows`, `Application Support/…/profiles`, `…/control` and `…/state.json`, when the
  new item is missing and the old one exists, the old one is renamed to the new place (one
  `rename`, same volume) and the daemon logs one line naming both paths. When both exist, nothing
  moves, the new one is used and the snapshot carries one warning "Old ColourmotionDMX folder
  left at <path>" (`brand: old-name`). A failed rename is a warning too and the daemon starts on
  the new (empty) item; it never stops. Only the default paths migrate; a path given by a flag is
  used as given. Logs and the pid file are not migrated.
- **BRD-09** Docs: `README.md` title `# CMDMX by Colourmotion`, `PROJECT.md`, `STATE.md`,
  `ARCHITECTURE.md`, `CLAUDE.md`, `BACKLOG.md` headings, the specs and the living checklists say
  CMDMX. Paths in checklists follow BRD-04/07.
- **BRD-10** The GitHub repository is renamed to `Aottens/CMDMX` (owner, in GitHub settings;
  GitHub redirects the old URL). Every link in living files points at the new URL;
  `subprojects/engine/Cargo.toml`'s `repository` becomes `https://github.com/Aottens/CMDMX`. The
  local checkout folder keeps its name (see question 3).
- **BRD-11** The hardware firmware's USB manufacturer string becomes `Colourmotion`; the product
  string `CMDMX Desk` (HW-20) does not change, so learned mappings keep matching.

### Logo and brand assets
- **BRD-20** Concepts first: three logo directions (each a mark plus the `CMDMX` wordmark and the
  `by Colourmotion` line), shown to the owner as one page with each on dark and light, at app-icon
  size (1024 px and 32 px) and as a website header. The owner picks one (or asks for a round of
  changes) before any asset below is made. Not a PR: the pick is recorded in DECISIONS.
- **BRD-21** Masters in `subprojects/website/brand/`, hand-made SVG (no embedded raster, text
  converted to paths so no font is needed to render it):
  `mark.svg` (the symbol alone, square), `wordmark.svg` (`CMDMX`), `lockup.svg` (mark + wordmark +
  `by Colourmotion`, horizontal), `lockup-stacked.svg`; each in a `-dark` variant for dark
  backgrounds. A `README.md` there gives the colours (hex, with the contrast ratio against the two
  backgrounds, ≥ 4.5:1 for text), the clear space and the minimum size, and the typeface (open
  licence, SIL OFL) with its licence file.
- **BRD-22** App icon: `brand/AppIcon.icns` built from `brand/app-icon.svg` (macOS icon grid,
  1024 px master) by `brand/make-icons.sh` with `iconutil` (in the Command Line Tools); the exported
  PNGs are committed so CI needs no SVG renderer. `bundle-app.sh` copies it to
  `Contents/Resources/AppIcon.icns` and sets `CFBundleIconFile`; `check-dmg.sh` checks it is there.
  Lifts "an app icon" from `release.md`'s out-of-scope list.
- **BRD-23** Web and social: `favicon.svg`, `favicon-32.png`, `apple-touch-icon.png` (180 px),
  `og-image.png` (1200×630, lockup on the brand background) for the website, and
  `social-preview.png` (1280×640) that the owner uploads as the GitHub repo's social preview.
- **BRD-24** The README shows `lockup.svg` (dark/light through `<picture>`) at the top.
- **BRD-25** No resemblance to Avolites or Titan branding (IDEA-21 owner answer): no orange-on-black
  Avolites look, no use of their names in the logo.

## Tests
- BRD-01: `scripts/check-brand.sh` in CI; it fails on today's tree.
- BRD-04: `check-dmg.sh` (REL-08 amended) in the release job and on `workflow_dispatch`.
- BRD-05: the sACN byte test with the id `brd_05_sacn_source_name`; `touchosc/test_generate.py`
  checks the page name.
- BRD-07/08: daemon tests `brd_08_migrates_old_folders`, `brd_08_both_exist_warns`,
  `brd_08_flag_paths_untouched`, `brd_08_rename_failure_is_warning` (temp home).
- BRD-02, 22: `swift build` and the `workflow_dispatch` DMG checked on the owner's Mac (About panel
  and the Dock icon), added to `tests/hardware/release.md`.
- BRD-10: the owner renames the repo; `check-brand.sh` covers the links.
- BRD-21..24: reviewed by the owner in the PR (rendered on GitHub).

## Out of scope
A trademark registration; renaming the crates or the binary (already `cmdmx-*`); a DMG background;
a new bundle id; merch, slide templates and the like.

## Open questions (owner)
See the PR description and `STATE.md` *Owner to decide*; answers are recorded here after approval.
