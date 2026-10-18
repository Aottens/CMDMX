# App palettes checklist (PAL-25..31) — template

No Swift test target exists (CLAUDE.md), so the palette windows, function buttons and softkeys of
`specs/modules/palettes.md` PAL-28..30 are checked by hand. Copy to
`tests/manual/runs/<date>-<task>.md` and tick it there.

Setup: two terminals, and a fresh shows folder so your real one stays untouched.
- `rm -rf /tmp/cmdmx-pal`
- Daemon: `cd subprojects/engine && cargo run -p cmdmx-daemon -- --no-sacn --show-dir /tmp/cmdmx-pal`.
- App: `cd subprojects/app && swift build && scripts/bundle-app.sh && open .build/ColourmotionDMX.app`.
- Workspace F1 (Groups + Palettes) shows Groups, Colours, Positions and Gobos & Beams.

Optional: an Art-Net monitor on universe 1 (demo rig: washes 101–104, RGBW pars 201–208,
RGB+Lime pars 211–216, RGB pars 221–226, front spots 301–306).

## Windows (PAL-28)
- [ ] Colours, Positions and Gobos & Beams each show at least 36 empty cells (rows of 6), with no
      fixed numbers on the empty cells, and the title row says "mask C · 0 palettes" (P, IGBES).
- [ ] No palette button is disabled with "P1-03" any more; Edit stays disabled with "phase 2".

## Recording (PAL-01..06, 29)
- [ ] Select group "Pars RGBW", set a red with the colour picker. Press Record (red ring), tap
      Colours cell 1.
      - The cell shows number 1, tag G, a red swatch, the legend "Colour 1" and a C in the mask strip.
      - Record is no longer armed.
- [ ] Select 211–216 and make them blue. Double-click Colours cell 2 without Record (Quick Record):
      palette 2 appears, blue swatch.
- [ ] Select 101 and 102, turn Pan to two different values. Record, tap Positions cell 1: tag N,
      a position icon with two points, a P in the mask strip.
- [ ] Select 101–104, set Zoom. Record, tap Gobos & Beams cell 1: tag S, the icon reads "ZOOM …%".
- [ ] Record, tap Colours cell 1 again: the softkeys show Replace, Merge, Quick Merge, Cancel.
      Cancel leaves palette 1 as it was. Record, tap cell 1, tap cell 1 again: Quick Merge.
- [ ] In the Record softkeys, Mask lets you toggle IPCGBES (starting at C for Colours), Record By
      switches Channel / Mixed, Record as switches Auto / Normal / Shared.
- [ ] Clear the programmer (Clear twice). Record, tap an empty cell: refused with a message on the
      command line ("Nothing in the programmer for Colours (mask C)").

## Applying (PAL-08..10, 13..15)
- [ ] Select 201–208 and tap palette 2 (blue): the pars turn blue; the Colours cell 2 gets the red
      top strip (in use); the Programmer window shows the colour in cyan with "Colour 2".
- [ ] Select only the spots 301–306: the colour palettes dim to 38 % (not relevant).
- [ ] Clear twice (nothing selected). Tap palette 1: every colour fixture turns red (quick
      palette), and nothing gets selected.
- [ ] Select 201–208, type `3` on the keypad, tap palette 2: the pars fade to blue over 3 s and
      the command line is empty again.
- [ ] Press Palette with an empty command line: the softkeys show Master Time, Master Overlap, Set
      Overlap, Cancel. Type `2`, Master Time; type `50`, Master Overlap. Select 201–208 and tap
      palette 1: the pars go red one after the other, the whole run about 9 s.
- [ ] Set Master Time and Master Overlap back (`0` Master Time, `100` Master Overlap).
- [ ] Select 201, type on the command line `Palette 2 Enter` (also P with Keys on): 201 turns blue.

## Managing (PAL-12, 17, 18, 20, 22, 29)
- [ ] Set Legend (softkey), tap palette 1: an editor with a text field, the number, an icon grid
      by category, and Auto. Type "Rood", pick an icon: the cell shows "Rood" and the icon. Pick
      Auto again: the red swatch is back.
- [ ] In the same editor, change the number to 30: the cell shows 30; `Palette 30 Enter` works.
- [ ] Copy, tap palette 30, tap an empty Positions cell: a copy with the next free number.
- [ ] Move, tap that copy, tap an empty Gobos & Beams cell: it moves, same number.
- [ ] Include, tap palette 2 with 201 selected: 201's colour shows red (not cyan) in the
      Programmer window.
- [ ] Clear twice, select 211, give it green with the picker. Update, tap palette 2: 211–216
      (the whole RGB+Lime type) take the green when palette 2 is applied to them.
- [ ] Delete, tap the copy: softkey A asks to confirm; confirm: the cell is empty.

## Restart (PAL-23)
- [ ] Stop the daemon (Ctrl+C) and start it again with the same command. The app reconnects and
      every palette is back with its number, cell, legend and icon.
- [ ] `/tmp/cmdmx-pal/Demo.cmdmx.json` has `"schema_version": 2` and a `palettes` key.
