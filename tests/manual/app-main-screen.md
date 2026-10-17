# App main-screen checklist (UI, `specs/modules/ui.md`) — template

No Swift test target exists (CLAUDE.md), so the main screen is checked by hand against the
sketch `specs/ux/main-screen.html` (open it in a browser next to the app). Copy to
`tests/manual/runs/<date>-<task>.md` and tick it there. Sections grow with UX-02b–d.

Setup: daemon `cd subprojects/engine && cargo run -p cmdmx-daemon -- --no-sacn`; app
`cd subprojects/app && swift build && scripts/bundle-app.sh && open .build/ColourmotionDMX.app`.

## UX-02a — the shell

### Console mode and frame (UI-01, UI-26)
- [ ] The app opens full screen: no Dock, no menu bar
- [ ] ⌘Q asks "Quit the app? DMX keeps running in the engine." — Cancel keeps the app open,
      the Art-Net output never stops either way
- [ ] Console off (status bar): a normal window. Resizing it only grows or shrinks the window
      grid; the bars keep their size. It does not get smaller than 1280 × 800
- [ ] Quit and reopen: the app comes back in the mode you left it in
- [ ] The six zones sit where the sketch has them: status bar, rail, 2×2 grid with the side column
      to its right, attribute bar, playback area. Flush, hairlines, no outer margins

### Theme and type (UI-02..05)
- [ ] Dark theme matches the sketch: dark grey, not black; no colour that does not mean a state
- [ ] Day/Night in the status bar switches to the day theme and back; it is remembered
- [ ] Legends are condensed, every number is monospaced (a fader level does not jitter while it
      changes)
- [ ] Fixture and group cells: 8 per row, number top-left; selected = blue border and blue number;
      in programmer = red top strip; the fill never changes

### Status bar (UI-06, IPC-20)
- [ ] Left: "Demo show", engine dot green, a dot per output node: Art-Net green, serial amber
      with "no interface" (no Eurolite on the desk); no sACN dot because of `--no-sacn`
- [ ] Restart the daemon without `--no-sacn`: an sACN dot appears, green
- [ ] Stop the daemon: the engine dot goes grey with an amber line in the bar (no big banner);
      start it again: green, the screen rebuilds
- [ ] Right: Console, Keys, Keypad, Day/Night toggles and a running clock

### Rail, workspaces and windows (UI-07..13)
- [ ] The rail shows four workspaces with F1–F4, then Patch, Macros (disabled), Show (disabled),
      then Shapes, Pixelmapper, Keystone disabled ("later")
- [ ] Each workspace shows its four windows: 1 Groups / Colours / Positions / Gobos & Beams,
      2 Fixtures / Groups / Colours / Programmer, 3 Fixtures / Colour picker / Colours /
      Programmer, 4 Cue list / Active playbacks / Fixtures / Programmer
- [ ] Select some fixtures, switch workspaces: the selection stays; a scrolled window keeps its
      scroll
- [ ] Fixtures: cells in handle order with user numbers; a tap selects (blue, order number
      top-right); a fixture with programmer values has the red strip
- [ ] Groups: 24 cells, groups 1–4 with their fixture counts; a tap selects the group
- [ ] Colours / Positions / Gobos & Beams: 32 numbered, disabled cells (1–32, 41–72, 81–112),
      info "Palettes · P1-03"; Cue list and Active playbacks: empty tables with their spec
- [ ] Programmer and Colour picker windows are there (filled in UX-02b)
- [ ] Patch (rail) opens the Patch screen in the grid's place; the playback area and status bar
      stay; the faders still move channels 1–8 while it is open (UI-10)

### Disabled zones (UX-02c/d fill them)
- [ ] The side column, attribute bar and playback area are there in the sketch's layout; what
      does not work yet is disabled and its tooltip names the spec it waits for
- [ ] Drag a fader and the GM up and down quickly: the bar follows the pointer without lagging
      or bouncing, and on release it stays where you let go (UI-22)
- [ ] Nothing that worked before is lost: faders 1–8, GM, the attribute bar wheels and command
      line, Record + group still work (`tests/manual/app-programmer.md` spot checks)

## UX-02b — attribute bar, Programmer window, colour picker

### Attribute bar (UI-20)
- [ ] Left to right as in the sketch: eight IPCGBES FX tabs (30 pt wide, group-hue underline),
      the HSI/RGB/CMY switch on the C tab, the wheels, the command line with its message zone,
      the programmer summary with Undo (disabled) and Clear
- [ ] Select 101: tabs I, P, C, B available, the rest greyed; the active tab is inverted; turn
      Dimmer: a red dot on the I tab
- [ ] Wheels look like rolling encoders: uppercase name, "enc 1/2/3", the value in large
      monospace (red when touched), a roller strip that scrolls while you drag, a position tick.
      At least three slots, empty ones show "—"
- [ ] Drag, scroll, Option (fine) and Shift (coarse) work as before (IPC-16); Hue shows degrees
- [ ] Programmer summary: "4 fx · I C" in red with values, "empty" in grey without; Clear glows
      red while something is selected or in the programmer
- [ ] The command line shows a `›` prompt in monospace; typing + Return still works; the message
      zone shows a refusal in amber (`201 Banana`)

### Programmer window (UI-14, IPC-21)
- [ ] Workspace 2: empty programmer shows the note; select 201–204 `@ 50`, then give 201 a blue
      colour: rows 201–204 with # and fixture, I column "50 %" in red, C column a blue swatch
      and its hex for 201, "·" where a fixture lacks an attribute (e.g. P for the pars)
- [ ] Clear twice: the table empties again

### Colour picker (UI-15, IPC-22, IPC-23)
- [ ] Workspace 3: with no colour fixture selected the picker shows its note
- [ ] Select group 3 (pars): a hue/saturation field with a marker at the first target's colour
      (top = full saturation, bottom = white); next to it bars for the current model (HSI: H, S;
      RGB: R G B; CMY: C M Y) plus White and Lime
- [ ] Click in the field on blue: all pars go blue on Art-Net, the marker jumps there, the wheels
      follow; drag around: the colour follows the pointer
- [ ] Drag the White bar: only the RGBW pars' white channel moves (PRG-34)
- [ ] The gel row is there but disabled (IDEA-02)
