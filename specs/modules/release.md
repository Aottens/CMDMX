# Module spec: release (REL)

Owner: `.github/workflows/release.yml` and the bundle/DMG scripts in `subprojects/app/scripts/`
(packaging); the daemon launcher in `subprojects/app` (starting and stopping the bundled daemon);
`cmdmx-daemon` (flush on quit).
Status: **REL-01..16 approved by owner 2026-10-27 (VS4-07)**, with the recommended answers to the
open questions (see *Owner decisions*). **REL-17..23 and the amendments of REL-05, 08, 09, 15, 16
proposed 2026-10-28 (VS8-08), awaiting owner approval** (see *Open questions (VS8-08)*).
Builds on DECISIONS 2026-10-27 (versioned DMG, not notarised; its reversal trigger is reached by
the website, DECISIONS 2026-10-27 *own website*), IPC-01/08 (`ipc.md`), SHW-11
(`showfile.md`), LIB-13 (`fixture-library.md`), CTL-53 (`control-surfaces.md`).

## Purpose
The church Mac runs CMDMX from a DMG that GitHub builds from a version tag: drag the app
to Applications, open it, and the engine runs. No Rust, no Swift, no build tree, no Terminal on
that Mac. Quitting the app frees the Eurolite for Lightkey (the fallback).

## Definitions
- **Version:** `MAJOR.MINOR.PATCH` (e.g. `0.2.0`), taken from a git tag `v<version>`.
- **Bundled daemon:** the `cmdmx-daemon` binary inside the app bundle.
- **Pid file:** `~/Library/Application Support/CMDMX/daemon.pid`, the process id of the
  bundled daemon the app last started.

## Acceptance criteria

### Version
- **REL-01** The version comes only from the tag: a push of a tag matching
  `v<MAJOR>.<MINOR>.<PATCH>` (digits only) starts the release job, and the version is the tag
  without `v`. Any other `v*` tag (`v1`, `v0.2`, `v0.2.0-rc1`) fails the job before building,
  with a message naming the tag. Cargo crate versions are not the product version and are not
  changed by a release.
- **REL-02** The app's `Info.plist` carries `CFBundleShortVersionString` = the version and
  `CFBundleVersion` = the GitHub run number. `CMDMX › About CMDMX` (the
  standard macOS About panel, no own code) shows `Version <version> (<run number>)`.
- **REL-03** A local build (`scripts/bundle-app.sh` without a version) stays possible and is marked
  as such: `CFBundleShortVersionString` `0.0.0`, `CFBundleVersion` `dev`.

### Bundle and DMG layout
- **REL-04** (amended by BRD-04) The release bundle `CMDMX.app` contains:
  - `Contents/MacOS/CMDMX` — the app (release build);
  - `Contents/MacOS/cmdmx-daemon` — the daemon (release build);
  - `Contents/Resources/qlcplus/` — the QLC+ fixture files from `scripts/fetch-qlcplus.sh`
    (`fixtures/`, `LICENSE`, `NOTICE`, `COMMIT`), Apache-2.0 notice kept;
  - `Contents/Info.plist` as REL-02.
  Both binaries are universal (arm64 and x86_64), so the DMG runs on any Mac with macOS 14 or
  later (`LSMinimumSystemVersion` 14.0, as today).
- **REL-05** (amended by REL-17) Without a signing identity, the bundle is signed ad hoc (`codesign --sign -`), the daemon first, then the app;
  `codesign --verify --deep --strict` passes. This is the local build; a release is signed and notarised as REL-17..20.
- **REL-06** (amended by BRD-04) The DMG is `CMDMX-<version>.dmg`, compressed (UDZO), volume name
  `CMDMX <version>`, and holds exactly two items: `CMDMX.app` and a symlink
  `Applications` → `/Applications` (drag to install). No background image, no window layout.
- **REL-07** One script builds it: `subprojects/app/scripts/make-dmg.sh <version> <build>` runs
  the daemon and app release builds, fills the bundle (REL-04), signs it (REL-05) and writes the DMG
  (REL-06) into `subprojects/app/.build/`. It runs the same on the owner's Mac and in CI.
- **REL-08** (amended by BRD-04, REL-21) `subprojects/app/scripts/check-dmg.sh <dmg> <version>` mounts the DMG read-only and fails unless:
  the two items of REL-06 are there; the plist versions match REL-02; both binaries are universal
  (`lipo -archs` lists `arm64` and `x86_64`); `codesign --verify --deep --strict` passes; and the
  bundled daemon, started as the app starts it (REL-10) with a temporary `--show-dir` and a free
  `--ipc-port`, runs `--ticks 20` and exits 0; and `Contents/Resources/qlcplus/fixtures` holds
  `.qxf` files; with `--notarised`, also the checks of REL-21. It unmounts in every case.

