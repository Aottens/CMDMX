#!/usr/bin/env python3
"""The risk gate (specs/modules/autoreview.md, ARV-03..09, ARV-17): paths, findings, checks and round give a lane.

Standard library only, no model, no network: the same input always gives the same lane (ARV-08).

Usage:
  risk-gate.py --impact < paths             worst impact of the paths on stdin (ARV-04)
  risk-gate.py --table findings.json        findings as a Markdown table for the job summary (ARV-03)
  risk-gate.py --check-review execution.json
      Counts the agents the review launched; exit 0 when it ran its reviewers, 1 when not (ARV-03).
      Used by .github/workflows/review.yml before it writes findings.json.
  risk-gate.py --base origin/main [--findings findings.json] [--round N] [--failed-check NAME]...
               [--comment out.md] [--body-out body.md]
      Decides from `git diff --numstat <base>...HEAD`; PR_TITLE and PR_BODY come from the environment.
      Prints the lane, then one "- reason" line per reason. Always exits 0; the workflow acts on the lane.
"""
import argparse
import json
import os
import re
import subprocess
import sys

# ARV-05: first match wins, anything not named is high. A prefix ends in "/" or is a whole path.
PATHS = [
    ("Cargo.lock", "high"),  # any Cargo.lock, anywhere
    ("scripts/risk-gate.py", "high"),
    ("scripts/test_risk_gate.py", "high"),
    ("scripts/", "medium"),
    ("subprojects/engine/cmdmx-core/", "high"),
    ("subprojects/engine/cmdmx-protocols/", "high"),
    ("subprojects/engine/cmdmx-daemon/", "high"),
    ("subprojects/engine/cmdmx-control/", "medium"),
    ("subprojects/engine/profiles/", "medium"),
    ("subprojects/app/scripts/", "high"),
    ("subprojects/app/Sources/", "medium"),
    ("subprojects/website/", "low"),
    ("touchosc/", "medium"),
    ("README.md", "low"),
    ("STATE.md", "low"),
    ("BACKLOG.md", "low"),
    ("IDEAS.md", "low"),
    ("docs/", "low"),
    ("tests/manual/", "low"),
]
ORDER = ["low", "medium", "high"]
SEVERITIES = ["minor", "major", "blocker"]
# ARV-03: the code-review plugin's steps launch three agents to prepare and four to review; a review that
# launched fewer did not run its reviewers.
# ponytail: counts launches, not what the agents did; read their results if that ever proves too little.
MIN_AGENTS = 7
MAX_LINES = 600
MAX_FIX_ROUNDS = 2
NOT_COUNTED = ("Cargo.lock", "touchosc/CMDMX.touchosc", "touchosc/addresses.txt")
LANES = {"green": "risk: green", "amber": "risk: amber", "red": "risk: owner"}
LABELS = list(LANES.values())
MARKER = "<!-- risk-gate -->"
MATRIX = {  # ARV-06: worst finding -> lane per impact (low, medium, high)
    "none": ["green", "green", "red"],
    "minor": ["green", "amber", "red"],
    "major": ["amber", "amber", "red"],
    "blocker": ["red", "red", "red"],
}


def is_cargo_lock(path):
    return path.rsplit("/", 1)[-1] == "Cargo.lock"


def impact(path):
    if re.fullmatch(r"subprojects/[^/]+/STATE\.md", path):
        return "low"
    if is_cargo_lock(path):
        return "high"
    for prefix, level in PATHS:
        if path == prefix or (prefix.endswith("/") and path.startswith(prefix)):
            return level
    return "high"


def worst_impact(paths):
    return max((impact(p) for p in paths), key=ORDER.index, default="low")


def worst_finding(findings):
    """Worst severity (None, minor, major, blocker); a plausible finding counts one lower."""
    levels = [SEVERITIES.index(x["severity"]) - (x.get("verdict") == "plausible") for x in findings]
    return SEVERITIES[max(levels)] if levels and max(levels) >= 0 else None


def count_lines(rows):
    n = lambda v: 0 if v == "-" else int(v)
    return sum(n(a) + n(d) for a, d, path in rows if not is_cargo_lock(path) and path not in NOT_COUNTED)


def decide(rows, findings, review_round=1, failed_checks=(), title="", body=""):
    worst = worst_finding(findings or [])
    level = worst_impact([path for _, _, path in rows])
    lane = MATRIX[worst or "none"][ORDER.index(level)]
    reasons = [f"impact {level}, worst finding {worst or 'none'} → {lane}"]
    red = [f"required check failed: {name}" for name in failed_checks]
    if findings is None:
        red.append("not reviewed: no findings.json")
    lines = count_lines(rows)
    if lines > MAX_LINES:
        red.append(f"{lines} changed lines, more than {MAX_LINES}")
    if review_round > MAX_FIX_ROUNDS and findings:
        red.append(f"fix rounds used up: review round {review_round} still has findings")
    if re.search(r"sub-release|(?<![\w.])v?\d+\.\d+\.0(?![\w.])", f"{title}\n{body}"):
        red.append("names a sub-release")
    return ("red" if red else lane), reasons + red


