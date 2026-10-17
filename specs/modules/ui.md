# Module spec: main screen (UI)

Owner: `subprojects/app` (rendering, input), with four small IPC additions in `cmdmx-daemon`.
Status: **draft for owner approval (UX-02.0, 2026-10-17)**.
Behavioural and visual reference: UI v0.1, `specs/ux/main-screen.html` (DECISIONS 2026-10-17),
background in `specs/ux/research.md` ("Design brief for iteration 2"). Titan is the reference for
names and behaviour.

## Purpose
Move the app from its plain working layout into the real main screen of UI v0.1, zone by zone
(DECISIONS 2026-10-17, "Work towards the real UI early"). Every zone of the sketch is on screen
from the start. A control whose engine part does not exist yet is shown **disabled**, and its
tooltip names the spec it waits for (e.g. "Palettes · P1-03"). New engine features then land
straight in their final place.

The app stays a thin client: everything it shows comes from the snapshot and deltas, and every
control sends an existing command (CLAUDE.md). Nothing in this spec adds lighting logic to Swift.

## Definitions
- **Zone:** one fixed area of the main screen: status bar, workspace rail, window grid, side
  column (function buttons, keypad, softkeys), attribute bar, playback area.
- **Workspace:** a named preset of four windows for the 2×2 grid.
- **Screen:** a full-size view that replaces the window grid, side column and attribute bar
  (Patch now; Show, Macros later). The status bar, rail and playback area stay.
- **Phase marks:** **[now]** is built in UX-02a–d on the current IPC (plus IPC-20..23 below).
  **[with X]** is built when spec or task X lands; until then it is a disabled control.

## Acceptance criteria

### Frame and theme (UX-02a)
- **UI-01 [now]** One main window with the six zones in the sketch's arrangement: status bar on
  top (26 pt), rail on the left (78 pt), the 2×2 window grid in the centre with the side column to
  its right, the attribute bar (64 pt) and the playback area (232 pt) along the bottom. Zones
  are flush, separated by hairlines, with no outer margins. Resizing the window only resizes the
  window grid. The other zones keep their size, and content never reflows into another zone. The
  minimum window size is 1280 × 800 pt.
- **UI-02 [now]** Dark theme from the sketch's tokens: background #121314, surface #1A1C1E /
  #222427, hairlines #2A2C2F / #3A3D41, text #D8DADC / #8A8E93 / #55595E. There is a Day theme
  with the sketch's day tokens, toggled in the status bar. Both themes are the app's own and do
  not follow the macOS appearance. The choice is remembered across launches.
- **UI-03 [now]** Colour means state only, everywhere: red = programmer or unstored or Record
  armed, blue = selection, green = running or OK, amber = fading or warning, cyan = from palette,
  violet = Blind. Attribute-group hues (sketch `--gI`…`--gFX`) are used only for tags and tab
  underlines. No other accent colours exist.
- **UI-04 [now]** Type: SF Pro condensed (SwiftUI `.width(.condensed)`) for legends and titles,
  SF Pro for UI text, SF Mono with tabular digits for every number and for the command line.
  Three sizes only: 10–11, 12–13 and 15–20 pt. Tags are uppercase with tracking.
- **UI-05 [now]** One button system for fixture, group and palette cells, as in the sketch:
  - The cell is 54 pt high, on a fixed 8-column grid with 2 pt gaps.
  - The number sits top-left and a tag top-right; the legend is condensed, two lines at most.
  - Empty = outline only with a faint number. Selected = 2 pt blue inner border and a blue
    number. In programmer = 3 pt red top strip. Not relevant = 38 % opacity.
  - State never changes the fill.

### Status bar (UX-02a)
- **UI-06 [now]** Left to right:
  - The show name, "Demo show" until the show file exists [with P1-09].
  - The engine link: green while connected, grey while offline. The offline banner of IPC-08
    becomes this indicator plus a one-line amber message in the bar.
  - One dot per output node with its name and state (IPC-20).
  - The autosave age [with P1-09], and the BLIND and DBO tags [with their specs].
  - On the right: the toggles Keys, Keypad and Day/Night, and the clock (SF Mono, hh:mm:ss).