### Release job
- **REL-09** (amended by BRD-04, REL-20) `.github/workflows/release.yml` runs on `push: tags: ['v*']` on `macos-14` (Swift
  5.10, as the `app` CI job): REL-01 check, `scripts/fetch-qlcplus.sh`, `make-dmg.sh`,
  `check-dmg.sh`, then a GitHub release named `CMDMX <version>` for the tag with the DMG
  attached and the install steps of REL-15 in its notes. It also runs on `workflow_dispatch`
  (version `0.0.0`, build = run number) and then uploads the DMG as a workflow artifact instead of
  making a release, so a build can be tried without a tag. It needs only `contents: write`, the
  default `GITHUB_TOKEN` and the signing secrets of REL-20. The PR CI (`ci.yml`) does not change, to keep its runners light.

### Starting and stopping the daemon from the app
- **REL-10** When the app runs from a bundle that contains `Contents/MacOS/cmdmx-daemon`, it starts
  that daemon at launch as a child process with `--qlcplus-dir <bundle>/Contents/Resources/qlcplus/fixtures`
  and no other arguments (default shows folder, state file and IPC port), writes its pid to the pid
  file, and connects as IPC-08. Without a bundled daemon (`swift run`, `bundle-app.sh` debug) the app
  starts nothing and behaves as today, so a developer's `cargo run` daemon is used.
- **REL-11** The app starts no second daemon when one of its own is still running: if the pid file
  names a live process whose executable is this bundle's `cmdmx-daemon` (after an app crash or a
  force quit, REL-14), the app adopts it instead. If the IPC port is taken by any other program, the
  new daemon exits with code 2 before opening any output (as today), the app connects to whatever
  answers on the port, and it shows the warning "Another engine is running on port 7319".
- **REL-12** The daemon's stdout and stderr go to `~/Library/Logs/CMDMX/daemon.log`
  (appended; at app launch a log over 10 MB is renamed to `daemon.log.1`, replacing the old one).
  When the daemon the app started exits while the app runs, the app shows the "engine offline"
  banner (IPC-08) with the exit code, and starts it again after 2 s; the log keeps both runs.
