# Release checklist (REL-01..16) — gate 3

Setup: the church Mac (macOS 14 or later), the Eurolite USB-DMX512 PRO MK2 on USB with a lamp, Lightkey
installed, and the DMG of the first tag (`v0.1.0`) from the GitHub release page. The install steps are
the release notes: `subprojects/app/scripts/release-notes.md`.

## Local check before the first tag (owner's Mac)
- [ ] GitHub › Actions › Release › Run workflow (branch `main`): the job is green and a workflow artifact `CMDMX-0.0.0.dmg` is there (REL-09)
- [ ] Download and unzip it, open the DMG: it shows `CMDMX.app` and `Applications`, nothing else (REL-06)
- [ ] `./subprojects/app/scripts/check-dmg.sh <path to dmg> 0.0.0` ends with `ok` (REL-08)

## Install on the church Mac (REL-15)
- [ ] Open the DMG, drag CMDMX to Applications, eject the DMG
- [ ] macOS 14: right-click › Open › Open. macOS 15 or later: double-click once, close the warning, System Settings › Privacy & Security › Open Anyway
- [ ] The app opens; macOS 15+: it opens normally the second time without any warning
- [ ] If asked: Local Network allowed, incoming connections allowed

## Run (REL-10..14, 16)
- [ ] CMDMX › About CMDMX shows `Version 0.1.0 (<run number>)` (REL-02)
- [ ] No Terminal is open, yet the status bar shows Engine online (REL-10)
- [ ] Copy the *Kerk* show into `~/Documents/CMDMX/Shows` and open it on the Show screen; the Eurolite (chosen on the Output screen) gives output on the lamp (REL-16)
- [ ] `~/Library/Logs/CMDMX/daemon.log` exists and has the daemon's lines (REL-12)
- [ ] Change something in the show, press ⌘Q at once, open the app again: the change is still there (REL-14)
- [ ] ⌘Q: the app quits, `pgrep cmdmx-daemon` finds nothing and `~/Library/Application Support/CMDMX/daemon.pid` is gone (REL-13)
- [ ] Open Lightkey: it opens the Eurolite and gives output (REL-13)
- [ ] Quit Lightkey, open CMDMX again: it gives output again (REL-16)
- [ ] Force quit the app (Activity Monitor): the lamp keeps its output; open the app again: Engine online, only one `cmdmx-daemon` in `pgrep -l cmdmx-daemon` (REL-11/14)
- [ ] `kill -9` the daemon while the app runs: the status bar shows "Engine exited (code …)" and the engine is back after ~2 s (REL-12)
- [ ] Start a different program on port 7319 (`nc -l 7319`), quit and open the app: the bar says "Another engine is running on port 7319" (REL-11)
- [ ] Install a newer DMG over the app: the shows and settings are still there (REL-16)
