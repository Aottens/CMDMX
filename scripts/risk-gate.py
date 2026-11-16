#!/usr/bin/env python3
"""The risk gate (specs/modules/autoreview.md, ARV-03..10, ARV-17..19): paths, findings, checks and round give a lane.

A spec file is medium, high when the PR changes a criterion line (ARV-05). Tests and .md files are not counted
in the 600 lines, and only the PR title can name a sub-release (ARV-07).

Standard library only, no model, no network: the same input always gives the same lane (ARV-08).

Usage:
  risk-gate.py --keep-review FINDINGS [--pr-files FILE] < paths
      Exit 0 when the earlier review is kept (ARV-04); FILE lists the PR's files, one per line (ARV-19).
  risk-gate.py --check-findings FINDINGS [--pr-files FILE]
      Exit 1 when a finding is outside the diff: the review is not reviewed and uses no fix round (FIX-01, ARV-19).
  risk-gate.py ... [--automerge on]      (ARV-10: only the exact value `on` lets a green PR merge by itself)
  risk-gate.py --table findings.json        findings as a Markdown table for the job summary (ARV-03)
  risk-gate.py --check-review execution.json
      Counts the agents the review launched and how many reported; exit 0 when it ran its reviewers and
      every agent reported, 1 when not (ARV-03).
      Used by .github/workflows/review.yml before it writes findings.json.
  risk-gate.py --base origin/main [--findings findings.json] [--round N] [--failed-check NAME]...
               [--comment out.md] [--body-out body.md]
      Decides from `git diff --numstat <base>...HEAD`; PR_TITLE and PR_BODY come from the environment.
      A finding outside the diff (line < 1, or a file the PR does not change) is not reviewed: red (ARV-19).
      Findings whose file:line the PR's added BACKLOG.md/STATE.md lines lack make a green lane amber (ARV-18).
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
    ("DECISIONS.md", "medium"),
    ("specs/", "medium"),  # high when a criterion changes: see changes_criteria
    ("subprojects/engine/cmdmx-core/", "high"),
    ("subprojects/engine/cmdmx-protocols/", "high"),
    ("subprojects/engine/cmdmx-daemon/", "high"),
    ("subprojects/engine/cmdmx-control/", "medium"),
    ("subprojects/engine/profiles/", "medium"),
    ("subprojects/app/scripts/", "high"),
    ("subprojects/app/Sources/", "medium"),
    ("subprojects/website/", "low"),
    ("touchosc/", "medium"),
    ("tests/hardware/", "high"),  # gate 3 is the owner's
    ("README.md", "low"),
    ("STATE.md", "low"),
    ("BACKLOG.md", "low"),
    ("IDEAS.md", "low"),
    ("docs/", "low"),
    ("tests/", "low"),
]
ORDER = ["low", "medium", "high"]
SEVERITIES = ["minor", "major", "blocker"]
# ARV-03: .github/review-prompt.md launches two agents to prepare and four to review; a review that
# launched fewer did not run its reviewers.
# ponytail: counts launches, not what the agents did; read their results if that ever proves too little.
MIN_AGENTS = 6
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
    if re.fullmatch(r"subprojects/[^/]+/INTERFACE\.md", path):
        return "medium"
    for prefix, level in PATHS:
        if path == prefix or (prefix.endswith("/") and path.startswith(prefix)):
            return level
    return "high"


def worst_impact(paths):
    return max((impact(p) for p in paths), key=ORDER.index, default="low")


def outside_diff(finding, paths):
    # ARV-19: a finding without a line, or on a file the PR does not change, is no review of this diff
    return finding["line"] < 1 or (paths is not None and finding["file"] not in paths)


def keep_review(paths, findings, pr_paths=None):
    # ARV-04: only low files changed, and none is named by a finding (BACKLOG.md and STATE.md excepted)
    # ARV-19: and no earlier finding is outside the diff
    named = {f.get("file") for f in findings if isinstance(f, dict)} - {"BACKLOG.md", "STATE.md"}
    return (worst_impact(paths) == "low" and not named.intersection(paths)
            and not any(outside_diff(f, pr_paths) for f in findings))


def worst_finding(findings):
    """Worst severity (None, minor, major, blocker); a plausible finding counts one lower."""
    levels = [SEVERITIES.index(x["severity"]) - (x.get("verdict") == "plausible") for x in findings]
    return SEVERITIES[max(levels)] if levels and max(levels) >= 0 else None


def is_test_or_doc(path):
    parts = path.split("/")
    name = parts[-1]
    return "tests" in parts[:-1] or name == "tests.rs" or name.endswith("_tests.rs") or name.endswith(".md")


def count_lines(rows):
    n = lambda v: 0 if v == "-" else int(v)
    return sum(n(a) + n(d) for a, d, path in rows
               if not is_cargo_lock(path) and path not in NOT_COUNTED and not is_test_or_doc(path))


def criteria(text):
    """Criterion id -> its lines: a `- **XYZ-nn**` line and the lines indented under it, blank lines included
    (ARV-06 holds a table after one), up to the next line that is not indented (ARV-05)."""
    found, current = {}, None
    for line in text.splitlines():
        m = re.match(r"- \*\*([A-Z][A-Z0-9]*-\d+)\*\*", line)
        if m:
            current = m.group(1)
            found[current] = [line.rstrip()]
        elif current and (line.startswith("  ") or not line.strip()):
            found[current].append(line.rstrip())
        else:
            current = None
    return {k: "\n".join(v).rstrip("\n") for k, v in found.items()}


def changes_criteria(old_text, new_text):
    return criteria(old_text) != criteria(new_text)


def decide(rows, findings, review_round=1, failed_checks=(), title="", body="", recorded="", criteria_changed=()):
    paths = [path for _, _, path in rows]
    outside = [x for x in findings or [] if outside_diff(x, paths)]
    findings = [x for x in findings if x not in outside] if findings is not None else None
    worst = worst_finding(findings or [])
    level = worst_impact([path for _, _, path in rows])
    if criteria_changed:
        level = "high"
    lane = MATRIX[worst or "none"][ORDER.index(level)]
    reasons = [f"impact {level}, worst finding {worst or 'none'} → {lane}"]
    reasons += [f"changes a criterion: {p}" for p in criteria_changed]
    red = [f"required check failed: {name}" for name in failed_checks]
    if findings is None:
        red.append("not reviewed: no findings.json")
    red += [f"not reviewed: finding outside the diff ({x['file']}:{x['line']}), re-run the review" for x in outside]
    lines = count_lines(rows)
    if lines > MAX_LINES:
        red.append(f"{lines} changed lines, more than {MAX_LINES}")
    if review_round > MAX_FIX_ROUNDS and findings:
        red.append(f"fix rounds used up: review round {review_round} still has findings")
    if re.search(r"sub-release|(?<![\w.])v?\d+\.\d+\.0(?![\w.])", title):
        red.append("names a sub-release")
    # ARV-18: every finding must be recorded by file:line in the PR's added BACKLOG.md/STATE.md lines.
    where = [f"{x['file']}:{x['line']}" for x in findings or []]
    unrecorded = [f"not recorded: {w}" for w in where if not re.search(rf"(?<![\w./-]){re.escape(w)}(?!\d)", recorded)]
    if lane == "green" and unrecorded:
        lane = "amber"
    return ("red" if red else lane), reasons + red + unrecorded


def added_lines(diff):
    return "\n".join(l[1:] for l in diff.splitlines() if l.startswith("+") and not l.startswith("+++"))


def label(lane):
    return LANES[lane]


def automerge_on(value):
    return value == "on"  # ARV-10: anything but the exact string "on" is shadow


def comment(lane, reasons, impact, worst, automerge=False):
    cells = lambda row: [f"**{c}**" if (row, i) == (worst or "none", ORDER.index(impact)) else c
                         for i, c in enumerate(MATRIX[row])]
    table = ["| worst finding \\ impact | low | medium | high |", "| --- | --- | --- | --- |"]
    table += [f"| {row} | " + " | ".join(cells(row)) + " |" for row in MATRIX]
    if automerge:  # ARV-10
        last = {"green": "This PR merges by itself once the required checks pass (ARV-10).",
                "amber": "Only a green lane goes in by itself; this one needs a fix first."}.get(lane, "The owner decides on this PR.")
    else:
        last = "The gate runs in shadow: AUTOMERGE is not `on`, so nothing merges by itself." + (
            " This PR would merge by itself." if lane == "green" else "")
    return "\n".join([MARKER, f"Lane: **{lane}**", "", *[f"- {r}" for r in reasons], "", *table, "",
                      last, ""])


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


def agents_reported(messages):
    """Launched agents that reported: a tool result that is no error and no "Async agent launched", or a completed
    task_notification (a background agent reports there, later)."""
    launched, done = set(), set()
    for m in messages if isinstance(messages, list) else []:
        if not isinstance(m, dict):
            continue
        body = m.get("message")
        content = body.get("content") if isinstance(body, dict) else None
        for block in content if isinstance(content, list) else []:
            if not isinstance(block, dict):
                continue
            kind = (m.get("type"), block.get("type"))
            if kind == ("assistant", "tool_use") and block.get("name") in ("Agent", "Task"):
                launched.add(block.get("id"))
            elif kind == ("user", "tool_result") and not block.get("is_error") \
                    and "Async agent launched" not in json.dumps(block.get("content")):
                done.add(block.get("tool_use_id"))
        if m.get("type") == "system" and m.get("subtype") == "task_notification" and m.get("status") == "completed":
            done.add(m.get("tool_use_id"))
    return len(launched & done)


def review_ran(messages):
    n = agents_launched(messages)
    return n >= MIN_AGENTS and agents_reported(messages) >= n


def read_pr_paths(path):
    try:
        return open(path).read().splitlines() if path else None
    except OSError:
        return None


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--keep-review")
    ap.add_argument("--pr-files")
    ap.add_argument("--check-findings")
    ap.add_argument("--table")
    ap.add_argument("--check-review")
    ap.add_argument("--base")
    ap.add_argument("--findings")
    ap.add_argument("--round", type=int, default=1)
    ap.add_argument("--failed-check", action="append", default=[])
    ap.add_argument("--automerge", default="")
    ap.add_argument("--comment")
    ap.add_argument("--body-out")
    args = ap.parse_args()
    if args.keep_review:
        paths = [p for p in sys.stdin.read().splitlines() if p.strip()]
        earlier = load_findings(args.keep_review)
        keep = earlier is not None and keep_review(paths, earlier, read_pr_paths(args.pr_files))
        print("keep the earlier review" if keep else "review again: no earlier findings" if earlier is None
              else "review again: a changed file is not low or is named by a finding")
        sys.exit(0 if keep else 1)
    elif args.check_findings:
        found = load_findings(args.check_findings)
        pr_paths = read_pr_paths(args.pr_files)
        out = [f"{f['file']}:{f['line']}" for f in found or [] if outside_diff(f, pr_paths)]
        print(f"finding outside the diff ({', '.join(out)})" if out else "findings in the diff")
        sys.exit(1 if found is None or out else 0)
    elif args.table:
        print(findings_table(load_findings(args.table) or []), end="")
    elif args.check_review:
        messages = load_findings(args.check_review) or []
        n = agents_launched(messages)
        ok = review_ran(messages)
        got = agents_reported(messages)
        print(f"review ran {n} agents, {got} reported"
              + ("" if ok else f"; the prompt's steps need {MIN_AGENTS}, all must report"))
        sys.exit(0 if ok else 1)
    elif args.base:
        root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
        diff = subprocess.run(["git", "diff", "--numstat", "--no-renames", f"{args.base}...HEAD"], cwd=root,
                              capture_output=True, text=True)
        rows = [tuple(line.split("\t", 2)) for line in diff.stdout.splitlines() if line.count("\t") >= 2]
        if diff.returncode:  # an empty diff would read as low and green; a diff we cannot see is the owner's
            args.failed_check.append(f"git diff {args.base}...HEAD")
        rec = subprocess.run(["git", "diff", f"{args.base}...HEAD", "--", "BACKLOG.md", "STATE.md"], cwd=root,
                             capture_output=True, text=True)
        recorded = "" if rec.returncode else added_lines(rec.stdout)
        findings = load_findings(args.findings) if args.findings else None
        title, body = os.environ.get("PR_TITLE", ""), os.environ.get("PR_BODY", "")
        changed = [r[2] for r in rows if r[2].startswith("specs/")]
        old_base = subprocess.run(["git", "merge-base", args.base, "HEAD"], cwd=root, capture_output=True,
                                  text=True).stdout.strip() or args.base
        show = lambda rev, path: subprocess.run(["git", "show", f"{rev}:{path}"], cwd=root, capture_output=True,
                                                text=True).stdout
        criteria_changed = [p for p in changed if changes_criteria(show(old_base, p), show("HEAD", p))]
        lane, reasons = decide(rows, findings, args.round, args.failed_check, title, body, recorded, criteria_changed)
        print(lane)
        print("\n".join(f"- {r}" for r in reasons))
        if args.comment:
            with open(args.comment, "w") as handle:
                handle.write(comment(lane, reasons, "high" if criteria_changed else worst_impact([r[2] for r in rows]), worst_finding(findings or []), automerge_on(args.automerge)))
        if args.body_out:
            with open(args.body_out, "w") as handle:
                handle.write(with_lane(body, lane))
    else:
        ap.print_help()


if __name__ == "__main__":
    main()
