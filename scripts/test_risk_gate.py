#!/usr/bin/env python3
"""Tests for the risk gate (specs/modules/autoreview.md, ARV-03..09, ARV-17..19). Run: scripts/test_risk_gate.py"""
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


# A BACKLOG.md row that records the default finding f() (ARV-13, ARV-18).
RECORDED = "| FIX-01 | thing | `a.rs:1` | ENG-01 | #300 | todo |"


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


def execution(*tools, background=(), reported=()):
    """An execution file of claude-code-action: per tool call an assistant message and its result, then the result.

    A foreground agent reports in its tool result. An agent whose index is in `background` gets the tool result
    "Async agent launched" and reports only with a completed task_notification, given when its index is in `reported`.
    """
    messages = [{"type": "system", "subtype": "init"}]
    for i, tool in enumerate(tools):
        tool = {**tool, "id": f"toolu_{i}"}
        messages.append({"type": "assistant", "message": {"content": [{"type": "text", "text": "ok"}, tool]}})
        text = "Async agent launched successfully.\nagentId: a1" if i in background else "issues: none"
        messages.append({"type": "user", "message": {"content": [
            {"type": "tool_result", "tool_use_id": f"toolu_{i}", "content": [{"type": "text", "text": text}]}]}})
    for i in reported:
        messages.append({"type": "system", "subtype": "task_notification", "tool_use_id": f"toolu_{i}",
                         "status": "completed"})
    return messages + [{"type": "result", "result": '{"findings":[]}'}]


FULL = [agent("List CLAUDE.md files"), agent("Summarize PR"),
        agent("CLAUDE.md compliance A"), agent("CLAUDE.md compliance B"), agent("Opus bug scan diff"),
        {"type": "tool_use", "name": "Task", "input": {"description": "Opus bug scan logic"}}]


def test_arv_03_review_without_its_agents_is_not_reviewed():
    bash = {"type": "tool_use", "name": "Bash", "input": {"command": "gh pr diff 298"}}
    assert gate.agents_launched(execution(*FULL, bash)) == 6
    assert gate.review_ran(execution(*FULL, bash))
    # Round 3 of #298: it prepared, then read the diff itself and returned no findings.
    assert not gate.review_ran(execution(*FULL[:2], bash, bash))
    assert not gate.review_ran(execution(*FULL[:5]))
    assert not gate.review_ran([])


def test_arv_03_review_before_its_agents_reported_is_not_reviewed():
    # #309..#317: six agents launched in the background, the four reviewers had not reported when it returned.
    assert not gate.review_ran(execution(*FULL, background=range(6), reported=(0, 1)))
    assert gate.agents_reported(execution(*FULL, background=range(6), reported=(0, 1))) == 2
    # Background agents that all completed before it returned did report.
    assert gate.review_ran(execution(*FULL, background=range(6), reported=range(6)))
    # A validator of step 4 that had not reported fails the review too.
    assert not gate.review_ran(execution(*FULL, agent("Validate issue"), background=(6,)))
    # A failed agent did not report.
    failed = execution(*FULL)
    failed[-2]["message"]["content"][0]["is_error"] = True
    assert not gate.review_ran(failed)


def test_arv_03_cli_says_how_many_reported():
    import json
    import tempfile
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "early.json"
        path.write_text(json.dumps(execution(*FULL, background=range(6), reported=(0, 1))))
        done = run("--check-review", str(path))
    assert done.returncode == 1
    assert "6" in done.stdout and "2 reported" in done.stdout, done.stdout


def test_arv_03_prompt_launches_agents_in_the_foreground():
    prompt = (ROOT / ".github/review-prompt.md").read_text()
    workflow = (ROOT / ".github/workflows/review.yml").read_text()
    assert "run_in_background" in prompt and "foreground" in prompt
    assert "CLAUDE_CODE_DISABLE_BACKGROUND_TASKS" in workflow


def test_arv_03_cli_checks_the_review():
    import json
    import tempfile
    with tempfile.TemporaryDirectory() as tmp:
        full, short = Path(tmp) / "full.json", Path(tmp) / "short.json"
        full.write_text(json.dumps(execution(*FULL)))
        short.write_text(json.dumps(execution(*FULL[:2])))
        assert run("--check-review", str(full)).returncode == 0
        done = run("--check-review", str(short))
        assert done.returncode == 1 and "2" in done.stdout and "6" in done.stdout
        assert run("--check-review", str(Path(tmp) / "missing.json")).returncode == 1


