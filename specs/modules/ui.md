# Module spec: main screen (UI)

Owner: `subprojects/app` (rendering, input), with four small IPC additions in `cmdmx-daemon`.
Status: **draft for owner approval (UX-02.0, 2026-10-17); open questions answered by owner 2026-10-17**.
UI-03, 13, 18, 19, 22, 23 amended for cue lists: **approved by owner 2026-10-21 (VS3-05)**.
UI-29 (cells, Unfold): **approved by owner 2026-10-25 (P1-13)**.
UI-30 (Fan): **approved by owner 2026-10-25 (P1-20)**.
UI-26 quit text amended to match REL-13 (VS4-08, 2026-10-28; for owner approval in its PR).
UI-26 (VS2-05) timed Clear from the keypad amended (FXB-07, 2026-10-29; for owner approval in its PR).
UI-18 BPM / Rate Master take the next empty handle (FXB-10, 2026-10-29; for owner approval in its PR).
UI-33 and UI-34 (selection orders, P2-02): **approved by owner 2026-10-29**.
UI-33a, UI-33b (Fan and Pattern keep the selection keys, SEL-09): **approved by owner 2026-11-01**.
UI-26 (console mode) quit, UI-19 message zone, UI-22 level bars, Lock in the Main context, and UI-35..40:
approved by owner 2026-11-01 (IDEA-35 batch A, P1-25).
UI-42..48 (IDEA-35 batch H, P1-27): owner's answers 2026-11-03; approved when the spec PR merges.
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
  - (Amended by UI-36: quitting asks in every mode, not only in console mode.) ⌘Q and closing the
    window ask for confirmation first, Cancel the default button.
    (Amended by VS4-08, 2026-10-28, to match REL-13.) When the app started or adopted the engine
    (REL-10, 11), the alert says "Quit the app?" / "The engine stops with the app: DMX output
    stops and the USB interface is free for another program."; when it did not (a developer's
    daemon, or another engine on the port), it says "Quit the app?" / "This app did not start
    the engine, so DMX keeps running."
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
  legend ("Group n" while its legend is empty, PRG-90). There are 24 cells, and empty ones show
  only a number.
  - A tap selects the group; the message line shows "Group 5 Pars outside in" ("Group 5" without
    a legend, PRG-90).
  - Delete, Copy, Move and Set Legend work on groups as PRG-94 describes.
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
  - The gel book (UI-32).
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
  - Delete, Copy, Move and Set Legend work on groups as PRG-94 describes (GRP-03): while one is
    armed a group tap does not select, and Clear disarms it, as for Record.
  - Record, Update, Copy, Move, Delete, Release, Options, Latch and Set Legend work on playback
    handles as PBK-38/39 describe (VS2-22).
  - Every other button is disabled with its spec in the tooltip: Edit "phase 2" (palettes
    decision 4); Off from VS2-05 (UI-26); Help
    opens the manual (UI-55); System later; Select If, Unfold, Blind and Shape "later" as in the
    sketch; Fan from UI-30.
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
    Shapes & Effects (phase 2) and Timecode (phase 3) are disabled; Lock works (UI-38); Undo
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
      *Amended 2026-10-29 (FXB-10):* as their softkey says ("next empty handle"), BPM Master and
      Rate Master no longer arm: the press sends `master.record` at once for the lowest empty slot
      of the current page (empty as PBK-07: a `Locked` playback from another page fills its slot,
      a `TransparentLock` one does not) and Record ends. On a full page nothing is sent, Record
      stays armed, and the app says "Page 2 is full: page 4 has room." (the lowest other page
      with an empty slot) or "Every page is full."
    - Record context, also (amended 2026-10-22, VS3-07; `group-masters.md` GRM-01, 16, 17):
      **Group Master** arms a group master; the next tap on a group cell picks the group, the
      tap after that on an empty handle sends `master.record {kind: "Group", group}`. Its strip
      shows the group's legend ("Group n" when it has none, PRG-90) and the meta line of GRM-16 (amber level per GRM-16; while disabled the bar
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
  zone below it shows the latest warning, error or notice in amber (IPC-17, 49), cleared by the next command; a click on it opens the Messages sheet (UI-39).
  While **Keys** is live (status bar toggle, off by default), the Mac keyboard drives the keypad:
  - The digits and `.`.
  - `-` = Thro, `+` = And, `@`, Return = Enter, Esc = Exit, Backspace = Back.
  - F = Fixture, G = Group, P = Palette, L = Lee (UI-32).
  - F1–F4 = workspaces.
  - Up and Down = command history (UI-27).

  - Space = Go and Shift+Space = Back on the connected list (CUE-47; amended 2026-10-21, VS3-05).
    Space does nothing while Keys is off.

  (Amended by UI-44: Esc works whatever Keys says.)
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
- **UI-29** Cells in the Fixtures window (P1-13, PRG-46; approved by owner 2026-10-25). A fixture
  with cells shows a small cell count in its button. **Unfold** (softkey in the Fixtures
  context, as Titan's Unfold): Unfold + tap a fixture, or Unfold with a selection → "Selected
  fixtures", unfolds those fixtures in place: right after the fixture's own button come its cell
  buttons, legend `901.1`, `901.2`, … in the button system of UI-05 (selection order, selected,
  in programmer per item, IPC-42), the grid flowing on after them. A tap on a cell button sends
  `programmer.select` with `cell`. Unfold + tap an unfolded fixture folds it; **Exit Unfold**
  folds all. Which fixtures are unfolded is view state in the app, not in the show.
  Amendment (VS2-34, approved by owner 2026-10-26): the Unfold function button (UI-16) arms Unfold and opens the
  Unfold softkeys (Selected fixtures, Exit Unfold); the next fixture tap unfolds or folds that
  fixture and disarms, as the other functions. Unfolding changes no selection. Cell buttons stay
  until folded, Unfold armed or not; the cell count shows as the button's tag.
  The Programmer window (UI-14) and the cue view (CUE-54) show a row per cell item right after
  its fixture's row (a cell row without a fixture row comes after the fixture's place), `#`
  `901.2`; a cell's values go to the column of the group `cell_attributes` (IPC-42) gives them.
  A cell value in the cue view is not pickable for Remove (`cue.remove` names fixtures only).

