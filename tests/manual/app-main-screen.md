# App main-screen checklist (UI, `specs/modules/ui.md`) — template

No Swift test target exists (CLAUDE.md), so the main screen is checked by hand against the
sketch `specs/ux/main-screen.html` (open it in a browser next to the app). Copy to
`tests/manual/runs/<date>-<task>.md` and tick it there. Sections grow with UX-02b–d.

Setup: daemon `rm -rf /tmp/cmdmx-main && cd subprojects/engine && cargo run -p cmdmx-daemon -- --no-sacn --show-dir /tmp/cmdmx-main` (a fresh Demo); app
`cd subprojects/app && swift build && scripts/bundle-app.sh && open .build/CMDMX.app`.

## UX-02a — the shell

### Console mode and frame (UI-01, UI-26)
- [ ] The app opens full screen: no Dock, no menu bar
- [ ] ⌘Q asks "Quit the app? This app did not start the engine, so DMX keeps running." (this
      checklist runs `swift run` against your own daemon; the bundled app's text is in
      `tests/hardware/release.md`) — Cancel keeps the app open, the Art-Net output never stops either way
- [ ] Console off (status bar): a normal window. Resizing it only grows or shrinks the window
      grid; the bars keep their size. It does not get smaller than 1280 × 800
- [ ] Quit and reopen: the app comes back in the mode you left it in
- [ ] The six zones sit where the sketch has them: status bar, rail, 2×2 grid with the side column
      to its right, attribute bar, playback area. Flush, hairlines, no outer margins

### Quit always asks (UI-36, REL-13)
Start each row from the stated mode (Console toggle in the status bar). The alert is the same every time:
"Quit the app?", the engine text from UI-26, Cancel the default button. Reopen the app after each Quit.
- [ ] ⌘Q, console on: Cancel keeps the app, full screen; Quit quits
- [ ] ⌘Q, console off: Cancel keeps the app and its window; Quit quits
- [ ] App menu › Quit, console on (move the pointer to the top for the menu bar): Cancel keeps the app; Quit quits
- [ ] App menu › Quit, console off: Cancel keeps the app and its window; Quit quits
- [ ] Window close button, console on (pointer at the top shows it): Cancel keeps the window; Quit quits
- [ ] Window close button, console off: Cancel keeps the window, the app is not left without one; Quit quits
- [ ] Return in the alert is Cancel
- [ ] Console off, a quit from another program (`osascript -e 'tell application id "nl.colourmotion.dmx" to quit'`): the
      alert shows; Cancel keeps the app and the script gets an error; Quit quits
- [ ] Force Quit (or `kill -9`): no question, the engine keeps running and Art-Net never stops; reopen, it reconnects
- [ ] Through all of the above the dev daemon runs on and Art-Net does not stop (a bundled engine stops on
      Quit: `tests/hardware/release.md`)

### The engine comes back (REL-24, 25)
Needs a bundle with the engine inside and its own `HOME`, so the app's engine leaves your shows alone (REL-10 gives it
no `--show-dir`). Do not `open` it: LaunchServices drops the `HOME`. The app itself does not follow `HOME`: its pid file and
`daemon.log` stay in your real `~/Library`, and Quit removes the pid file.
- `cd subprojects/engine && cargo build --release -p cmdmx-daemon`
- `cd subprojects/app && scripts/bundle-app.sh && cp ../engine/target/release/cmdmx-daemon .build/CMDMX.app/Contents/MacOS/`
- `mkdir -p /tmp/cmdmx-rel24/home && HOME=/tmp/cmdmx-rel24/home .build/CMDMX.app/Contents/MacOS/CMDMX &`
- The pid: `cat ~/Library/Application\ Support/CMDMX/daemon.pid`.

A fresh `HOME` opens the Demo show, and this engine cannot be given `--no-artnet`: it sends the Demo patch at level 0 on
Art-Net and sACN. Ask the owner first when Lightkey runs or a rig is on the network. Without `--serial` the Eurolite stays closed.