def label(lane):
    return LANES[lane]


def comment(lane, reasons, impact, worst):
    cells = lambda row: [f"**{c}**" if (row, i) == (worst or "none", ORDER.index(impact)) else c
                         for i, c in enumerate(MATRIX[row])]
    table = ["| worst finding \\ impact | low | medium | high |", "| --- | --- | --- | --- |"]
    table += [f"| {row} | " + " | ".join(cells(row)) + " |" for row in MATRIX]
    tail = " This PR would merge by itself." if lane == "green" else ""
    return "\n".join([MARKER, f"Lane: **{lane}**", "", *[f"- {r}" for r in reasons], "", *table, "",
                      "The gate runs in shadow: nothing merges by itself yet (ARB-03)." + tail, ""])


def findings_table(findings):
    if not findings:
        return "No findings.\n"
    rows = [f"| {x['file']} | {x['line']} | {x['severity']} | {x['verdict']} | "
            f"{str(x['summary']).replace('|', chr(92) + '|')} |" for x in findings]
    return "\n".join(["| file | line | severity | verdict | summary |", "| --- | --- | --- | --- | --- |", *rows, ""])


def with_lane(body, lane):
    line = f"Lane: **{lane}** (risk-gate)"
    body = re.sub(r"^Lane: \*\*\w+\*\* \(risk-gate\)\n?", "", body, flags=re.M)
    if "## Tracking" not in body:
        return (body + "\n\n" if body else "") + f"## Tracking\n{line}\n"
    return re.sub(r"^## Tracking[ \t]*\n?", lambda m: f"## Tracking\n{line}\n", body, count=1, flags=re.M)


def load_findings(path):
    try:
        with open(path) as handle:
            data = json.load(handle)
    except (OSError, ValueError, TypeError):
        return None
    return data if isinstance(data, list) else None


def agents_launched(messages):
    """Agent launches in a claude-code-action execution file (a list of SDK messages)."""
    count = 0
    for m in messages if isinstance(messages, list) else []:
        body = m.get("message") if isinstance(m, dict) and m.get("type") == "assistant" else None
        content = body.get("content") if isinstance(body, dict) else None
        for block in content if isinstance(content, list) else []:
            if isinstance(block, dict) and block.get("type") == "tool_use" and block.get("name") in ("Agent", "Task"):
                count += 1
    return count


def review_ran(messages):
    return agents_launched(messages) >= MIN_AGENTS


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--impact", action="store_true")
    ap.add_argument("--table")
    ap.add_argument("--check-review")
    ap.add_argument("--base")
    ap.add_argument("--findings")
    ap.add_argument("--round", type=int, default=1)
    ap.add_argument("--failed-check", action="append", default=[])
    ap.add_argument("--comment")
    ap.add_argument("--body-out")
    args = ap.parse_args()
    if args.impact:
        print(worst_impact([p for p in sys.stdin.read().splitlines() if p.strip()]))
    elif args.table:
        print(findings_table(load_findings(args.table) or []), end="")
    elif args.check_review:
        messages = load_findings(args.check_review) or []
        n = agents_launched(messages)
        ok = review_ran(messages)
        print(f"review ran {n} agents" + ("" if ok else f", the plugin's steps need {MIN_AGENTS}"))
        sys.exit(0 if ok else 1)
    elif args.base:
        root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
        diff = subprocess.run(["git", "diff", "--numstat", "--no-renames", f"{args.base}...HEAD"], cwd=root,
                              capture_output=True, text=True)
        rows = [tuple(line.split("\t", 2)) for line in diff.stdout.splitlines() if line.count("\t") >= 2]
        if diff.returncode:  # an empty diff would read as low and green; a diff we cannot see is the owner's
            args.failed_check.append(f"git diff {args.base}...HEAD")
        findings = load_findings(args.findings) if args.findings else None
        title, body = os.environ.get("PR_TITLE", ""), os.environ.get("PR_BODY", "")
        lane, reasons = decide(rows, findings, args.round, args.failed_check, title, body)
        print(lane)
        print("\n".join(f"- {r}" for r in reasons))
        if args.comment:
            with open(args.comment, "w") as handle:
                handle.write(comment(lane, reasons, worst_impact([r[2] for r in rows]), worst_finding(findings or [])))
        if args.body_out:
            with open(args.body_out, "w") as handle:
                handle.write(with_lane(body, lane))
    else:
        ap.print_help()


if __name__ == "__main__":
    main()
