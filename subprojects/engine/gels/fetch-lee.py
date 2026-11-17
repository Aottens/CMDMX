#!/usr/bin/env python3
"""Builds lee.json from the tungsten (3200 K) x, y on leefilters.com (see NOTICE). Run: python3 -I fetch-lee.py"""
import html, json, os, re, time, urllib.request

BASE = "https://leefilters.com/"
UA = {"User-Agent": "CMDMX gel table builder"}
ND = re.compile(r"neutral density|^[\d.]+\s*ND$|polari[sz]", re.I)


def get(url):
    time.sleep(0.5)
    return urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=30).read().decode("utf-8", "replace")


def text(s):
    s = re.sub(r"(?s)<(script|style).*?</\1>", "", s)
    return re.sub(r"\s+", " ", html.unescape(re.sub(r"<[^>]+>", " ", s)))


def converts(x, y):  # same test as the engine's gel table: on the diagram and a positive RGB component
    if y <= 0 or x < 0 or x + y > 1:
        return False
    xx, zz = x / y, (1 - x - y) / y
    return max(1.7166512 * xx - 0.3556708 - 0.2533663 * zz, -0.6666844 * xx + 1.6164812 + 0.0157685 * zz, 0.0176399 * xx - 0.0427706 + 0.9421031 * zz) > 0


slugs = sorted(set(re.findall(r'href="https://leefilters\.com/colour/([^"/]+)/', get(BASE + "lighting/colour-effect-lighting-filters/"))))
rows, skipped = {}, []
for slug in slugs:
    page = get(f"{BASE}colour/{slug}/")
    h1 = re.search(r"<h1[^>]*>\s*(\d+)\s+([^<]*)", page)
    m = re.search(r"Tungsten Transmission Y [\d.]+ x ([\d.]+) y ([\d.]+)", text(page))
    if not h1:
        skipped.append((slug, "no heading"))
        continue
    number, name = int(h1.group(1)), html.unescape(h1.group(2)).strip()
    if not m:
        skipped.append((slug, "no tungsten data"))
    elif number == 239 or ND.search(name):
        skipped.append((slug, "ND/polariser"))
    elif not converts(float(m.group(1)), float(m.group(2))):
        skipped.append((slug, f"x {m.group(1)}, y {m.group(2)} is off the chromaticity diagram"))
    elif number in rows:
        skipped.append((slug, f"duplicate of {number}"))
    else:
        rows[number] = {"number": number, "name": name, "x": float(m.group(1)), "y": float(m.group(2))}

out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "lee.json")
with open(out, "w") as f:
    f.write("[\n" + ",\n".join("  " + json.dumps(rows[n]).replace('{"', '{ "').replace("}", " }") for n in sorted(rows)) + "\n]\n")
for s in skipped:
    print("skipped", *s)
print(len(rows), "rows,", len(skipped), "skipped")
