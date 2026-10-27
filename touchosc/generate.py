#!/usr/bin/env python3
"""Generate the CMDMX TouchOSC Mk1 layout (CTL-60..64).

Writes touchosc/CMDMX.touchosc (a zip holding index.xml) and
touchosc/addresses.txt (what the layout sends / shows; read by the Rust test
ctl_64). `--check` fails when the checked-in files differ from a fresh run.

Mk1 format facts (index.xml, version 15): names, texts and OSC addresses are base64;
`osc_cs` is the control's custom OSC address. Example layout used as a reference:
https://in-thread.sonic-pi.net/t/using-touchosc-with-sonic-pi/275/12
`osc_cs` also in https://gist.github.com/aolkin/2bf11995ae18dc600da37a14110aa8c0.
Orientation: a horizontal (landscape) layout is stored in the device's portrait frame and the
app turns it a quarter. Evidence: an iPhone (320x480) horizontal layout with controls down to
y 436 (https://gist.github.com/rbnpi/e825b3917e80f6d72953273447855177), and ENTTEC's Mk1 guide
("use Fader H, because we just rotated the screen"). So the layout is drawn below in the
landscape frame (1024x768) and `add` turns each control into the portrait frame (768x1024):
landscape bottom = portrait left, so a faderh's 0 end is at the bottom. Types turn with it:
faderv -> faderh, labelh -> labelv. Not yet seen on the iPad (tests/hardware/touchosc.md).
"""
import argparse
import base64
import io
import sys
import zipfile
from pathlib import Path
from xml.sax.saxutils import quoteattr

HERE = Path(__file__).resolve().parent
# TouchOSC Mk1 mode 1 (iPad), landscape. The app's own toolbar (the (i) button) takes a strip on
# the landscape left and pushes the layout right (church iPad, 2026-10-25), so the layout stays
# TOOLBAR narrower. ponytail: guessed width, tune it if the iPad shows a gap or a cut on the right.
TOOLBAR = 48
W, H = 1024 - TOOLBAR, 768
GAP = 4
TOP_H = 48
SIDE_W = 220  # right column

controls = []  # xml lines
sends = []  # addresses a control sends
shows = []  # addresses a control displays


def b64(s):
    return base64.b64encode(s.encode()).decode()


TURNED = {"faderv": "faderh", "labelh": "labelv"}


def add(kind, name, x, y, w, h, color, address=None, **extra):
    """x, y, w, h in the landscape frame; written in the portrait frame (see the docstring)."""
    kind = TURNED.get(kind, kind)
    attrs = {"name": b64(name), "x": H - y - h, "y": x, "w": h, "h": w, "color": color, "type": kind}
    if address:
        attrs["osc_cs"] = b64(address)
    attrs.update(extra)
    controls.append("    <control " + " ".join(f"{k}={quoteattr(str(v))}" for k, v in attrs.items()) + "/>")


def label(name, address, x, y, w, h, color="gray", size=16, text="", plain=False):
    box = "false" if plain else "true"
    add("labelh", name, x, y, w, h, color, address, text=b64(text), size=size,
        background=box, outline=box)
    if address:
        shows.append(address)


def button(kind, name, address, x, y, w, h, color, caption, feedback=True):
    # feedback=False: the daemon sends nothing back for back/stop/page prev/next.
    # Buttons carry no text in Mk1: the caption is a label laid over the button.
    # ponytail: assumes labels do not take touches; if the iPad shows otherwise, move captions off the buttons.
    add(kind, name, x, y, w, h, color, address, scalef="0.0", scalet="1.0", local_off="false")
    sends.append(address)
    if feedback:
        shows.append(address)
    if caption:
        add("labelh", name + " caption", x, y, w, h, color, None, text=b64(caption), size=18,
            background="false", outline="false")


def fader(name, address, x, y, w, h, color):
    add("faderv", name, x, y, w, h, color, address, scalef="0.0", scalet="1.0",
        response="absolute", inverted="false", centered="false")
    sends.append(address)
    shows.append(address)