### Workspace rail and window grid (UX-02a)
- **UI-07 [now]** The rail lists four workspaces: Groups + Palettes, Fixtures + Groups, Colour,
  and Playbacks. Each one shows its F-key (F1–F4), and the active one is highlighted. Below a
  divider come the screens Patch, Macros [later] and Show [with P1-09], then Shapes,
  Pixelmapper and Keystone, disabled and marked "later". F1–F4 switch workspaces while Keys is
  live (UI-19).
- **UI-08 [now]** Each workspace fills the 2×2 grid with four fixed windows, as in the sketch:
  1. Groups, Colours, Positions, Gobos & Beams.
  2. Fixtures, Groups, Colours, Programmer.
  3. Fixtures, Colour picker, Colours, Programmer.
  4. Cue list, Active playbacks, Fixtures, Programmer.

  Switching workspaces never changes show state, and every window keeps its scroll position.
- **UI-09 [now]** A window has a 22 pt title row, with the uppercase title on the left and info
  in SF Mono on the right (e.g. "3 selected · order shown"). Its content scrolls inside the
  window and never resizes the window.
- **UI-10 [now]** Patch (rail) opens the Patch screen of PAT-10 in the place of the grid, side
  column and attribute bar. The playback area and the status bar stay, so the faders and GM stay
  reachable while patching. Its layout follows the VS2-12 run: the panels are on the left of the
  table.

### Windows (UX-02a; Programmer and Colour picker in UX-02b)
- **UI-11 [now]** Fixtures: one cell per fixture, in handle order (PAT-14). There are 8 cells per
  row and `handles_per_page` cells per handle page, with page tabs in the title row when there is
  more than one page.
  - The cell shows the user number, the selection order top-right in blue, and the fixture's
    name, or else its profile's model, as the legend.
  - Selected, current target (a double blue border during a sub-selection) and in programmer
    follow IPC-15 and UI-05.
  - A tap sends `programmer.select`.
  - The output lamp strip along the bottom edge is a disabled stripe [with IDEA-10,
    open question 3].
- **UI-12 [now]** Groups: one cell per group with its number, its fixture count as the tag, and a
  legend ("Group n" until groups have names [with Set Legend]). There are 24 cells, and empty
  ones show only a number.
  - A tap selects the group.
  - Recording a group moves from the window's Record button to the Record function button
    (UI-16).
  - The IPCGBES mask strip is drawn empty [later; it needs the groups' attribute groups over
    IPC].
- **UI-13 [now]** The Colours, Positions and Gobos & Beams windows show their 32 empty, numbered,
  disabled cells (Colours 1–32, Positions 41–72, Gobos & Beams 81–112, as in the sketch) and the
  info "Palettes · P1-03". Cue list and Active playbacks show the sketch's column headers, an
  empty table and the info "Cue lists · P1-08" or "Playbacks · P1-04".
- **UI-14 [now, UX-02b]** Programmer: a table with one row per fixture that has programmer values
  (IPC-21): # (user number), Fixture (name or model), I, P, C, B.
  - A touched value is red. A Colour value shows a swatch plus its hex. An attribute the fixture
    lacks shows "·".
  - Palette references in cyan [with P1-03].
  - When the programmer is empty, the window shows the sketch's one-line note.
- **UI-15 [now, UX-02b]** Colour picker:
  - A hue/saturation field with a marker at the first target's colour (IPC-22). Clicking or
    dragging in it sends `programmer.colour` with the hue and saturation under the pointer
    (IPC-23).
  - Next to the field, bars for the current colour model's components plus one per emitter the
    targets have (from `wheels`). Each bar is draggable and sends `programmer.wheel` on its
    attribute.
  - The gel book is a disabled row of swatches [with IDEA-02].
  - With no colour-mixing target, the window shows the sketch's note.

  The picker never computes a colour itself. It sends the hue and saturation under the
  pointer, sends wheel steps for the bars, and draws the values the engine reports.

