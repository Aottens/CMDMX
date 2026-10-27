"""Checks on the generated layout (python3 touchosc/test_generate.py; CI runs it)."""
import base64
import unittest
import xml.etree.ElementTree as ET

import generate


def controls():
    xml, _ = generate.build(8)
    out = {}
    for c in ET.fromstring(xml).iter("control"):
        out[base64.b64decode(c.get("name")).decode()] = c
    return out


class Layout(unittest.TestCase):
    def test_ctl_63_current_cue_large_and_marked_next_cue_small_and_grey(self):
        c = controls()
        cur, nxt, prefix = c["cue current"], c["cue next"], c["cue next prefix"]
        # Landscape height is the portrait `w` (generate.add turns the frame).
        self.assertEqual((cur.get("w"), cur.get("size"), cur.get("color")), ("70", "28", "green"))
        self.assertEqual((cur.get("background"), cur.get("outline")), ("true", "true"))
        for label in (nxt, prefix):
            self.assertEqual((label.get("w"), label.get("size"), label.get("color")), ("30", "14", "gray"))
            self.assertEqual((label.get("background"), label.get("outline")), ("false", "false"))
        self.assertEqual(base64.b64decode(prefix.get("text")).decode(), "Next:")
        self.assertIsNone(prefix.get("osc_cs"))
        # Same line: the prefix sits left of the next cue text (landscape x is portrait y).
        self.assertEqual(prefix.get("x"), nxt.get("x"))
        self.assertLess(int(prefix.get("y")), int(nxt.get("y")))

    def test_brd_05_page_is_named_cmdmx(self):
        xml, _ = generate.build(8)
        page = ET.fromstring(xml).find("tabpage")
        self.assertEqual(base64.b64decode(page.get("name")).decode(), "CMDMX")


if __name__ == "__main__":
    unittest.main()