- [ ] A child dies: `kill -9` the engine. The status bar shows "Engine exited (code 9), restarting" and offline; within 5 s
      a new pid, Engine green, "Engine restarted: <show> is back, nothing is running"
- [ ] Adopt: `kill -9` the app, the engine keeps its pid. Start the app again: the same pid, one engine, Engine green,
      no amber line
- [ ] An adopted engine dies: `kill -9` the engine. "Engine exited, restarting" and offline; within 5 s a new pid, Engine green,
      "Engine restarted: <show> is back, nothing is running"
- [ ] The show is back: a group recorded and autosaved before the kill is still there after it
- [ ] Not while stopping: adopt again, Quit in the alert. The app and the engine are gone, the pid file too, and no
      engine comes back within 5 s

### A failing engine (REL-26..28)
A bundle whose engine is a script, so no real show is touched and every attempt leaves a time. No lamps needed; the engine sends nothing.
- `cd subprojects/engine && cargo build --release -p cmdmx-daemon && mkdir -p /tmp/cmdmx-rel28/home /tmp/cmdmx-rel28/shows && cp target/release/cmdmx-daemon /tmp/cmdmx-rel28/real-daemon`
- `cd subprojects/app && scripts/bundle-app.sh`
- `Contents/MacOS/cmdmx-daemon` becomes a script: `date +%H:%M:%S >> /tmp/cmdmx-rel28/attempts; exit 1`
- `HOME=/tmp/cmdmx-rel28/home .build/CMDMX.app/Contents/MacOS/CMDMX &`

Then:
- [ ] The attempts in the file wait 2, 4, 8, 16, then 30 s, and stay at 30 s
- [ ] From the third failure the status bar has one steady line, "Engine does not start (code 1), trying every 30 s. Log: ~/Library/Logs/CMDMX/daemon.log"; "back" never shows
- [ ] Messages after several minutes: two lines, "Engine exited (code 1), restarting" and the steady line
- [ ] The script becomes `exec /tmp/cmdmx-rel28/real-daemon --show-dir /tmp/cmdmx-rel28/shows --no-artnet --no-sacn --no-serial "$@"`. At the next attempt: Engine green, "Engine connected" and "Engine restarted: Demo is back, nothing is running" with the same time
- [ ] A change of workspace leaves the line; Clear ends it; it stays in Messages
- [ ] After 60 s of running, `kill -9` the engine three times with less than 60 s between them: 2, 4, then 8 s of wait; the third gives the steady line with code 9, the connect replaces it
- [ ] `chmod -x` the script, `kill -9` the engine: "Could not start the engine: …", from the third failure the steady line without a code. `chmod +x`: connected, "back"
- [ ] Quit during a wait, with the failing script in place: the app is gone and no attempt follows
- [ ] Another engine on 7319 before the app starts (`/tmp/cmdmx-rel28/real-daemon --show-dir /tmp/cmdmx-rel28/other --no-artnet --no-sacn --no-serial`): "Another engine is running on port 7319"; Clear leaves it; `kill -9` that engine ends it

### Theme and type (UI-02..05)
- [ ] Dark theme matches the sketch: dark grey, not black; no colour that does not mean a state
- [ ] Day/Night in the status bar switches to the day theme and back; it is remembered
- [ ] Legends are condensed, every number is monospaced (a fader level does not jitter while it
      changes)
- [ ] Fixture and group cells: 8 per row, number top-left; selected = blue border and blue number;
      in programmer = red top strip; the fill never changes

### Status bar (UI-06, IPC-20)
- [ ] Left: the show name ("Demo"), engine dot green, a dot per output node: Art-Net green, serial amber
      with "no interface" (no Eurolite on the desk); no sACN dot because of `--no-sacn`
- [ ] Restart the daemon without `--no-sacn`: an sACN dot appears, green
- [ ] Stop the daemon: the engine dot goes grey with an amber line in the bar (no big banner);
      start it again: green, the screen rebuilds