### Side column (UX-02c)
- **UI-16 [now]** Function buttons, in two columns and in the sketch's Titan order: Record,
  Update, Edit, Select If, Delete, Unfold, Copy, Move, Include, Release, Patch, Disk, Help,
  System, Blind, Off, Fan, Options, Latch, Shape.
  - Live now: **Record**, which arms with a red ring; the next group tap sends `group.record`
    and Clear disarms it. **Patch** opens the Patch screen.
  - Every other button is disabled with its spec in the tooltip: Update, Edit, Delete, Copy,
    Move and Include with P1-03; Release, Options and Latch with P1-04; Off with VS2-05; Disk
    with P1-09; Help and System later; Select If, Unfold, Blind, Fan and Shape "later" as in the
    sketch.
- **UI-17 [now]** The keypad has the sketch's 30 keys in its order. Keys that work now:
  - The digits, `.`, Fixture, Group, Thro, And and `@`, which add a token to the command line.
  - Enter, which sends the tokens as `programmer.command`.
  - Back, which removes the last token or digit, and Exit, which empties the line.
  - Clear (`programmer.clear`), which glows red while the selection or programmer is non-empty.
  - Fix −1 / Fix +1 (`programmer.step`) and Odd/Even (`programmer.pattern` 2).

  Disabled: Palette (P1-03), Macro (later), Avo and View (later), Set (with Set Legend),
  Highlight and Locate (VS2-05).

  The Keypad toggle in the status bar hides the keypad, and it is hidden automatically on a
  window narrower than 1440 pt. The sketch hides it below 1280 px, but with the 1280 pt
  minimum window of UI-01 the threshold has to be wider.
- **UI-18 [now]** Softkeys A–G in the sketch's arrow shape, with the context name in the
  header.
  - Main context: Edit Times, Playback Options and Set Legend are disabled with their specs;
    Shapes & Effects (phase 2), Timecode (phase 3) and Lock (later) are disabled; Undo is
    disabled until P1-10.
  - Selection context (when something is selected): Next and Previous (Fix ±1), All
    (`programmer.all`) and Odd/Even work; Set Legend and Playback Options are disabled.
  - Record context (Record armed): only Cancel works; Merge/Replace, Record by and Mask are
    disabled with P1-03.

  A right-click on a cell is reserved for its context softkeys [with P1-03/P1-04].

### Command line and Mac keys (UX-02c; the field itself in UX-02b)
- **UI-19 [now]** The command line in the attribute bar shows tokens, not a free text field: the
  prompt `›`, the tokens and a blinking cursor (no blinking under Reduce Motion). The message
  zone below it shows the latest warning or error in amber (IPC-17), cleared by the next command.
  While **Keys** is live (status bar toggle, off by default), the Mac keyboard drives the keypad:
  - The digits and `.`.
  - `-` = Thro, `+` = And, `@`, Return = Enter, Esc = Exit, Backspace = Back.
  - F = Fixture, G = Group, P = Palette (disabled until P1-03).
  - F1–F4 = workspaces.

  Space = Go comes with P1-08. Keys never fire while a text field (Patch, legends) has focus.

### Attribute bar (UX-02b)
- **UI-20 [now]** Left to right:
  - **Tabs:** eight IPCGBES FX tabs, 30 × 34 pt, each with its group hue underline. The active
    tab is inverted, available tabs have normal text, unavailable ones are disabled, and a red
    dot means the group has programmer values (IPC-11).
  - **Wheels:** a rolling encoder per `wheels` entry, at least three slots, with "enc n" labels.
    Each shows the uppercase name and the value (SF Mono 15 pt, red when touched), and a roller
    strip that scrolls with the value plus a position tick.
  - **Colour model:** on the C tab, the HSI / RGB / CMY switch sits left of the wheels (IPC-19).
  - **Command line:** the command line and message zone (UI-19).
  - **Programmer summary:** "n fx · I P C" in red (from `in_programmer` and `attribute_groups`),
    "empty" in grey, with Undo (disabled until P1-10) and Clear.

  Dragging and scrolling a wheel keep IPC-16's steps and modifiers. Values stay in % (Hue in
  degrees); Pan and Tilt in degrees come with the profile's ranges [later].

### Playback area (UX-02d, built with VS1-06)
- **UI-21 [now]** Pages column: page buttons with their number (legends [with P1-04]), plus a
  "per page" read-out of `faders_per_page`. Paging itself comes with VS1-06 in the same task.
