# Cue lists

A cue list is a playback with more than one cue. The cues are numbered and run one after the
other: each **Go** starts the next one, with its own times. Some cues can follow on by themselves.
A cue list sits on a handle like any playback, with the same options for priority, release,
playback groups, paging, legend and halo (see [Playbacks and cues](playbacks.md)).

## Building a cue list

Start with a playback on a handle (see [Recording a playback](playbacks.md#recording-a-playback)).
Then, for each next cue, set up the look in the programmer, press **Record**, tap the handle and
choose:

- **Append**: a new cue at the end. Its number is the last whole number plus one: after Q7 comes
  Q8.
- **Insert**: a new cue right after the current cue, numbered half way to the next one: between
  Q2 and Q3 it is Q2.5. Go runs it next.

To give a cue a number of your own, press **Record**, type the number on the keypad, then tap the
handle: `Record`, `4.5`, tap the handle records Q4.5. If that cue exists, the softkeys ask
**Replace**, **Merge** or **Update**.

Cue numbers go from 0.001 to 9999.999, with up to three decimals. The list always runs them in
number order. A new cue gets the *New cue times* of the Settings tab in Playback Options (in a new
show 0 s, the out times As In and overlap 100 %, see [Times and fader modes](playbacks.md#times-and-fader-modes)) and waits for Go.

### Stay in Record

To build a list in one go, press **Record**, then softkey **Create List**, or **Create Chase** for a chase. Tap an
empty handle: the programmer becomes the first cue and Record stays armed on that handle. Make the next look, then tap
the handle or press softkey **Append** for the next cue. A number on the keypad before the tap records to that number.

While you build, fixtures, groups, palettes and the command line do their normal work, so you can make the look; only
the handle stores. Another handle and a cue row are refused. Tapping a used handle first appends to it; Create Chase
refuses one that is not a chase. Softkey **Exit**, Esc or opening a screen ends it. Each cue is one undo step. Go and
Space stay live.

You do not need to Clear between cues. With tracking on (the default), each cue stores only what
you changed, and the rest carries on from the cues before it. See [Tracking](#tracking).

## Running a cue list

Raise the fader of a cue list and its first cue runs. After that, press **Go**:

- the **GO** button in the Go block;
- the upper button of the list's own strip (labelled **GO**);
- **Space** on the Mac keyboard, while **Keys** is on in the status bar.

The Go block, on the right of the playback area, shows the **connected** cue list (see
[The connected list](#the-connected-list)):

- the header: the list's legend and handle, and the fade times of the live and the next cue
  (`live 3s · next 2/4s` means a fade in of 2 s and a fade out of 4 s). An amber `fader 40 %`
  warns that the list's fader is not at full, so a Go may look dimmer than you expect. At 0 % it reads
  `fader 0 % · no output`: a Go then runs the cue without anything on stage;
- the **now** line, large and green: the current cue's number and legend, and the progress while
  it fades. *paused* in amber while the list is paused;
- the **GO** button, with the next cue in small grey text (`Next: Q4 Sermon`). It reads *end of
  list* and is disabled when there is nothing next;
- **Back**, **Stop** and **Release** below it.

![The Playbacks workspace with the Cue list window, Active playbacks and the Go block](images/cue-list.png)

*The Playbacks workspace (F4): the Cue list window with Q2 Welcome running (green) and Q3 next
(outlined), Active playbacks next to it, and the Go block on the right showing the same list.*

What the keys do:

- **Go** starts the next cue. On a list that is off, it starts the list at its next cue, which is
  the first cue unless you tapped another row.
- **Back** goes to the cue before the current one, with that cue's fade times but without its
  delay. Shift+Space does the same while Keys is on. On the first cue it is refused.
- **Stop** pauses the list: every fade and every waiting link stops where it is, and the light
  freezes. **Go** then carries on from there. **Stop** on a paused list goes back one cue.
- **Release** lets the list go (see [Release](playbacks.md#release)). The next time it starts
  from its first cue again.

When Go reaches the last cue, the list stays there: Go has nothing next. With **Loop** on the Cue
list tab set to *Loop*, the first cue comes next after the last one.

### A second Go too soon

Space and a mouse repeat easily in the dark. A Go that comes less than half a second after the last
Go the same list took is ignored, with the warning "Go ignored: 0.3 s after the last Go (guard
0.5 s)", so a double press never skips a cue. It counts the same from every source: the Go block,
Space, a handle button, a control surface, the phone or OSC. Back, Stop, Go to cue, setting the
next cue and a Go that resumes a paused list are never ignored. Each list has its own guard, so two
lists can get Go at once.

The time is the setting **Ignore a second Go within** on the Settings tab of Options, 0 to 2 s
(default 0.5 s). Set it to 0 for no guard, as on a Titan. It is saved with the show.

## The Cue list window

The Cue list window is in the **Playbacks** workspace (F4). Its title shows the connected list,
its handle and how many cues it has. Each row is a cue:

- **Q**: the cue number.
- **Legend**.
- **Delay** and **Fade**: the fade shows `2` when in and out are the same, else `2/4`.
- **Link**: `·` waits for Go, `follow +1s` and `with +2s` start by themselves (see
  [Links](#links)).
- **Track**: `·`, or B, CO or S for a Block, Cue Only or Solo cue.
- **Groups**: the attribute groups the cue holds, its effects included (a cue with only a circle shows P).

The current cue is green, the next one has an outline, and a fading cue shows its progress along
the bottom of its row.

The window **follows** the live cue: after a Go, Back, a link, Go to Cue or another connected list it scrolls so
the current cue and the one after it are in sight, with one row above when there is room. A cue that was already
in sight does not scroll. **Follow** at the top of the window turns this off and on; the app remembers it. Scrolling
by hand does not turn it off, so the next Go brings the live cue back.

**Tap a row** to make that cue the next one. Nothing changes on stage until you press Go, so a
tap in the list can never cue the wrong thing. **View** at the end of a row shows the
[Cue view](#the-cue-view) and **⋯** the cue softkeys; neither changes the stage either. With a function
armed the whole row is its target, these two buttons included.

**Tap the legend of a playback with one cue** to show that cue here, titled "Cue · 1.2": one row, and its
softkeys are View Cue, Edit Times and Set Legend. It changes nothing on stage and the connected list stays
connected, as a memory has no Go. A tap on a list's legend, another connected list or **Esc** brings the list
back. Outside the Playbacks workspace the message tells you where to look.

**Times** at the top of the window shows only the timing columns: Delay in, Delay out, Fade in,
Fade out and Overlap, an out time that is *as in* dim (see [Times per cue](#times-per-cue)). A chase
has no Times view. In this view a tap selects a time and does not change Next:

- **Tap** a time to select it; **drag** down the column, or **Shift-tap**, to select several. The softkeys
  show what you have, as "Q2–Q5 · Fade out".
- Type the number on the keypad and press **Set** or **Enter**: every selected cue takes it, and one
  Undo takes them all back. **As In** links the out times again. **Done** ends the selection.
- A number the console refuses changes none of them, and a change to a cue that is fading now is used
  from the next Go on. Arming a function clears the selection. **Esc** clears it, and again leaves the view.

**Right-click a row**, or press **⋯**, for the cue softkeys:

- **Go to Cue**: runs that cue now, with its own times.
- **View Cue**: shows everything the cue puts on stage (see [The Cue view](#the-cue-view)).
- **Edit Times**: opens the Times tab of Playback Options for that cue.
- **Set Legend**: a name of up to 32 characters.
- **Renumber** and **Copy**: type a free number on the keypad first, then press the softkey.
- **Delete**: confirm on softkey **A**. The last cue of a playback cannot be deleted; delete the
  playback instead. The message says when it is the live cue: the output stays until the next Go. With the **Delete**
  function armed, **⇧-tap** a range of rows or **⌘-tap** to add or drop one; softkey A names the count. Every cue of
  a playback cannot go at once: delete the playback instead.

**Move or copy a block of cues**, a song of four cues for instance: arm **Move** or **Copy** and tap a row; **⇧-tap**
picks a range, **⌘-tap** adds or drops one. Then type the first new number and press **Enter** (or **Renumber** for Move,
**Copy** for Copy), or press **End** to put them after the last cue. The cues keep their order and take that number,
the next whole number and so on: Q10–Q13 to 21 become Q21–Q24. To send them to another cue list or chase, tap its
handle before the number ("Move 4 cues to 1.3: type the first number"). If a new number is already taken by a cue that
is not moving, nothing happens and the message names it ("Q21 is in the way: renumber or pick another number"): nothing
is renumbered without asking. A copy is the whole cue, legend, times, effects and switched-off values included; a moved
cue keeps what it holds, and in a tracking list it tracks from the cues before its new place. The live cue of a running
list cannot move to another list, and a list cannot give away all its cues. One Undo takes the whole block back.

**Record**, then a tap on a row, records onto that cue: **Replace**, **Merge** or **Update**.
**Include**, then a tap on a row, puts what that cue holds itself in the programmer: nothing tracked and
nothing that is off, so **Merge** afterwards turns nothing tracked into a value of this cue. See
[Playbacks](playbacks.md).
**Renumber All** on the Cue list tab of Playback Options numbers the cues 1, 2, 3 … in their order.

## Times per cue

Every cue has its own times. Open **Playback Options** on the list (or **Edit Times** on a row)
and go to the **Times** tab. The **Cue** menu at the top picks the cue; it starts on the current
cue. To set one time on many cues at once, use the [Times view](#the-cue-list-window) of the Cue list window. For that cue you set:

- **Delay**, **Delay out**, **Fade in**, **Fade out** and **Overlap**. Intensity that goes up uses
  the delay and the fade in; intensity that goes down uses the delay out and the fade out. Colour,
  position and the rest use the delay and the fade in.
- **As In**: an out field that says *as in* has no number of its own, and takes the delay or the fade
  in, whatever you make those later. This is how a new cue starts. Type a number to give it its own
  time; empty the field and press Return to link it again. A cue with fade in 3 and an empty fade
  out goes down over 3 s, and with fade out 5 over 5 s. A 0 is a time: that intensity drops at once.
  Back has no delay at all, out included. Group times replace all four, and a chase has neither.
- **Link** (next part).
- **Track** (see [Tracking](#tracking)).
- **Group times**: a delay and fade of its own for any attribute group. Tap **—** under a group
  to give it its own times; **×** gives it back the cue's times. A cue with a fade of 2 s and
  position at delay 1 s, fade 5 s brings up the colour in 2 s and starts the movement after 1 s,
  so the heads arrive after 6 s.

![The Times tab of Playback Options for cue Q3, with group times for position](images/cue-times.png)

*The Times tab for Q3 Worship: fade 3 s, and position with its own delay of 1 s and fade of 5 s.*

A change to the times of a cue that is fading now is used from the next Go on.

### Fixture order

With an overlap below 100 %, a cue does not fade all fixtures at once: it starts them one after
the other, in the cue's **fixture order**. Recording a cue stores the order of the selection, so
a selection you reversed, shuffled or laid out Outside in (see
[Order](programming.md#order)) plays back that way. A cue recorded without a selection
goes by fixture number.

The **Fixture order** part of the Times tab shows the cue's fixtures with their step number
(step 1 starts first). To change it, press **Edit**: the fixtures keep their numbers, and every
tap overwrites the step of the fixture you tap, starting at step 1:

- With **Auto increment: On** (every Edit starts with it on), every tap gives the next step: tap
  205, 204, 206, 203 … to go from the middle out.
- With **Auto increment: Off**, the taps share one step, so several fixtures start together;
  **Next step** moves on to the next one. Tap 204 and 205, Next step, 203 and 206, and so on.

**Done** stores the new order (Undo takes it back) and shows **Saved** for a moment; the grid
then shows the stored order. Fixtures you did not tap keep the step they had. **Cancel** leaves
the order as it was.

### Links

A cue's link decides how it starts:

- **Wait** (the default): it waits for Go.
- **After**, with a number of seconds: it starts by itself that long after the cue before it has
  finished fading. Use this to follow on: Sermon, then the video cue starts when Sermon is done.
- **With**, with a number of seconds: it starts that long after the cue before it *started*.

Links follow after Go, Go to Cue and other links, not after Back. **Stop** pauses a waiting link,
and **Go** while a link is waiting starts that cue at once.

## The fader on a cue list

The fader of a cue list is its intensity master: it scales the intensity of the whole list at
once. The cue times do the fading, so the fader modes of a single look are not used. The **Cue
list** tab of Playback Options sets:

- **Fader**: what 0 means. *Kill with Off* (the default) keeps the list on its current cue: the
  intensity goes out, everything else stays, and Go still works. Only Release stops it.
  *Kill at 0* stops the list when the fader reaches 0, and it starts from the top next time.
- **Fire first cue** (on by default): raising the fader of a list that is off runs its first cue.
  Off: the list comes alive with nothing on stage until you press Go.
- **Loop**: *Stop on final* (the default) or *Loop*.
- **Tracking**: on or off (next part).

The handle's **Flash** works as on a single look; on a list that is off, it starts the list.

## Tracking

With **tracking** on (the default), a cue holds only the values you recorded into it: its own
values. Everything else **tracks** from the cues before it. If Q3 changes only the colour of the
washes, the pars stay as Q2 left them, and you never have to record the pars again.

When you jump to a cue (Go to Cue, or Back), the light goes to what that cue would show had you run
the list from the top: its own values plus the tracked ones, all in that cue's times.

With tracking off, a cue shows only its own values. Intensity that a cue does not hold fades out.

Per cue, the **Track** option on the Times tab changes how it takes part:

- **Track** (the default): as above.
- **Block**: nothing tracks into this cue; it shows only its own values. Later cues track from it.
- **Cue only**: its values do not carry on. The cue after it shows what was there before, so a
  video cue that takes the pars down brings them back on the next Go.
- **Solo**: Block and Cue only together.

### Updating a tracking list

**Update** and **Merge** on a cue of a tracking list store each changed value by the **update
mode**. The show-wide default is on the Settings tab; the **Update Mode** softkey in the Record
softkeys changes it for one record:

- **Forwards** (the default): in this cue; later cues that do not hold the value take it too.
- **Cue Only**: in this cue only. The next cue gets the old value, so later cues stay as they were.
- **Backwards**: in the cue the value tracks from, so every cue in between changes too, and not
  after this cue.
- **Both**: in the cue the value tracks from, and onwards until a cue holds the value itself.

On a tracking list, Update takes the values the cue shows, tracked ones included. With tracking
off, every mode stores in the cue itself.

### The Cue view

**View Cue** shows the whole state of a cue in place of the Cue list window: a row per fixture,
with columns for intensity, position, colour and beam. Values the cue holds itself are in normal
text, tracked values are **magenta**, and palette references are **cyan** with the palette's
name. When a fixture's values in one column all come from the same palette, the cell shows the palette once:
`■ Warm` for a colour palette, so a cue of twelve pars in one colour stays one line per fixture. Right-click such a
cell to see its values; then a right-click on a value picks it as below.

Right-click a value the cue holds itself and press **Remove** (then confirm) to take it out of the
cue, so it tracks from the cues before it again. A tracked value cannot be removed here: remove it
from the cue it comes from. **Back to List** returns to the Cue list window.

Under the fixtures, each effect of the cue has a row: its look or shape, its attributes and how many fixtures it
runs on, **magenta** when it tracks from an earlier cue. Right-click an effect the cue holds itself and press
**Remove**, then confirm: it stops on stage on the next tick when that cue is live.

Right-click a fixture number to take the whole fixture out: **Remove** names it ("fixture 201", or "901.2" for a cell)
and takes out what this cue holds of it, its values and its place in the cue's own effects. A cell row is taken out on
its own. What tracks from an earlier cue stays visible.

**Off** takes a value out of play without forgetting it: the cue then behaves as if it did not hold that value, so the
attribute tracks from the cues before it, and a live cue shows that on the next tick. It is not a recorded zero. Right-click
a value the cue holds itself, or a fixture number for all of it, and press **Off**. The value stays in its column, struck
through and dim. Right-click it and press **On** to bring it back (a fixture number brings back all of that fixture's).
Recording or merging that attribute into the cue stores the new value and forgets the old one; Replace forgets all of
the cue's values that are off; Update and Include do not read them.

## The connected list

The Go block, Space and the Cue list window always work on one cue list or chase: the
**connected** list. Its strip has a blue frame. It changes when:

- you tap the legend of a list's strip (with no function button armed);
- a list starts, or gets Go from its own handle, while **Auto connect** is on (the default, on the
  Settings tab);
- the connected list is deleted or there was none: the first list in the show takes over.

Connecting a list never changes what is on stage. Each list on a handle keeps its own Go button,
so you can run two lists at once.

## Handle buttons

The two buttons of a handle follow what is on it. The **Handle buttons** setting on the Settings
tab chooses the set for the whole show:

- **Best practice** (the default): Swop and Flash on a single look, Go and Flash on a cue list, Tap
  and Flash on a chase, Tap on a BPM master, Flash on a group master.
- **Titan**: Swop and Flash on every kind (Tap on a BPM master). Go is then only in the Go block
  and on Space.
- **Custom**: pick both buttons per kind, from what that kind can do: Flash, Swop, Go, Back, Stop,
  Tap or Release.

A handle's own Go always works on its own list, whichever list is connected.

Next: [Running a show](running.md).