- [ ] Right: Console, Keys, Keypad, Day/Night toggles and a running clock

### Rail, workspaces and windows (UI-07..13)
- [ ] The rail shows four workspaces with F1–F4, then Patch, Macros (disabled), Show,
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

## Messages sheet and dots (UI-39, UI-40, IPC-51, IPC-52, REL-12, REL-25)
Keys off unless a line says otherwise. A warning: `Fixture 999 Enter` on the keypad.
- [ ] Message zone empty: a click on it opens the sheet; Done closes it; Escape closes it
- [ ] A warning shows in amber on the line and a count `1` next to it. A second one: `2`
- [ ] The next command clears the line, the count stays
- [ ] A click on the count opens the sheet, newest first: local time `hh:mm:ss`, level, full text, wrapped; text can be selected and copied
- [ ] After closing, the count is gone. A warning while the sheet is open does not leave a count behind
- [ ] A notice (`Group 1 Enter`: `Group 1 …`) is in the sheet and does not raise the count
- [ ] A `say` hint (Copy, then tap an empty group cell) shows on the line, is not in the sheet, no count
- [ ] An `error` reply (import a broken profile file) is in the sheet with level `error` and raises the count
- [ ] A node's state change (IPC-52: an unplugged or wrong serial node) is in the sheet as a warning; back to Ok as a notice
- [ ] Engine down and up (`kill` the daemon, start it again): `Engine offline` and `Engine connected` in the sheet; the engine's earlier lines stay; none shows twice after the reconnect
- [ ] App restart with the engine running: the engine's history is back with its own times; the earlier `error` and the app's own lines are gone
- [ ] Bundled app: `kill -9` the engine: `Engine exited (code …), restarting` and `Engine restarted: the show is back, nothing is running` in the sheet
- [ ] Keys on, sheet open: Escape closes the sheet; digits and Space do nothing while it is open (no token, no Go)
- [ ] A click on the Art-Net dot opens the Output screen; on the Control dot the Control screen; the hover text still shows on a dot with a problem

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

### Help opens the manual (UI-55)
Bundled = the app from a test release (REL-29); local = `scripts/bundle-app.sh`. Keys off and Keys on; console mode too.

- [ ] Bundled: the Help key opens the copy inside the app in the default browser (a `file://` address ending in
      `Contents/Resources/manual/index.html`); its front page names the version; search and the mark work with the
      network off. On an app from a DMG with the quarantine flag it opens without a warning.
- [ ] Local build: the Help key opens `https://aottens.github.io/CMDMX/manual/`.
- [ ] The menu has *Help › CMDMX Manual* and no *CMDMX Help*; the item and ⌘? do the same as the key, also in console
      mode where the menu bar is hidden.
- [ ] Select fixtures and listen to Art-Net: Help changes neither the output nor the programmer.
- [ ] Record armed, Help: still armed, the ring stays. The same with Copy armed, and with Copy while building a list.
- [ ] Locked: the key is under the cover; ⌘? and the menu item do nothing.
- [ ] A failing browser (not reachable by hand; read in the code): the message zone says "The manual could not be
      opened." and the app goes on.

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

## UX-02d — playback area and pages (UI-21..24, PBK-01..05)

### Pages (UI-21, PBK-01..03, 05)
- [ ] The pages column shows page buttons 1–10, page 1 highlighted, and "per page 8" below;
      clicking anywhere on a button selects its page
- [ ] Page 1: strips 1–8 show playbacks 1–8 (legend = playback number); fader 1 up drives channel 1
- [ ] Click page 2: page 2 is highlighted; strip 1 shows playback 9, strip 2 playback 10, strips
      3–8 are empty and disabled (no cap)
- [ ] On page 2, fader 1 (playback 9) to full: the four washes come on (Art-Net channels 9, 18,
      27, 36); fader 2 (playback 10): the front spots (channels 145–150)
