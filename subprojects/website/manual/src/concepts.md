# Concepts

This chapter explains the words you will meet in CMDMX.

## Fixtures and patch

A **fixture** is a lamp. A **profile** describes its channels: what each DMX channel does, and
which modes it has. CMDMX comes with a large library of ready-made profiles, and you can make your
own from a plain list of channels.

The **patch** is the list of your fixtures: which profile, in which mode, on which universe and
DMX address, and with which user number. You use the user number when you select a fixture. The
patch refuses two fixtures that would overlap.

## The programmer

The **programmer** is your scratch pad. When you select fixtures and change a value, the change
goes into the programmer, shown in red. It is what you are working on right now, and it sits above
what the playbacks are doing. **Clear** empties it: the selection first, then the values, and the
playbacks show through again.

## Attribute groups: I P C G B E S FX

Every fixture value (an *attribute*) belongs to a group, named by a letter. The attribute bar lets
you switch between them.

- **I** is intensity: dimmers and brightness.
- **P** is position: pan, tilt and similar.
- **C** is colour.
- **G** is gobo: patterns and gobo wheels.
- **B** is beam: focus, zoom, iris and the like.
- **E** is for effect-like optics, such as a prism.
- **S** is for control channels: speed, built-in effects and maintenance.
- **FX** is for the effects generator, which is not built yet.

Which group an attribute belongs to is set in the fixture's profile.

## HTP and LTP

When several sources want to set the same value, CMDMX has to pick one.

- **HTP** (highest takes precedence) is used for intensity. The highest value wins. Two playbacks
  at 60 % and 40 % on the same dimmer give 60 %.
- **LTP** (latest takes precedence) is used for other attributes such as colour and position. The
  most recently activated source wins. When it is released, the value returns to the one before.

## Priority

A playback has a priority: Low, Normal, High, Programmer or Very High. A source never changes an
attribute that a higher priority already holds. You set it in the playback's options. Normal is
the default.

## Palettes are references

A **palette** is a saved colour, position or beam setting. When you record a cue or playback with
a palette applied, the cue stores *which palette* it used, not a copy of the numbers.

This means that if you change a palette later, every cue and playback that uses it follows. Fix a
colour once and the whole show is updated.

## Playbacks and cue lists

A **playback** is something you can play from a handle: a fader and buttons. It can hold:

- a single look;
- a **cue list**, a sequence of cues you step through with Go and Back;
- a **chase**, which steps through its cues at a tempo.

Cues have fade times. A cue list can loop, link to other cues and track values from one cue to the
next.

## Pages

Playbacks are arranged on **pages**. A page is a bank of handles, and the fader strips at the
bottom of the screen show the current page. Switching the page changes which playbacks your faders
and any connected controller are working with.

The number of faders per page is a setting of the show. The default is eight. The screen and
control surfaces follow it, so it is not fixed at eight.

Next: [Patching](patching.md), [Programming](programming.md), [Palettes](palettes.md),
[Playbacks and cues](playbacks.md), [Cue lists](cuelists.md) and [Running a show](running.md) cover
these in detail.