def build(n):
    controls.clear(), sends.clear(), shows.clear()
    # Top bar (CTL-62)
    x = GAP
    button("push", "page prev", "/cmdmx/page/prev", x, GAP, 90, TOP_H, "gray", "Page <", feedback=False)
    x += 90 + GAP
    label("page", "/cmdmx/page", x, GAP, 60, TOP_H, size=24)
    x += 60 + GAP
    label("page legend", "/cmdmx/page/legend", x, GAP, 200, TOP_H)
    x += 200 + GAP
    button("push", "page next", "/cmdmx/page/next", x, GAP, 90, TOP_H, "gray", "Page >", feedback=False)
    x += 90 + GAP
    label("message", "/cmdmx/message", x, GAP, W - GAP - x, TOP_H, "orange")
    # Fader strips (CTL-61), equal width from n
    top = GAP + TOP_H + GAP
    area = W - SIDE_W - 3 * GAP
    sw = (area - (n - 1) * GAP) // n
    btn_h, leg_h = 60, 40
    for i in range(1, n + 1):
        sx = GAP + (i - 1) * (sw + GAP)
        base = f"/cmdmx/fader/{i}"
        label(f"legend {i}", f"{base}/legend", sx, top, sw, leg_h, size=14)
        button("push", f"upper {i}", f"{base}/upper", sx, top + leg_h + GAP, sw, btn_h, "yellow", "")
        fy = top + leg_h + btn_h + 2 * GAP
        fh = H - GAP - btn_h - GAP - fy
        fader(f"fader {i}", base, sx, fy, sw, fh, "blue")
        button("push", f"lower {i}", f"{base}/lower", sx, H - GAP - btn_h, sw, btn_h, "yellow", "")
    # Right column (CTL-63)
    rx, rw = W - SIDE_W - GAP, SIDE_W
    y = top
    label("cue current", "/cmdmx/cue/current", rx, y, rw, 70, "green", 28)
    y += 70 + GAP
    pw = 54
    label("cue next prefix", None, rx, y, pw, 30, "gray", 14, "Next:", plain=True)
    label("cue next", "/cmdmx/cue/next", rx + pw + GAP, y, rw - pw - GAP, 30, "gray", 14, plain=True)
    y += 30 + GAP
    button("push", "go", "/cmdmx/go", rx, y, rw, 150, "green", "GO")
    y += 150 + GAP
    half = (rw - GAP) // 2
    button("push", "back", "/cmdmx/back", rx, y, half, 90, "red", "BACK", feedback=False)
    button("push", "stop", "/cmdmx/stop", rx + half + GAP, y, half, 90, "red", "STOP", feedback=False)
    y += 90 + GAP
    button("toggle", "dbo", "/cmdmx/dbo", rx, y, rw, 70, "red", "DBO")
    y += 70 + GAP
    fader("gm", "/cmdmx/gm", rx, y, rw, H - GAP - y, "purple")
    xml = (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        '<layout version="15" mode="1" orientation="horizontal">\n'
        f'  <tabpage name="{b64("CMDMX")}" scalef="0.0" scalet="1.0">\n'
        + "\n".join(controls) + "\n  </tabpage>\n</layout>\n"
    )
    lines = [f"send {a}" for a in sends] + [f"show {a}" for a in shows]
    return xml.encode(), ("\n".join(lines) + "\n").encode()


def make_zip(xml):
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w") as z:
        info = zipfile.ZipInfo("index.xml", date_time=(1980, 1, 1, 0, 0, 0))
        info.compress_type = zipfile.ZIP_DEFLATED
        info.external_attr = 0o644 << 16
        z.writestr(info, xml)
    return buf.getvalue()


def main():
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--faders", type=int, default=8, help="faders per page (default 8)")
    p.add_argument("--check", action="store_true", help="fail if checked-in files differ")
    a = p.parse_args()
    if a.faders < 1:
        sys.exit("--faders must be at least 1")
    xml, addresses = build(a.faders)
    files = {HERE / "CMDMX.touchosc": make_zip(xml), HERE / "addresses.txt": addresses}
    if a.check:
        stale = [f.name for f, data in files.items() if not f.exists() or f.read_bytes() != data]
        if stale:
            sys.exit(f"stale (run touchosc/generate.py): {', '.join(stale)}")
        return
    for f, data in files.items():
        f.write_bytes(data)


if __name__ == "__main__":
    main()
