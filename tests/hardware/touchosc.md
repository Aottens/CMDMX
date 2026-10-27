# TouchOSC checklist — the shipped layout on the church iPad (VS4-04, CTL-36, 37, 60..66)

Gate 3: only the owner ticks these. Copy to `tests/hardware/runs/<date>-VS4-04.md` for a run.

Setup: church iPad mini (1st gen, iOS 9.3.5, TouchOSC Mk1 1.9.8) and the Mac on the same wifi.
Console running with the church show (a copy in `--show-dir /tmp/…`, never the Demo show on the
real rig). Control screen: OSC **enabled**, port 8000, feedback port 9000.

**Before you change anything, write down the iPad's current TouchOSC settings** (they drive
Lightkey on Sundays):
- OSC: host ________ (expected 192.168.178.36), outgoing port ________ (21600), incoming port ________
- MIDI Bridge: on / off, host ________
- Layout in use: ________

Mac IP for this run: ________ (2026-10-25: 192.168.178.221). iPad IP: ________ (192.168.178.27).

## Load the layout (CTL-65)
- [ ] On the Mac: `touchosc/serve.sh` runs and says it serves `CMDMX.touchosc`
- [ ] iPad: TouchOSC → Layout → Add: the Mac is listed under **Found Hosts**; tap it: the layout
      `CMDMX` downloads; tick it in the Layout list and tap Done: it opens
- [ ] Only if the line above failed: TouchOSC Editor on the Mac, open
      `touchosc/CMDMX.touchosc`, press Sync, then Add on the iPad as above. Which way
      worked: serve.sh / Editor
- [ ] Ctrl-C stops `serve.sh`

## Connect (CTL-35, 37)
- [ ] iPad: TouchOSC → OSC: host = the Mac's IP, outgoing port 8000, incoming port 9000; MIDI Bridge off
- [ ] Back in the layout, it fills in at once: fader legends of the current page, page number and
      legend, cue texts, faders at the console's levels (the full state on first contact, CTL-37)
- [ ] The Control screen lists the iPad as a client heard

## Layout (CTL-60..63)
- [ ] Landscape, nothing cut off or overlapping; one strip per fader of the page (count ____ =
      the show's `faders_per_page`)
- [ ] Fader 1..N move playbacks 1..N of the current page on the console and the lamps
- [ ] Upper button of fader 1 → the handle's upper button (Flash/Swop/Go as set); it lights while on;
      lower button likewise
- [ ] Page ▶ / Page ◀ change the console page: the iPad faders jump to the new page's levels and
      the legends and page label are rewritten (CTL-21, 37)
- [ ] The page number label shows the number (if Mk1 shows nothing there, note it: ________)
- [ ] A page change on the Mac screen updates the iPad too
- [ ] GO runs the next cue of the connected list; current and next cue texts change (`"<number> <legend>"`)
- [ ] From the operator's seat the current cue is the large green label and the next cue the small
      grey `Next:` line under it; you cannot mix them up (CTL-63, VS4-06)
- [ ] BACK goes back one cue; STOP stops the list
- [ ] DBO tap → blackout, button lit; tap again → light back, button off; DBO on the Mac screen
      lights/unlights the iPad button
- [ ] GM fader dims everything; GM moved on the Mac moves the iPad fader
- [ ] A fader moved on the Mac (or the BCF2000) moves the same fader on the iPad
- [ ] Message line: cause a warning (e.g. unplug the DMX interface briefly) → it shows on the iPad

## Restore the iPad (always, also after a failed run)
- [ ] TouchOSC OSC settings back to the values written down above (host 192.168.178.36, port 21600, …)
- [ ] MIDI Bridge back as written down
- [ ] The old layout selected again; Lightkey answers the iPad as before
- [ ] OSC disabled again in the console if this Mac is not used on Sunday

## Open points
- none
