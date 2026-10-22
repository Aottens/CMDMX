# App Locate, Highlight, Off, Rem Dim and Clear checklist (UI-26) — VS2-05

No Swift test target exists (CLAUDE.md), so the app side of `specs/modules/ui.md` UI-26 is checked
by hand. The engine side (PRG-23..29, 38..44, ENG-10, IPC-32..34) is covered by
`cmdmx-core/tests/locate_clear.rs`, two scenarios and the daemon tests; this run checks that the
app shows and drives it. Copy to `tests/manual/runs/<date>-VS2-05.md` and tick it there.

Setup: two terminals, and a fresh shows folder so your real one stays untouched.
- `rm -rf /tmp/cmdmx-vs205`
- Daemon: `cd subprojects/engine && cargo run -p cmdmx-daemon -- --no-sacn --show-dir /tmp/cmdmx-vs205`.
- App: `cd subprojects/app && swift build && scripts/bundle-app.sh && open .build/ColourmotionDMX.app`.
- Demo show: church rig (washes 101–104, pars 201–226, front spots 301–306), demo playbacks.

Output cannot be seen without lamps: Claude listens to the daemon's Art-Net on universe 1 and
reports levels where a step says so. **[Claude]** steps Claude does by computer use and IPC;
**[Jij]** steps the owner does by hand, one per message.

## Locate (PRG-23, 24, 38; UI-26)
- [ ] [Claude] Keypad Locate and Highlight are enabled; side-column Off is enabled; softkey "Rem Dim"
      is in the Main context.
- [ ] [Jij] Tap wash 101 and 102, press **Locate**. Both buttons show `LOC`; the programmer window
      stays empty. Claude reads Dimmer full, Pan/Tilt centred.
- [ ] [Jij] Type `201 Thro 204 Locate` (Locate instead of Enter): 201–204 selected and `LOC`;
      Claude reads them at full, open white, White emitter 0.
- [ ] [Claude] Turn the Pan wheel on 101: Pan enters the programmer starting from centre; 101 keeps
      `LOC` for the rest.

## Highlight (PRG-26, 39)
- [ ] [Jij] Select 201–204, press **Highlight** (it lights). Fix +1: Claude reads 201 full and
      202–204 at the lowlight level (25 %). Fix +1 again: 202 full.
- [ ] [Jij] Press Highlight again: it goes dark, the pars return to what the programmer gives.
      Clear (deselect) with Highlight on also switches it off.

## Off (PRG-27, 40; IDEA-07)
- [ ] [Jij] Select 201–204, set colour red and White 50 %. Double-click the **name** "White" above its
      wheel: White leaves the programmer for all four, red stays. Dragging the wheel never does this.
- [ ] [Jij] Double-click the name "Hue": Colour leaves the programmer; the pars go back to open
      white (Clear setting Release).
- [ ] [Jij] Put Dimmer and Colour on 201, active group Colour, press side-column **Off**: only Dimmer
      stays. Off again: the message line says "Off: nothing of the selection is in the programmer".

## Rem Dim (PRG-28, 41)
- [ ] [Jij] Select 301–306, `@ 80 Enter`, softkey **Rem Dim**: every other fixture shows Dimmer 0 in
      the programmer; Claude reads 301–306 at 80 % and the rest at 0. With nothing selected, Rem Dim
      is refused with a message.

## Clear settings (PRG-29, 42, 43)
- [ ] [Claude] Playback Options screen, show-wide part: a Programmer section with Clear order,
      LTP on Clear (Release / Freeze) and Lowlight level, showing the defaults
      (Selection then programmer, Release, 25 %).
- [ ] [Jij] Set LTP on Clear to **Freeze**. Put 101 Pan to the far end, Clear twice: Pan stays
      where it was (Claude reads it). Set **Release**, repeat: Pan returns to its default.
- [ ] [Jij] Set Clear order to "Selection with programmer": one Clear takes both. Set it back.

## Undo (PRG-44)
- [ ] [Jij] Locate 101, then Undo: `LOC` goes, Claude reads 101 dark again. Locate, Clear twice,
      Undo: `LOC` is back. Highlight on/off adds no undo step.

## Timed Clear (ENG-10, IPC only)
- [ ] [Claude] Over IPC: 301 @ 100, `programmer.clear {"time": 3}`: Claude reads 301 falling to 0
      over about 3 s while the programmer window empties at once.