- **UI-30** Fan (P1-20, PRG-56..62, IPC-43; approved by owner 2026-10-25). The Fan function button
  sends `programmer.fan {}` and is lit while `fan` is not `null`. While Fan is on, the softkeys show
  the Fan context: Line, Mirror, Wings, Arrow (the current one marked; a tap sends
  `programmer.fan` with that curve) and Fan off; the attribute bar shows an amber tag
  `FAN <CURVE>` next to the wheels, as in UI v0.1. The wheels send `programmer.wheel` as always;
  the app computes no fanned value.
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
  - **Colour model:** on the C tab, the HSI / RGB / CMY / K switch sits left of the wheels (IPC-19, UI-31).

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
    level bar with its % in SF Mono, moved by dragging as UI-37 says (`fader.set`), then the two handle
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

### Locate, Highlight, Off, Rem Dim and Clear (VS2-05, approved by owner 2026-10-22)
- **UI-26 [now]** Keypad Locate (`programmer.locate`) and Highlight (`programmer.highlight`,
  lit while on) work; a line ending in Locate sends it as the last token (PRG-10). The side
  column's Off sends `programmer.off` for the active group. Rem Dim is softkey E of the Selection context (it needs a selection; Set Legend stays on the Set key and in Main; owner run 2026-10-22).
  Double-clicking a wheel's **name** (never the wheel) sends `programmer.off {attribute}`
  (IDEA-07). A located fixture's button shows the sketch's `LOC` tag. The show-wide part of the
  Playback Options screen gets a Programmer section with the three PRG-42 settings.
  (Amended by FXB-07, 2026-10-29.) A number alone on the command line followed by Clear (keypad
  or attribute bar) is a timed Clear: `3` then Clear sends `programmer.clear {"time":3}` (ENG-10)
  and empties the line; Clear on any other line sends the plain `programmer.clear` (PRG-29).

### Colour temperature and auto-mix (IDEA-32; approved by owner 2026-10-28)
- **UI-31** The colour-model switch (IPC-19) gets a fourth button **K**; the Kelvin wheel shows its
  value as "3200 K" ("—" for black); the four model buttons sit two by two (HSI RGB / CMY K). In the
  colour picker (UI-15), in its Mix view (UI-32), two rows CTO ⅛ ¼ ½ Full and CTB ⅛ ¼ ½ Full send
  `programmer.ct_step` (IPC-46), whatever the model (layout amended in the VS2-46 run, owner). An emitter bar or wheel whose `wheels` entry has `auto` shows a small "+AUTO" tag
  under its name: its value is the direct part, added to what the engine mixes (ENG-22). The app
  computes no colour. Verified by `tests/manual/app-programmer.md` (amended), with the auto-mixed
  output read on Art-Net at home.

### Gel numbers (IDEA-02; approved by owner 2026-10-28)
- **UI-32** The gel book in the colour picker (UI-15). A switch **Mix | Gels** (layout amended in the
  VS2-49 run, owner): Mix shows the field, the bars and the CT rows; Gels turns the whole picker
  into the gel book, its grid as wide as the window. The gel book: a LEE tab (the only range
  for now), a search field that filters by number prefix or by name (`10` → 101, 102, 103, …;
  `congo` → 181; Return, Esc or a swatch tap gives the keys back, VS2-49 run), and a scrolling grid of swatches with number and name, sorted by number, drawn
  from `library.gels` (IPC-47, asked once per connection). Tapping a swatch sends
  `programmer.gel`. When the `colour` object has a `gel`, its swatch is outlined and the picker
  shows "LEE 201 Full C.T. Blue" above the field. While Keys is live, L types the `Lee` token
  (UI-19): `L 2 0 1 Return`. The app computes no colour. Verified by
  `tests/manual/app-programmer.md` (amended), the output read on Art-Net at home.

- **UI-33** Selection orders (P2-02, PRG-71..82, IPC-48; approved by owner 2026-10-29). In the
  Selection context (UI-18) two softkeys open sub-contexts, each with Back:
  - **Order:** Reverse, Shuffle, Sort, Revert (`programmer.order`).
  - **Pattern:** Odd/Even, 1 in *n*, *n* in *m*, Blocks *n*, Divide *n*, Outside in, Inside out,
    Grid off (`programmer.grid` with the shortcut). A number on the command line is *n* (1 in
    `3`; Divide `2`), `n Thro m` gives *n* in *m*; without a number 1 in *n* and Blocks *n* take 3
    and 2, Divide 2. The current shortcut is marked.

  The keypad's Odd/Even key stays (`programmer.pattern` 2). While the grid is on, the attribute bar
  shows an amber tag next to the FAN tag (UI-30) with its name and the stage, `GRID 1 IN 3 · 2/3`
  (`· ALL` after All), from `grid`, `stage` and `position_count`; a tap on the tag switches the grid
  off. Selected fixture cells (UI-11, cells UI-29) show their position *k* + 1 in a corner while
  the grid is on or the order differs from the select order. In the Shapes workspace the Effect
  editor's *More* (FX-38) gains **Flip** next to Wings and an **Order** row: Reverse, Shuffle, Sort
  (FX-45). The app computes no position. The manual's *Selecting fixtures* page and *Shapes and
  effects* page grow in the same PR (WEB-33).
  Amended by SEL-09 (from the SEL-07 run; approved by owner 2026-11-01):
  - **UI-33a** While Fan is on and fixtures are selected, the Fan context (UI-30) keeps Line,
    Mirror, Wings, Arrow and Fan off on A–E and adds **Next** (Fix +1), **All**, **Order** and
    **Pattern** on F–I, as in the Selection context. Order and Pattern open their sub-contexts while
    Fan stays on; their Back returns to the Fan context. Without a selection the Fan context is
    unchanged.
  - **UI-33b** The Pattern sub-context gets **Next** (Fix +1) before Back, so the stages of a grid
    can be stepped without leaving it.
