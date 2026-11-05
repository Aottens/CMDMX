#!/usr/bin/env python3
"""Check that BACKLOG.md, IDEAS.md and the run files are kept up to date (run by CI, also locally).

- Every BACKLOG id is used once.
- Every BACKLOG row's Status starts with todo, doing, done, gate3 or blocked.
- A gate3 status names a checklist path (in backticks) that exists.
- Every `### IDEA-nn` section in IDEAS.md has an index row, and every row a section.
- A task status named in an IDEAS index row ("VS2-26 todo") matches its BACKLOG row.
- A run file from 2026-10-24 on has `## Open points`: `- none`, or one line per point ending in
  `→ <where it went>` (a BACKLOG id, an IDEA id or an existing file in backticks).
- README.md's `## Status` has at most STATUS_MAX_LINES lines of STATUS_MAX_WIDTH characters, no version
  number (link the latest release), and names a not-yet-done BACKLOG id (what is next), none done.
- STATE.md (TRK-01..03): exactly STATE_SECTIONS, at most STATE_MAX_LINES lines of STATUS_MAX_WIDTH characters, no
  version number, no done task named; ## Next names an open task; every gate3 task is under
  ## Waiting on the owner, every blocked task under ## Blocked and only those there.
- With PR_TITLE set: every backlog task id in the title is not `todo` (the PR updates its row), and
  PR_BODY has a `## Tracking` section.

With --summary <base> it prints, in Markdown, what this branch changes against <base>: new and
moved BACKLOG and IDEAS rows and the open points of the run files it touches. CI writes it to the
job summary; paste it into the PR description under "## Tracking". It also warns (never fails)
when subprojects/app/Sources changes without subprojects/website/manual/ (WEB-33), and when a row
moved to done/gate3 but README `## Status` is unchanged.

Usage: scripts/check-tracking.py [--summary origin/main]  (PR_TITLE="core: … (VS2-26)" to check a title)
"""
import os
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
STATUSES = ("todo", "doing", "done", "gate3", "blocked")
TASK = r"[A-Z][A-Z0-9]*-[0-9A-Za-z.]+"
RUNS_FROM = "2026-10-24"
STATUS_MAX_LINES = 8
STATUS_MAX_WIDTH = 120
STATE_MAX_LINES = 70
STATE_SECTIONS = ["Now", "Next", "Waiting on the owner", "Blocked", "Owner to decide", "Good to know"]
errors = []


def backlog_rows(text):
    """{id: (line number, task text, status cell)} of every BACKLOG table row."""
    rows = {}
    for n, line in enumerate(text.splitlines(), 1):
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if line.startswith("| ") and cells[0] not in ("Id", "---"):
            rows[cells[0]] = (n, cells[1], cells[-1])
    return rows


def idea_rows(text):
    """{IDEA-nn: (idea text, status cell)} of the IDEAS index table."""
    return {m[0]: (m[1].strip(), m[2].strip()) for m in re.findall(r"^\| (IDEA-\d+) \|([^|]*)\|([^|]*)\|", text, re.M)}


def open_points(text):
    """The `- ` lines under `## Open points`, or None when the section is missing."""
    m = re.search(r"^## Open points[^\n]*\n(.*?)(?=^## |\Z)", text, re.M | re.S)
    return None if m is None else [l[2:].strip() for l in m.group(1).splitlines() if l.startswith("- ")]


def status_text(readme):
    """The text under README's `## Status`, or "" when the section is missing."""
    m = re.search(r"^## Status\n(.*?)(?=^## )", readme, re.M | re.S)
    return m.group(1) if m else ""


backlog_text = (ROOT / "BACKLOG.md").read_text()
backlog = backlog_rows(backlog_text)
for task, count in Counter(re.findall(r"^\| ([^|]+?) \|", backlog_text, re.M)).items():
    if count > 1 and task not in ("Id", "---"):
        errors.append(f"BACKLOG.md: {task} is used by {count} rows; one row would hide the other's status")