def test_arv_04_low_only_push_keeps_the_review():
    assert gate.worst_impact(["STATE.md", "BACKLOG.md", "subprojects/app/STATE.md"]) == "low"
    assert gate.worst_impact(["STATE.md", "subprojects/app/Sources/X.swift"]) == "medium"
    assert gate.keep_review(["STATE.md", "IDEAS.md"], [f("minor", file="x.rs", line=7)])
    assert gate.keep_review(["STATE.md"], [])
    assert not gate.keep_review(["STATE.md", "subprojects/app/Sources/X.swift"], [])
    assert not gate.keep_review(["STATE.md", "CLAUDE.md"], [])


def test_arv_04_push_to_a_file_a_finding_names_is_reviewed_again():
    found = [f("minor", file="docs/a.md", line=3)]
    assert not gate.keep_review(["docs/a.md"], found)
    assert not gate.keep_review(["STATE.md", "docs/a.md"], found)
    assert gate.keep_review(["docs/b.md"], found)


def test_arv_04_recording_never_starts_a_round():
    found = [f("minor", file="BACKLOG.md", line=3), f("minor", file="STATE.md", line=5)]
    assert gate.keep_review(["BACKLOG.md", "STATE.md"], found)
    # A subproject's STATE.md is not where findings are recorded: a finding on it is reviewed again.
    assert not gate.keep_review(["subprojects/app/STATE.md"], [f("minor", file="subprojects/app/STATE.md")])


def test_arv_04_review_with_a_finding_outside_the_diff_is_not_kept():
    placeholder = [f("minor", "plausible", file="N/A", line=0)]
    assert not gate.keep_review(["STATE.md"], placeholder)
    assert not gate.keep_review(["STATE.md"], [f("minor", file="x.rs", line=0)])
    # With the PR's files known, a finding on a file the PR does not change is outside the diff too.
    assert not gate.keep_review(["STATE.md"], [f("minor", file="y.rs", line=3)], pr_paths=["x.rs", "STATE.md"])
    assert gate.keep_review(["STATE.md"], [f("minor", file="x.rs", line=3)], pr_paths=["x.rs", "STATE.md"])


def test_arv_04_cli_keeps_or_reviews_again():
    import json
    import tempfile
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "findings.json"
        path.write_text(json.dumps([f("minor", file="docs/a.md", line=3)]))
        assert run("--keep-review", str(path), stdin="STATE.md\nIDEAS.md\n").returncode == 0
        assert run("--keep-review", str(path), stdin="STATE.md\ndocs/a.md\n").returncode == 1
        assert run("--keep-review", str(path), stdin="CLAUDE.md\n").returncode == 1
        # An earlier review that cannot be read is no review to keep.
        assert run("--keep-review", str(Path(tmp) / "missing.json"), stdin="STATE.md\n").returncode == 1


def test_arv_05_path_table():
    high = ["subprojects/engine/cmdmx-core/src/lib.rs", "subprojects/engine/cmdmx-protocols/src/a.rs",
            "subprojects/engine/cmdmx-daemon/src/main.rs", "subprojects/engine/Cargo.lock",
            "subprojects/app/scripts/bundle-app.sh", ".github/workflows/ci.yml", "scripts/risk-gate.py",
            "scripts/test_risk_gate.py", "CLAUDE.md", "tests/hardware/church.md", "tests/hardware/runs/x.md",
            "shows/kerk.json", "somewhere/new.txt", "subprojects/engine/Cargo.toml", ".claude/commands/x.md",
            ".github/review-prompt.md"]
    medium = ["subprojects/app/Sources/App/Main.swift", "subprojects/engine/cmdmx-control/src/lib.rs",
              "subprojects/engine/profiles/par.json", "touchosc/generate.py", "scripts/check-tracking.py",
              "scripts/dashboard.py", "DECISIONS.md", "subprojects/engine/INTERFACE.md",
              "subprojects/app/INTERFACE.md", "specs/modules/autoreview.md", "specs/PRD.md"]
    low = ["README.md", "STATE.md", "BACKLOG.md", "IDEAS.md", "subprojects/engine/STATE.md",
           "subprojects/website/manual/src/patch.md", "docs/research/x.md", "tests/manual/app-patch.md",
           "tests/fixtures/shows/church-demo.cmdmx.json", "tests/scenarios/eng-01-htp.json"]
    for path in high:
        assert gate.impact(path) == "high", path
    for path in medium:
        assert gate.impact(path) == "medium", path
    for path in low:
        assert gate.impact(path) == "low", path
    assert gate.worst_impact(["README.md", "touchosc/generate.py", "CLAUDE.md"]) == "high"