- **REL-13** Quitting the app (⌘Q, or the menu) sends SIGTERM to the daemon it started or adopted,
  waits until it has exited (at most 5 s, then SIGKILL), removes the pid file, and then quits. The
  serial port (the Eurolite) is free afterwards, so Lightkey can open it. A daemon the app did not
  start (a developer's) is left alone.
- **REL-14** On SIGINT or SIGTERM the daemon writes pending show changes (SHW-11 autosave) before
  it exits, besides blanking the decks (CTL-53), so quitting the app never loses the last 5 s of
  programming. If the app itself crashes, the daemon keeps running and keeps output (show-proof);
  the next app launch adopts it (REL-11).

### First launch on a Mac
- **REL-15** (amended by REL-22) The release notes and `tests/hardware/release.md` give the install steps for an
  app that is not notarised: open the DMG, drag CMDMX to Applications, eject; then the
  first open is **right-click › Open › Open** (macOS 14). On macOS 15 or later, where right-click
  no longer offers Open: double-click once, close the warning, then **System Settings › Privacy &
  Security › Open Anyway**. After that the app opens normally until the next version is installed.
  The same notes name the prompts that may follow on first use: *Local Network* (allow, for Art-Net
  and sACN) and the firewall question for incoming connections (allow, for OSC and TouchOSC).
- **REL-16** (amended by REL-23) `tests/hardware/release.md` (gate 3, owner): on the church Mac, install the DMG of the
  first tag as REL-15; About shows the version; the app starts the engine without Terminal; the
  *Kerk* show (copied into `~/Documents/CMDMX/Shows`) opens and the Eurolite gives output;
  ⌘Q, then Lightkey opens the Eurolite and gives output; open CMDMX again and it gives
  output again. Installing a newer DMG over it keeps the shows and settings.

### Developer ID and notarisation (VS8-08)
- **REL-17** `make-dmg.sh` signs with the identity named in the environment variable
  `CMDMX_SIGN_IDENTITY` (e.g. `Developer ID Application: <name> (<team id>)`); when it is unset or
  empty it signs ad hoc as REL-05, so a local build needs no certificate. With an identity it signs
  `Contents/MacOS/cmdmx-daemon` first, then `CMDMX.app`, each with
  `codesign --force --timestamp --options runtime --sign "$CMDMX_SIGN_IDENTITY"` (no `--deep`), and
  after `hdiutil` it signs the DMG itself with `codesign --timestamp --sign` and the same identity.
  `codesign --verify --deep --strict` passes on the app and `codesign --verify` on the DMG.
- **REL-18** Both binaries run under the hardened runtime with no entitlements and outside the App
  Sandbox. Outside the sandbox macOS has no entitlement for the local network or for `/dev/cu.*`
  serial ports, and the hardened runtime restricts neither (it restricts JIT, unsigned libraries,
  debugging and DYLD variables, none of which CMDMX uses). The `Info.plist` (`bundle-app.sh`) gains
  `NSLocalNetworkUsageDescription` = "CMDMX sends DMX to Art-Net and sACN nodes on your network.",
  the text of the macOS 15 Local Network prompt. The daemon is a child of the app, so macOS asks
  once, for the app; `check-dmg.sh` fails when the key is missing. An entitlement is added only when a gate-3 run shows a hardened-runtime denial,
  with that denial recorded in DECISIONS.
- **REL-19** With an identity and the notary variables `CMDMX_NOTARY_KEY` (path of the App Store
  Connect API key `.p8`), `CMDMX_NOTARY_KEY_ID` and `CMDMX_NOTARY_ISSUER` set, `make-dmg.sh`
  notarises twice, so both the DMG and the app carry a ticket and the first open works offline:
  1. the signed app, zipped with `ditto -c -k --keepParent`, goes to
     `xcrun notarytool submit --wait --key … --key-id … --issuer …`; then
     `xcrun stapler staple CMDMX.app`;
  2. the DMG is built from the stapled app and signed (REL-17), submitted the same way, and stapled
     with `xcrun stapler staple CMDMX-<version>.dmg`.
  A submission whose status is not `Accepted` prints `xcrun notarytool log <id>` and fails the
  script; nothing is retried. An identity without the notary variables builds a signed, not
  notarised DMG (for a local check of REL-18). Notarisation lives in `make-dmg.sh`, not in
  `release.yml`, because the app has to be stapled before the DMG is made (REL-07: one script).
- **REL-20** `release.yml` reads five repository secrets: `DEVELOPER_ID_P12` (the Developer ID
  Application certificate with its private key, `.p12`, base64), `DEVELOPER_ID_P12_PASSWORD`,
  `NOTARY_KEY_P8` (the API key file, base64), `NOTARY_KEY_ID` and `NOTARY_ISSUER_ID`. Before
  `make-dmg.sh` it creates a keychain in `$RUNNER_TEMP` with a random password, imports the `.p12`
  for `codesign` only (`security import … -T /usr/bin/codesign`,
  `security set-key-partition-list`), puts it in the search list, takes the identity name from
  `security find-identity -v -p codesigning` and writes the `.p8` to `$RUNNER_TEMP` (mode 600).
  A last step with `if: always()` deletes the keychain and the `.p8`. No secret is echoed, written
  into the repo, the DMG or a workflow artifact. On a tag, a missing secret fails the job before the
  build with a message naming it: a release is never unnotarised. On `workflow_dispatch` with the
  secrets missing it builds ad hoc as before and says so in the job summary; with the secrets it
  builds notarised, so the owner can try a notarised `CMDMX-0.0.0.dmg` before tagging.
- **REL-21** `check-dmg.sh <dmg> <version> --notarised` fails unless, besides REL-08:
  `xcrun stapler validate` passes on the DMG and on the mounted `CMDMX.app`;
  `spctl --assess --type open --context context:primary-signature -v` on the DMG and
  `spctl --assess --type execute -v` on the app both say `accepted` with
  `source=Notarized Developer ID`; and `codesign -dv` on both binaries shows the `runtime` flag and an
  `Authority=Developer ID Application` line. The release job passes `--notarised` whenever it signed.
- **REL-22** (amends REL-15 and WEB-12) From the first notarised release the install steps in
  `subprojects/app/scripts/release-notes.md`, the landing page `subprojects/website/index.html`
  and the manual (`getting-started.md`) are: open the DMG, drag **CMDMX** onto **Applications**,
  eject the DMG, open CMDMX from Applications (macOS asks once to confirm an app downloaded from the
  internet: **Open**). The Local Network and incoming-connections prompts stay as REL-15. The
  right-click and *Open Anyway* steps leave those three places; the manual keeps them once, in the
  troubleshooting part of `reference.md`, for versions before the first notarised one, and the
  *not notarised* lines in `introduction.md` and `release-notes.md` (manual) are updated.
  `subprojects/website/test-build.sh` checks the page has the drag step and no *Open Anyway*.
- **REL-23** `tests/hardware/release.md` (gate 3, owner) gets a *Notarised install* part: on the
  church Mac, with the DMG of the first notarised tag downloaded in a browser (so it is
  quarantined) and Wi-Fi off before the first open, drag CMDMX to Applications and double-click:
  it opens after the one standard confirmation, no *Open Anyway*;
  `spctl -a -vv /Applications/CMDMX.app` says `source=Notarized Developer ID`; Wi-Fi on, the
  Local Network prompt shows the REL-18 text; Art-Net output reaches a node and the Eurolite gives
  output (serial under the hardened runtime); the rest of REL-16 as before.

## Tests
- REL-01: `subprojects/app/scripts/release-version.sh <tag>` (used by `release.yml`), and
  `subprojects/app/scripts/test-release-tag.sh` feeding it good and bad tags (runs in CI's engine
  job, no macOS needed).
- REL-02..08: `check-dmg.sh` on the DMG in the release job and on `workflow_dispatch`; it fails
  today (no `make-dmg.sh`).
- REL-14: a daemon test `rel_14_sigterm_flushes_pending_autosave` (change, signal before 5 s,
  the file holds the change).
- REL-17, 19..21: `check-dmg.sh --notarised` in the release job on a tag and on
  `workflow_dispatch` with secrets; a notarisation that is not `Accepted` fails the job (REL-19).
  REL-20's missing-secret path: a `workflow_dispatch` on a fork or with a secret removed shows the
  ad hoc warning in the summary (checked once, by hand).
- REL-18: `check-dmg.sh` (the plist key); the denial check is REL-23 (gate 3).
- REL-22: `subprojects/website/test-build.sh` (WEB-12 lines) and the manual link check.
- REL-10..13, 15, 16, 23: no Swift tests (CLAUDE.md); `tests/hardware/release.md` (gate 3), and a
  local check on the owner's Mac with a `workflow_dispatch` DMG before the first tag.

## Out of scope (this spec)
The Mac App Store and the App Sandbox; a Developer ID Installer (`.pkg`); auto-update (Sparkle);
a DMG background or icon; an app icon; a launchd agent that runs the daemon without the app;
showing the daemon's own version; bumping the Cargo versions; distributing to anyone besides the
owner (IDEA-20).

## Owner decisions (2026-10-27)
1. Universal build (arm64 and x86_64), so the church Mac's chip does not matter (REL-04).
2. Quit stops the engine (REL-13): the fallback needs the Eurolite free.
3. The first tag is `v0.1.0`, after this task is merged.

## Open questions (VS8-08, for the owner)
What the owner must provide before the build task starts:
- **Apple Developer Program** membership (99 USD a year) on the owner's Apple ID.
- A **Developer ID Application** certificate (only the Account Holder can create it): made in
  Xcode › Settings › Accounts or on developer.apple.com, exported from Keychain Access as `.p12`
  with a password.
- An **App Store Connect API key** (Users and Access › Integrations › Team Keys, role *Developer*):
  the `.p8` file (downloadable once), its key id and the issuer id.
- The five repository secrets of REL-20, set by the owner in GitHub › Settings › Secrets and
  variables › Actions (Claude never sees the `.p12`, its password or the `.p8`).

Questions, with the recommended answer first:
1. **Enrol as an individual or as the organisation Colourmotion?** The name in the certificate is
   what macOS shows as the developer. Recommended: as Colourmotion if it is a registered business
   (needs a free D-U-N-S number, a few days to weeks); otherwise as an individual (owner's name),
   which is quick; converting later needs a new certificate, and older releases keep the old name.
2. **Staple the app as well as the DMG (two notarisation rounds, REL-19)?** Recommended: yes, so
   the church Mac opens a new version without internet; it adds a few minutes per release.
3. **`workflow_dispatch` without secrets: ad hoc build with a warning (REL-20), or fail?**
   Recommended: ad hoc with a warning, so forks and tests without the secrets still build.
4. **First notarised version:** the next tag after the build task is merged (e.g. `v0.2.0`), and
   from then on the site is announced (DECISIONS 2026-10-27). Recommended: yes.
