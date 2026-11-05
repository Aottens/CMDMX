#!/usr/bin/env python3
"""Tests for the tracking tools (specs/modules/tracking.md). Run: scripts/test_tracking.py"""
import importlib.util
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def load_dashboard():
    spec = importlib.util.spec_from_file_location("dashboard", ROOT / "scripts/dashboard.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def check_with(state_edit, backlog_edit=lambda b: b):
    """Run check-tracking.py on a copy of the repo's tracking files with STATE.md edited; return its output."""
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        (tmp / "scripts").mkdir()
        shutil.copy(ROOT / "scripts/check-tracking.py", tmp / "scripts")
        for name in ("BACKLOG.md", "IDEAS.md", "README.md"):
            shutil.copy(ROOT / name, tmp)
        shutil.copytree(ROOT / "tests/hardware", tmp / "tests/hardware", ignore=shutil.ignore_patterns("runs"))
        (tmp / "BACKLOG.md").write_text(backlog_edit((ROOT / "BACKLOG.md").read_text()))
        (tmp / "STATE.md").write_text(state_edit((ROOT / "STATE.md").read_text()))
        done = subprocess.run([sys.executable, tmp / "scripts/check-tracking.py"], capture_output=True, text=True)
        return done.stdout


def trk_01_repo_state_passes():
    assert "tracking ok" in check_with(lambda s: s)


def trk_01_missing_section_fails():
    assert "sections" in check_with(lambda s: s.replace("## Good to know", "## Notes"))


def trk_01_too_many_lines_fail():
    assert "lines" in check_with(lambda s: s + "- filler\n" * 80)


def trk_01_long_line_fails():
    assert "characters" in check_with(lambda s: s + "x" * 121 + "\n")


def trk_01_version_number_fails():
    assert "version" in check_with(lambda s: s + "- out as v9.9.9\n")


def trk_02_done_task_fails():
    assert "VS8-09" in check_with(lambda s: s + "- VS8-09 is done\n")


def trk_02_next_without_open_task_fails():
    edit = lambda s: re.sub(r"## Next\n.*?(?=## Waiting)", "## Next\nNothing.\n\n", s, flags=re.S)
    assert "## Next names no open" in check_with(edit)


def trk_03_gate3_missing_from_waiting_fails():
    assert "SPF-03" in check_with(lambda s: re.sub(r"- SPF-03[^\n]*\n", "", s))


def trk_03_blocked_missing_fails():
    assert "VS2-50" in check_with(lambda s: re.sub(r"- VS2-50[^\n]*\n", "- none\n", s))


def trk_03_open_task_under_blocked_fails():
    assert "REC-10" in check_with(lambda s: s.replace("## Blocked\n", "## Blocked\n- REC-10 waits\n"))


def trk_09_duplicate_id_fails():
    row = "| REC-10 | Again | X | S | done |\n"
    assert "REC-10 is used by 2 rows" in check_with(lambda s: s, lambda b: b + row)


def trk_07_backlog_rows_carry_their_section():
    tasks = load_dashboard().parse_backlog(
        "## Phase\n### Batch\n| Id | Task | Spec | Size | Status |\n|---|---|---|---|---|\n"
        "| A-1 | Do it | X-1 | S | done (2026-11-05, #1) |\n| A-2 | Run it | X-2 | M | gate3: `t.md` |\n")
    assert [(t["id"], t["status"], t["section"]) for t in tasks] == [("A-1", "done", "Batch"), ("A-2", "gate3", "Batch")]


def trk_07_state_splits_into_sections():
    assert load_dashboard().parse_sections("# T\nintro\n## Now\n- a\n## Next\n1. b\n") == {"Now": "- a", "Next": "1. b"}


def trk_08_failing_source_keeps_last_good_data():
    dashboard = load_dashboard()
    snapshot = {"sources": {}}
    dashboard.update(snapshot, "prs", lambda: ["good"], now=100)

    def fails():
        raise OSError("offline")

    dashboard.update(snapshot, "prs", fails, now=200)
    assert snapshot["prs"] == ["good"]
    assert snapshot["sources"]["prs"] == {"at": 100, "error": "offline"}


if __name__ == "__main__":
    tests = [(n, f) for n, f in list(globals().items()) if n.startswith("trk_")]
    failed = 0
    for name, test in tests:
        try:
            test()
        except AssertionError as error:
            failed += 1
            print(f"FAILED {name} {error}")
    print(f"{len(tests) - failed}/{len(tests)} passed")
    sys.exit(1 if failed else 0)
