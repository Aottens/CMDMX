# Taking over a show

You did not program this show and you have to run it. This page is the short way in. Each section
links to the full chapter.

**(stage)** marks what changes the stage. Not in a live moment.

## Which show is open

The status bar shows the open show and how long ago it was saved. To see the others, press **Show**
in the rail on the left, or the **Disk** function key. The open show is marked `open` in the list.

To open another: select it in the list, press **Open**, then press **Open** again to confirm.
**(stage)** The stage changes to that show. **Save As** makes a copy under a new name and changes
nothing on stage. More in [The Show screen](shows.md#the-show-screen).

## What is on the handles and pages

A handle is a place for a playback, written page and slot: **2.3** is slot 3 on page 2. The page
buttons on the left of the playback area switch pages, and changing the page never changes what is
running. A green strip along the bottom
of a strip is running, an amber one is releasing, and an empty handle says *empty*.

To see everything that is running, on any page, open the **Active playbacks** window in the
**Playbacks** workspace (F4). A tap on a row there releases that playback **(stage)**.

What a handle holds depends on its strip:

- **A single look** has one cue. The **Playbacks** workspace (F4) shows it under **Active
  playbacks** once it runs; a tap on its legend only tells you so and changes nothing on stage.
- **A cue list** shows the cue it is on and the next one, such as `Q3 › 4`. Tap its legend to
  connect it: the **Go** block and the Cue list window follow it. This does not change the stage.
  Tap a row to make that cue next. Nothing happens until you press **Go**.
- **View**, at the end of a row of the Cue list window, shows everything that cue puts on stage,
  fixture by fixture. It changes nothing on stage. **Back to List** returns.

More in [Handles and strips](playbacks.md#handles-and-strips) and
[The Cue view](cuelists.md#the-cue-view).

## What a group holds

A group is a saved selection of fixtures. The **Groups** window (in the **Fixtures + Groups**
workspace, F2) shows each group with its name and its number of fixtures.

Tap a group and its fixtures are selected, in the group's own order, and the message line names
it. The selection is only a selection: it puts nothing on stage by itself. It does change what a
wheel or value you touch next applies to, so check what is selected first. Tap it again to
deselect. **Clear** deselects everything. More in [Groups](programming.md#groups).

## Which lamp a fixture is

1. Press **Patch** in the rail. The table lists every fixture by number, name, profile and
   address (`universe.address`). See [The patch table](patching.md#the-patch-table).
2. To see which lamp it is, go back to the **Fixtures + Groups** workspace (F2), tap the fixture
   button, and press **Highlight**. **(stage)** The selected fixtures go to full in open white.
3. Press **Highlight** again, or deselect, to switch it off. It is never recorded.

See
[Locate, Highlight and Rem Dim](programming.md#locate-highlight-and-rem-dim).

## Running it

1. Find the playback on its page and raise its fader. A cue list starts at its first cue.
2. For a cue list, press **Go** on its strip or in the **Go** block. **Space** is **Go** only while
   **Keys** is on in the status bar. **Back** and **Stop** are in the same block.
3. To let a playback go, press **Release** and tap its handle **(stage)**.

More in [Running a cue list](cuelists.md#running-a-cue-list) and [Running a show](running.md).

### A late change with Update

1. Select the fixtures and change the value. It goes into the programmer, shown in red.
   **(stage)** It is on stage at once.
2. Press **Update** and then tap the playback's handle. The programmer's values are stored into
   that playback, but only for the attributes and fixtures it already holds.
3. Or press **Update** and tap nothing. The softkeys offer each running playback whose current cue
   holds something you changed, ticked, and palettes holding it, not ticked. Press **Store**.
   A palette you tick changes every cue that uses it.
4. Press **Clear** twice: the first press deselects, the second empties the programmer.

A running playback you update changes on stage without a fade. On a cue list, Update works on the
current cue. To update another cue, press **Record**, tap its row in the Cue list window, then
**Update**. See
[Playbacks](playbacks.md) and [Cue lists](cuelists.md#updating-a-tracking-list).

## Why is it dark?

Check in this order.

1. **DBO.** Dead black out sets all intensity to 0. You see it as an amber **DBO** button under the
   faders on the right, and a **DBO** tag in the status bar. Press the button to switch it off.
   **(stage)**
2. **Grand Master.** The **GM** fader on the far right scales everything. It turns amber below
   100 %. Drag it up. **(stage)** It is the same on every page.
3. **The playback's fader.** Go to the page of the playback that should be lit and look at its
   level bar. A level of 0 is dark. The **Go** block shows an amber `fader 40 %` for the connected list. Raise it. **(stage)**
4. **Group masters, on every page.** A group master sits on a handle and darkens its group, whatever
   else brings it up. While one does, the status bar shows an amber **GRP** tag: click it and the
   message line says which, such as `Master Zaal (1.8) holds Group 3 at 0 %`. The same masters are
   listed in amber at the bottom of **Active playbacks**; a tap on one goes to its page. Raise its
   fader. When a show opens, each master that holds its group down says so in the message line.
   A strip that says `LOCKED` and its level in amber is disabled: its fader does nothing. To free
   it, press **Options**, tap the strip, and on the **Master** tab switch **Disabled** off. See
   [Group masters](running.md#group-masters).
5. **The Output screen.** Press **Output** in the rail. The fixture's universe needs an enabled
   node with a green dot. The **On** column switches a node on, and a line such as
   `Universe 2: 6 fixtures, no output` is amber when none is. Amber or red means a cable or
   network problem.

Still dark? The rest of the checks are in [Troubleshooting](reference.md#no-light).

## What Undo can reach

**Undo** (Command-Z, or the **Undo** softkey, which names what it will undo) steps back through
edits: recording, editing, deleting, patching, and also taps on the selection. Redo is Shift-Command-Z. It never touches what is running: Go, Back, faders, Flash, pages, the Grand Master
and DBO are not steps.

For you that means:

- The history belongs to the engine, not the app. Quitting and reopening the app, or a different
  person at the desk, does not clear it. As long as the engine ran on, the last operator's edits
  are still there, newest first.
- It holds the last 100 steps. Restarting the engine empties it.
- Each press undoes one edit **(stage)**, such as a look someone recorded or a patch change. The
  first presses may only undo selection taps. Read the **Undo** softkey before each press.

More in [Undo](running.md#undo) and [Undo](programming.md#undo).

## A forgotten lock code

If the console shows "Locked" and nobody knows the lock code, type the master code `68340` and
press **Enter**. It always unlocks. It is printed in the manual on purpose. See
[Locking the console](running.md#locking-the-console). While locked, output keeps running and the app's buttons do nothing.
