# Module spec: output-protocols (OUT)

Owner: `cmdmx-protocols` (framing) and `cmdmx-daemon` (sockets, serial, scheduling). Status: **OUT-01..04, 11, 12, 14, 15 approved by owner 2026-10-16**; rest draft. References: Art-Net 4 specification (Artistic Licence), ANSI E1.31-2018 (sACN), ENTTEC DMX USB Pro API, Avolites Titan v19 manual *DMX Settings*.

## Purpose
Deliver each universe's frame to one or more *nodes* (Titan's model: lines → nodes). Framing is pure and byte-exact tested; I/O lives in the daemon and must never block the tick.

## Model
- A **line** is a universe produced by the engine (numbered 1..n, free numbering later).
- A **node** is a destination: `ArtNet { target: ipv4, port_address }` (broadcast or unicast is just the choice of target), `Sacn { universe, priority, multicast|unicast ip }`, `Serial { port, protocol: EnttecPro }`.
- A line may feed several nodes; a node belongs to one line.

## Acceptance criteria

### Art-Net
- **OUT-01** `artnet::art_dmx(universe15: u16, sequence: u8, physical: u8, data: &[u8]) -> Vec<u8>` produces: ID `"Art-Net\0"`, OpCode `0x5000` little-endian, ProtVer `14` big-endian, Sequence, Physical, SubUni (low byte), Net (high byte), Length big-endian (even, 2..512), then the data. Golden bytes for a 512-channel frame on universe 0 are in the test.
- **OUT-02** Sequence increments 1..255 and wraps to 1 (0 means "disabled"); the daemon keeps one counter per node.
- **OUT-03** Continuous mode: the daemon sends one ArtDmx per tick (40 Hz) to every enabled Art-Net node, whether or not the frame changed. This is the only mode in phase 1 and matches Titan's default "Continuous Art-Net DMX". The change-only mode is OUT-14.
- **OUT-04** Each Art-Net node has one IPv4 target; the socket has `SO_BROADCAST` set, so a target can be the limited broadcast `255.255.255.255` (default — reaches the Mac's primary LAN and a monitor on the same Mac), a directed broadcast (`2.255.255.255`, `192.168.1.255`, …) or a unicast node IP. Destination port 6454. Packets leave from an ephemeral source port, so an Art-Net monitor on the same Mac can own port 6454. Per-interface broadcast enumeration and ArtPoll/ArtPollReply discovery are later (OUT-10, not in phase 1; OUT-10 will also need to bind 6454).

### sACN (E1.31)
- **OUT-05** `sacn::data_packet(cid: [u8;16], source_name: &str, priority: u8, sequence: u8, universe: u16, data: &[u8]) -> Vec<u8>` produces a valid E1.31 data packet: Root layer (preamble `0x0010`, postamble `0x0000`, ACN packet identifier `"ASC-E1.17\0\0\0"`, flags+length `0x7xxx`, vector `VECTOR_ROOT_E131_DATA = 0x00000004`, CID), Framing layer (vector `0x00000002`, 64-byte UTF-8 source name zero-padded, priority, sync address, sequence, options, universe), DMP layer (vector `0x02`, address type `0xA1`, first address 0, increment 1, property value count = 1 + data length, start code 0 + data). Golden bytes in the test.
- **OUT-06** Multicast to `239.255.{hi}.{lo}` for universe `hi<<8|lo`, port 5568; unicast optional per node.
- **OUT-07** Priority defaults to 100 and is configurable 0–200 per node (Titan uses this for backup consoles).

### ENTTEC Pro over serial
- **OUT-08** `enttec::send_dmx_frame(data: &[u8]) -> Vec<u8>` produces `0x7E`, label `6`, length LSB, length MSB (length = 1 + data.len(), data padded to at least 24 channels), start code `0x00`, data, `0xE7`. Golden bytes in the test.
- **OUT-09** Daemon identifies the owner's interface by USB product string containing `"DMX512 Pro"` (Eurolite USB-DMX512 PRO MK2 reports VID/PID `0403:6001` like any FTDI device and the product string `Eurolite DMX512 Pro`), opens the macOS VCP device (`/dev/cu.usbserial-*`) at 250 kbaud-independent settings (FTDI handles the line; use 115200 8N1 on the VCP), and sends one frame per tick. Never treat it as ENTTEC Open DMX.

### Scheduling and safety
- **OUT-11** Output I/O runs on its own thread(s); a slow or failing node never delays `engine.tick`. The tick hands each frame over through a bounded channel without blocking; when the output thread has not taken the previous frame yet, the new one is dropped (the next tick carries a fresh frame). A panic inside the output thread is caught, logged, and the thread carries on with the next frame.
- **OUT-12** On a send error (including failing to open the socket) the node is marked `Degraded`, the last frame is kept, the error is logged once per state change (`Ok → Degraded`, `Degraded → Ok`; a different error while degraded is not logged again), and sending retries every tick.
- **OUT-13** New show defaults: 16 lines; line n → Art-Net universe n-1 (net 0, subnet 0) *disabled* and sACN universe n *enabled*, mirroring Titan's defaults; serial node auto-assigned to line 1 when a known interface is present.

- **OUT-14** *(later, not in vertical slice 1)* Change-only mode (`continuous = false`): send only when the frame changed, plus a keep-alive at least every 800 ms. Needs per-node config in the show file.
- **OUT-15** *(vertical slice 1 only, superseded by OUT-13 once node config lives in the show file)* Without a show file the daemon outputs line 1 to one Art-Net node, port-address 0, **enabled**, target `255.255.255.255`. `--artnet-target <ipv4>` sets the target; `--no-artnet` disables the node. An unparsable target is a startup error (exit code 2), not a silent fallback.

## Hardware checklist
`tests/hardware/output.md` covers OUT-03/04/06/09 on the owner's network and Eurolite MK2.