status = {task: cell.split(":")[0].split(" ")[0] for task, (_, _, cell) in backlog.items()}
for task, (n, _, cell) in backlog.items():
    if status[task] not in STATUSES:
        errors.append(f"BACKLOG.md:{n}: {task} status must start with one of {', '.join(STATUSES)}: {cell[:60]}")
    if status[task] == "gate3":
        path = re.match(r"gate3: `([^`]+)`", cell)
        if not path or not (ROOT / path.group(1)).exists():
            errors.append(f"BACKLOG.md:{n}: {task} gate3 needs an existing checklist: gate3: `tests/…`")

ideas_text = (ROOT / "IDEAS.md").read_text()
ideas = idea_rows(ideas_text)
sections = set(re.findall(r"^### (IDEA-\d+)\b", ideas_text, re.M))
for idea in sorted(sections - set(ideas)):
    errors.append(f"IDEAS.md: {idea} has a section but no row in the index table")
for idea in sorted(set(ideas) - sections):
    errors.append(f"IDEAS.md: {idea} has an index row but no section")
for idea, (_, cell) in ideas.items():
    for ids, said in re.findall(rf"((?:{TASK}(?: #\d+)?(?:,| and)? )+)({'|'.join(STATUSES)})\b", cell):
        for task in re.findall(TASK, ids):
            if task in status and status[task] != said:
                errors.append(f"IDEAS.md: {idea} says {task} {said}, BACKLOG says {status[task]}")

for run in sorted(ROOT.glob("tests/*/runs/*.md")):
    if run.name[:10] < RUNS_FROM:
        continue
    where = run.relative_to(ROOT)
    points = open_points(run.read_text())
    if points is None:
        errors.append(f"{where}: needs a `## Open points` section (`- none` when there are none)")
        continue
    for point in points:
        if point == "none":
            continue
        target = re.search(r"→ *(`([^`]+)`|\S+)\s*$", point)
        ok = target and (target.group(2) and (ROOT / target.group(2)).exists() or target.group(1) in status or target.group(1) in ideas)
        if not ok:
            errors.append(f"{where}: open point needs `→ <BACKLOG id | IDEA id | `existing file`>`: {point[:60]}")

readme = (ROOT / "README.md").read_text()
status_lines = [line for line in status_text(readme).splitlines() if line.strip()]
if len(status_lines) > STATUS_MAX_LINES:
    errors.append(f"README.md: ## Status has {len(status_lines)} lines, at most {STATUS_MAX_LINES}; rewrite it and move features under What works today")
for line in status_lines:
    if len(line) > STATUS_MAX_WIDTH:
        errors.append(f"README.md: ## Status line is {len(line)} characters, at most {STATUS_MAX_WIDTH}: {line[:40]}…")
status_blob = "\n".join(status_lines)
if re.search(r"\bv?\d+\.\d+\.\d+\b", status_blob):
    errors.append("README.md: ## Status names a version number; link the latest release instead, a version there is stale once a tag is pushed")
status_ids = [t for t in (i.rstrip(".,") for i in re.findall(TASK, status_blob)) if t in status]
if not status_ids:
    errors.append("README.md: ## Status names no BACKLOG id; name what is next")
for task in dict.fromkeys(status_ids):
    if status[task] == "done":
        errors.append(f"README.md: ## Status names {task}, which is done; rewrite the status")


def named(text):
    """The BACKLOG ids named in text."""
    return {t for t in (i.rstrip(".,") for i in re.findall(TASK, text)) if t in status}


def state_section(text, title):
    m = re.search(rf"^## {title}\n(.*?)(?=^## |\Z)", text, re.M | re.S)
    return m.group(1) if m else ""


state = (ROOT / "STATE.md").read_text()
if re.findall(r"^## (.+)$", state, re.M) != STATE_SECTIONS:
    errors.append(f"STATE.md: its sections must be exactly: {', '.join(STATE_SECTIONS)}")
if len(state.splitlines()) > STATE_MAX_LINES:
    errors.append(f"STATE.md: {len(state.splitlines())} lines, at most {STATE_MAX_LINES}; it is the handoff, not a log")
