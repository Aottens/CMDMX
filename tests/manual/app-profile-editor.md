# App profile editor checklist (LIB-23..27, LIB-39, IPC-37..39, IPC-46) — template

No Swift test target exists (CLAUDE.md), so the profile editor of `specs/modules/fixture-library.md`
LIB-23..27 is checked by hand. The engine side (LIB-25, IPC-37..39) is unit tested (VS2-24).
Copy to `tests/manual/runs/<date>-<task>.md` and tick it there; tag each step [Claude] or [Jij].

Setup, part A works on copies so nothing real changes:

```
rm -rf /tmp/cmdmx-pe && mkdir -p /tmp/cmdmx-pe/shows /tmp/cmdmx-pe/profiles
cp ~/Documents/CMDMX/Shows/Kerk.cmdmx.json /tmp/cmdmx-pe/shows/
cp ~/Library/Application\ Support/CMDMX/profiles/*.json /tmp/cmdmx-pe/profiles/
cd subprojects/engine && cargo run -p cmdmx-daemon -- --no-sacn --no-serial \
  --show-dir /tmp/cmdmx-pe/shows --profiles-dir /tmp/cmdmx-pe/profiles
```

Open *Kerk* on the Show screen if the Demo opens. App: `cd subprojects/app && swift build &&
scripts/bundle-app.sh && open .build/CMDMX.app`. Art-Net listener on universe 1 for the
output steps (Phantom 25 spots 201–204 at 1.109, 1.120, 1.131, 1.142; Pan +0/+2, Tilt +1/+3, Colour
Wheel +5, Gobo +8, so 201's Colour Wheel is channel 114).

## A. Opening and leaving (LIB-23)
- [ ] Add panel: Edit profile… is disabled without a pick; pick Showtec Phantom 25 LED Spot
      (`USER`) and click it: a full-screen "Patch › Profile editor" with "Showtec · Phantom 25 LED
      Spot", the mode picker on 11 Channel, Test, Cancel and Save
- [ ] Manufacturer and model cannot be edited; the mode picker lists the profile's modes
- [ ] Cancel without changes returns to the Patch screen at once
- [ ] Select 201 in the patch table: the Selected panel has Edit profile…; it opens the show's copy on
      the fixture's mode (11 Channel)
- [ ] Pick a QLC+ model that is not in the show (Showtec LED Par 56): Edit profile… opens it from the
      library; its mode picker starts on the mode picked in the Mode column

## B. Channels (LIB-23)
- [ ] The channel list shows Ch, Attribute, Grp, Kind, Default (%) and Slots; Pan reads 1 with
      its fine on 3 ("fine of Pan", no fields of its own)
- [ ] Select Gobo: the right side shows Name, Group (I P C G B E S FX), Kind HTP/LTP, Default,
      Locate and Highlight as % with the DMX value beside it (50 % → 128)
- [ ] On the Showtec LED Par 56 (6 Channel): Red/Green/Blue show their role (colour component) and
      only the name can change
- [ ] + adds "Channel 12" (S, LTP, 0) at the end; ▲▼ move it and the Ch column renumbers;
      − removes it
- [ ] Change something, then Cancel: one question "Discard changes?"; No stays, Yes leaves

## C. Slots (LIB-24)
- [ ] Colour Wheel has no slots: + Slot makes one slot 0–255; the bar under the table shows it
- [ ] Split: two halves 0–127 and 128–255, the lower keeps name and colour; Split again on the
      upper half gives 128–191 and 192–255
- [ ] Clicking a range in the bar selects its row
- [ ] Merge with next on 128–191: it becomes 128–255, keeps its name and colour
- [ ] Remove the middle slot: the slot below grows over its values; remove the first one: the next
      one starts at 0
- [ ] Type From/To by hand to leave a gap: allowed, the bar shows the gap; the rows stay sorted by
      From
- [ ] Group C channel: a colour well per slot (sRGB); a gobo (group G) has no colour column
- [ ] Pan fine has no slots

## D. Save (LIB-25, IPC-38)
- [ ] Set Pan and Tilt default 50 % on the Phantom 25 and Save: the editor closes or shows
      "Saved"; Art-Net shows Pan and Tilt (and their fine bytes) at 128/0 on 201–204 (109–112 on 201)
- [ ] ⌘Z on the main screen: the message says "Undone: Edit profile Phantom 25 LED Spot" and Pan/Tilt
      go back to 0; ⌘⇧Z brings them back
- [ ] Make two slots overlap (0–100 and 90–255) and Save: refused with the engine's warning, the
      editor stays open with the edits
- [ ] Remove Gobo, after recording it in a palette or cue on 201 (or with it in the programmer):
      Save is refused with a warning naming where it is used
- [ ] Add a channel to the Phantom 25's 11 Channel mode (footprint 12) and Save: refused (a patched
      mode keeps its footprint)
