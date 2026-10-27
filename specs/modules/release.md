# Module spec: release (REL)

Owner: `.github/workflows/release.yml` and the bundle/DMG scripts in `subprojects/app/scripts/`
(packaging); the daemon launcher in `subprojects/app` (starting and stopping the bundled daemon);
`cmdmx-daemon` (flush on quit).
Status: **REL-01..16 approved by owner 2026-10-27 (VS4-07)**, with the recommended answers to the
open questions (see *Owner decisions*).
Builds on DECISIONS 2026-10-27 (versioned DMG, not notarised), IPC-01/08 (`ipc.md`), SHW-11
(`showfile.md`), LIB-13 (`fixture-library.md`), CTL-53 (`control-surfaces.md`).

## Purpose
The church Mac runs ColourmotionDMX from a DMG that GitHub builds from a version tag: drag the app
to Applications, open it, and the engine runs. No Rust, no Swift, no build tree, no Terminal on
that Mac. Quitting the app frees the Eurolite for Lightkey (the fallback).

## Definitions
- **Version:** `MAJOR.MINOR.PATCH` (e.g. `0.2.0`), taken from a git tag `v<version>`.
- **Bundled daemon:** the `cmdmx-daemon` binary inside the app bundle.
- **Pid file:** `~/Library/Application Support/ColourmotionDMX/daemon.pid`, the process id of the
  bundled daemon the app last started.

## Acceptance criteria

### Version
- **REL-01** The version comes only from the tag: a push of a tag matching
  `v<MAJOR>.<MINOR>.<PATCH>` (digits only) starts the release job, and the version is the tag
  without `v`. Any other `v*` tag (`v1`, `v0.2`, `v0.2.0-rc1`) fails the job before building,
  with a message naming the tag. Cargo crate versions are not the product version and are not
  changed by a release.
- **REL-02** The app's `Info.plist` carries `CFBundleShortVersionString` = the version and
  `CFBundleVersion` = the GitHub run number. `ColourmotionDMX › About ColourmotionDMX` (the
  standard macOS About panel, no own code) shows `Version <version> (<run number>)`.
- **REL-03** A local build (`scripts/bundle-app.sh` without a version) stays possible and is marked
  as such: `CFBundleShortVersionString` `0.0.0`, `CFBundleVersion` `dev`.

### Bundle and DMG layout
- **REL-04** The release bundle `ColourmotionDMX.app` contains:
  - `Contents/MacOS/ColourmotionDMX` — the app (release build);
  - `Contents/MacOS/cmdmx-daemon` — the daemon (release build);
  - `Contents/Resources/qlcplus/` — the QLC+ fixture files from `scripts/fetch-qlcplus.sh`
    (`fixtures/`, `LICENSE`, `NOTICE`, `COMMIT`), Apache-2.0 notice kept;
  - `Contents/Info.plist` as REL-02.
  Both binaries are universal (arm64 and x86_64), so the DMG runs on any Mac with macOS 14 or
  later (`LSMinimumSystemVersion` 14.0, as today).
- **REL-05** The bundle is signed ad hoc (`codesign --sign -`), the daemon first, then the app;
  `codesign --verify --deep --strict` passes. No Developer ID, no notarisation (DECISIONS
  2026-10-27).
- **REL-06** The DMG is `ColourmotionDMX-<version>.dmg`, compressed (UDZO), volume name
  `ColourmotionDMX <version>`, and holds exactly two items: `ColourmotionDMX.app` and a symlink
  `Applications` → `/Applications` (drag to install). No background image, no window layout.
- **REL-07** One script builds it: `subprojects/app/scripts/make-dmg.sh <version> <build>` runs
  the daemon and app release builds, fills the bundle (REL-04), signs it (REL-05) and writes the DMG
  (REL-06) into `subprojects/app/.build/`. It runs the same on the owner's Mac and in CI.
