# Module spec: main screen (UI)

Owner: `subprojects/app` (rendering, input), with four small IPC additions in `cmdmx-daemon`.
Status: **draft for owner approval (UX-02.0, 2026-10-17); open questions answered by owner 2026-10-17**.
UI-03, 13, 18, 19, 22, 23 amended for cue lists: **approved by owner 2026-10-21 (VS3-05)**.
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
  its right, the attribute bar (96 pt: wheels row and command-line row, UI-20) and the playback
  area (232 pt) along the bottom. Zones
  are flush, separated by hairlines, with no outer margins. Resizing the window only resizes the
  window grid. The other zones keep their size, and content never reflows into another zone. The
  minimum window size is 1280 × 800 pt.
- **UI-02 [now]** Dark theme from the sketch's tokens: background #121314, surface #1A1C1E /
  #222427, hairlines #2A2C2F / #3A3D41, text #D8DADC / #8A8E93 / #55595E. There is a Day theme
  with the sketch's day tokens, toggled in the status bar. Both themes are the app's own and do
  not follow the macOS appearance. The choice is remembered across launches.
- **UI-03 [now]** Colour means state only, everywhere: red = programmer or unstored or Record
  armed, blue = selection, green = running or OK, amber = fading or warning, cyan = from palette,
  violet = Blind, magenta = tracked value (CUE-54; amended 2026-10-21, VS3-05: #E54BB4 dark,
  #B8288A day). Attribute-group hues (sketch `--gI`…`--gFX`) are used only for tags and tab
  underlines. No other accent colours exist.
- **UI-04 [now]** Type: SF Pro condensed (SwiftUI `.width(.condensed)`) for legends and titles,
  SF Pro for UI text, SF Mono with tabular digits for every number and for the command line.
  Three sizes only: 10–11, 12–13 and 15–20 pt. Tags are uppercase with tracking.
- **UI-05 [now]** One button system for fixture, group and palette cells, as in the sketch:
  - The cell is about 72 pt high, on a fixed 6-column grid with 2 pt gaps (amended 2026-10-18,
    owner, VS2-19 run: 54 pt in 8 columns was too small next to a Titan).
  - The number sits top-left and a tag top-right; the legend is condensed, two lines at most.
  - Empty = outline only with a faint number. Selected = 2 pt blue inner border and a blue
    number. In programmer = 3 pt red top strip. Not relevant = 38 % opacity.
  - State never changes the fill.

- **UI-26 [now]** Console mode, on by default at launch, with a Console toggle in the status bar:
  - The window goes native full screen with the Dock and the menu bar hidden.
  - ⌘Q and closing the window ask for confirmation first ("Quit the app? DMX keeps running in
    the engine.").
  - Leaving console mode gives a normal resizable window (UI-01's minimum size).
  - The choice is remembered across launches (owner, 2026-10-17: now, not later).

### Status bar (UX-02a)
- **UI-06 [now]** Left to right:
  - The show name, "Demo show" until the show file exists [with P1-09].
  - The engine link: green while connected, grey while offline. The offline banner of IPC-08
    becomes this indicator plus a one-line amber message in the bar.
  - One dot per output node with its name and state (IPC-20).
  - The autosave age [with P1-09], and the BLIND and DBO tags [with their specs].
  - On the right: the toggles Console (UI-26), Keys, Keypad and Day/Night, and the clock (SF
    Mono, hh:mm:ss).

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
- **UI-11 [now]** Fixtures: one cell per fixture, in handle order (PAT-14). There are 6 cells per
  row (UI-05) and `handles_per_page` cells per handle page, with page tabs in the title row when there is
  more than one page.
  - The cell shows the user number, the selection order top-right in blue, and the fixture's
    name, or else its profile's model, as the legend.
  - Selected, current target (a double blue border during a sub-selection) and in programmer
    follow IPC-15 and UI-05.
  - A tap sends `programmer.select`.
  - The output lamp strip along the bottom edge is a disabled stripe [with IDEA-10].
- **UI-12 [now]** Groups: one cell per group with its number, its fixture count as the tag, and a
  legend ("Group n" until groups have names [with Set Legend]). There are 24 cells, and empty
  ones show only a number.
  - A tap selects the group.
  - Recording a group moves from the window's Record button to the Record function button
    (UI-16).
  - The IPCGBES mask strip is drawn empty [later; it needs the groups' attribute groups over
    IPC].
- **UI-13 [now]** The Colours, Positions and Gobos & Beams windows are live palette windows as
  `palettes.md` PAL-28 describes (palettes on handles, no fixed number ranges; amended
  2026-10-18, VS2-19). Active playbacks is live as `playbacks.md` PBK-40 describes (VS2-22). Cue
  list is live as `cuelists.md` CUE-43 describes, and View Cue shows the Cue view in its place
  (CUE-54); amended 2026-10-21, VS3-05.
- **UI-14 [now, UX-02b]** Programmer: a table with one row per fixture that has programmer values
  (IPC-21): # (user number), Fixture (name or model), I, P, C, B.
  - A touched value is red. A Colour value shows a swatch plus its hex. An attribute the fixture
    lacks shows "·".
  - Palette references in cyan with the palette's legend (PAL-30).
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
    and Clear disarms it. **Patch** opens the Patch screen. **Disk** opens the Show screen
    (SHW-17; owner, 2026-10-18).
  - Update, Delete, Copy, Move and Include work on palettes as PAL-29 describes (VS2-19).
  - Record, Update, Copy, Move, Delete, Release, Options, Latch and Set Legend work on playback
    handles as PBK-38/39 describe (VS2-22).
  - Every other button is disabled with its spec in the tooltip: Edit "phase 2" (palettes
    decision 4); Off from VS2-05 (UI-26); Help
    and System later; Select If, Unfold, Blind, Fan and Shape "later" as in the
    sketch.
- **UI-17 [now]** The keypad has the sketch's 30 keys in its order. Keys that work now:
  - The digits, `.`, Fixture, Group, Thro, And and `@`, which add a token to the command line.
  - Enter, which sends the tokens as `programmer.command`.
  - Back, which removes the last token or digit, and Exit, which empties the line.
  - Clear (`programmer.clear`), which glows red while the selection or programmer is non-empty.
  - Fix −1 / Fix +1 (`programmer.step`) and Odd/Even (`programmer.pattern` 2).

  Palette adds the `Palette` token, and with an empty line opens the Palette softkeys (PAL-29).
  Set arms Set Legend (PAL-29, as Titan's Set key). Disabled: Macro (later), Avo and View (later).
  Highlight and Locate: UI-26.

  The Keypad toggle in the status bar hides the keypad, and it is hidden automatically on a
  window narrower than 1440 pt. The sketch hides it below 1280 px, but with the 1280 pt
  minimum window of UI-01 the threshold has to be wider.
- **UI-18 [now]** Softkeys A–G in the sketch's arrow shape, with the context name in the
  header.
  - Main context: Set Legend works on palettes and handles (PAL-29, PBK-38); Playback Options
    and Edit Times open the Playback Options screen for the last tapped handle (PBK-39);
    Shapes & Effects (phase 2), Timecode (phase 3) and Lock (later) are disabled; Undo
    (G, also in the Selection context) undoes the newest step, its label under the name, disabled
    while there is none (amended 2026-10-22, VS5-03; `undo.md` UNDO-18).
  - Selection context (when something is selected): Next and Previous (Fix ±1), All
    (`programmer.all`), Odd/Even, Set Legend (PAL-29) and Playback Options (PBK-39) work.
  - Record context (Record armed): Cancel, plus the palette softkeys of PAL-29 (Replace, Merge,
    Quick Merge after a tap on a used cell; Mask, Record By, Record as).
  - Palette context (PAL-29): Master Time, Master Overlap, Set Overlap, Cancel.
  - Cue lists (amended 2026-10-21, VS3-05; `cuelists.md` CUE-43..46, 54):
    - Record context on a used handle: Replace, Merge, Append, Insert, Cancel (CUE-45); after a
      tap on a Cue list row: Replace, Merge, Update, Cancel for that cue; on a tracking list an
      Update Mode softkey cycles Forwards / Cue Only / Backwards / Both for this record (CUE-52).
      With digits on the command line, a handle tap records to that cue number (CUE-04) and
      empties the line.
    - Record context, also: **BPM Master** and **Rate Master** arm a master; the next tap on an
      empty handle sends `master.record` without a number (IPC-26).
    - Record context, also (amended 2026-10-22, VS3-07; `group-masters.md` GRM-01, 16, 17):
      **Group Master** arms a group master; the next tap on a group cell picks the group, the
      tap after that on an empty handle sends `master.record {kind: "Group", group}`. Its strip
      shows "Group n" and the meta line of GRM-16 (amber level per GRM-16; while disabled the bar
      follows the fader and the meta line reads "LOCKED 50%", IPC-29), and Playback Options shows Handle and Master
      tabs: mode, a scale picker only in Scale, and a Disabled toggle (GRM-17).
    - Cue context (a right-click on a Cue list row): Go to Cue, View Cue, Edit Times, Set Legend,
      Renumber, Copy, Delete (two-step on softkey A), Cancel. Renumber and Copy take the number
      typed on the command line.
    - Cue view context: Back to List, Remove (two-step, on the hard value last right-clicked).

  A right-click on a strip's legend opens Playback Options (PBK-39); on a Cue list row the Cue
  context; on other cells it is reserved for their context softkeys [later].
  Playback Options gains a **Settings** tab, marked "show-wide", with the CUE-38 settings: Auto
  connect, the cue-list fader for new lists, the update mode and the handle buttons (Best
  practice, Titan, or Custom with a pair per kind) (CUE-46; amended 2026-10-21, VS3-05).

### Command line and Mac keys (UX-02c; the field itself in UX-02b)
- **UI-19 [now]** The command line in the attribute bar shows tokens, not a free text field: the
  prompt `›`, the tokens and a blinking cursor (no blinking under Reduce Motion). The message
  zone below it shows the latest warning or error in amber (IPC-17), cleared by the next command.
  While **Keys** is live (status bar toggle, off by default), the Mac keyboard drives the keypad:
  - The digits and `.`.
  - `-` = Thro, `+` = And, `@`, Return = Enter, Esc = Exit, Backspace = Back.
  - F = Fixture, G = Group, P = Palette.
  - F1–F4 = workspaces.
  - Up and Down = command history (UI-27).

  - Space = Go and Shift+Space = Back on the connected list (CUE-47; amended 2026-10-21, VS3-05).
    Space does nothing while Keys is off.

  ⌘1–⌘4 switch workspaces too, also while Keys is off (owner, UX-02a run). Keys never fire while a text field (Patch, Show, legends) has focus. The Keys toggle is
  remembered across launches, like Console.
- **UI-27 [now]** Command history (IDEA-12; owner, 2026-10-18): every line sent with Enter is
  added to the history, except a line equal to the newest entry. While Keys is live, Up replaces
  the command line with the previous entry and Down with the next one; Down past the newest
  entry gives an empty line. A recalled line can be edited with the keypad or keys before Enter.
  The history keeps the last 100 lines in the app's preferences, so it survives a restart of
  the app. It is the app's own; the engine and the show file never see it.
- **UI-28 [now]** Every disabled control looks the same: one dim level (a theme token) for rail
  entries, status bar toggles, function buttons, keypad keys, softkeys, attribute tabs and
  buttons, with the spec it waits for in the tooltip (owner, UX-02b run). Empty palette cells keep
  their own empty-cell look (UI-13): dimming them would hide their numbers.

### Attribute bar (UX-02b)
- **UI-20 [now]** Two rows (owner, UX-02b run 2026-10-17: the command line gets its own row, so the
  wheels have the full width, up to the BCF2000's eight encoders).

  Row 1 (64 pt), left to right:
  - **Tabs:** eight IPCGBES FX tabs, 30 × 34 pt, each with its group hue underline. The active
    tab is inverted, available tabs have normal text, unavailable ones are disabled, and a red
    dot means the group has programmer values (IPC-11).
  - **Wheels:** a rolling encoder per `wheels` entry, at least three slots, with "enc n" labels.
    They share the rest of the row: 168 pt each when there is room, narrower down to 110 pt,
    so eight wheels fit at the 1280 pt minimum window. A wheel is never clipped or scrolled.
    Each shows the uppercase name and the value (SF Mono 15 pt, red when touched), and a roller
    strip that scrolls with the value plus a position tick.
  - **Colour model:** on the C tab, the HSI / RGB / CMY switch sits left of the wheels (IPC-19).

  Row 2 (32 pt), left to right:
  - **Command line:** the command line takes the free width, with the message zone (UI-19) on
    the same row to its right, truncated.
  - **Programmer summary:** "n fx · I P C" in red (from `in_programmer` and `attribute_groups`),
    "empty" in grey, with Undo, Redo (each labelled by a tooltip with the step it acts on, disabled while there is
    none; amended 2026-10-22, VS5-03, `undo.md` UNDO-18) and Clear. The interim Fix ±1, Odd/Even and
    All buttons sit here until UX-02c moves them to the keypad and softkeys.

  Dragging and scrolling a wheel keep IPC-16's steps and modifiers. Values stay in % (Hue in
  degrees); Pan and Tilt in degrees come with the profile's ranges [later].

### Playback area (UX-02d, built with VS1-06)
- **UI-21 [now]** Pages column: page buttons (the whole button is clickable) with their number and page legend (PBK-08, VS2-22), plus a
  "per page" read-out of `faders_per_page`. Paging itself comes with VS1-06 in the same task.
- **UI-22 [now]** `faders_per_page` playback strips, each 92 pt wide:
  - From top to bottom: a legend (PBK-37: halo bar, icon, legend or `page.slot`, lock mark), a
    level bar with its % in SF Mono and a draggable cap (`fader.set`), then the two handle
    buttons, live while held (PBK-37, VS2-22). They are labelled and act as `handle_buttons`
    assigns (GO, TAP, FLASH, …), the legend shows the cue, step or tempo, the connected list
    has a blue frame and a plain tap on a list's legend connects it (CUE-44; amended
    2026-10-21, VS3-05).
  - An active playback has a green strip along the bottom edge, a releasing one an amber strip.
  - The strips a connected control surface drives sit in a framed bank labelled with that
    surface and its faders, e.g. "BCF2000 · faders 1–8" (research §5). Without a connected
    surface there is no frame (owner, UX-02d run 2026-10-18); the frame comes with VS1-07.
  - Scrolling over a strip moves its level (Option fine, Shift coarse).
  - While the operator holds a fader or the GM, the engine's deltas for it do not move it. The
    app keeps its own value, as a motor fader does while touched. On release the bar shows the
    engine's latest value, so the engine stays authoritative (IPC-05). Amended 2026-10-17 after
    the owner saw faders lag and bounce in the UX-02a run.
  - After a release or a scroll step, older engine values still on their way are not shown: the
    bar waits until the engine reports the value the app sent last, or at most 1 s, and then
    follows the engine again. The level never jumps back to an earlier value on its way to 0
    (owner, UX-02d run 2026-10-18).
- **UI-23 [now]** Go block (210 pt): the header, the "now" line and the big green-outlined GO
  button with Back / Stop / Release, live on the connected list as `cuelists.md` CUE-42
  describes (amended 2026-10-21, VS3-05; was disabled until P1-08), with Tap and the BPM field on
  a chase. It is never covered by anything.
- **UI-24 [now]** Grand Master (70 pt): a level bar like a strip (`grand_master.set`), turning
  amber below 100 %, with DBO under it, live as PBK-41 describes (VS2-22).

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

### Locate, Highlight, Off, Rem Dim and Clear (VS2-05, draft for owner approval)
- **UI-26 [now]** Keypad Locate (`programmer.locate`) and Highlight (`programmer.highlight`,
  lit while on) work; a line ending in Locate sends it as the last token (PRG-10). The side
  column's Off sends `programmer.off` for the active group. Rem Dim is a main-context softkey.
  Double-clicking a wheel's **name** (never the wheel) sends `programmer.off {attribute}`
  (IDEA-07). A located fixture's button shows the sketch's `LOC` tag. The show-wide part of the
  Playback Options screen gets a Programmer section with the three PRG-42 settings. A timed
  Clear has no app control yet (IPC only).

### Checks
- **UI-25** `tests/manual/app-main-screen.md` (written with UX-02a, grown with b–d) walks each
  zone against the sketch. `tests/manual/app-programmer.md` and `tests/manual/app-patch.md` keep
  passing on the new layout. The README gets screenshots of the main screen when UX-02a lands,
  updated as zones land (owner, 2026-10-17).

## Out of scope (this spec)
Palette, cue and playback behaviour (their own specs fill in the disabled controls), Blind, undo
(P1-10), freely configurable workspaces, legends and halos, picture legends, the fixture output
lamps (IDEA-10), a remote/touch layout
(IDEA-06), and the narrow single-column layout of the sketch (below 900 px).

## Owner decisions (2026-10-17)
1. Screens (Patch now) keep the playback area and the status bar visible (UI-10).
2. The Fixtures window is in handle order (UI-11, PAT-14), not in rows per fixture type.
3. Output lamps on fixture cells are a separate idea (IDEA-10); a disabled stripe until then.
4. Full-screen console mode comes now (UI-26, UX-02a).