- **UI-34** Cue fixture order (PRG-81; approved by owner 2026-10-29). The Playback Options screen's
  Times tab (PBK-39, CUE-46) gets **Fixture order** for the selected cue: the Fixtures window shows
  each item's step number (`order` + 1) in its cell; a tap gives the item the next step number,
  with **Auto increment** on (default) the number goes up after each tap, off several items share
  it; Done sends `cue.order` (items not tapped keep their order after the tapped ones), Cancel sends
  nothing. Amended by SEL-08 (from the SEL-07 run; approved by owner 2026-11-01):
  - **UI-34a** Edit starts the draft from the cue's current steps, so every cell keeps its number
    while editing; a tap overwrites that item's step, the first tap of an Edit gives step 1. Done
    sends the whole draft (untapped items keep the step they had); Done without a tap sends nothing.
  - **UI-34b** After Done, a short **Saved** note (about 2 s) stands next to Edit, and the grid shows
    the order the engine stored (the cue view after the change).
  - **UI-34c** The Auto increment button reads **Auto increment: On** or **Auto increment: Off**,
    and every Edit starts with it On (Done and Cancel do not carry it over).

### Show-proof (IDEA-35 batch A; approved by owner 2026-11-01 (IDEA-35 batch A, P1-25))
- **UI-35** While console mode is on, the app keeps the display awake
  (`ProcessInfo.beginActivity` with `.idleDisplaySleepDisabled`); leaving console mode ends that. System
  sleep is the engine's (OUT-38). An operator must not lose the screen in a quiet part of a show.
- **UI-36** Quitting always asks, in and out of console mode: ⌘Q, the app menu, closing the main window,
  and a quit from another program or from a logout or shutdown (Cancel cancels that too). Same alert as
  UI-26: Cancel the default button, the same two texts. Closing the window goes through the quit path in both
  modes, so Cancel leaves the window open; today, outside console mode, it would close first and leave an
  app without a window. Force Quit does not ask, and the engine keeps running (REL-14). The manual's
  *Getting started* already says this; `running.md` says what Force Quit does.
- **UI-37** A level bar (strip, group master, GM) never jumps. A press changes no level. A drag moves the
  level by the distance dragged, 1:1 with the bar's height, from the level at the press, clamped to 0–100 %.
  The press still counts as a touch (UI-22) and still arms Learn (CTL-42). Scroll (UI-22) is unchanged. So a
  stray click on the Grand Master at a live moment does nothing, as a physical fader does nothing until it
  is moved. Amends UI-22 and UI-24.