- [ ] With playback 1 up on page 1, go to page 2: channel 1 stays on, Art-Net does not change;
      back to page 1: strip 1 shows playback 1 still at its level, with the green active strip
- [ ] Restart the daemon: the app comes back on page 1 (the page is not part of the show)

### Strips, Go block and GM (UI-22..24)
- [ ] Each strip is 92 pt wide: legend at the top, level bar with its % in monospace and a
      draggable cap, then Swop and Flash greyed ("Playbacks · P1-04")
- [ ] An active playback has a green strip along the bottom edge
- [ ] No bank frame around the strips while no control surface is connected (comes with VS1-07)
- [ ] Scrolling over a strip moves its level; Option scrolls fine, Shift coarse; a fast trackpad
      scroll down to 0 % never jumps back up on the way (also not the %)
- [ ] Holding a fader: it follows the pointer and does not bounce; on release it shows the
      engine's value
- [ ] A level bar never jumps (UI-37), on a strip, a group master and the GM, each at a level
      in the middle of the bar:
  - a press anywhere on the bar, far from the cap: the % stays and Art-Net does not change
  - a drag of a quarter of the bar's height moves the level by 25 %, from where it was, wherever the press was
  - a drag past the top or bottom stops at 100 % or 0 %; dragging back moves it again from the press level
  - with the Learn panel open, a press on a bar arms Learn for it and the level stays
  - scrolling over a strip still moves its level
- [ ] The Go block (210 pt): header, "now" line, big green-outlined GO, Back / Stop / Release, all
      greyed with "Cue lists · P1-08"; nothing covers it
- [ ] Grand Master: a level bar like a strip, amber below 100 %, DBO greyed under it

## An armed function on every target (UI-42, UI-43)
At home, no lamps: `--show-dir` in a temp folder, a show with a group, a palette, a memory on a handle, a cue list of three cues and a
fixture with cells. Listen to Art-Net for the Flash and Swop lines. A refused tap: the message names where the function works, nothing else
happens, the function stays armed.

- [ ] Group tile: Update, Include, Options, Release, Latch and Unfold are refused and do not select. An empty group cell refuses them too.
- [ ] Fixture tile: Record says to tap a group, palette, cue or handle. Delete, Copy, Move and Set Legend point to the Patch screen. Update, Include, Options, Release and Latch are refused. None selects.
- [ ] Unfold: a fixture tile unfolds (UI-29); a cell button is refused and does not select.
- [ ] Palette cell: Options, Release, Latch and Unfold are refused, each with its own places, not "works on playback handles" for all.
- [ ] Handle legend: Include and Unfold are refused and the last tapped handle does not move. Latch says "Latch: press Flash on a handle."
- [ ] Cue row, Update: updates that cue, disarms; no change to the Next row.
- [ ] Cue row, Copy: the Cue softkeys, "Copy cue 3: type the number, then Copy."; Move: "Move cue 3: type the number, then Renumber."
- [ ] Cue row, Delete: the softkeys wait on Confirm delete; a second tap on the row does not confirm; Confirm deletes that cue.
- [ ] Cue row, Delete, then Delete on a palette: softkey A now confirms the palette, not the cue.
- [ ] Cue row, Set Legend: the cue's legend editor. Options: Playback Options on that cue's Times tab.
- [ ] Cue row, Include, Release, Latch, Unfold: refused, and Next does not move.
- [ ] Shapes, Effects, Palettes: a cell with anything armed is refused and applies nothing.
- [ ] Active playbacks: a row with anything but Release armed is refused and keeps running; with Release armed it releases and disarms.
- [ ] Not targets: with a function armed, page buttons, the rail, faders, GM, DBO, Go and Back work as always.
- [ ] Flash with Record armed: the used-handle softkeys, Art-Net does not change, nothing to release.
- [ ] Swop with Delete armed: the delete waits on softkey A, Art-Net does not change.
- [ ] Flash with Update armed on a latched playback: an Update, it stays latched.
- [ ] Latch armed: Flash latches and disarms; Swop says "Latch: press Flash on a handle." and stays armed.
- [ ] A Go or Back on a handle button with Record armed stays live.
- [ ] A Flash held when a function is armed still ends when let go: by reading, as arming takes the only mouse (`HoldActions.release` does not look at what is armed).
- [ ] Learn on, a function armed: a Flash press arms Learn; an empty handle's buttons stay dead.

