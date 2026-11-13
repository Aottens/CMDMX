# Module spec: autoreview (ARV)

Owner: `.github/workflows/review.yml` (the review), `scripts/risk-gate.py` (the decision), the PR steps of a build
session in `CLAUDE.md`. Tests: `scripts/test_risk_gate.py`.
Status: asked for by the owner 2026-11-13 (IDEA-57: "zoveel mogelijk handwerk automatiseren", then "ik wil het
gewoon zo gaan doen, ook voor dit project"). **ARV-01..17 approved by the owner 2026-11-13**, with the recommended
answer to every question under *Owner questions*.

The owner reviews and merges every PR (gate 2) but cannot catch a defect by reading a diff (DECISIONS 2026-10-24).
A reviewer on GitHub that did not write the code, and a fixed rule that decides from its findings and the paths,
take over the PRs where a mistake is cheap, and keep the owner for those where it is not. This follows the practice
of November 2026: an agent may stop a merge but never decide one; the decision is code with tests, not a model's
judgement; sensitive paths never merge by themselves; the decision is made again at every push; every decision
stays readable afterwards (sources in IDEA-57).

## Definitions
- **Finding:** one defect the review reports: file, line, severity (`blocker`, `major`, `minor`), verdict
  (`confirmed` when checked against the code, `plausible` when not) and one sentence.
- **Impact:** how much a defect in a changed file can cost, `low`, `medium` or `high` (ARV-05).
- **Lane:** what happens to the PR: `green` (merges by itself), `amber` (the harness fixes it first) or `red` (the
  owner decides).
- **Shadow:** the gate decides and says so, but nothing merges by itself.

## Review on GitHub
- **ARV-01** Every PR from a branch of this repo gets a review by Claude on GitHub when it opens, at every push and
  when it leaves draft; a draft gets none. `anthropics/claude-code-action`, pinned to a commit SHA, runs the
  code-review plugin with `--comment`, so findings are inline comments on the lines concerned. It authenticates with
  the owner's `CLAUDE_CODE_OAUTH_TOKEN` secret.
- **ARV-02** The review looks for correctness against `CLAUDE.md` (show-proof rules, architecture constraints) and
  the criteria the PR names, not for style, and gives every finding a severity and a verdict.
- **ARV-03** The review job writes its findings as `findings.json` (a list of finding objects) to the job's artifacts
  and a table of them to the job summary. Without the secret, or when the review fails, it writes no file and the
  summary says why; the gate then reads *not reviewed*.
- **ARV-04** A push that changes only `low` files keeps the earlier review: no new review runs, and the gate reads the
  last `findings.json` of the PR. So rewriting `STATE.md` or adding backlog rows does not start another round.

## The gate
- **ARV-05** `scripts/risk-gate.py` gives each changed file an impact from one table at the top of the script; the
  highest impact of any changed file counts:
  - `high`: `subprojects/engine/cmdmx-core/`, `cmdmx-protocols/`, `cmdmx-daemon/`, `Cargo.lock`,
    `subprojects/app/scripts/`, `.github/`, `scripts/risk-gate.py` and its test, `CLAUDE.md`, `DECISIONS.md`,
    `specs/` (gate 1 is the owner's), `tests/hardware/` (gate 3 is the owner's), `shows/`, and any path the table
    does not name.
  - `medium`: `subprojects/app/Sources/`, `subprojects/engine/cmdmx-control/`, `subprojects/engine/profiles/`,
    `touchosc/`, the other files in `scripts/`.
  - `low`: `README.md`, `STATE.md`, `BACKLOG.md`, `IDEAS.md`, a subproject's `STATE.md`, `subprojects/website/`,
    `docs/`, `tests/manual/`.
- **ARV-06** The lane follows from the worst finding that is `confirmed` and the impact; a `plausible` finding counts
  one severity lower:

  | worst finding \ impact | low | medium | high |
  | --- | --- | --- | --- |
  | none | green | green | red |
  | minor | green | amber | red |
  | major | amber | amber | red |
  | blocker | red | red | red |

- **ARV-07** The lane is red, whatever the table says, when: a required check fails; the PR is *not reviewed*; it
  changes more than 600 lines (`Cargo.lock` and `touchosc/` output not counted); the fix rounds are used up (ARV-11);
  or its title or body names a sub-release.
- **ARV-08** The gate is Python standard library only, with no model and no network: the same paths, line count,
  findings, checks and round always give the same lane. It prints the lane and one line per reason. Every rule above
  has a test in `scripts/test_risk_gate.py` that carries its id.
- **ARV-09** The gate runs as the check `risk-gate` after every review and after every push that keeps one (ARV-04).
  Green passes; amber fails (a fix is due); red passes, turns auto-merge off on the PR and labels it `risk: owner`,
  so the owner merges as today. It keeps one comment on the PR up to date with the table, the PR's cell, the lane and
  the reasons, and sets one label of `risk: green`, `risk: amber`, `risk: owner`.

## Acting on the lane
- **ARV-10** Green: when the repository variable `AUTOMERGE` is `on`, auto-merge is switched on and GitHub merges once
  the required checks pass (`engine (Rust)`, `tracking (BACKLOG, IDEAS)`, `risk-gate`). The merge is made by an
  identity whose merge starts the workflows on `main`, not by the default `GITHUB_TOKEN`. With any other value of
  `AUTOMERGE`, and when it is not set, the gate runs in shadow and the comment says *would merge*.
- **ARV-11** Amber: the build session, with Auto-fix on, fixes the confirmed findings in the same PR, each fix with a
  test that carries the id of the criterion it broke, and pushes; review and gate run again. At most two fix rounds:
  a third review that still has findings makes the PR red. A fix that needs a criterion changed is not made; the PR
  goes red with that question for the owner.
- **ARV-12** Red: the session says so in chat with the gate's reasons, and the owner reviews and merges as today.
- **ARV-13** Minor findings a green PR leaves become rows under *Fixes from reviews* in `BACKLOG.md`
  (`FIX-nn`: the finding, the file, the criterion it breaks) in that PR, before it merges (ARV-04 keeps that push
  from starting a new round).
- **ARV-14** A fix block: the harness may build open `FIX-nn` rows without asking the owner, since they repair
  criteria that exist and need no spec. At most three rows per build, in its own session, branch and PR, through the
  same review and gate. It starts none while another fix block's PR is open.
- **ARV-15** After an auto-merged PR that changes the app or the engine, Claude pushes the patch tag (`0.x.y`) within
  the hour, with release notes as WEB-37 asks, and checks that the release has its DMG. A sub-release (`0.x.0`,
  REL-26) and its gap review stay with the owner.

## Calibration
- **ARV-16** `AUTOMERGE` stays off until five PRs have gone through the gate in shadow. A run file lists for each the
  lane, what the owner did, and whether a fix to that PR followed within a week. The owner turns `AUTOMERGE` on after
  reading it, or not.
- **ARV-17** The `## Tracking` section of every PR names its lane, so the owner sees what merged by itself next to
  what it records.

## Not now
- Claude's managed Code Review: Team and Enterprise plans only. The workflow of ARV-01 runs the same plugin.
- A reviewer that approves: GitHub can let Copilot's approval count toward a merge; here the review never approves,
  the gate decides.
- PRs from forks get no secrets on a public repo, so they are *not reviewed* and red.
- The reviewer reads `CLAUDE.md` from the PR's own checkout. A PR that changes it is `high`, so it never merges by
  itself.

## Owner questions
Answered by the owner 2026-11-13: every recommendation accepted ("ja, alle vijf").
1. **Do medium PRs (app, control, profiles) with no findings merge by themselves?** Recommended: yes. Most PRs are
   there, and the core, protocols and daemon stay with the owner. Otherwise: only `low` PRs.
2. **How many fix rounds before the owner?** Recommended: two. Each round is a full review on the owner's plan.
3. **Size limit?** Recommended: 600 changed lines. Larger diffs are where a review misses most.
4. **A patch tag without asking after an auto-merge?** Recommended: yes (ARV-15). A sub-release stays the owner's.
5. **Which token?** Recommended: the owner's subscription (`claude setup-token`), not an API key: no separate bill,
   but every review counts against the plan's usage.