- **UI-38** (UI-48 adds the status bar button and ⌘L.) Lock, softkey in the Main context (Titan's Avo+Lock, *Useful functions*): a prompt for a code
  (the keypad's digits; empty allowed), Enter sends `lock.set` (IPC-50). While locked:
  - a full-window cover hides workspace, softkeys and playback area, with "Locked", the show name, the
    clock and a keypad for the code. The status bar stays, so output state is readable;
  - keypad Enter sends the code; a wrong one shows "Wrong code" and stays locked;
  - quitting does not ask but refuses: "The console is locked. Unlock it to quit." (UI-36's alert after unlock).
    With the engine offline nothing could unlock, so quitting asks as UI-36;
  - the cover follows the engine's `locked`, so an app that reconnects to a locked engine shows it, and a
    restarted engine (unlocked) takes it away. The status bar shows state but its buttons do nothing.
  The prompt and the cover take the Mac's digits, Delete and Return, whatever the Keys toggle says (Esc
  cancels the prompt), and nothing else from the keyboard reaches the console; ⌘ shortcuts still reach the
  menu, so ⌘Q is refused as above. The prompt shows the digits, so a slip is seen before it locks; the cover
  shows dots.
  The lock is an operational control and does not protect the show file (as Titan); the manual says so.
- **UI-39** Messages sheet. A click on the message zone, or on a count next to it, opens a sheet with
  the history (IPC-51), newest first: local time `hh:mm:ss`, level, full wrapped selectable text. Close with
  Done or Escape. The count shows the warnings and errors that arrived since the sheet was last open, in
  amber, and stays when the next command clears the line. The app adds its own lines to the list, not
  to the engine's: engine offline, engine connected, the launcher's notes (REL-12, REL-24, REL-25) and `error`
  replies. `say` hints (UI-19) stay out. An `error` the app keeps is gone after an app restart.
- **UI-40** Output detail needs no hover: a click on a protocol dot (OUT-36) opens the Output screen
  (OUT-37, which shows state and detail per node), a click on the Control dot (CTL-43) the Control screen.
  The hover text stays.

- **UI-41** (approved by owner 2026-11-02 (IDEA-35 batch A2, P1-26)) A Flash or Swop held with the mouse when the cover comes (UI-38; the prompt's Enter from
  the Mac keyboard, or a second client's lock): letting go ends it (IPC-53). When the cover takes the press away, so
  the view never sees the release, the app sends the release the moment the cover comes. Which of the two happens
  only a real mouse can show (`tests/manual/app-main-screen.md`): the run of 2026-11-02 shows the cover ending it. A mouse
  still down after the unlock can bring it on again, and letting go ends it (owner, 2026-11-02).

### Functions and consequences (IDEA-35 batch H, P1-27)
Checked in the code first: `tests/manual/runs/2026-11-03-H-checks.md`. Arming stays view state in the app, so a surface's
press and a surface keypad line go on as before (J4, J5).

- **UI-42** An armed function or Record never does the normal action. A tap does what the table says, or is refused: the message
  zone names where the function works ("Update works on palettes, playback handles and cues."), nothing else happens and
  the function stays armed. Titan: Record, Include, Options, Legend and Unfold are each followed by a Select press (*Button reference*).

  | Armed | Group tile | Fixture tile, cell button | Palette cell | Handle legend | Cue row |
  | --- | --- | --- | --- | --- | --- |
  | Record | records (UI-16), used: PRG-97 | refused | PAL-29 | PBK-38 | CUE-45 |
  | Update | refused | refused | PAL-29 | PBK-38 | updates that cue, as Record's Update softkey (CUE-45) |
  | Include | refused | refused | PAL-29 | PBK-48 | PBK-48 |
  | Copy | PRG-94 | refused | PAL-29 | PBK-38 | the Cue softkeys on that row (CUE-43), "Copy cue 3: type the number, then Copy." |
  | Move | PRG-94 | refused | PAL-29 | PBK-38 | the same, "Move cue 3: type the number, then Renumber." |
  | Delete | PRG-94 | refused | PAL-29 | PBK-38 | the Cue softkeys waiting on Confirm delete (CUE-07, UI-47) |
  | Set Legend | PRG-94 | refused | PAL-29 | PBK-38 | the cue's legend editor |
  | Options | refused | refused | refused | PBK-39 | Playback Options on that cue's Times tab |
  | Release | refused | refused | refused | PBK-38 | refused |
  | Latch | refused | refused | refused | "Latch: press Flash on a handle." | refused |
  | Unfold | refused | fixture tile UI-29, cell button refused | refused | refused | refused |

  A refused Delete, Copy, Move or Set Legend on a fixture tile points to the Patch screen, where fixtures are changed (PAT-10); a
  refused Record on one says to tap a group, palette, cue or handle. Titan sets a legend on a fixture button and deletes one
  in Patch mode; here the legend stays in Patch (PAT-09), because one place for patch data is enough for now.
  With anything armed, an effect palette cell (Shapes) is refused and does not apply, and an Active playbacks row is refused
  unless Release is armed. Page buttons, the workspace rail, faders, GM, DBO, Go, Back, Clear, Locate, Fix ±1 and softkeys are not targets
  and stay as they are: a show keeps running, and the page and the workspace are how the target is reached.
- **UI-43** Flash and Swop while armed. With Record or a function armed, except Latch, a press of a strip's button doing Flash or Swop is a tap on that
  strip's legend (UI-42, PBK-38) and no `playback.button` goes out, so the stage does not change and there is nothing to release.
  Latch stays as PBK-28: Flash latches, and Swop is refused like a legend tap. A button with another function (Go, Back, Tap, …) stays live.
  A Flash already held when a function is armed still ends when it is let go. Learn goes first (CTL-42), an empty handle's buttons stay dead,
  and a surface's Flash stays live, as arming is not known to the daemon. Titan uses Select as the target; Select swops by default, and the
  manual does not say what Flash does while a function is armed. Here both act alike, because two buttons on one strip that
  answer differently would surprise (owner, 2026-11-03).
- **UI-44** Esc goes one level up on every screen, whatever Keys says, and never changes the stage. Titan: Exit "Leaves the current menu and goes one
  level back up the menu structure". One press does the first that applies:
  1. the Lock prompt and cover keep UI-38; Learn keeps CTL-42;
  2. an open sheet or dialog takes it; a text field drops its edit and gives up focus, so the next Esc goes on (a form field keeps its text until the screen goes: nothing of it was applied; run of 2026-11-03);
  3. Record, a function, a softkey menu or a typed command line: the same as the keypad's Exit (disarms, empties the line);
  4. the deck focus goes back to Devices; the profile editor closes, asking first when it has changes (run of 2026-11-03); a screen (Patch, Show, Output, Control, Playback Options) goes back to the workspace, and any
     two-step waiting there goes with it; the Cue view goes back to the Cue list;
  5. otherwise nothing.

  A key held down counts once, so holding Esc cannot leave a screen by accident.
- **UI-45** Opening a screen disarms, from the rail, a function button or a status dot, as hiding the keys must not leave a function live: Record armed, Patch, and a
  tap on a handle no longer records. Changing workspace does not disarm (armed in one workspace, target in another); the page buttons do not either.
- **UI-46** The command line reaches an armed function. With Record or a function armed, Enter on exactly `Group n` or `Palette n` is a tap on that
  cell (UI-42), with the same confirmation, refusal and disarm. `Palette n` that does not exist, as a record or copy target, is refused ("Palette 12 is empty: tap a
  cell in a palette window"), because the cell a new palette goes to is a window and a handle. Any other line, with Enter or Locate, is refused
  ("Record: type Group n or Palette n, or tap a target."): nothing is sent, the line stays to be corrected, and the function stays armed.
  Record, a number, then a handle tap stays (CUE-45), since no Enter is pressed. With a Delete waiting and an empty line, Enter confirms, as softkey A does (on Titan Enter
  confirms too). A second tap on the target does not confirm: Titan's does, but a mouse double-click would then delete.
  Titan also has `Record Cue n` and `Copy … @ n`; here there is no Cue or handle key, and `@` adds nothing to two steps; I and J look at the keypad again.
- **UI-47** A delete says what it does, before the confirmation (Titan confirms by a second press and warns only that a fixture's programming goes; the owner
  wants the count, since an undo cannot take back what a running show already showed). The app asks IPC-54 and replaces the message when the answer comes; the
  confirmation works before it does.
  - Palette: "Delete palette 12 Warm? 3 cues on 2 playbacks use it and keep its values. Confirm on softkey A." or "… No cue uses it. …".
  - Playback, running (`live.state` not `off`): "Delete playback 1.4 Service? It is running: its output ends at once. 8 cues go with it. Confirm on softkey A." Not
    running leaves out that sentence; one cue leaves out the last.
  - Cue, the current one: "Delete cue 3 Song? It is the live cue: the output stays until the next Go. Confirm on softkey A." (CUE-07).
  - Fixtures, under the Patch screen's Confirm remove, as that screen has no message zone (UI-10): "In 2 groups, 14 cues on 3 playbacks and 5 palettes. 1 group and 2 cues end up empty and
    stay. Undo brings it all back." Nothing to say gives "Not used anywhere."
  Empty groups and cues stay (owner, 2026-11-03): an action removes no more than was asked, and a list keeps its numbers, links and timing. Group delete stays as PRG-85 and 86.
- **UI-48** Lock from anywhere. A **Lock** button in the status bar and ⌘L open UI-38's prompt from every screen and context, whatever Keys says; the softkey stays. Offline the button is
  disabled and ⌘L does nothing, as the softkey. Under a sheet ⌘L waits for the sheet. Titan's Avo+Lock works from anywhere too. A surface key is left to J4: a surface cannot type a
  code, and a key that only opens a prompt on the Mac does not lock anything; it needs a saved code, as Titan's Set Saved Password.
Checked by hand: `tests/manual/app-main-screen.md` gets one section per criterion, with the UI-42 table walked for each target. `swift build` is the app's gate (no Swift tests, `CLAUDE.md`).

### Before batch B (P1-30)
Approved by owner 2026-11-04, with Keys staying off by default. From the third gap review (K2, IDEA-40).
- **UI-49** With Keys off, a key that Keys would act on (UI-19), with no text field focused and no sheet or lock cover up, does nothing except say once in the message zone: "The Mac keyboard is off: turn on Keys in the status bar, or use the keypad." A click on the command line says the same. Neither goes in the Messages sheet (UI-39). Chords and Esc are not concerned. `getting-started.md` step 3 and `programming.md` name Keys or the keypad.
- **UI-50** A message the app said because of an armed function (its hint, a refusal, what a delete touches) goes when the function disarms, by Esc, Exit, Cancel, a screen opening (UI-45), or doing its job. A warning or notice from the engine (IPC-17, 49) stays until the next command, as UI-19; so does a message that came after the function's own. Nothing else about the message zone changes: a selection notice stays through an engine restart, which is not this criterion.
Checked by hand: `tests/manual/app-main-screen.md` gets one section per criterion.

### Help at hand (P1-32; approved when the spec PR merges)
From the fourth gap review (L6, F7).
- **UI-55** **Help opens the manual.** The **Help** function key, the menu item *Help › CMDMX Manual* (replacing *CMDMX Help*) and its shortcut ⌘? open the manual in the default browser: the copy inside the app (REL-29) when it is there, else the website (a local build). It never changes the stage or the programmer. Help is no target (UI-42): with Record or a function armed it stays armed, also while building a list. While locked it does nothing, as the other function keys (UI-38). If the browser cannot open it, the message zone says "The manual could not be opened."; the app goes on. Checked by hand: `tests/manual/app-main-screen.md`, in console mode too.

### Changing what is recorded (IDEA-35 batch B, P1-29; approved by owner 2026-11-04)
- **UI-51** **Times in the grid** (I6; Titan: tap a value in Playback View, or drag over several, type, Enter; a Times button leaves only the timing columns). The Cue list window gets a **Times** toggle (not on a chase, CUE-29): columns Q, Legend, Delay in, Delay out, Fade in, Fade out, Overlap, an As In value dim. In this view a tap selects a time cell and does not set Next; a drag or Shift-tap extends within that column. The softkeys show "Q2–Q5 · Fade out": **Set** (a number on the line; Enter on such a line does the same), **As In** (out columns only), **Done**. It sends one `cue.times` (IPC-56), so a running transition keeps its times (CUE-09). Arming a function clears the cells; Esc clears them, then leaves the view (UI-44). The plain view with Link, Track and Groups stays as it is, and a tap there still sets Next.
- **UI-52** **A refused number goes back** (IDEA-36). Half a second after a `NumberField` sent a value, it shows what the engine holds, focused or not. Accepted, that is the same number; refused, the old number returns under the warning already shown. No warning needs to name its field, and the app does not know the range. Every `NumberField` of Playback Options, also in UI-51's route. Checked by typing fade in 602.
- **UI-53** **One-cue playback in the Cue window** (B1; amends CUE-43, 44; Titan: touching the playback's display opens Playback View, whatever it holds). A plain tap on a memory's legend shows it in the Cue list window ("Cue · 1.2", one row), and changes nothing on stage, in the Go block or in what is connected: a memory has no Go (PBK out of scope). A row tap there does not set Next. Its softkeys are View Cue, Edit Times and Set Legend; Copy, Move or Delete armed on its row is refused and names its handle, as the softkeys that do them are not there (own choice, build REC-05). It ends on a tap on a list's legend, a change of the connected list, or Esc. With no Cue window on show the message says: "1.2 holds one cue: the Playbacks workspace shows it."
- **UI-54** **A row says what it can do** (K11; Titan: a View button at the end of each row). Each row of the Cue list window ends in **View** (opens the Cue view) and **⋯** (the Cue softkeys, as a right-click, which stays). Neither changes the stage: Go to Cue stays a softkey, one step further. With a function armed the whole row is the target (UI-42).
- **UI-42 amended:** Include + handle or cue row: PBK-48; Record + used group tile: PRG-97; Update armed without a tap: PBK-49.; Record while building a list: CUE-62. The other cells stay.
Checked by hand: `tests/manual/app-cuelists.md` gets one section per criterion of this batch, with fades read on Art-Net. The manual's `cuelists.md`, `playbacks.md`, `programming.md` and `release-notes.md` grow in each task's PR (WEB-33).

### Seeing what the rig does (IDEA-35 batch C, P1-33; approved by owner 2026-11-05)
From `docs/research/everyday-gaps.md` C, K8 and the fourth review (L1, J8, L2, L3, L5); checked in `tests/manual/runs/2026-11-05-C-checks.md`. Titan pages used: *Viewing and editing fixture values* (Channel Grid, Intensity View), *Changing the patch* (DMX View), *Playback controls*, *Editing cue lists*. What a page does not say is marked as our own choice.
- **UI-56** **A master that holds its group down is seen everywhere** (L1; amends UI-06, PBK-40). While any group master is holding (GRM-22):
  - the status bar shows a **GRP** tag in amber beside DBO; a click lists them in the message line ("Master Zaal (1.8) holds Group 3 at 0 %");
  - Active playbacks lists each holding master in amber after the running playbacks: home handle, legend, "GRP · 0 %" or "LOCKED 0 %". A tap on its row does not release it (a master is not released, PBK-22) but goes to its page, so its strip is in reach (own choice).
  The show-open notice is GRM-23. Checked by hand: Master Zaal at 0 % and disabled, seen from page 2.
- **UI-57** **Fixture info** (J8, L3, L5). A right-click on a fixture tile, or on an Intensity block (UI-61), opens a sheet for that fixture: number, name, manufacturer, model and mode, universe and address; the groups it is in, the cues and palettes that use it (IPC-60 `usage` detail; Titan's *Select If* goes the other way only); and **Now**: one row per attribute with its value and, in words, why (IPC-60 `why`): "Dimmer 0 %: Dienst Q4 gives 100 %, Master Zaal (1.8) scales it to 0 %". It asks again every second while open. A row with a playback or a master goes to its page on a tap. Checked by hand on Art-Net with a master down, a tracked value and an effect.
- **UI-58** **The selection stays in sight** (L2; amends UI-11). When the selection changes by anything but a tap in the Fixtures window (a group, the command line, a surface, Next), the Fixtures window shows the page of the first fixture in the selection order and scrolls it into view. Page tabs that hold a selected fixture carry a blue dot, and the title info says "6 selected · 4 on page 2" when not all are on the page shown. Titan says nothing on this; own choice. Checked by hand with a group spread over two pages.
- **UI-59** **A tile says what it is, and can be found** (L3; amends UI-11). A tile with a name shows the profile's model on a second small line under it (the model alone stays one line). The Fixtures window's title row gains a find field: typing matches number, name and model as the operator types (Titan's Intensity View search, legend or user number); tiles that do not match dim, the window goes to the first match's page, and Return selects the matches in number order, as taps would. Esc empties the field. Checked by hand.
- **UI-60** **Live output on the tiles** (C3, IDEA-10; amends UI-11, makes FX-41 true). The bottom stripe of a fixture tile shows what the fixture outputs (IPC-58): its colour at its intensity, dark grey outline at 0; a cell's tile shows the cell. The app draws what the engine sends and animates nothing (FX-41). Checked by hand against Art-Net with a list, a chase and a master at 0.
- **UI-61** **Views: Intensity, Grid and DMX** (C1, C2; owner decision 1). A **Views** item on the rail opens a screen in place of the window grid, side column and attribute bar, as Patch does (UI-10); the playback area and status bar stay. Its tabs:
  - **Intensity** (Titan's Intensity View): one block per fixture with number, legend, intensity in % and a bar, and the source label (IPC-58); a tracked intensity carries "=" in magenta, one from an effect "~" in yellow; blue header when selected, red when in the programmer. Filters All, Stage (intensity above 0), Programmer, Selected; the find field of UI-59. A tap selects as a tile does; a right-click opens Fixture info (UI-57).
  - **Grid** (Titan's Channel Grid): a row per fixture, a column per attribute, the attribute group tabs I P C G B E S FX as filter, the same four fixture filters. A cell shows the value as the Programmer window does (UI-62), a palette as its legend when *Palettes* is on; the *Sources* toggle shows each cell's source label instead (IPC-59). Read only: typing values in the grid is not in this batch (owner decision 4).
  - **DMX** (Titan's DMX View): a line picker with every output line (OUT-13), the 512 channels as 0–255 (owner decision 2), each run of channels tinted per fixture with the fixture number at its first channel; a tap on a channel names fixture and attribute in the message line.
  Each tab watches its view (IPC-59) while open and stops when left. Checked by hand on Art-Net: the values on screen equal what a listener reads.
- **UI-62** **A palette value on one line** (K8; amends UI-14, CUE-54 in the app). In the Programmer window, the Cue view and the Grid, when every value of a fixture in one attribute group refers to the same palette, the cell shows that once: the palette's legend in cyan, with a swatch for colour, and no hex or per-attribute lines ("■ Warm"). Values set by hand, or from different palettes, show as today. Titan's Cue View toggles between the palette and its value (*View Palettes*); here a right-click on the cell shows the values. Checked by hand: a 12-fixture cue with a colour palette fits the Cue view.

### The screen fits (IDEA-39; the owner asked for the fix on 2026-11-07)
- **UI-63** **The bottom zones always show whole** (amends UI-01). At every window size from UI-01's minimum
  (1280 × 800 pt of content, which also covers a MacBook window with the Dock, 832 pt with its title bar) and in
  console mode, the status bar, the attribute bar and the playback area show whole: every strip's fader and its
  buttons are on screen. The row between them (rail, window grid and side column, or the screen that replaces them,
  UI-10) takes the height that is left and never pushes the zones below it off screen. When the rail's items do not
  fit, the rail scrolls; the function keys and softkeys shrink to no less than 34 pt, so all of them show at 800 pt;
  the windows of the grid scroll as they do now. Found because the rail grew (Remote, RMT-15) past the height a
  MacBook's full screen leaves it. Checked by hand: `tests/manual/app-main-screen.md`, in console mode and in a
  window with the Dock, on two workspaces and on the Patch screen.

### Working on many at once (IDEA-35 batch D, P1-36; approved by owner 2026-11-07)
From `docs/research/everyday-gaps.md` D; checked in `tests/manual/runs/2026-11-07-D-checks.md`. Behaviour in PBK-50..52, PAL-34, PRG-98, CUE-63 and SHW-19. Titan pages used: as PBK-50, and *Clearing the Console*.
- **UI-64** **Picking several** (D1, D2; amends UI-42, PBK-38, PAL-29, PRG-94; owner decision 5). With Copy, Move or Delete armed, the first tap on a used handle legend, palette cell or group tile picks it, outlined in the function's colour. A ⇧-tap on another of the same window picks the range between them (Titan: hold the first, press the last); a ⌘-tap adds or drops one (Titan *And*). The softkey **All** picks every used item on the page or in the window of the first pick, so a page or a pool is cleared or copied in three taps. Handle picks stay through a page change. The message zone counts ("Copy 4 playbacks: tap the target"). Copy and Move then take a tap on the target slot or cell; Delete waits on its confirm softkey naming the count ("Delete 8 playbacks on page 2", two-step, UI-47). While Copy or Move is armed, softkeys **Retain layout** / **Bunch up** (one toggle, as last used, Retain when the app starts) and, for Move, **Swap if required** (off when armed). A refusal keeps the picks, so another target can be tried. One pick behaves as today. In the Cue list window, ⇧ and ⌘ pick cue rows for Delete (CUE-63). Checked by hand on Art-Net.
- **UI-65** **Menu latch** (D1; Titan *Menu Latch*: Copy, Move or Delete stay active until pressed again). A **Menu latch** softkey, shown while Copy, Move or Delete is armed, keeps the function armed after each action, with the picks cleared, until the function button again, Clear or Esc. It is lit while on, not saved, off when the app starts. Checked by hand.
- **UI-66** **Release all, or a page** (D3; amends UI-42: a page button is a target while Release is armed). Release armed and Release pressed again sends Release all (Titan: Release twice). Release armed and a tap on a page button shows the softkeys **Release page 2** and **Release all but page 2** (Titan has a third for fader groups, which we do not have). The message zone names how many playbacks go ("Release all: 5 playbacks"). Checked by hand on Art-Net with a release time of 2 s.
- **UI-67** **New show from this patch** (D2; amends SHW-17). The Show screen's New gets a **Keep the patch** toggle, off when the screen opens. The two-step confirm names what goes: "New show Kerk kerst with the patch of Kerk: groups, palettes and playbacks start empty". Checked by hand.

### Snappy while the rig moves (owner, 2026-11-08)
The owner felt the app grow sluggish in the last releases. Cause found in the code: since UI-60 the live output deltas
(IPC-58, up to 20 a second while anything fades or runs) land in the one object every view watches, so the whole
console redraws on each.
- **UI-68** **Live output redraws only what shows it** (amends UI-60, UI-61). The live output (IPC-58) and the Views
  data (IPC-59: grid and DMX) live apart from the rest of the app's state, so an output delta redraws only the tile
  stripes and the open Views tab, not the playback area, the windows or the side column. What is shown does not change.
  Checked by measurement: the app's CPU with the daemon's `--demo` (fader 1 breathes), before and after, in the run file.
- **UI-69** **A moving playback redraws only what shows it** (amends UI-22, CUE-40 in the app). The playbacks' live part
  (PBK-35's `playback` delta: level, state, flash, cue, BPM, holding; up to 40 a second per moving playback, drawn at
  most 30 a second) lives apart from their stored part. A live change redraws only what shows it: the playback strips,
  the open cue list windows, the Active playbacks window, the status bar tags and the open Playback options or Fixture
  info sheet. The windows grid, the side column, the rail and the attribute bar redraw only on a stored change, a
  selection or the operator's own input. A softkey or tap that needs a live value reads it when pressed. What is shown
  does not change.
- **UI-70** **The CPU budget** (owner, 2026-11-08: "unacceptable for a lighting desk"). `scripts/app-cpu.sh` starts the
  daemon (release, temp show folder, no outputs but Art-Net) and the release app in console mode, waits 12 s and
  prints the app's average `%cpu` over 20 s, for two loads: idle, and `--demo` (fader 1 breathes). On the owner's Mac
  the app stays at or under **5 % idle** and **15 % with `--demo`**. Every app PR that touches `EngineClient` or a view
  that watches it runs the script and puts both numbers in its description. Measured before and after in the run file.
- **UI-71** **Live values drawn outside SwiftUI** (amends UI-22, UI-60, UI-69; SNP-04 found that any SwiftUI update in
  the main window costs about 10 ms, however small the view, so 20–40 updates a second cannot fit UI-70). A value that
  changes many times a second while the rig moves is drawn by a small AppKit view on its own layer, which takes the
  value straight from the engine client without a SwiftUI update: a tile's output stripe, a fader strip's level fill,
  its % and its cue progress, and the levels in the Active playbacks window. The blinking command-line cursor and the
  clocks likewise. SwiftUI redraws a zone only when something else it shows changes (state, cue, legend, a stored
  change). What is shown does not change. Checked with `scripts/app-cpu.sh` (UI-70) and by eye on Art-Net.

### Checks
- **UI-25** `tests/manual/app-main-screen.md` (written with UX-02a, grown with b–d) walks each
  zone against the sketch. `tests/manual/app-programmer.md` and `tests/manual/app-patch.md` keep
  passing on the new layout. The README gets screenshots of the main screen when UX-02a lands,
  updated as zones land (owner, 2026-10-17).

### Patch and profiles (IDEA-35 batch E, P1-39; proposed, awaiting the owner)
From `docs/research/everyday-gaps.md` E, K10, M2, M3. Behaviour in `patch.md` PAT-37..47, `fixture-library.md` LIB-41, 42; the Patch screen is PAT-10, 25, 35.
- **UI-72** **Exchange** (M2). With rows selected and a model and mode picked in the Add strip, the Selected panel shows **Exchange to Wash 19 · 16 Channel**, a two-step confirm stating what the fixtures are in (UI-47's line, "In 3 groups, 62 cues on 28 playbacks and 11 palettes. Undo brings it all back."). It sends the Add strip's Universe and Address only when Address is filled in. A refusal shows in the panel; the selection stays on the same ids. Disabled without a pick or a selection. Checked by hand on Art-Net.
- **UI-73** **Find in Patch** (M3). The Patch window's title row gets UI-59's find field: number, name and model, as the operator types. The table shows only the rows that match (Titan filters Patch View by fixture type) and the title counts them ("12 of 254"). Rows that leave the table leave the selection, so a range edit never reaches what cannot be seen. Return selects every row shown. Esc empties the field (UI-44). The Add strip gets a **Hide** button, remembered on this Mac, shown by default, so the table takes the screen. Checked by hand on a rig of about 250.
- **UI-74** **Add starts over** (K10, E2). After an Add that the engine accepted, Number and Address go empty, so both are *next free* (PAT-03); Count, Universe and the pick stay. A refused Add leaves them as typed, to correct (the app sees acceptance in the `fixtures` list: a fixture with the typed number or a longer list). The strip gets an **Auto groups** toggle (PAT-43), remembered on this Mac, off by default (owner question 6). Checked by hand.
- **UI-75** **Copy, swap, update, profiles** (E2, E3). The Selected panel gets **Copy** (with an optional start number), **Swap addresses** (armed, the message says "Pick the fixtures to swap with"; a new selection and Confirm send it; Esc or Clear disarms) and **Update from library** (enabled when every selected row has the same profile). The Add strip gets **Duplicate…** on any pick (asks a new model name) and **Delete profile** on a user profile, two-step: "Delete user profile X? Shows that use it keep their copy." Checked by hand.
- **UI-76** **Behaviour** (E1). The Selected panel gets **Behaviour…**, a sheet over the attributes every selected row has: Invert, Low, High and Offset in %, a value shown empty where the selection differs (Titan shows no Inverted on a mixed selection), Swap Pan and Tilt when all have both, and Reset. A row with behaviour shows a small `INV` or `LIM` tag in the Mode column. Checked by hand on Art-Net.

## Out of scope (this spec)
Palette, cue and playback behaviour (their own specs fill in the disabled controls), Blind, undo
(P1-10), freely configurable workspaces, legends and halos, picture legends, a remote/touch layout
(IDEA-06), and the narrow single-column layout of the sketch (below 900 px). Not in batch C (P1-33): editing values in the Grid, Titan's *View If* and *Frozen* filters, the DMX View's input column, a fixture's position on a rig plan (IDEA-25). Not in batch D (P1-36): Unfold of a cue list or chase onto faders and a linked playback copy (owner decisions 3 and 4: IDEA-46, IDEA-47), copying or moving several cues, Titan's *Copy Legends* option and *Unassign*, the Release Priority setting and *Release all Masters*.

## Owner decisions (P1-36, 2026-11-07: every recommendation accepted)
1. **Layout by default (PBK-50, UI-64):** Retain layout, so a copied page looks like the page? Recommended: yes. Not chosen: Bunch up first, which closes gaps nobody asked to close.
2. **Clearing a show (SHW-19, UI-67):** a new show under a new name that keeps the patch, leaving the old file as it was? Recommended: yes, it can never lose a show. Not chosen: a wipe of the open show in place (one wrong click empties the Sunday show; Undo history goes with it), or Titan's New Show only.
3. **Unfold a cue list or chase onto the faders (D4):** Titan's Unfold is an editing view: each cue on its own fader, run by raising it, changed with Record Step, left with Unfold again. Recommended: not in batch D, a spec of its own later in phase 1 (an idea now). It is as large as the rest of D together, and the Cue list window already edits every cue. Not chosen: in D as a sixth task.
4. **A linked playback copy (D4):** Titan's Link shows the same cues on another handle with its own times and options. Recommended: not in batch D, an idea for later: cues shared between playbacks change the show file and every cue edit. Not chosen: in D.
5. **How to pick several (UI-64):** ⇧ for a range, ⌘ to add one, and an **All** softkey, as on the Mac? Recommended: yes, it needs one hand on a touch screen with All. Not chosen: Titan's hold-the-first-and-press-the-last, which needs two fingers on a trackpad.

## Owner decisions (P1-33, 2026-11-05: every recommendation accepted)
1. **Where the views live (UI-61):** a Views screen on the rail with Intensity, Grid and DMX tabs, as Patch? Recommended: yes, they need the width. Not chosen: windows inside a workspace (UI-08 would change), or three rail items.
2. **DMX values (UI-61):** 0–255? Recommended: yes, it is the DMX view; % is in Intensity and Grid. Not chosen: %, or a toggle.
3. **Find (UI-59):** Return selects the matches? Recommended: yes, so a stand-in can take `P29` by name. Not chosen: find only dims and jumps.
4. **Typing in the Grid (UI-61):** later, not in batch C? Recommended: later; the batch is about seeing, and the Programmer and command line already set values. Not chosen: in C, as Titan's Channel Grid.
5. **After the batch:** sub-release 0.6.0, and a gap review only after the first service on CMDMX? Recommended: yes, the fourth review said a fifth needs another angle or real use. Not chosen: a gap review straight after 0.6.0.

## Owner decisions (P1-27, 2026-11-03: every recommendation accepted)
1. **UI-43, Flash or Swop while armed:** it picks the target and the stage does not change. Not chosen: doing nothing with a message, or staying live.
2. **UI-42, cue rows and fixture tiles:** a cue row does what the function says through what exists; a fixture tile refuses and points to Patch. Not chosen: refusing everywhere, or fixture actions on the tile.
3. **UI-44, Esc on a screen:** one level up, so with nothing armed it leaves the screen. Not chosen: Esc only disarms.
4. **UI-47, fixture removal:** empty groups and cues stay and the confirmation says so. Not chosen: removing them too.
5. **UI-48, Lock:** status bar button and ⌘L, no surface key yet. Not chosen: ⌘L alone, or a surface key with a saved code now.

## Owner decisions (2026-10-17)
1. Screens (Patch now) keep the playback area and the status bar visible (UI-10).
2. The Fixtures window is in handle order (UI-11, PAT-14), not in rows per fixture type.
3. Output lamps on fixture cells are a separate idea (IDEA-10); a disabled stripe until then.
4. Full-screen console mode comes now (UI-26, UX-02a).
