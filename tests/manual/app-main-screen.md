# App main-screen checklist (UI, `specs/modules/ui.md`) — template

No Swift test target exists (CLAUDE.md), so the main screen is checked by hand against the
sketch `specs/ux/main-screen.html` (open it in a browser next to the app). Copy to
`tests/manual/runs/<date>-<task>.md` and tick it there. Sections grow with UX-02b–d.

Setup: daemon `rm -rf /tmp/cmdmx-main && cd subprojects/engine && cargo run -p cmdmx-daemon -- --no-sacn --show-dir /tmp/cmdmx-main` (a fresh Demo); app
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
- [ ] Two rows. Row 1: eight IPCGBES FX tabs (30 pt wide, group-hue underline), the HSI/RGB/CMY
      switch on the C tab, the wheels. Row 2: the command line with its message zone, the
      programmer summary with Undo (disabled) and Clear
- [ ] Select group 3, C tab (Hue, Saturation, White, Lime): no wheel is cut off, also in a
      1280 pt wide window (Console off, make it as small as it goes)
- [ ] Select 101: tabs I, P, C, B available, the rest greyed; the active tab is inverted; turn
      Dimmer: a red dot on the I tab
- [ ] Wheels look like rolling encoders: uppercase name, "enc 1/2/3", the value in large
      monospace (red when touched), a roller strip that scrolls while you drag, a position tick.
      At least three slots, empty ones show "—"
- [ ] Drag, scroll, Option (fine) and Shift (coarse) work as before (IPC-16); Hue shows degrees
- [ ] Programmer summary: "4 fx · I C" in red with values, "empty" in grey without; Clear glows
      red while something is selected or in the programmer
- [ ] The command line shows a `›` prompt in monospace; the message zone shows a refusal in amber
      (UX-02c replaces the text field with tokens, see below)

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

## UX-02c — side column, token command line, Mac keys

### Function buttons (UI-16)
- [ ] Two columns in Titan order: Record, Update, Edit, Select If, Delete, Unfold, Copy, Move,
      Include, Release, Patch, Disk, Help, System, Blind, Off, Fan, Options, Latch, Shape
- [ ] Record, Patch and Disk are live; every other button is greyed and its tooltip names its spec
- [ ] Select 201–204, press Record: a red ring; the softkeys header says Record; tap group cell 5
      in the Groups window: group 5 is recorded with 201–204, the ring goes out
- [ ] Record, then Clear (keypad): the ring goes out, nothing is recorded
- [ ] The Groups window has no Record button of its own any more
- [ ] Patch opens the Patch screen, Disk the Show screen

### Keypad and token command line (UI-17, UI-19)
- [ ] 30 keys in the sketch's order; Palette, Macro, Avo, View, Set, Highlight and Locate greyed
      with their spec in the tooltip
- [ ] Clicks only: `Fixture 2 0 1 Thro 2 0 4 @ 5 0 Enter`: the line shows the tokens
      `Fixture 201 Thro 204 @ 50` while typing, Enter sends it, 201–204 go to 50 % and the line
      empties
- [ ] Digits join into one number (2, 0, 1 → `201`); Back removes the last digit, then the last
      token; Exit empties the line
- [ ] `Group 3 Enter` selects group 3; `2 0 1 And 3 0 1 Enter` selects both
- [ ] A refused line (`Thro Enter`) shows the warning in amber and the line empties
- [ ] Clear glows red while something is selected or in the programmer; it clears (PRG-29)
- [ ] Fix −1 / Fix +1 step through the selection, Odd/Even keeps every other fixture
- [ ] The cursor blinks after the tokens (not with Reduce Motion on in macOS Accessibility)
- [ ] The Keypad toggle hides and shows the keypad; on a window narrower than 1440 pt it hides
      by itself

### Softkeys (UI-18)
- [ ] Nothing selected: header "Softkeys · Main"; Edit Times, Playback Options, Set Legend,
      Shapes & Effects, Timecode, Lock and Undo all greyed with their spec
- [ ] Select something: header "Selection"; Next, Previous, All and Odd/Even work; Set Legend and
      Playback Options greyed
- [ ] Record armed: header "Record"; only Cancel works (it disarms); Merge/Replace, Record by and
      Mask greyed

### Mac keys (UI-19) and history (UI-27)
- [ ] Keys off (default the first time): typing on the Mac keyboard does nothing to the command
      line; ⌘1–⌘4 still switch workspaces
- [ ] Keys on: `201-204@50` + Return selects 201–204 at 50 %; `f` = Fixture, `g` = Group,
      `+` = And, Esc empties the line, Backspace = Back; F1–F4 switch workspaces
- [ ] Keys stays on after quitting and reopening the app
- [ ] With Keys on, typing in the Patch screen's search field or the Show screen's name fields
      types there, not on the command line
- [ ] Send three lines, then Up: the last line comes back; Up again: the one before; Down: forward
      again; Down past the newest: an empty line
- [ ] Recall a line with Up, change the level with Backspace and digits, Enter: the edited line is
      sent and is the newest history entry; sending the same line twice adds it once
- [ ] Quit and reopen the app: Up still recalls the lines from before

### One grey (UI-28)
- [ ] Rail entries (Macros, Shapes, …), the status bar toggles, function buttons, keypad keys,
      softkeys, unavailable attribute tabs and Undo all look equally dim when disabled (empty
      palette cells keep their own look)
