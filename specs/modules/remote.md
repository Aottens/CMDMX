# Module spec: remote (RMT)

Owner: `cmdmx-daemon` (listener, login, the page built into the binary) and `subprojects/app` (the login in
Settings). Status: **RMT-01..16 approved (P1-34, IDEA-42, 2026-11-06).** Builds on
`patch.md` (PAT-04, 23, 29..33), `programmer.md` (PRG-26, 39) and `ipc.md` (IPC-06, 50).
Behavioural reference: Titan Remote (phone or tablet on the console's Wi-Fi) shows the DMX address patch
but does not edit it; Titan's Web API is plain HTTP on the console. This page goes further: it edits.

## Purpose
Alone on a stage, the console at the back and no printed patch: open a page on the phone, walk the rig
fixture by fixture, see which lamp is which, put a wrong address right and go on to the next. The
console serves the page itself as soon as the engine runs, behind a login set in the console. It is the
first slice of the web remote (IDEA-06, phase 2), which later adds faders and cues to the same listener.

## Definitions
- **Remote listener:** the daemon's second port, on every network interface, apart from the app's
  loopback port (IPC-01). It serves the page and the page's WebSocket.
- **Login:** one username and password for the remote, set in the console. Not show data.
- **Remote client:** a WebSocket connection on the remote listener. It is not an app client: it speaks
  only the commands of RMT-07.
- **Card:** the page's view of one fixture.

## Acceptance criteria

### Listener and login
- **RMT-01** The remote listener is off until a login is set. `remote.set_login {"username","password"}`
  (both 1–64 characters) turns it on and `remote.off` turns it off and closes every remote client. It
  listens on port 7320 (`remote.set_login` takes an optional `port`, 1024–65535); a port in use is a
  warning and the listener stays off. The login, the port and on/off live in `state.json` (SHW-09), so
  opening another show keeps them, and come back when the daemon starts.
- **RMT-02** The password is stored as a salted hash, never as itself, and never leaves the daemon: the
  snapshot carries `remote: {"on":true,"username":"aernout","port":7320,"addresses":["192.168.1.20"],"clients":1}`,
  no password. `addresses` are the Mac's IPv4 addresses on the network, so the app can show the URL.
  A change to any of them reaches every app client as `{"v":1,"type":"remote","remote":{…}}`. In `state.json`
  the login is `remote: {on, username, salt, hash, port}`: PBKDF2-HMAC-SHA256, a 16-byte salt from `/dev/urandom`.
- **RMT-03** On the remote listener a plain HTTP `GET /` answers the page (RMT-09); any other path is
  404; a WebSocket handshake on `/ws` is accepted only when its `Origin` is the listener's own
  `http://<host>:<port>` (a page on another site cannot drive the console through the phone, as IPC-02
  guards the app's port). Plain HTTP: the password crosses the network readable to anyone on it who
  listens (owner decision 3).
- **RMT-04** A remote client's first message must be `{"v":1,"cmd":"login","username","password"}` or
  `{"v":1,"cmd":"login","token"}`. Right → `{"type":"login","token":"…"}` and then the remote snapshot
  (RMT-06); wrong → `{"type":"login","error":"Wrong username or password"}` and the connection closes.
  Anything else before a login closes it. A token stays valid until the daemon stops or the login
  changes, so a phone that reconnects does not ask again.
- **RMT-05** After 5 wrong logins from one address, logins from it are refused for 30 s
  ("Too many tries, wait 30 s"). The app's port and the tick are never slowed by the remote: a remote
  client is served like any client (IPC rule "no client can slow the tick").

### What a remote client sees and may do
- **RMT-06** The remote snapshot: `show` (name), `locked` (IPC-50), `highlight` (the fixture the page
  highlights or `null`) and `fixtures` in number order, each `{id, number, name, profile, mode, universe,
  address, footprint, parked}` (PAT-09's fields that the page needs, `parked` from PAT-34). A change to
  the patch, the show (another show opened), the lock or the highlight sends it again, at most once per
  tick, to every remote client.
- **RMT-07** A remote client may send only: `patch.move {id, universe, address, park_conflicting?}`
  (PAT-04, 32), `patch.name {id, name}` (PAT-23), `patch.park {ids}` and `patch.unpark {ids}` (PAT-29,
  31), `remote.highlight {id}` and `remote.highlight {id:null}` (RMT-08). Any other `cmd` →
  `error` to that client (IPC-06). Edits are edit commands as from the app: one undo step each at the
  console, refused while the console is locked (IPC-50); a refusal reaches the remote client as a
  `warning` and the app's clients as usual. Adding, removing, renumbering and changing the mode stay on
  the console (owner decision 1).
- **RMT-08** `remote.highlight {id}` makes that fixture the console's whole selection and switches
  Highlight on (PRG-26, 39); the fixture shows its highlight values on the rig. `{id:null}` switches
  Highlight off; the selection stays. It uses the console's own selection and Highlight, so the
  operator at the console sees it change (owner decision 2). A parked fixture cannot be highlighted
  (PAT-30): refused with "201 is parked". A remote client that disconnects leaves things as they are.

### The page
- **RMT-09** One HTML page with its script and style inline, built into the daemon: no outside
  resources, so it works on a church network without internet. It is laid out for a phone held
  upright (375 pt wide) and works on a tablet; touch targets at least 44 pt; Safari on iOS and Chrome on
  Android. The first visit shows a login form; a right login keeps the token in the browser (RMT-04), so a
  reload or a sleeping phone does not ask again.
- **RMT-10** The list: every fixture in number order as `201 · Front L · Phantom 25 · 1.37–1.50`
  (number, name, model, universe.first–last channel). A parked fixture is greyed with "parked (was
  1.37)". A search field at the top filters by number or name; typing a number jumps to it. Tapping a row
  opens its card.
- **RMT-11** The card: number, name (editable), model and mode, universe and address (number fields
  with the phone's number keypad), the footprint as `1.37–1.50`, and Save. Save sends `patch.move`;
  the result shows on the card: the new range, or the warning in the card itself ("201 would overlap 301
  at 1.45") with a button **Park 301 and move** that sends the move again with `park_conflicting`
  (PAT-32). A name is saved when the field is left.
- **RMT-12** Walking: **Previous** and **Next** on the card go to the neighbour in number order; with
  **Highlight** on, the card's fixture is highlighted (RMT-08) and Previous / Next take the highlight
  along, so walking the rig is one tap per lamp: the lamp that lights is the fixture on the card, a wrong
  one means a wrong address. Highlight off or leaving the card switches it off. Next skips parked
  fixtures while Highlight is on.
- **RMT-13** **Park** / **Unpark** on the card (PAT-29, 31): a lamp that does not hang is parked where it
  is found. Unpark that is refused (its old address is taken) shows the warning on the card, with the
  address fields ready to give it a new one.
- **RMT-14** Offline: when the connection drops the page shows "Console offline" over the list and
  retries every 2 s, logging in again with its token (RMT-04); another show opened at the console
  shows the new list, with its name at the top. While the console is locked the page shows "Console
  locked" and its edit buttons are disabled.

### In the console
- **RMT-15** In the app, Settings gains **Remote**: username, password (shown as dots, never read back),
  port, Turn on / Turn off, and when on: the address `http://192.168.1.20:7320` for each of `addresses`,
  a QR code of the first (Core Image, no dependency) and how many phones are connected. Turning on with an
  empty field is refused in the panel. The login commands are not undoable, not in the show, and allowed
  while locked only from the app (class neutral, UNDO-04).
- **RMT-16** The manual (WEB-33) gets a section *Walking the patch from a phone*: turn it on, scan the
  code, walk with Highlight, park what does not hang, and that the password crosses the network
  readable. Verified by `tests/manual/remote-patch.md`: a real phone on the owner's network, login, a
  wrong password, walking ten fixtures on Art-Net with Highlight, a readdress with a clash and Park
  and move, a lock at the console, the engine restarted under the page.

## Out of scope (this spec)
HTTPS (owner decision 3); more than one login, or rights per user (IDEA-34); faders, cues and playbacks on
the page (IDEA-06, phase 2); adding, removing, renumbering or changing the mode from the phone (owner
decision 1); the DMX values on the card's channels (the DMX view, UI-61, can come to the card later); a
highlight apart from the console's selection (owner decision 2).

## Owner decisions (2026-11-06, IDEA-42)
1. The page edits universe, address and name of fixtures that are patched; adding and removing stay on the
   console. Park and Unpark on the card are proposed with P1-35 (the stage case: a lamp that does not hang).
2. Highlight goes through the console's own selection and Highlight: simple, and the operator at the console
   sees it. Not chosen: a highlight layer of its own.
3. Proposed, to approve with this spec: plain HTTP. HTTPS on a LAN needs a self-signed certificate that every
   phone warns about; the login keeps strangers out of the patch, not out of the Wi-Fi.
