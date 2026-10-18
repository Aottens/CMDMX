# Module spec: playbacks (PBK)

Owner: `cmdmx-core` (pure), `cmdmx-daemon` (IPC), the playback area in `subprojects/app`.
Status: **PBK-01..05 approved by owner 2026-10-18 (VS1-06 subset); the rest is a skeleton
to be written with P1-04 before any further code in this module.**
Behavioural reference: Avolites Titan v19, *Playbacks* and *Pages*.

## Purpose
Playbacks are what the operator runs: a fader with content, a priority and a fader mode. This
first subset only puts them on pages, so the on-screen faders (and later the BCF2000) can reach
more than one bank. Recording playbacks, their options and stored handle layouts are P1-04.

## Definitions
- **Page:** a bank of `faders_per_page` handles (Settings, default 8, never hard-coded).
- **Slot:** a handle on a page, 1-based, 1 … `faders_per_page`.
- **Current page:** the page the faders show now. Live state: not in the show file (SHW), back to
  page 1 when the daemon starts.

## Acceptance criteria (VS1-06 subset)
- **PBK-01** Until P1-04 stores playbacks on handles, slot *s* of page *p* holds playback
  (p − 1) · `faders_per_page` + *s* when that playback exists, otherwise nothing. With 8 per page:
  page 1 holds 1–8, page 2 holds 9–16. There are 10 pages.
- **PBK-02** `Engine::set_page(p)` changes the current page; 0 or a page above 10 is refused with
  a warning and nothing changes. A page change never changes a playback's level or whether it
  is active: a playback up on page 1 keeps running while page 2 is shown, and shows its level
  again when page 1 comes back (Titan, unlocked handles; locks are P1-04).
- **PBK-03** IPC (extends `ipc.md`): the snapshot carries `"page":{"number":1,"count":10,
  "slots":[{"slot":1,"playback":1},{"slot":2,"playback":null},…]}` with `faders_per_page`
  entries; `page.set {"page":2}` changes it; a `page` delta with the same object follows every
  change. `fader.set {playback, level}` is unchanged: the app sends the playback of the slot.
- **PBK-04** The demo show (IPC-14) gains two playbacks on page 2: 9 = the washes 101–104 at full
  Dimmer, 10 = the front spots 301–306 at full, both priority Normal, fader mode 0.
- **PBK-05** App (UI-21, UI-22): the pages column shows page buttons 1–10 with the current one
  highlighted; a click sends `page.set`. The strips show the current page's slots: the legend is
  the playback number, the level and green active strip come from that playback; a slot without
  a playback is an empty, disabled strip without a cap. Checked in
  `tests/manual/app-main-screen.md` (UX-02d).

## Scope still to cover with P1-04
Fader modes 0/1/2 (3 later); priority; release mask + time; kill point; curve; handle paging
Unlocked / TransparentLock / Locked; Flash / Swop + key profiles; stored handles and page
legends; BPM / Rate / Size masters; group masters (Scale / HTP / Limit / TakeOver); playback
groups (mutual exclusion, kill point, kill action); chases (BPM, crossfade, direction, tap); DBO.

## Out of scope (this subset)
Recording a playback, page legends, locks, physical faders (VS1-07 uses PBK-02/03 for the
BCF2000's bank).