SPEC = """# Module spec: x (XYZ)

Status: proposed.

## Criteria
- **XYZ-01** The first rule,
  wrapped onto a second line.
- **XYZ-02** The second rule.

## Owner questions
1. **A question?** Recommended: yes.
"""


def test_arv_05_criterion_change_in_specs_is_high():
    edit = lambda old, new: SPEC.replace(old, new)
    assert gate.criteria(SPEC) == {"XYZ-01": "- **XYZ-01** The first rule,\n  wrapped onto a second line.",
                                   "XYZ-02": "- **XYZ-02** The second rule."}
    # A criterion added, removed, changed on its first line or on a wrapped line: the owner's (gate 1).
    for new in (SPEC + "- **XYZ-03** A third rule.\n", edit("- **XYZ-02** The second rule.\n", ""),
                edit("The second rule.", "The second rule, changed."), edit("wrapped onto", "folded onto"),
                ""):
        assert gate.changes_criteria(SPEC, new), new
    # A status line, an owner answer or a criterion that only moves: medium.
    for new in (edit("Status: proposed.", "Status: approved 2026-11-15."), edit("yes.", "yes. Accepted."), SPEC,
                edit("- **XYZ-01** The first rule,\n  wrapped onto a second line.\n", "")
                + "- **XYZ-01** The first rule,\n  wrapped onto a second line.\n"):
        assert not gate.changes_criteria(SPEC, new), new
    assert gate.changes_criteria("", SPEC)  # a new spec file brings criteria
    # A criterion that holds a table after a blank line, as ARV-06 does: the table is part of it.
    table = "- **XYZ-05** The lane:\n\n  | a | b |\n  | --- | --- |\n  | x | y |\n\n## Next\n"
    assert gate.criteria(table)["XYZ-05"].endswith("| x | y |")
    assert gate.changes_criteria(table, table.replace("| x | y |", "| x | z |"))
    spec = files("specs/modules/x.md", "README.md")
    assert gate.decide(spec, [])[0] == "green"
    lane, reasons = gate.decide(spec, [], criteria_changed=["specs/modules/x.md"])
    assert lane == "red" and any("specs/modules/x.md" in r for r in reasons), reasons


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
            found = [f(severity, file=paths[impact])] if severity else []
            lane, reasons = gate.decide(files(paths[impact]), found, recorded=f"`{paths[impact]}:1`")
            assert lane == want, (severity, impact, lane, reasons)


def test_arv_06_plausible_counts_one_lower():
    assert gate.worst_finding([f("blocker", "plausible")]) == "major"
    assert gate.worst_finding([f("major", "plausible")]) == "minor"
    assert gate.worst_finding([f("minor", "plausible")]) is None
    assert gate.worst_finding([f("minor", "plausible"), f("major")]) == "major"
    swift = "subprojects/app/Sources/A.swift"
    assert gate.decide(files(swift), [f("minor", "plausible", file=swift)], recorded=f"`{swift}:1`")[0] == "green"


def test_arv_07_failed_check_is_red():
    lane, reasons = gate.decide(files("README.md"), [], failed_checks=["engine (Rust)"])
    assert lane == "red" and any("engine (Rust)" in r for r in reasons)


APP = "subprojects/app/Sources/CMDMX/A.swift"


def test_arv_07_more_than_600_lines_is_red():
    assert gate.decide([(600, 0, APP)], [])[0] == "green"
    lane, reasons = gate.decide([(400, 201, APP)], [])
    assert lane == "red" and any("601" in r for r in reasons)


