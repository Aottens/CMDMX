#!/usr/bin/env python3
"""Check that BACKLOG.md and IDEAS.md are kept up to date (run by CI, also locally).

- Every BACKLOG row's Status starts with todo, doing, done, gate3 or blocked.
- A gate3 status names a checklist path (in backticks) that exists.
- Every `### IDEA-nn` section in IDEAS.md has an index row, and every row a section.
- With PR_TITLE set: every backlog task id in the title is not `todo` (the PR updates its row).

Usage: scripts/check-tracking.py  (PR_TITLE="core: … (VS2-26)" to check a PR title)
"""
import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
STATUSES = ("todo", "doing", "done", "gate3", "blocked")
errors = []

status = {}
for n, line in enumerate((ROOT / "BACKLOG.md").read_text().splitlines(), 1):
    cells = [c.strip() for c in line.strip().strip("|").split("|")]
    if not line.startswith("| ") or cells[0] in ("Id", "---"):
        continue
    task, state = cells[0], cells[-1]
    status[task] = state.split(":")[0].split(" ")[0]
    if status[task] not in STATUSES:
        errors.append(f"BACKLOG.md:{n}: {task} status must start with one of {', '.join(STATUSES)}: {state[:60]}")
    if status[task] == "gate3":
        path = re.match(r"gate3: `([^`]+)`", state)
        if not path or not (ROOT / path.group(1)).exists():
            errors.append(f"BACKLOG.md:{n}: {task} gate3 needs an existing checklist: gate3: `tests/…`")

ideas = (ROOT / "IDEAS.md").read_text()
rows = set(re.findall(r"^\| (IDEA-\d+) \|", ideas, re.M))
sections = set(re.findall(r"^### (IDEA-\d+)\b", ideas, re.M))
for idea in sorted(sections - rows):
    errors.append(f"IDEAS.md: {idea} has a section but no row in the index table")
for idea in sorted(rows - sections):
    errors.append(f"IDEAS.md: {idea} has an index row but no section")

for task in re.findall(r"[A-Z][A-Z0-9]*-[0-9A-Za-z.]+", os.environ.get("PR_TITLE", "")):
    if status.get(task) == "todo":
        errors.append(f"BACKLOG.md: {task} is in the PR title but still todo; set it to done (date, #PR)")

print("\n".join(errors) or "tracking ok")
sys.exit(1 if errors else 0)
