# Release notes

## 0.5.4

Published 7 November 2026 as "CMDMX 0.5.4".
[Release on GitHub](https://github.com/Aottens/CMDMX/releases/tag/v0.5.4)

Seeing what the rig does, and a screen that fits.

- **Live output on the fixture buttons:** the stripe along the bottom of a fixture button shows the colour and
  intensity that leave the console right now, with masters, the Grand Master, DBO and effects in, so a chase runs
  across the buttons (see [Tapping](programming.md#tapping)).
- **The faders stay whole on a small screen,** such as a MacBook with the Dock: the rail scrolls and the keys on the
  right get a little smaller (see [First launch](getting-started.md#first-launch)).

## 0.5.3

Published 7 November 2026 as "CMDMX 0.5.3".
[Release on GitHub](https://github.com/Aottens/CMDMX/releases/tag/v0.5.3)

Knowing a fixture, walking the rig with a phone, and parking what does not hang.

- **Fixture info:** a right-click on a fixture button shows its type, address, groups, cues and palettes, and why
  each value is what it is (see [Fixture info](programming.md#fixture-info)).
- **The selection stays in sight:** the Fixtures window goes to the page of what you select, with a dot on each page
  tab that holds a selected fixture; a named button shows its model, and **Find** looks up a fixture by number, name
  or model (see [Tapping](programming.md#tapping)).
- **The phone remote:** the **Remote** screen sets a login and shows a QR code; a phone on the same network walks the
  patch fixture by fixture, readdresses, names and highlights (see [The phone remote](remote.md)).
- **Park:** a fixture that does not hang leaves the output, the selection and live groups, and keeps its programming
  for when it is unparked, from the console or the phone (see
  [Parking a fixture that does not hang](patching.md#parking-a-fixture-that-does-not-hang)).

## 0.5.2

Published 5 November 2026 as "CMDMX 0.5.2".
[Release on GitHub](https://github.com/Aottens/CMDMX/releases/tag/v0.5.2)

Why is it dark?

- **Masters that hold the stage down:** a group master at 0 or disabled, also on another page, shows an amber
  **GRP** tag in the status bar and a row in Active playbacks; a tap goes to its page (see
  [Why is it dark?](taking-over.md#why-is-it-dark)).

## 0.5.1

Published 5 November 2026 as "CMDMX 0.5.1".
[Release on GitHub](https://github.com/Aottens/CMDMX/releases/tag/v0.5.1)

Help at hand, from the review of a stand-in taking over a show.

- **Help opens this manual:** the **Help** key, *Help › CMDMX Manual* and ⌘? open the copy inside the app, the
  one for your version, so it works without internet (see [Help](running.md#help)).
- **Taking over a show:** a new chapter for someone who did not program the show, with what to check when the
  stage is dark (see [Taking over a show](taking-over.md)).

## 0.5.0

Published 5 November 2026 as "CMDMX 0.5.0".
[Release on GitHub](https://github.com/Aottens/CMDMX/releases/tag/v0.5.0)

This release is about changing what is recorded. A show saved with it does not open on an older version.

- **As In:** out times and delays can follow their in time, also in *New cue times* (see
  [Times per cue](cuelists.md#times-per-cue)).
- **Release fades intensity** over the release time.
- **The Times view** in the Cue list window sets one time on many cues.
- **The Cue view** shows effects and takes out a value, an effect or a whole fixture. **Off** and **On** take a value
  out of play and bring it back. Each row has **View** and **⋯**, and a one-cue playback shows there too.
- **Include** loads a cue back into the programmer, from a handle or a cue row.
- **Update** offers what it can store into, cues and palettes, on softkeys.
- **Record over a group** asks Replace, Merge or Cancel, and a group master can change its group
  (see [Groups](programming.md#groups)).
- **Create List and Create Chase** stay in Record, so you build a list by making a look and tapping its handle
  (see [Stay in Record](cuelists.md#stay-in-record)).

## 0.4.1

Published 4 November 2026 as "CMDMX 0.4.1".
[Release on GitHub](https://github.com/Aottens/CMDMX/releases/tag/v0.4.1)

A small patch from the review of an operator's evening with 0.4.0.

- **A built-in dimmer:** *Generic* / *Dimmer* is always in the library, with a 1-channel and a 16-bit mode, also
  without QLC+.
- **Keys off says so:** with **Keys** off, a Mac key the console would use or a click on the command line says how to
  turn Keys on, or to use the keypad (see [the command line](programming.md)).
- **A function's message goes with it:** the hint, refusal or delete text of an armed function goes when it disarms.
  A warning from the engine stays until the next command, also through Cancel.

## 0.4.0

Published 3 November 2026 as "CMDMX 0.4.0".
[Release on GitHub](https://github.com/Aottens/CMDMX/releases/tag/v0.4.0)

This release makes functions and their consequences behave the same everywhere (see
[What an armed function does](reference.md#what-an-armed-function-does)).

- **An armed function never does the normal thing:** every tap does what the table in the manual says, or the
  message line says where the function works. A Flash or Swop pressed while armed does not change the stage.
- **The command line reaches an armed function:** `Group 5` or `Palette 3` with Enter acts as a tap, and Enter
  on an empty line confirms a waiting Delete.
- **Esc goes one level up** on every screen, whatever Keys says, and never changes the stage. Opening a screen
  disarms.
- **Lock from anywhere:** a **Lock** button in the status bar and ⌘L, on every screen.
- **A delete says what it does** before you confirm: how many cues on how many playbacks use a palette, that a
  running playback's output ends, that a cue is the live one. Under **Confirm remove** in Patch: where a fixture
  is used and what ends up empty.

## 0.3.1

Published 3 November 2026 as "CMDMX 0.3.1".
[Release on GitHub](https://github.com/Aottens/CMDMX/releases/tag/v0.3.1)

This release fixes defects in what was already there (see [Running a show](running.md) and
[Patching](patching.md#the-patch-table)).

- **Letting go still lets go:** a Flash, a Swop or a momentary DBO you hold when the lock comes ends when
  you let go, from the app, MIDI, OSC or a Stream Deck.
- **The restart line tells the truth:** "Engine restarted: Demo is back, nothing is running" only appears
  once the engine is really back, with the name of your show, and goes at your next command.
- **An engine that keeps failing:** the app waits longer each time, up to 30 seconds, and the status bar says so.
- **Removing a fixture** also takes it out of cues, palettes and effects, including an effect that is running.
  The other fixtures close up, and Undo brings it back in its place.
- **Changing a fixture's mode** takes out the effects the new mode cannot do, with a warning. Palettes keep
  what they store, and a warning names each palette that holds something the new mode cannot do.

## 0.3.0

Published 2 November 2026 as "CMDMX 0.3.0".
[Release on GitHub](https://github.com/Aottens/CMDMX/releases/tag/v0.3.0)

This release makes the console harder to stop or disturb by accident (see
[Running a show](running.md)).

- **The Mac stays awake** while the engine runs, and in Console the display stays on too. A closed lid,
  Sleep in the Apple menu or an empty battery still put the Mac to sleep.
- **Quitting always asks**, with Cancel as the default, whether Console is on or not.
- **Level bars never jump:** a drag moves the level by the distance you drag, so a stray click changes
  nothing.
- **Lock** covers the console with a code while the show keeps running. Input from the app, MIDI, OSC and
  Stream Deck stops until you unlock.
- **Messages** keeps a history of warnings and notices: click the message line or the amber count.
- **An engine that stops is restarted**, also after you reopened the app following a crash. The show is
  back, but nothing is running, so start your look again.
- **New cue times** in the Settings tab set the times of the cues you record from then on.

## 0.2.7

Published 1 November 2026 as "CMDMX 0.2.7".
[Release on GitHub](https://github.com/Aottens/CMDMX/releases/tag/v0.2.7)

The message line now names a group however you select it (see [Groups](programming.md#groups)).

- Tapping a group, typing `Group 3 Enter` and pressing its button on a control surface all show
  "Group 5 Pars outside in".
- An empty group says "Group 7 is empty", and the selection stays.

## 0.2.6

Published 1 November 2026 as "CMDMX 0.2.6".
[Release on GitHub](https://github.com/Aottens/CMDMX/releases/tag/v0.2.6)

Groups can now be managed from the Groups window (see [Groups](programming.md#groups)).

- **Delete** a group, confirmed on softkey A. A group that a group master uses is not deleted: the
  message line names the handle.
- **Copy** and **Move** a group to an empty cell. Group masters follow a Move, so nothing changes on
  stage.
- **Set Legend** names a group. The name shows in the cell, on the group master's strip and on the
  message line when you tap the group.
- A strip without an icon gives its legend the full width.

Shows from older versions load unchanged; saving one writes the new show version.

## 0.2.5

Published 1 November 2026 as "CMDMX 0.2.5".
[Release on GitHub](https://github.com/Aottens/CMDMX/releases/tag/v0.2.5)

Selection orders: the order of your selection now steers fans, effects and timing (see
[Selecting fixtures](programming.md)).

- **Order** reverses, shuffles or sorts the selection, or reverts it to the order you picked. A
  dimmer chase on a shuffled selection is a random chase.
- **Pattern** lays Odd/Even, 1 in *n*, *n* in *m*, Blocks, Divide, Outside in or Inside out over
  the selection; Next steps through it, an amber `GRID` tag shows where you are. Fan, new effects
  and palette overlap follow the pattern: Outside in with a Fan Line mirrors around the middle.
- A group remembers its order and pattern; a cue remembers its order, so an overlap replays who
  starts first. Playback Options ▸ Times ▸ **Fixture order** edits it by tapping the fixtures.
- The Effect editor has **Flip** and Reverse, Shuffle and Sort for a running effect.
- While Fan is on, Next, All, Order and Pattern stay on the softkeys.

## 0.2.4

Published 29 October 2026 as "CMDMX 0.2.4".
[Release on GitHub](https://github.com/Aottens/CMDMX/releases/tag/v0.2.4)

The FX generator: fixtures that move by themselves (see [Shapes and effects](effects.md)).

- A **Shapes** workspace (F5): select fixtures, tap a look such as a dimmer chase, a circle or a
  rainbow wave, and it runs at once around what the fixtures are already doing. **Size**, **Speed**
  and **Spread** shape it, on the screen or on the encoders.
- An effect's tempo can come from its own BPM, Tap, or a BPM or Rate master.
- Effects record into cues and into palettes. A timed Go between two circles carries on smoothly;
  the playback fader makes the effect smaller. An effect palette ("circle around the pulpit")
  runs on whatever is selected.
- A number then **Clear** (`3` Clear) clears the programmer over that time; effects shrink to
  nothing instead of stopping with a jump.
- Record ▸ **BPM Master** and **Rate Master** put the master on the first empty handle of the
  page, or say which page has room.
- The washes of the Demo show start pointing at the middle, so a circle runs around the centre.
- A cue that holds only an effect lists its attribute group in the Cue list window.

## 0.2.3

Published 29 October 2026 as "CMDMX 0.2.3".
[Release on GitHub](https://github.com/Aottens/CMDMX/releases/tag/v0.2.3)

- Groundwork for the FX generator: the engine runs effects in the programmer, cues and palettes,
  and the app can talk to it about them. There are no controls for effects in the app yet; they
  come in 0.2.4.

## 0.2.2

Published 29 October 2026 as "CMDMX 0.2.2".
[Release on GitHub](https://github.com/Aottens/CMDMX/releases/tag/v0.2.2)

- Gel numbers: the colour picker has a **Gels** side with the LEE gel book. Search by number or
  name and tap a swatch, or type `Lee 201` on the command line (see
  [The gel book](programming.md#the-gel-book)).
- The fixture types of the Demo show can be opened in the profile editor (Patch ›
  **Edit profile…**), and a change there reaches every fixture of that type.

## 0.2.1

Published 28 October 2026 as "CMDMX 0.2.1".
[Release on GitHub](https://github.com/Aottens/CMDMX/releases/tag/v0.2.1)

- Extra emitters (White, Warm White, Amber, Lime) can be mixed automatically from the colour you
  pick: switch **Auto-mix emitters** on for a fixture type in the profile editor, with each
  emitter's colour and output. A white on an RGBW par then comes from its White LED; the emitter
  wheels show **+AUTO** and what you set on them comes on top.
- A **K** colour model: one Kelvin wheel from 1800 K to 10000 K.
- Colour-correction steps in the colour picker: CTO and CTB in ⅛, ¼, ½ and Full, working like real
  correction gels.
- When the engine stops with the app, the quit question now says so.

## 0.2.0

Published 28 October 2026 as "CMDMX 0.2.0".
[Release on GitHub](https://github.com/Aottens/CMDMX/releases/tag/v0.2.0)

- The app is now called CMDMX. Shows and settings move over from the old folders by themselves
  on the first start.
- A website with the latest download and this manual.
- The app tells macOS why it asks for Local Network access (Art-Net and sACN).
- The release build can be signed and notarised by Apple; this release is not yet notarised, so
  the first open still needs the extra step described in [Getting started](getting-started.md).

## 0.1.0

Published 27 October 2026 as "ColourmotionDMX 0.1.0", before the rename. <!-- brand: old-name -->
[Release on GitHub](https://github.com/Aottens/CMDMX/releases/tag/v0.1.0)

The first installable version. It contains:

- A DMX engine at 40 Hz with Art-Net, sACN and ENTTEC Pro compatible USB output.
- Patching from a large fixture library, a profile editor and GDTF import.
- The programmer, with a command line, wheels, a colour picker and palettes.
- Playbacks, cue lists and chases, with pages and fader modes.
- MIDI, OSC and Stream Deck control.
- Shows saved automatically, and Undo and Redo.
- The engine keeps output running if the app crashes.

It is installed from a DMG that is not notarised, so the first open needs the extra step described
in [Getting started](getting-started.md).
