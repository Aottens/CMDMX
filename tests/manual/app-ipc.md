# App ↔ daemon checklist (IPC-08/09) — template

No Swift test target exists (CLAUDE.md), so the app side of `specs/modules/ipc.md` is checked
by hand. Copy to `tests/manual/runs/<date>-<task>.md` and tick it there.

Setup: two terminals. Daemon: `cd subprojects/engine && cargo run -p cmdmx-daemon -- --no-sacn`.
App: `cd subprojects/app && swift build && scripts/bundle-app.sh && open .build/ColourmotionDMX.app`.
To run beside another daemon, start both on another port: `--ipc-port 7400` on the daemon and `open .build/ColourmotionDMX.app --args --ipc-port 7400` (IPC-08).
Optional: Wireshark on `en0`, filter `artnet`, to see the DMX values.

- [ ] Start the app with the daemon **stopped**: the "engine offline" banner shows (IPC-08)
- [ ] Start the daemon: within ~1 s the banner disappears and the app shows `faders_per_page` faders (IPC-08)
- [ ] Move on-screen fader 1 to about half: Art-Net channel 1 follows (~50%); faders 2..n move channels 2..n (IPC-09)
- [ ] Move the grand master down: all channels scale down; back up restores them (IPC-09)
- [ ] Stop the daemon (Ctrl+C): the banner returns; restart it: the app reconnects and its faders show the daemon's state (all at 0 after a restart) (IPC-08)
- [ ] With the app connected, restart the daemon with `--demo`: on-screen fader 1 moves by itself with the breathing demo (deltas reach the app, IPC-05/09)