- **UI-22 [now]** `faders_per_page` playback strips, each 92 pt wide:
  - From top to bottom: a legend (playback number; name and halo with P1-04), a level bar with
    its % in SF Mono and a draggable cap (`fader.set`), then Swop and Flash, disabled until
    P1-04.
  - An active playback has a green strip along the bottom edge.
  - The first eight strips sit in a framed bank labelled "BCF2000 · faders 1–8", mirroring the
    hardware bank (research §5).
  - Scrolling over a strip moves its level (Option fine, Shift coarse).
- **UI-23 [now]** Go block (210 pt): the header, the "now" line and the big green-outlined GO
  button with Back / Stop / Release are drawn disabled with "Cue lists · P1-08". It is never
  covered by anything.
- **UI-24 [now]** Grand Master (70 pt): a level bar like a strip (`grand_master.set`), turning
  amber below 100 %, with DBO under it disabled [with its engine criterion, P1-04].

### IPC additions (daemon, UX-02a/b; extend `ipc.md` when approved)
- **IPC-20** The snapshot gains `"outputs":[{"name":"art-net","state":"Ok"}]` with one entry per
  output node (`art-net`, `sacn`, `serial`; state `Ok`, `Degraded` or `Stopped`; OUT-11/12). An
  `outputs` delta is sent when one changes, at most once per tick. Serial with no interface
  found is `Degraded` with `"detail":"no interface"`.
- **IPC-21** The programmer state (IPC-11) gains
  `"contents":[{"fixture":201,"values":{"Dimmer":0.5,"Colour":{"r":1.0,"g":0.2,"b":0.0}}}]`,
  sorted by fixture id. It holds programmer values only, levels as 0.0–1.0 and colours as sRGB
  0.0–1.0. Each attribute's group comes from a new `"attributes"` map in the fixture entries
  of PAT-09, `{"Dimmer":"I","Pan":"P",…}`, so the app can place values in the I/P/C/B columns
  without knowing profiles.
- **IPC-22** The programmer state gains
  `"colour":{"hue":0.66,"saturation":1.0,"r":0.0,"g":0.0,"b":1.0}`: the current target colour
  of the first target that has Colour (programmer value, or else the output, as PRG-20), as HSI
  hue and saturation (0.0–1.0, like the wheels) and sRGB 0.0–1.0, or `null`.
- **IPC-23** `programmer.colour {"hue":0.66,"saturation":1.0}` sets the targets' Colour to that
  hue and saturation, as the HSI wheels would (PRG-30, PRG-31), and touches it. A missing or
  out-of-range field → `error` to the sender.

### Checks
- **UI-25** `tests/manual/app-main-screen.md` (written with UX-02a, grown with b–d) walks each
  zone against the sketch. `tests/manual/app-programmer.md` and `tests/manual/app-patch.md` keep
  passing on the new layout. The README gets screenshots of the main screen when UX-02a lands,
  updated as zones land (owner, 2026-10-17).

## Out of scope (this spec)
Palette, cue and playback behaviour (their own specs fill in the disabled controls), Blind, undo
(P1-10), full-screen console mode and Cmd-Q protection, freely configurable workspaces, legends
and halos, picture legends, the fixture output lamps (open question 3), a remote/touch layout
(IDEA-06), and the narrow single-column layout of the sketch (below 900 px).

## Open questions for the owner
1. **Screens keep the playback area** (UI-10). Patch now fills the whole window. The proposal is
   that the faders, GM and later Go stay visible on every screen. Agreed?
2. **Fixtures window in handle order** (UI-11, PAT-14) instead of the sketch's rows per fixture
   type. Handles are what you arrange yourself, as on Titan. Agreed, or keep type rows as an
   option?
3. **Output lamps** on fixture cells (the coloured strip in the sketch). This needs the engine to
   send each fixture's output colour and intensity several times a second. The proposal is to
   leave it as a disabled stripe now and make it a separate idea (IDEA-10). Agreed?
4. **Full-screen console mode** (research §5: hides the Dock and menu bar, protects Cmd-Q). Now,
   or later?