def test_arv_07_lock_file_and_touchosc_output_not_counted():
    rows = [(590, 0, APP), (5000, 4000, "subprojects/engine/Cargo.lock"),
            (900, 0, "touchosc/CMDMX.touchosc"), (300, 0, "touchosc/addresses.txt")]
    assert gate.count_lines(rows) == 590
    assert gate.count_lines([("-", "-", "docs/a.png"), (3, 2, APP)]) == 5


def test_arv_07_tests_and_docs_not_counted():
    rows = [(300, 200, "subprojects/engine/cmdmx-control/src/action.rs"),
            (260, 0, "subprojects/engine/cmdmx-core/tests/chases.rs"),
            (100, 7, "subprojects/engine/cmdmx-daemon/src/control/tests.rs"),
            (3, 0, "subprojects/engine/cmdmx-daemon/src/control/osc_tests.rs"),
            (40, 0, "tests/scenarios/eng-01-htp.json"), (2, 0, "tests/fixtures/shows/a.cmdmx.json"),
            (400, 0, "specs/modules/cuelists.md"), (12, 0, "subprojects/website/manual/src/cuelists.md"),
            (10, 0, "README.md")]
    assert gate.count_lines(rows) == 500
    # A source file that only looks like a test is code.
    assert gate.count_lines([(5, 0, "subprojects/engine/cmdmx-core/src/contests.rs"),
                             (5, 0, "subprojects/engine/cmdmx-core/src/test_util.rs")]) == 10
    assert gate.decide(rows, [])[0] == "red"  # still high: core and daemon
    assert not any("lines" in r for r in gate.decide(rows, [])[1])


def test_arv_07_fix_rounds_used_up_is_red():
    found = [f("minor", file="README.md")]
    low = files("README.md")
    assert gate.decide(low, found, review_round=2, recorded="`README.md:1`")[0] == "green"
    lane, reasons = gate.decide(low, found, review_round=3)
    assert lane == "red" and any("round" in r for r in reasons)
    assert gate.decide(low, [], review_round=3)[0] == "green"


def test_arv_07_sub_release_is_red():
    low = files("README.md")
    for title in ("release: v0.4.0", "docs: notes for 1.0.0", "Cut the sub-release now"):
        lane, reasons = gate.decide(low, [], title=title)
        assert lane == "red", title
        assert any("sub-release" in r for r in reasons)
    for title in ("release: v0.4.1", "ci: pin claude-code-action v1.0.275", "docs: 10.0 percent"):
        assert gate.decide(low, [], title=title)[0] == "green", title
    # Only the title counts: a body that mentions the next sub-release is no release (#304, #305).
    for body in ("With FIL-06, batch G is complete: next is its sub-release.", "v0.9.0 was cut by Claude.",
                 "**1.0.0 = the first service**: the next sub-release."):
        assert gate.decide(low, [], title="tracking: FIL-06 done", body=body)[0] == "green", body


def test_arv_07_every_reason_is_listed():
    lane, reasons = gate.decide([(700, 0, "CLAUDE.md")], [f("blocker")], failed_checks=["tracking"])
    assert lane == "red" and len(reasons) >= 3


def test_arv_01_review_follows_the_repo_prompt():
    workflow = (ROOT / ".github/workflows/review.yml").read_text()
    prompt = (ROOT / ".github/review-prompt.md").read_text()
    assert "review-prompt.md" in workflow
    assert "plugins: code-review" not in workflow and "/code-review:code-review" not in workflow
    assert "claude-plugins-official" in prompt and "9780cf4" in prompt and "Apache" in prompt
    # The plugin's steps: two agents to prepare, four to review, one per issue to validate, inline comments.
    for step in ("CLAUDE.md files", "summary of the changes", "Launch 4 agents in parallel", "validate",
                 "mcp__github_inline_comment__create_inline_comment"):
        assert step in prompt, step
    # Not its first step: no stop for an automated, trivial or already-reviewed PR (#305..#307).
    for stop in ("does not need code review", "automated PR", "already commented", "haiku agent to check"):
        assert stop not in prompt, stop
    assert gate.MIN_AGENTS == 6