- **REL-08** `subprojects/app/scripts/check-dmg.sh <dmg> <version>` mounts the DMG read-only and fails unless:
  the two items of REL-06 are there; the plist versions match REL-02; both binaries are universal
  (`lipo -archs` lists `arm64` and `x86_64`); `codesign --verify --deep --strict` passes; and the
  bundled daemon, started as the app starts it (REL-10) with a temporary `--show-dir` and a free
  `--ipc-port`, runs `--ticks 20` and exits 0; and `Contents/Resources/qlcplus/fixtures` holds
  `.qxf` files. It unmounts in every case.

### Release job
- **REL-09** `.github/workflows/release.yml` runs on `push: tags: ['v*']` on `macos-14` (Swift
  5.10, as the `app` CI job): REL-01 check, `scripts/fetch-qlcplus.sh`, `make-dmg.sh`,
  `check-dmg.sh`, then a GitHub release named `ColourmotionDMX <version>` for the tag with the DMG
  attached and the install steps of REL-15 in its notes. It also runs on `workflow_dispatch`
  (version `0.0.0`, build = run number) and then uploads the DMG as a workflow artifact instead of
  making a release, so a build can be tried without a tag. It needs only `contents: write` and the
  default `GITHUB_TOKEN`. The PR CI (`ci.yml`) does not change, to keep its runners light.

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
- **REL-12** The daemon's stdout and stderr go to `~/Library/Logs/ColourmotionDMX/daemon.log`
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
- **REL-15** The release notes and `tests/hardware/release.md` give the install steps for an
  app that is not notarised: open the DMG, drag ColourmotionDMX to Applications, eject; then the
  first open is **right-click › Open › Open** (macOS 14). On macOS 15 or later, where right-click
  no longer offers Open: double-click once, close the warning, then **System Settings › Privacy &
  Security › Open Anyway**. After that the app opens normally until the next version is installed.
  The same notes name the prompts that may follow on first use: *Local Network* (allow, for Art-Net
  and sACN) and the firewall question for incoming connections (allow, for OSC and TouchOSC).
- **REL-16** `tests/hardware/release.md` (gate 3, owner): on the church Mac, install the DMG of the
  first tag as REL-15; About shows the version; the app starts the engine without Terminal; the
  *Kerk* show (copied into `~/Documents/ColourmotionDMX/Shows`) opens and the Eurolite gives output;
  ⌘Q, then Lightkey opens the Eurolite and gives output; open ColourmotionDMX again and it gives
  output again. Installing a newer DMG over it keeps the shows and settings.

## Tests
- REL-01: `subprojects/app/scripts/release-version.sh <tag>` (used by `release.yml`), and
  `subprojects/app/scripts/test-release-tag.sh` feeding it good and bad tags (runs in CI's engine
  job, no macOS needed).
- REL-02..08: `check-dmg.sh` on the DMG in the release job and on `workflow_dispatch`; it fails
  today (no `make-dmg.sh`).
- REL-14: a daemon test `rel_14_sigterm_flushes_pending_autosave` (change, signal before 5 s,
  the file holds the change).
- REL-10..13, 15, 16: no Swift tests (CLAUDE.md); `tests/hardware/release.md` (gate 3), and a
  local check on the owner's Mac with a `workflow_dispatch` DMG before the first tag.

## Out of scope (this spec)
Notarisation and a Developer ID (DECISIONS 2026-10-27, reversal trigger); auto-update (Sparkle);
a DMG background or icon; an app icon; a launchd agent that runs the daemon without the app;
showing the daemon's own version; bumping the Cargo versions; distributing to anyone besides the
owner (IDEA-20).

## Owner decisions (2026-10-27)
1. Universal build (arm64 and x86_64), so the church Mac's chip does not matter (REL-04).
2. Quit stops the engine (REL-13): the fallback needs the Eurolite free.
3. The first tag is `v0.1.0`, after this task is merged.