- [ ] Edit the Showtec LED Par 56 (not in the show) and Save: the Add panel lists it with `USER`
      afterwards, and `/tmp/cmdmx-pe/profiles/Showtec - LED Par 56.json` exists

## E. New from channel list (LIB-26)
- [ ] New from channel list… opens the editor with Manufacturer, Model and Mode name editable and no
      channels
- [ ] + offers the LIB-11 types; Emitter asks a name ("Royal Blue"), Other a name, group and default
- [ ] Build `Test / RGBL Par / 4 Channel`: Red, Green, Blue, Lime; Save: the Add panel picks
      Test RGBL Par with mode 4 Channel, `USER`, 4 ch
- [ ] Saving with an empty model or no channels is refused (by the engine, the editor stays open)

## F. Test (LIB-27, IPC-39)
- [ ] With nothing selected in the programmer, Test is disabled
- [ ] On Program select 201 Thru 204, back in the editor: Test reads "Test on 201–204"; turn it on:
      it is red
- [ ] Click a Colour Wheel slot 32–63: Art-Net Colour Wheel of 201–204 (114 on 201) reads 47 ((32 + 63) / 2)
- [ ] Move Gobo's Default slider: the Gobo channel (117 on 201) follows live
- [ ] Leave the editor: the values stay in the programmer (Program shows them); Clear removes them

## H. Auto-mix emitters (LIB-39, IPC-46)
On the demo show (no `--show-dir`), Art-Net on universe 1; RGBW pars 201–208 at 45, 50, … (Dimmer, R G B, White).
- [ ] Open the RGBW Par's editor (201's Selected panel): the mode bar has **Auto-mix emitters**, off;
      the Phantom 25 (no colour mixing) and a CMY mode have no such switch
- [ ] Switch it on: White gets CIE x 0.3127, y 0.3290 (LIB-37 default) and the status "Auto-mixed";
      Output reads 100 %
- [ ] Clear CIE y only: shown as invalid and Save is refused; fill it again
- [ ] Type CIE x 0.17, y 0.005: the status reads "Direct only — outside the reference gamut"; put
      0.3127 / 0.3290 back
- [ ] On an RGB+Lime type, switching on fills Lime with 0.4100 / 0.5500; an emitter named UV gets
      nothing and reads "Direct only — no colour"
- [ ] Save, then on Program: 201 `@ 100` white puts the white on channel 49 and R G B low (see
      `app-programmer.md`, Kelvin section); ⌘Z undoes "Edit profile RGBW Par"
- [ ] Set Output 200 % and Save: White drops to half for the same white

## G. Church Phantom 25 (owner, on the real show)
Stop the daemon, start it without `--show-dir`/`--profiles-dir` (Kerk, real profiles folder) and
Art-Net only. Have the Phantom 25 manual at hand.
- [ ] [Jij] Colour Wheel: one slot per colour with its name and colour, from the manual's DMX table;
      check each with Test on 201–204 (lamps or Art-Net)
- [ ] [Jij] Gobo: one slot per gobo (and shake/rotate ranges) from the manual; check with Test
- [ ] Save: no warning; `~/Library/Application Support/CMDMX/profiles/Showtec - Phantom 25 LED Spot.json`
      has the slots; the show autosaves (status bar)
- [ ] Reopen the editor from 201's Selected panel: the slots are there
