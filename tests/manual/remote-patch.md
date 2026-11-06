# Walking the patch from a phone (RMT-09..12, RMT-14, RMT-16) — template

The phone page on a real phone on the owner's network, checked by hand: the page has no test target of its own
(the daemon tests only check that it is served whole and without outside resources). Copy to
`tests/manual/runs/<date>-<task>.md` and tick it there. Claude drives the daemon, the app and an Art-Net listener;
the owner holds the phone. Each step message shows done/total ("Stap 3/8").

Setup (Claude), at home without lamps; never the Demo show on the real rig:
- `rm -rf /tmp/cmdmx-remote`
- Daemon: `cd subprojects/engine && cargo run -p cmdmx-daemon -- --no-sacn --no-serial --show-dir /tmp/cmdmx-remote`
  (Demo show; Art-Net on, so a listener on port 6454 sees universe 1).
- App: `cd subprojects/app && swift build && scripts/bundle-app.sh && open .build/CMDMX.app`.
- The remote on a free port (the owner's dashboard holds 7320, TRB-03): the Remote screen, or `remote.set_login`
  over IPC with a test password the owner is told in chat.
- Demo rig on universe 1: dimmers 1–8 on 1–8, washes 101–104 from 9, RGBW pars 201–208, RGBL pars 211–216,
  RGB pars 221–226, dimmers 301–306.

## Login (RMT-09)
- [ ] Owner: scan the QR code; the page opens with a login form, nothing loaded from the internet.
- [ ] A wrong password: "Wrong username or password" under the form.
- [ ] The right password: the list, the show name *Demo* on top. Reload the page: the list again, no login asked.
      Lock the phone for a minute and wake it: the list again, no login asked.

## The list and the card (RMT-10, RMT-11)
- [ ] Rows read `201 · RGBW Par · …· 1.x–1.y` in number order; typing `221` in search starts the list at 221, typing a name
      filters. Rows and buttons are easy to hit with a thumb, upright, nothing runs off the screen.
- [ ] Tap 201: the card with number, name, model and mode, universe and address with the number keypad, the range.
- [ ] Rename 201 *Front L* and leave the field: the console's patch shows *Front L* (Claude checks the console over IPC or in the app).

## Walking with Highlight (RMT-12)
- [ ] Highlight on at 201: the console's selection is 201 with Highlight (Claude checks the console over IPC or in the app), and Art-Net shows
      201's channels at their highlight values, nothing else lit.
- [ ] Next nine times (202 … 208, 211, 212): after each tap Claude confirms on Art-Net that only the
      card's fixture is at its highlight values.
- [ ] Previous once: the highlight goes back with the card. Back to the list: Highlight off at the console, the
      selection stays.

## A readdress with a clash (RMT-11)
- [ ] On 202, Save an address inside 203's channels: the card shows the warning "202 would overlap 203 at 1.x";
      nothing moved (Claude checks the patch).
- [ ] Save a free address (universe 2, address 1): the card shows the new range `2.1–2.n`, the console's patch too;
      Undo at the console puts it back and the phone's card follows.

## Lock, another show, offline (RMT-14)
- [ ] Lock the console (Claude): the page says "Console locked", the fields, Save and Highlight are greyed. Unlock:
      they work again.
- [ ] Open another show at the console (Claude): the page shows its list and its name on top.
- [ ] Stop the engine (Claude): "Console offline" on the phone. Start it again: within a few seconds the page
      asks for the login (a restart ends the token, RMT-04); log in and walk on.