for n, line in enumerate(state.splitlines(), 1):
    if len(line) > STATUS_MAX_WIDTH:
        errors.append(f"STATE.md:{n}: {len(line)} characters, at most {STATUS_MAX_WIDTH}")
if re.search(r"\bv?\d+\.\d+\.\d+\b", state):
    errors.append("STATE.md: names a version number; a version there is stale once a tag is pushed")
for task in sorted(named(state)):
    if status[task] == "done":
        errors.append(f"STATE.md: names {task}, which is done; rewrite the handoff")
if not any(status[t] != "done" for t in named(state_section(state, "Next"))):
    errors.append("STATE.md: ## Next names no open BACKLOG task; say what is next")
waiting, blocked = named(state_section(state, "Waiting on the owner")), named(state_section(state, "Blocked"))
for task in sorted(t for t in status if status[t] == "gate3" and t not in waiting):
    errors.append(f"STATE.md: {task} is gate3 in BACKLOG but not under ## Waiting on the owner")
for task in sorted(t for t in status if status[t] == "blocked" and t not in blocked):
    errors.append(f"STATE.md: {task} is blocked in BACKLOG but not under ## Blocked")
for task in sorted(t for t in blocked if status[t] != "blocked"):
    errors.append(f"STATE.md: {task} is under ## Blocked but {status[task]} in BACKLOG")

if os.environ.get("PR_TITLE") and "## Tracking" not in os.environ.get("PR_BODY", ""):
    errors.append("PR description: add a `## Tracking` section with `scripts/check-tracking.py --summary origin/main`")

for task in re.findall(TASK, os.environ.get("PR_TITLE", "")):
    if status.get(task) == "todo":
        errors.append(f"BACKLOG.md: {task} is in the PR title but still todo; set it to done (date, #PR)")


def git(*args):
    return subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True).stdout


def summary(base):
    out = [f"### Tracking changes against `{base}`", ""]
    old = backlog_rows(git("show", f"{base}:BACKLOG.md"))
    for task, (_, text, cell) in backlog.items():
        if task not in old:
            out.append(f"- **New task {task}** ({cell}): {text[:140]}")
        elif old[task][2] != cell:
            out.append(f"- **{task}** {old[task][2][:60]} → **{cell[:100]}** — {text[:80]}")
    old_ideas = idea_rows(git("show", f"{base}:IDEAS.md"))
    for idea, (text, cell) in ideas.items():
        if idea not in old_ideas:
            out.append(f"- **New idea {idea}** ({cell}): {text}")
        elif old_ideas[idea][1] != cell:
            out.append(f"- **{idea}** {old_ideas[idea][1]} → **{cell}** — {text}")
    for name in git("diff", "--name-only", base, "--", "tests/manual/runs", "tests/hardware/runs").split():
        points = open_points((ROOT / name).read_text()) if (ROOT / name).exists() else None
        out.append(f"- Run `{name}`, open points: " + ("none" if not points or points == ["none"] else ""))
        out.extend(f"  - {p}" for p in points or [] if p != "none")
    changed = git("diff", "--name-only", base).split()
    if any(f.startswith("subprojects/app/Sources") for f in changed) and not any(
            f.startswith("subprojects/website/manual/") for f in changed):
        out.append("- **Warning (WEB-33):** changes subprojects/app/Sources but not the manual (subprojects/website/manual/)")
    moved = [t for t, (_, _, cell) in backlog.items() if status[t] in ("done", "gate3")
             and (old[t][2].split(":")[0].split(" ")[0] if t in old else "") != status[t]]
    if moved and status_text(readme) == status_text(git("show", f"{base}:README.md")):
        out.append(f"- **Warning:** {', '.join(moved)} finished but README ## Status did not change: is it still true?")
    if len(out) == 2:
        out.append("No BACKLOG, IDEAS or run-file changes.")
    return "\n".join(out)


if "--summary" in sys.argv:
    print(summary(sys.argv[sys.argv.index("--summary") + 1]))
    print()
print("\n".join(errors) or "tracking ok")
sys.exit(1 if errors else 0)