def test_arv_03_review_execution_is_kept():
    workflow = (ROOT / ".github/workflows/review.yml").read_text()
    assert "name: review-execution" in workflow and "retention-days: 7" in workflow


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
    args = (files("subprojects/app/Sources/A.swift"), [f("minor", file="subprojects/app/Sources/A.swift")])
    assert gate.decide(*args) == gate.decide(*args) == ("amber", gate.decide(*args)[1])


def test_arv_08_cli_prints_lane_then_reasons():
    done = run("--base", "HEAD", "--findings", "/nonexistent/findings.json")
    assert done.returncode == 0, done.stderr
    lines = done.stdout.splitlines()
    assert lines[0] == "red"
    assert any(line.startswith("- ") and "not reviewed" in line for line in lines[1:])


# Low files the PR changes, so a finding on them is in the diff (ARV-19).
DOCS = files("README.md", "docs/x.md", "docs/a.md", "docs/b.md", "docs/c.md", "docs/sub/x.md")


def test_arv_18_unrecorded_finding_makes_green_amber():
    lane, reasons = gate.decide(DOCS, [f("minor", file="docs/x.md", line=7)])
    assert lane == "amber", reasons
    assert "not recorded: docs/x.md:7" in reasons
    # A plausible minor counts as none, but it is still a finding to record.
    lane, reasons = gate.decide(DOCS, [f("minor", "plausible", file="docs/x.md", line=7)])
    assert lane == "amber" and "not recorded: docs/x.md:7" in reasons


def test_arv_18_recorded_finding_stays_green():
    row = "+| FIX-03 | off by one | `docs/sub/x.md:7` | ENG-01 | #300 | todo |"
    lane, reasons = gate.decide(DOCS, [f("minor", file="docs/sub/x.md", line=7)], recorded=row)
    assert lane == "green", reasons
    assert not any(r.startswith("not recorded") for r in reasons)


def test_arv_18_file_and_line_match_exactly():
    found = [f("minor", file="docs/x.md", line=7)]
    for text in ("`docs/x.md:70`", "`docs/x.md:17`", "`lib/docs/x.md:7`", "`docs/xx.md:7`", "docs/x.md line 7"):
        lane, reasons = gate.decide(DOCS, found, recorded=text)
        assert lane == "amber" and "not recorded: docs/x.md:7" in reasons, text
    for text in ("`docs/x.md:7`", "docs/x.md:7, docs/x.md:9", "(docs/x.md:7)"):
        assert gate.decide(DOCS, found, recorded=text)[0] == "green", text


def test_arv_18_every_unrecorded_finding_is_named():
    found = [f("minor", file=f"docs/{n}.md", line=i) for i, n in enumerate("abc", 1)]
    lane, reasons = gate.decide(DOCS, found, recorded="`docs/b.md:2`")
    assert lane == "amber"
    assert "not recorded: docs/a.md:1" in reasons and "not recorded: docs/c.md:3" in reasons
    assert "not recorded: docs/b.md:2" not in reasons


def test_arv_18_red_stays_red_and_shows_what_to_record():
    lane, reasons = gate.decide(DOCS, [f("blocker", file="docs/x.md", line=7)])
    assert lane == "red" and "not recorded: docs/x.md:7" in reasons
    lane, reasons = gate.decide(files("CLAUDE.md", "docs/x.md"), [f("minor", file="docs/x.md", line=7)])
    assert lane == "red" and "not recorded: docs/x.md:7" in reasons
    lane, reasons = gate.decide(DOCS, [f("major", file="docs/x.md", line=7)])
    assert lane == "amber" and "not recorded: docs/x.md:7" in reasons


def test_arv_18_not_reviewed_or_clean_names_nothing():
    for found in (None, []):
        reasons = gate.decide(files("README.md"), found)[1]
        assert not any(r.startswith("not recorded") for r in reasons), found


def test_arv_18_only_added_lines_record():
    diff = "\n".join([
        "diff --git a/BACKLOG.md b/BACKLOG.md",
        "--- a/BACKLOG.md",
        "+++ b/BACKLOG.md",
        "@@ -1,2 +1,2 @@",
        " | FIX-01 | kept | `k.rs:1` | ENG-01 | #299 | todo |",
        "-| FIX-02 | gone | `g.rs:2` | ENG-01 | #299 | todo |",
        "+| FIX-03 | new | `n.rs:3` | ENG-01 | #300 | todo |",
    ])
    text = gate.added_lines(diff)
    assert "n.rs:3" in text
    assert "k.rs:1" not in text and "g.rs:2" not in text and "b/BACKLOG.md" not in text