## The command line reaches an armed function (UI-46)
- [ ] Record, `Group 5`, Enter on an empty group: records it, disarms, the line empties. `Group 5` is in the history.
- [ ] Record, `Palette 1`, Enter on a used palette: the Replace, Merge, Quick Merge softkeys. Copy, `Palette 1`, Enter: the source, then tap an empty cell.
- [ ] Record, `Palette 12`, Enter with no palette 12: "Palette 12 is empty: tap a cell in a palette window"; the line stays, Record stays.
- [ ] Update, `Group 1`, Enter: the same refusal as a tap, the line stays, Update stays.
- [ ] Delete, `Group 1`, Enter: waits on softkey A, the line empties. Enter on the empty line confirms.
- [ ] (owner) Delete, `Group 1`, Return held down: it waits on softkey A, a held key does not confirm.
- [ ] Delete: a palette, a handle and a cue row in turn, each confirmed by Enter on an empty line, from the keypad and from the Mac's Return.
- [ ] A right-click on a cue row, Delete: Enter on an empty line confirms. A second tap on any target never confirms.
- [ ] Record, `Fixture 1`, Enter: "Record: type Group n or Palette n, or tap a target."; nothing sent, the line stays, Record stays. The same with Locate, and with `Group 1` and Locate.
- [ ] Record, `3`, a tap on a handle: records cue 3 (CUE-45). Record, `3`, Enter: refused, the line stays.
- [ ] Options, `Palette 12`, Enter with no palette 12: Options's refusal, not "is empty".
- [ ] Nothing armed: Enter and Locate behave as before, and Enter closes the Palette softkeys.

## Esc, one level up (UI-44)
At home, no lamps: `--show-dir` in a temp folder. Each line with Keys on and with Keys off (`-keysLive NO`); listen to Art-Net, as no Esc may change it.

- [ ] A workspace: Record armed, Esc disarms. A function armed, a softkey menu (Palette, Order), a typed line: Esc does what Exit does. With nothing left, Esc does nothing.
- [ ] Patch, Output, Control, Show and Playback Options: Esc goes back to the workspace; a typed line waits there untouched.
- [ ] Patch: Confirm remove waiting, Esc goes back to the workspace and nothing is removed; back on Patch, it no longer waits.
- [ ] Patch, the profile editor: Esc cancels when clean; with changes it asks "Discard changes?", and Esc on that dialog stays in the editor.
- [ ] Control: a deck's focus goes back to Devices, the next Esc to the workspace.
- [ ] Cue view: Esc goes back to the Cue list; with Remove waiting, the first Esc cancels that and the second goes back.
- [ ] A text field that commits on losing focus (BPM in the Cue list, a time in Playback Options, a port on Control): type, Esc: the old value shows and nothing was sent; the next Esc goes on. Clicking away still sends.
- [ ] A form field on Patch or Show: Esc gives up focus and the next Esc leaves the screen. Nothing was applied.
- [ ] A sheet (a legend editor, Messages): Esc closes the sheet only. Lock prompt: Esc cancels. Learn: Esc ends it.
- [ ] Esc held on a screen: leaves it once, and an armed Record on the workspace is not touched by the repeats. A running chase and a held Flash do not change.

## Opening a screen disarms (UI-45)
- [ ] Record armed, then Patch (rail), Patch (softkey), Disk (softkey), Output, Control, Show (rail), a protocol dot, the Control dot: no armed function on return; a tap on a handle from that screen does not record.
- [ ] A function armed, then a screen: the same.
- [ ] Record armed, another workspace by the rail, F-key and ⌘-key: stays armed, and a tap there records. A page button: stays armed.

