# Reference

Lists to look things up: keyboard shortcuts, OSC addresses, the engine's command-line options,
where CMDMX keeps its files, what to check when something does not work, and the words this manual
uses.

## Keyboard shortcuts

These always work:

| Keys | What it does |
| --- | --- |
| ⌘1 to ⌘4 | the workspaces Groups + Palettes, Fixtures + Groups, Colour, Playbacks |
| ⌘Z | undo the last edit (not while typing in a text field) |
| ⇧⌘Z | redo |
| Esc | ends Learn mode (see [Learn](controls.md#learn)) |
| ⌘Q | quit; with **Console** on it asks first |

These work while **Keys** is on in the status bar, and never while you are typing in a text field
(see [The keyboard](running.md#the-keyboard)):

| Key | What it does |
| --- | --- |
| F1 to F4 | the four workspaces |
| Space | Go on the connected cue list (holding it down goes once) |
| Shift+Space | Back on the connected cue list |
| 0 to 9, `.` | into the command line |
| `-` | Thro |
| `+` or `=` | And |
| `@` | @ |
| `f`, `g` | Fixture, Group |
| `p` | Palette; on an empty line it also opens the palette menu |
| Return, Enter | Enter: runs the line |
| Esc | Exit: empties the line and leaves the mode you are in |
| Backspace | removes the last word |
| ↑ and ↓ | the lines you ran before (the last 100) |

The command-line keys work on the workspaces, not on the Patch, Output, Control and Show screens.
See [The command line](programming.md#the-command-line).

## OSC addresses

CMDMX listens on UDP port 8000 once OSC is on (see [OSC](controls.md#osc)). A button address
pressed with a non-zero value is a press and with 0 a release; sent without a value it is a press
and release at once. `<slot>` follows the page shown; `<page>/<slot>` and `<id>` do not.

| Address | Value | What it does |
| --- | --- | --- |
| `/cmdmx/fader/<slot>` | 0.0 to 1.0 | fader of a slot on the current page |
| `/cmdmx/fader/<page>/<slot>` | 0.0 to 1.0 | fader of a slot on a given page |
| `/cmdmx/playback/<id>/level` | 0.0 to 1.0 | level of playback `<id>` |
| `/cmdmx/fader/<slot>/upper`, `/cmdmx/fader/<slot>/lower` | 1 / 0 | the upper or lower button |
| `/cmdmx/go`, `/back`, `/stop`, `/release`, `/tap` | button | on the connected cue list |
| `/cmdmx/playback/<id>/go` (and `/back`, `/stop`, `/release`, `/tap`) | button | on playback `<id>` |
| `/cmdmx/page` | whole number | go to that page |
| `/cmdmx/page/next`, `/cmdmx/page/prev` | button | next or previous page |
| `/cmdmx/gm` | 0.0 to 1.0 | Grand Master |
| `/cmdmx/dbo` | 1 / 0 | DBO on while 1 |
| `/cmdmx/key/<name>` | button | a programmer key: `clear`, `locate`, `highlight`, `rem_dim`, `undo`, `redo`, `all`, `step_next`, `step_prev` |
| `/cmdmx/keypad/<token>` | button | a keypad key: `0` to `9`, `.`, `@`, `Thro`, `And`, `Not`, `Fixture`, `Group`, `Palette`, `Full`, `Enter`, `Backspace` |
| `/cmdmx/wheel/<n>` | steps | turns wheel `<n>` of the open attribute group |
| `/cmdmx/attribute_group/<g>` | button | opens group `I`, `P`, `C`, `G`, `B`, `E`, `S` or `FX` |
| `/cmdmx/group/<id>`, `/cmdmx/palette/<id>` | button | selects the group, applies the palette |

CMDMX sends back, on the feedback port (9000):

| Address | Value | Meaning |
| --- | --- | --- |
| `/cmdmx/fader/<slot>` | 0.0 to 1.0 | the slot's level on the current page |
| `/cmdmx/fader/<slot>/upper`, `/lower` | 1.0 / 0.0 | the button is lit |
| `/cmdmx/fader/<slot>/legend` | text | the slot's legend |
| `/cmdmx/page`, `/cmdmx/page/legend` | number, text | the current page and its name |
| `/cmdmx/gm`, `/cmdmx/dbo` | value | the Grand Master and DBO |
| `/cmdmx/go` | 1.0 / 0.0 | the connected list is fading |
| `/cmdmx/key/highlight` | 1.0 / 0.0 | Highlight is on |
| `/cmdmx/attribute_group/<g>` | 1.0 / 0.0 | that group is open |
| `/cmdmx/cue/current`, `/cmdmx/cue/next` | text | number and legend of the current and next cue |
| `/cmdmx/message` | text | the last warning |

A client that sent a value does not get that value echoed back; other clients do. An unknown
`/cmdmx` address is ignored.

## Engine options

You do not normally start the engine yourself: the app does that. To start it by hand, for
example to keep a protocol quiet for one run, quit the app and run the engine from Terminal:

```bash
/Applications/CMDMX.app/Contents/MacOS/cmdmx-daemon --qlcplus-dir /Applications/CMDMX.app/Contents/Resources/qlcplus/fixtures --no-sacn
```

The `--qlcplus-dir` part points the engine at the fixture library inside the app, as the app
itself does.

Then open the app. It finds the engine already running and connects to it; the status bar notes
that another engine is running on port 7319.

| Option | What it does |
| --- | --- |
| `--no-artnet`, `--no-sacn`, `--no-serial` | send no Art-Net, sACN or USB DMX this run; the show keeps its nodes |
| `--ipc-port <n>` | the port the app connects to on this Mac (default 7319) |
| `--show-dir <path>` | another shows folder; the last show and control settings are then kept in that folder too |
| `--profiles-dir <path>` | another folder for your own fixture profiles |
| `--qlcplus-dir <path>` | the folder of QLC+ fixture files (the app passes the one inside it) |
| `--demo` | fader 1 breathes, for testing |
| `--ticks <n>` | stop after n ticks (for tests) |
| `-h`, `--help` | print the options and stop |

An unknown option prints the list and stops without opening a show. The output options of older
versions (`--serial`, `--artnet-target`, `--sacn-unicast`, `--sacn-priority`) are gone: outputs are
set per show on the **Output** screen.

## File locations

| What | Where |
| --- | --- |
| Shows | `~/Documents/CMDMX/Shows/<name>.cmdmx.json` |
| A show's copy before an update | `~/Documents/CMDMX/Shows/<name>.cmdmx.json.bak` |
| Your fixture profiles | `~/Library/Application Support/CMDMX/profiles/` |
| Control surface mappings | `~/Library/Application Support/CMDMX/control/` |
| Last show, enabled devices, OSC settings | `~/Library/Application Support/CMDMX/state.json` |
| Engine log | `~/Library/Logs/CMDMX/daemon.log` (moved to `daemon.log.1` at 10 MB) |

`~/Library` is hidden in Finder: choose **Go › Go to Folder…** and type the path.

## Troubleshooting

### No light

Work from the console to the lamp:

1. **Engine** in the status bar is green. If it is grey, the app is not connected to the engine;
   see [The engine and the app](getting-started.md#the-engine-and-the-app).
2. The **GM** is up and **DBO** is off.
3. Something is sending intensity: a playback is up, or the programmer has a value.
4. The **Output** screen has an enabled node for the fixture's universe, with a green dot. A
   universe that has fixtures but no enabled node shows an amber line `Universe 2: 6 fixtures, no
   output`. Hover over a protocol dot in the status bar to see what is wrong with a node.
5. **USB DMX**: the Serial row shows `no interface` or `port not found` when it cannot find the
   interface. Choose its port in the dropdown (a Eurolite is never found by itself; see
   [Connect output](getting-started.md#connect-output)), and check the cable.
6. **Art-Net or sACN**: the Mac is on the same network as the node, and macOS was allowed to use
   the local network (System Settings › Privacy & Security › Local Network).
7. The engine was not started by hand with `--no-artnet`, `--no-sacn` or `--no-serial`.
8. The fixture's address and mode in **Patch** match the fixture itself.

### Port in use

- **The engine's port (7319)**: another engine is already running, often one started by hand. The
  status bar says `Another engine is running on port 7319`, and the app uses that engine. Quit it
  if that is not what you want.
- **The OSC port (8000)**: another program (for example another lighting program) has it. The OSC
  panel shows the error in amber. Choose another port, or quit the other program.
- **The USB interface**: only one program can use it. Quit the other lighting program, or quit
  CMDMX before you start the other one.

### macOS will not open the app

The app is not notarised by Apple yet. Open it once by hand as described in
[Install](getting-started.md#install).

### A control surface does nothing

- On the **Control** screen, the device is **On** and **Ok**, and has a mapping.
- **LAST INPUT** changes when you move a control. If not, the device is not sending, or is in the
  wrong mode (a BCF2000 has to be in a Mackie Control preset for the profile).
- A fader without a motor waits for **Pickup**: move it through the level shown on screen.
- A Stream Deck that is **Busy** is held by the Elgato app: quit that app.

### The save age is amber

The autosave is failing, usually because the disk is full or the shows folder may not be written.
The show keeps running; see [Autosave](shows.md#autosave).

### "Engine speaks protocol …"

The app and the engine come from different versions. Install the app from one DMG; the engine is
inside it.

## Glossary

- **Attribute group**: one of I (intensity), P (position), C (colour), G (gobo), B (beam),
  E (effect-like optics), S (control channels) and FX. Every attribute of a fixture belongs to one.
  See [Attribute groups](concepts.md#attribute-groups-i-p-c-g-b-e-s-fx).
- **Binding**: one input on a control device bound to one action.
- **Connected cue list**: the list that Go and Back act on when no handle is named.
- **Cue list**: a playback holding cues that run one after the other with Go.
- **DBO**: dead black out; all intensity to 0 while it is on.
- **Deck page**: a page of a Stream Deck's own keys, separate from the console page.
- **Engine**: the program that runs the show and sends DMX. The app starts it and is its screen.
- **Feedback**: what CMDMX sends back to a control: a motor fader's position, an LED, a key image.
- **Fixture**: one lamp or device in the patch, with a number and a DMX address.
- **GM**: Grand Master, the fader that scales all intensity.
- **Group master**: a fader that controls the intensity of one group.
- **Handle**: a place for a playback on a page: a fader with its buttons.
- **HTP / LTP**: highest takes precedence (intensity, the highest level wins) and latest takes
  precedence (everything else, the last change wins). See [HTP and LTP](concepts.md#htp-and-ltp).
- **Learn**: binding a hardware control by moving it.
- **Mapping**: a named list of bindings for a device or for OSC.
- **Node**: one output on the Output screen: an Art-Net, sACN or USB DMX destination for a
  universe.
- **Page**: one set of handles; the page buttons switch between them.
- **Palette**: a stored colour, position or beam look, used by reference.
- **Pickup**: a fader without a motor that waits until it meets the level before it acts.
- **Playback**: something stored on a handle that you play: a look, a cue list, a chase or a
  master.
- **Profile**: the description of a fixture type: its modes and channels. Also a mapping that
  comes with the app, such as Mackie Control.
- **Programmer**: where you build a look before you record it; it sits above the playbacks.
- **Show**: one file with everything you programmed.
- **Universe**: 512 DMX channels; one line of output.

Next: [Release notes](release-notes.md).