def gate_in_repo(tmp, findings, changed="docs/x.md"):
    """Run the gate CLI in a throwaway repo whose last commit changes `changed`, against HEAD~1."""
    import json
    import shutil
    repo = Path(tmp) / "repo"
    (repo / "scripts").mkdir(parents=True)
    shutil.copy(SCRIPT, repo / "scripts/risk-gate.py")
    git = lambda *a: subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *a], cwd=repo,
                                    capture_output=True, text=True, check=True)
    git("init", "-q")
    git("add", ".")
    git("commit", "-qm", "base")
    (repo / changed).parent.mkdir(parents=True, exist_ok=True)
    (repo / changed).write_text("x\n")
    git("add", ".")
    git("commit", "-qm", "change")
    path = Path(tmp) / "findings.json"
    path.write_text(json.dumps(findings))
    return subprocess.run([sys.executable, repo / "scripts/risk-gate.py", "--base", "HEAD~1", "--findings", str(path)],
                          capture_output=True, text=True, cwd=repo)


def test_arv_18_cli_reads_the_findings():
    import tempfile
    with tempfile.TemporaryDirectory() as tmp:
        done = gate_in_repo(tmp, [f("minor", file="docs/x.md", line=7)])
    assert done.returncode == 0, done.stderr
    lines = done.stdout.splitlines()
    assert lines[0] == "amber", lines
    assert "- not recorded: docs/x.md:7" in lines


PR_317 = {"file": "N/A", "line": 0, "severity": "minor", "verdict": "plausible",
          "summary": "Review incomplete, not a code finding: the four reviewers of PR #317 had not reported when "
                     "output was forced, so nothing was validated or posted. Re-run the review; do not treat this "
                     "PR as clean."}


def test_arv_19_pr_317_placeholder_is_not_reviewed():
    lane, reasons = gate.decide(files("subprojects/app/Sources/A.swift"), [PR_317])
    assert lane == "red", reasons
    assert "not reviewed: finding outside the diff (N/A:0), re-run the review" in reasons
    assert not any(r.startswith("not recorded") for r in reasons), reasons


def test_arv_19_line_below_1_or_unchanged_file_is_outside_the_diff():
    rows = files("a.rs", "README.md")
    for finding in (f("minor", file="a.rs", line=0), f("minor", file="b.rs", line=4)):
        lane, reasons = gate.decide(rows, [finding], recorded="")
        where = f"{finding['file']}:{finding['line']}"
        assert lane == "red" and f"not reviewed: finding outside the diff ({where}), re-run the review" in reasons
        assert f"not recorded: {where}" not in reasons
    # A finding in the diff is a finding as before.
    lane, reasons = gate.decide(files("README.md"), [f("minor", file="README.md", line=4)])
    assert lane == "amber" and "not recorded: README.md:4" in reasons


def test_arv_19_real_findings_beside_a_placeholder_still_need_recording():
    lane, reasons = gate.decide(files("a.rs"), [PR_317, f("major", file="a.rs", line=3)])
    assert lane == "red"
    assert "not recorded: a.rs:3" in reasons and "not recorded: N/A:0" not in reasons


def test_arv_19_cli_pr_317_is_red():
    import tempfile
    with tempfile.TemporaryDirectory() as tmp:
        done = gate_in_repo(tmp, [PR_317])
    assert done.returncode == 0, done.stderr
    lines = done.stdout.splitlines()
    assert lines[0] == "red", lines
    assert "- not reviewed: finding outside the diff (N/A:0), re-run the review" in lines


def test_arv_09_labels():
    assert gate.label("green") == "risk: green"
    assert gate.label("amber") == "risk: amber"
    assert gate.label("red") == "risk: owner"
    assert set(gate.LABELS) == {"risk: green", "risk: amber", "risk: owner"}


def test_arv_09_comment():
    lane, reasons = gate.decide(files("subprojects/app/Sources/A.swift"), [f("minor", file="subprojects/app/Sources/A.swift")])
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