## The bottom zones always show whole (UI-63)
- [ ] Console mode on the MacBook, *Groups + Palettes*, *Fixtures + Groups* and *Shapes*: the status bar, the attribute bar and every strip whole, fader and the buttons under it included; the rail scrolls to Keystone.
- [ ] The Patch screen and Playback Options in console mode: both buttons under each fader show.
- [ ] Console off, a window with the Dock (832 pt high), then dragged to its minimum (1280 × 800 pt of content): the same; the function keys and the ten Pattern softkeys all show, none cut.
- [ ] A big window (1440 × 900 or larger): the keys are 40 pt as before and the rail does not scroll.

## Keys off says so (UI-49)
- [ ] Keys off, the workspace, nothing focused: a digit, `f`, Return, Backspace, Up: the line stays empty, and the message zone says "The Mac keyboard is off: turn on Keys in the status bar, or use the keypad."
- [ ] Keys off: F2 does not change workspace and Space runs no Go; both say the same, also on a screen (Output). A digit on a screen says nothing.
- [ ] Keys off: a key Keys does not use (`x`) says nothing. ⌘2 still switches workspace and says nothing. Esc does its own work (UI-44) and says nothing.
- [ ] A held digit shows the message without flicker or a repeat beep; the Messages sheet has no such line.
- [ ] Keys off: a click on the command line says the same. Keys on: a click there says nothing, and digits fill the line.
- [ ] Keys off, typing in a text field (Patch search, a sheet's name field), under an open sheet, under the Lock prompt and cover: no such message.

## An armed function's message goes with it (UI-50)
- [ ] Delete armed (its hint shows): Esc, the keypad's Exit, the Cancel softkey and a screen opening each take the hint away with the function.
- [ ] Delete, tap a palette (what it touches shows), softkey A: the message goes with the delete. The same with a refusal said by a tap on a wrong target, then Exit.
- [ ] Copy on a palette, then an empty cell: the hint goes when the copy is done. Update, Move and Set Legend likewise.
- [ ] Delete armed, then Record, or Delete then Copy: the old hint does not stay under the new function.
- [ ] Record armed, a tap on a used cell (its message), Cancel: it goes.
- [ ] Delete armed (its hint shows), then a warning from the engine (a second client's refused command): the warning stays through Esc, Exit and Cancel, until the next command.
- [ ] Delete armed, then a notice from the engine (IPC-49, a group selected by a second client): the notice stays through Esc.
- [ ] Keys off with Delete armed: a digit says UI-49's message; Esc disarms and that message stays, as it is not the function's own.

## Lock from anywhere (UI-48)
- [ ] **Lock** in the status bar opens the prompt from each workspace, with a selection, on each screen, and with a function armed. The softkey still does.
- [ ] ⌘L does the same, with Keys off and with focus in a text field.
- [ ] Engine offline: the button is dim and ⌘L does nothing. Under a sheet, ⌘L does nothing until the sheet closes.
- [ ] Locked: the button does nothing and ⌘L does not open a second prompt.
- [ ] Learn on: the prompt ends Learn, as the softkey does.

## A delete says what it does (UI-47)
- [ ] Palette used by two cues on one playback, by a value or by an effect: Delete, tap it: "Delete palette n Legend? 2 cues on 1 playback use it and keep its values. Confirm on softkey A." One cue: "1 cue on 1 playback uses it and keeps its values."
- [ ] Unused palette: "… No cue uses it. …". No legend: the number alone.
- [ ] Softkey A confirms, and Enter on an empty line; afterwards the cue still gives the same output with plain values.
- [ ] Palette waiting, another palette tapped: the second one's text shows, not the older count.
- [ ] Running playback with 8 cues: "Delete playback 1.4 Service? It is running: its output ends at once. 8 cues go with it. Confirm on softkey A."
- [ ] Not running: without the running sentence. One cue: without the cues sentence. A master with no cues: neither.
- [ ] The live cue, by the armed row tap and by the Cue softkey: "Delete cue 3 Song? It is the live cue: the output stays until the next Go. Confirm on softkey A." Art-Net: nothing changes on confirming, the next Go moves on.
- [ ] Another cue of that list, or the list not running: without the live sentence.
- [ ] Patch: Remove on one fixture, then on a selection, then **Confirm remove** shows the count underneath; "In 1 group." singular forms; a set that together empties a group says so, one of them alone does not. Compare with the group and cue windows after confirming. Undo brings it all back.
- [ ] Patch: a fixture in nothing: "Not used anywhere." The text leaves with Esc, a changed selection or the confirmation. Confirm remove works at once.
- [ ] Group delete: the message is unchanged (PRG-85).
- [ ] Locked (second client): `usage` still answers; the console is unchanged and Undo shows no new step.

## Lock (UI-38, IPC-50)
Claude does the setup and the second-client checks and listens to Art-Net; the owner does the steps marked
(owner). Every step message shows done/total ("Stap 3/12"). Before the first step: a playback running a chase
or a long fade, so output can be seen going on.

- [ ] Main context, nothing selected: **Lock** is live. With something selected the Selection softkeys show instead.
- [ ] Lock opens the prompt, digits show as typed; Back and Delete remove one; Cancel and Esc leave it unlocked.
- [ ] (owner) Lock with `1234`, Enter: the cover hides all below the status bar, with "Locked", show name, a running
      clock, dots and a keypad. The status bar still shows the dots; its buttons do nothing.
- [ ] Art-Net: frames keep coming and the chase or fade runs on (no gap, the levels keep moving).
- [ ] Space, digits on the Mac keyboard with Keys on, ⌘Z: no cue moves, nothing on the console changes; digits fill the dots.
- [ ] (owner) A wrong code, Enter: "Wrong code", still locked, dots empty. The next digit clears the line.
- [ ] A second client's `fader.set`: `warning` "Console is locked", no change on Art-Net.
- [ ] ⌘Q and the close button: no alert, the cover says "The console is locked. Unlock it to quit."
- [ ] (owner) `1234`, Enter: the cover is gone, with the same workspace and scroll position as before. ⌘Q now asks
      as UI-36; Cancel.
- [ ] Master code: lock with `1234`, unlock with `68340`.
- [ ] Empty code: Lock, Enter with no digits; Enter unlocks.
- [x] ⌘1–⌘5 under the cover switch nothing; after the unlock the same workspace shows (passed by raw key input, `tests/manual/runs/2026-11-02-A2-checks.md`).
- [ ] `kill -9` of the app while locked (bundled, adopted engine): the reopened app shows the cover at once.
- [ ] Kill the engine while locked: within a moment ⌘Q asks as UI-36 while offline; the restarted engine is unlocked
      and the cover goes away.
- [ ] Learn on, then Lock: Learn ends first; after the unlock Learn is off.

What is held when the cover comes (UI-41, IPC-53). Only a real mouse can show these: a click through accessibility cannot
hold. Claude locks from a second client and listens to Art-Net; the owner holds the mouse.

- [ ] (owner) Hold Flash on a strip with the mouse. Claude locks. The Flash ends when the mouse lets go, or at the moment
      the cover comes if the cover takes the press away; which of the two is noted in the run. It does not stay on.
- [ ] (owner) The same with Swop.
- [ ] (owner) Unlock: the key does not show as down, nothing is on, and the next hold of the same Flash works and ends when let go.
- [ ] (owner) Hold Flash, lock, and unlock while still holding (Claude does both): it may come on again while the mouse
      is down, and letting go ends it as usual; nothing stays on.


## Live values on layers (UI-71, SNP-05)
- [ ] With a cue list running a 5 s fade: the GO block shows the fade % and its progress bar fills, the current cue row in the Cue list window shows its progress line, Active playbacks shows the level; with its fader at 60 % the GO block says "fader 60 %".
- [ ] The same in Day theme: colours as before.
