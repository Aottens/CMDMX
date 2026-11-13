#!/usr/bin/env python3
"""Tests for the risk gate (specs/modules/autoreview.md, ARV-03..09, ARV-17). Run: scripts/test_risk_gate.py"""
import ast
import importlib.util
import subprocess
import sysconfig
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "scripts/risk-gate.py"


def load():
    spec = importlib.util.spec_from_file_location("risk_gate", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


gate = load()


def f(severity, verdict="confirmed", file="a.rs", line=1):
    return {"file": file, "line": line, "severity": severity, "verdict": verdict, "summary": f"{severity} thing"}


def files(*paths, lines=10):
    """numstat rows (added, deleted, path) for paths of a few lines each."""
    return [(lines, 0, p) for p in paths]


def run(*args, stdin=""):
    return subprocess.run([sys.executable, SCRIPT, *args], input=stdin, capture_output=True, text=True, cwd=ROOT)


def test_arv_03_findings_table_for_the_job_summary():
    table = gate.findings_table([f("major", file="x.rs", line=7), f("minor", "plausible")])
    assert "| x.rs | 7 | major | confirmed |" in table
    assert "minor" in table and "plausible" in table
    assert "no findings" in gate.findings_table([]).lower()


def test_arv_03_not_reviewed_reads_red():
    lane, reasons = gate.decide(files("README.md"), None)
    assert lane == "red"
    assert any("not reviewed" in r for r in reasons)


def agent(description):
    return {"type": "tool_use", "name": "Agent", "input": {"description": description, "prompt": "..."}}


def execution(*tools):
    """An execution file of claude-code-action: one assistant message per tool call, then the result."""
    messages = [{"type": "system", "subtype": "init"}]
    messages += [{"type": "assistant", "message": {"content": [{"type": "text", "text": "ok"}, t]}} for t in tools]
    return messages + [{"type": "result", "result": '{"findings":[]}'}]


FULL = [agent("Check PR eligibility"), agent("List CLAUDE.md files"), agent("Summarize PR"),
        agent("CLAUDE.md compliance A"), agent("CLAUDE.md compliance B"), agent("Opus bug scan diff"),
        {"type": "tool_use", "name": "Task", "input": {"description": "Opus bug scan logic"}}]


def test_arv_03_review_without_its_agents_is_not_reviewed():
    bash = {"type": "tool_use", "name": "Bash", "input": {"command": "gh pr diff 298"}}
    assert gate.agents_launched(execution(*FULL, bash)) == 7
    assert gate.review_ran(execution(*FULL, bash))
    # Round 3 of #298: it prepared, then read the diff itself and returned no findings.
    assert not gate.review_ran(execution(*FULL[:3], bash, bash))
    assert not gate.review_ran([])


def test_arv_03_cli_checks_the_review():
    import json
    import tempfile
    with tempfile.TemporaryDirectory() as tmp:
        full, short = Path(tmp) / "full.json", Path(tmp) / "short.json"
        full.write_text(json.dumps(execution(*FULL)))
        short.write_text(json.dumps(execution(*FULL[:3])))
        assert run("--check-review", str(full)).returncode == 0
        done = run("--check-review", str(short))
        assert done.returncode == 1 and "3" in done.stdout and "7" in done.stdout
        assert run("--check-review", str(Path(tmp) / "missing.json")).returncode == 1


def test_arv_04_low_only_push_keeps_the_review():
    assert gate.worst_impact(["STATE.md", "BACKLOG.md", "subprojects/app/STATE.md"]) == "low"
    assert gate.worst_impact(["STATE.md", "subprojects/app/Sources/X.swift"]) == "medium"
    done = run("--impact", stdin="STATE.md\nIDEAS.md\n")
    assert done.returncode == 0 and done.stdout.strip() == "low"
    assert run("--impact", stdin="STATE.md\nCLAUDE.md\n").stdout.strip() == "high"


def test_arv_05_path_table():
    high = ["subprojects/engine/cmdmx-core/src/lib.rs", "subprojects/engine/cmdmx-protocols/src/a.rs",
            "subprojects/engine/cmdmx-daemon/src/main.rs", "subprojects/engine/Cargo.lock",
            "subprojects/app/scripts/bundle-app.sh", ".github/workflows/ci.yml", "scripts/risk-gate.py",
            "scripts/test_risk_gate.py", "CLAUDE.md", "DECISIONS.md", "specs/modules/autoreview.md",
            "tests/hardware/church.md", "shows/kerk.json", "somewhere/new.txt", "subprojects/engine/Cargo.toml"]
    medium = ["subprojects/app/Sources/App/Main.swift", "subprojects/engine/cmdmx-control/src/lib.rs",
              "subprojects/engine/profiles/par.json", "touchosc/generate.py", "scripts/check-tracking.py",
              "scripts/dashboard.py"]
    low = ["README.md", "STATE.md", "BACKLOG.md", "IDEAS.md", "subprojects/engine/STATE.md",
           "subprojects/website/manual/src/patch.md", "docs/research/x.md", "tests/manual/app-patch.md"]
    for path in high:
        assert gate.impact(path) == "high", path
    for path in medium:
        assert gate.impact(path) == "medium", path
    for path in low:
        assert gate.impact(path) == "low", path
    assert gate.worst_impact(["README.md", "touchosc/generate.py", "CLAUDE.md"]) == "high"


def test_arv_06_matrix():
    expected = {
        None: ("green", "green", "red"),
        "minor": ("green", "amber", "red"),
        "major": ("amber", "amber", "red"),
        "blocker": ("red", "red", "red"),
    }
    paths = {"low": "README.md", "medium": "subprojects/app/Sources/A.swift", "high": "CLAUDE.md"}
    for severity, lanes in expected.items():
        for impact, want in zip(("low", "medium", "high"), lanes):
            found = [f(severity)] if severity else []
            lane, reasons = gate.decide(files(paths[impact]), found)
            assert lane == want, (severity, impact, lane, reasons)


def test_arv_06_plausible_counts_one_lower():
    assert gate.worst_finding([f("blocker", "plausible")]) == "major"
    assert gate.worst_finding([f("major", "plausible")]) == "minor"
    assert gate.worst_finding([f("minor", "plausible")]) is None
    assert gate.worst_finding([f("minor", "plausible"), f("major")]) == "major"
    assert gate.decide(files("subprojects/app/Sources/A.swift"), [f("minor", "plausible")])[0] == "green"


def test_arv_07_failed_check_is_red():
    lane, reasons = gate.decide(files("README.md"), [], failed_checks=["engine (Rust)"])
    assert lane == "red" and any("engine (Rust)" in r for r in reasons)


def test_arv_07_more_than_600_lines_is_red():
    assert gate.decide([(600, 0, "README.md")], [])[0] == "green"
    lane, reasons = gate.decide([(400, 201, "README.md")], [])
    assert lane == "red" and any("601" in r for r in reasons)


def test_arv_07_lock_file_and_touchosc_output_not_counted():
    rows = [(590, 0, "README.md"), (5000, 4000, "subprojects/engine/Cargo.lock"),
            (900, 0, "touchosc/CMDMX.touchosc"), (300, 0, "touchosc/addresses.txt")]
    assert gate.count_lines(rows) == 590
    assert gate.count_lines([("-", "-", "docs/a.png"), (3, 2, "README.md")]) == 5


def test_arv_07_fix_rounds_used_up_is_red():
    found = [f("minor")]
    low = files("README.md")
    assert gate.decide(low, found, review_round=2)[0] == "green"
    lane, reasons = gate.decide(low, found, review_round=3)
    assert lane == "red" and any("round" in r for r in reasons)
    assert gate.decide(low, [], review_round=3)[0] == "green"


def test_arv_07_sub_release_is_red():
    low = files("README.md")
    for title, body in (("release: v0.4.0", ""), ("docs: notes", "This is the 0.5.0 sub-release."),
                        ("docs: notes", "Cut the sub-release now")):
        lane, reasons = gate.decide(low, [], title=title, body=body)
        assert lane == "red", (title, body)
        assert any("sub-release" in r for r in reasons)
    for title in ("release: v0.4.1", "ci: pin claude-code-action v1.0.275", "docs: 10.0 percent"):
        assert gate.decide(low, [], title=title)[0] == "green", title


def test_arv_07_every_reason_is_listed():
    lane, reasons = gate.decide([(700, 0, "CLAUDE.md")], [f("blocker")], failed_checks=["tracking"])
    assert lane == "red" and len(reasons) >= 3


def test_arv_08_stdlib_only():
    tree = ast.parse(SCRIPT.read_text())
    names = {a.name.split(".")[0] for n in ast.walk(tree) if isinstance(n, ast.Import) for a in n.names}
    names |= {n.module.split(".")[0] for n in ast.walk(tree) if isinstance(n, ast.ImportFrom) and n.module}
    stdlib = sysconfig.get_paths()["stdlib"]
    for name in names:
        spec = importlib.util.find_spec(name)
        assert spec.origin in ("built-in", "frozen") or spec.origin.startswith(stdlib), (name, spec.origin)
    assert not names & {"urllib", "http", "socket", "ssl"}, names


def test_arv_08_same_input_same_lane():
    args = (files("subprojects/app/Sources/A.swift"), [f("minor")])
    assert gate.decide(*args) == gate.decide(*args) == ("amber", gate.decide(*args)[1])


def test_arv_08_cli_prints_lane_then_reasons():
    done = run("--base", "HEAD", "--findings", "/nonexistent/findings.json")
    assert done.returncode == 0, done.stderr
    lines = done.stdout.splitlines()
    assert lines[0] == "red"
    assert any(line.startswith("- ") and "not reviewed" in line for line in lines[1:])


def test_arv_09_labels():
    assert gate.label("green") == "risk: green"
    assert gate.label("amber") == "risk: amber"
    assert gate.label("red") == "risk: owner"
    assert set(gate.LABELS) == {"risk: green", "risk: amber", "risk: owner"}


def test_arv_09_comment():
    lane, reasons = gate.decide(files("subprojects/app/Sources/A.swift"), [f("minor")])
    text = gate.comment(lane, reasons, impact="medium", worst="minor")
    assert text.startswith(gate.MARKER)
    assert "| minor | green | **amber** | red |" in text
    for row in ("none", "major", "blocker"):
        assert f"| {row} |" in text
    assert "Lane: **amber**" in text
    for reason in reasons:
        assert reason in text
    assert "shadow" in text.lower()


def test_arv_09_comment_green_says_would_merge():
    lane, reasons = gate.decide(files("README.md"), [])
    assert "would merge" in gate.comment(lane, reasons, impact="low", worst=None)


def test_arv_17_tracking_names_the_lane():
    body = "Summary.\n\n## Tracking\n### Tracking changes\n- **ARB-01** todo → done\n\n🤖 Generated"
    once = gate.with_lane(body, "amber")
    assert "## Tracking\nLane: **amber** (risk-gate)\n" in once
    twice = gate.with_lane(once, "red")
    assert twice.count("Lane: ") == 1 and "Lane: **red** (risk-gate)" in twice
    assert twice.startswith("Summary.") and twice.endswith("🤖 Generated")
    assert "## Tracking\nLane: **green** (risk-gate)" in gate.with_lane("No section.", "green")
    assert gate.with_lane("", "green").strip().startswith("## Tracking")


if __name__ == "__main__":
    tests = [(n, t) for n, t in list(globals().items()) if n.startswith("test_arv_")]
    failed = 0
    for name, test in tests:
        try:
            test()
        except AssertionError as error:
            failed += 1
            print(f"FAILED {name} {error!r}")
    print(f"{len(tests) - failed}/{len(tests)} passed")
    sys.exit(1 if failed else 0)
