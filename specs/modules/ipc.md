# Module spec: ipc (IPC)

Owner: `cmdmx-daemon` (server) and `subprojects/app` (client). Status: **IPC-01..09 draft for owner approval (VS1-05)**. References: ADR-002 (engine as separate process), DECISIONS 2026-10-17 (WebSocket), RFC 6455.

## Purpose
The app (and later remotes, IDEA-06) sees engine state and sends commands over one versioned JSON contract. The daemon owns all state; a client only renders what the daemon reports. No client can ever slow down or stop the tick.

## Definitions
- **Client:** any WebSocket connection to the daemon (the app now; browser/iPad remotes later).
- **Snapshot:** full state a client needs to render, sent once on connect.
- **Delta:** one changed piece of state, sent to every client.

## Acceptance criteria

### Transport
- **IPC-01** The daemon listens for WebSocket connections on `127.0.0.1:7319` (loopback only); `--ipc-port <n>` changes the port. Several clients may be connected at once.
- **IPC-02** A WebSocket handshake that carries an `Origin` header is refused. Browsers always send one, so a web page on the Mac cannot drive the lights; the native app sends none. Browser remotes come with authentication later (IDEA-06).

### Messages (JSON text frames, every message has `"v": 0`)
- **IPC-03** On connect the client first receives a snapshot:
  `{"v":0,"type":"snapshot","faders_per_page":8,"grand_master":1.0,"playbacks":[{"id":1,"level":0.0,"active":false}]}`. `playbacks` lists every playback in the engine, sorted by id; `active` means the playback is in the merge.
- **IPC-04** Commands from a client:
  `{"v":0,"cmd":"fader.set","playback":1,"level":0.5}` and `{"v":0,"cmd":"grand_master.set","level":0.8}`. Levels are clamped to 0.0..1.0. A command takes effect in the frame of the next tick.
- **IPC-05** Deltas to every client, at most one per changed item per tick, only when the value changed:
  `{"v":0,"type":"playback","id":1,"level":0.5,"active":true}`, `{"v":0,"type":"grand_master","level":0.8}`. A change made by one client therefore reaches all clients, including the sender (its optimistic local value is replaced by the engine's).
- **IPC-06** Errors: invalid JSON, an unknown `cmd`, missing fields, or `"v"` other than 0 → `{"v":0,"type":"error","message":"…"}` to that client only; the connection stays open. An engine refusal (e.g. unknown playback) → `{"v":0,"type":"warning","message":"…"}` to every client (engine warnings are shared state).

### Isolation (show-proof)
- **IPC-07** Each client runs on its own thread. The tick never waits on a client: commands reach the tick through a non-blocking channel drained at the start of each tick; messages to a client go through a bounded queue (64 messages). A client whose queue is full is disconnected (it resyncs from a fresh snapshot when it reconnects). A panic in a client thread is caught and closes only that client.

### App behaviour (verified by `swift build` + `tests/manual/app-ipc.md`; no Swift test target)
- **IPC-08** The app connects to `ws://127.0.0.1:7319` on launch. While not connected it shows the "engine offline" banner and retries every second; on (re)connect it rebuilds its state from the snapshot.
- **IPC-09** Moving an on-screen fader sends `fader.set` (slot n → playback n for now; paging is VS1-06) and the grand master sends `grand_master.set`; the displayed levels follow the deltas.

## Out of scope (this version)
Remote access over the LAN with a password (IDEA-06), pages and paging (VS1-06/PBK), programmer and palette messages, command acknowledgements, binary encoding.
