# Release checklist (REL-01..23) — gate 3

Setup: the church Mac (macOS 14 or later), the Eurolite USB-DMX512 PRO MK2 on USB with a lamp, Lightkey
installed, and the DMG of the first tag (`v0.1.0`) from the GitHub release page. The install steps are
the release notes: `subprojects/app/scripts/release-notes.md`. The *Notarised install* part needs the DMG of the
first notarised tag instead; its prerequisites come first. The REL-15 part below is for versions before it.

## Local check before the first tag (owner's Mac)
- [ ] GitHub › Actions › Release › Run workflow (branch `main`): the job is green and a workflow artifact `CMDMX-0.0.0.dmg` is there (REL-09)
- [ ] Download and unzip it, open the DMG: it shows `CMDMX.app` and `Applications`, nothing else (REL-06)
- [ ] `./subprojects/app/scripts/check-dmg.sh <path to dmg> 0.0.0` ends with `ok` (REL-08)
- [ ] Rename (VS8-01): drag `CMDMX.app` to Applications and open it; CMDMX › About CMDMX shows `CMDMX`, the version line and `© 2026 Colourmotion`; the window title is CMDMX (BRD-02)
- [ ] The shows from `~/Documents/ColourmotionDMX/Shows` are on the Show screen, now in `~/Documents/CMDMX/Shows`, and the old folder has no `Shows` left (BRD-07/08) <!-- brand: old-name -->

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
- [ ] ⌘Q asks "Quit the app? The engine stops with the app: DMX output stops and the USB interface is free for another program." (UI-26); Quit: the app quits, `pgrep cmdmx-daemon` finds nothing and `~/Library/Application Support/CMDMX/daemon.pid` is gone (REL-13)
- [ ] Open Lightkey: it opens the Eurolite and gives output (REL-13)
- [ ] Quit Lightkey, open CMDMX again: it gives output again (REL-16)
- [ ] Force quit the app (Activity Monitor): the lamp keeps its output; open the app again: Engine online, only one `cmdmx-daemon` in `pgrep -l cmdmx-daemon` (REL-11/14)
- [ ] `kill -9` the daemon while the app runs: the status bar shows "Engine exited (code …)" and the engine is back after ~2 s (REL-12)
- [ ] Start a different program on port 7319 (`nc -l 7319`), quit and open the app: the bar says "Another engine is running on port 7319" (REL-11)
- [ ] Install a newer DMG over the app: the shows and settings are still there (REL-16)

## Notarised install (REL-17..23)
- [ ] Prerequisite: Apple Developer Program membership
- [ ] Prerequisite: a Developer ID Application certificate with its private key, exported as `.p12`
- [ ] Prerequisite: an App Store Connect API key (`.p8`, key id and issuer id)
- [ ] Prerequisite: repo secrets `DEVELOPER_ID_P12`, `DEVELOPER_ID_P12_PASSWORD`, `NOTARY_KEY_P8`, `NOTARY_KEY_ID`, `NOTARY_ISSUER_ID` are set (REL-20)
- [ ] Actions › Release › Run workflow (branch `main`) is green, the log shows `check-dmg.sh … --notarised` ending `ok (notarised)`, and the job summary has no ad hoc warning (REL-20/21)
- [ ] On the church Mac, download the DMG of the first notarised tag in a browser (so it is quarantined) and turn Wi-Fi off before the first open (REL-23)
- [ ] Open the DMG, drag CMDMX onto Applications, eject, double-click CMDMX
- [ ] It opens after one standard confirmation (**Open**), no *Open Anyway* (REL-22)
- [ ] `spctl -a -vv /Applications/CMDMX.app` says `source=Notarized Developer ID`
- [ ] Wi-Fi on: the Local Network prompt shows "CMDMX sends DMX to Art-Net and sACN nodes on your network." (REL-18)
- [ ] Art-Net output reaches a node
- [ ] The Eurolite gives output (serial under the hardened runtime)
- [ ] The rest of REL-16 as before (Run part above)
