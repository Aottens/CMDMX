# Module spec: tracking (TRK)

Owner: `scripts/check-tracking.py`, `scripts/dashboard.py`, the SessionStart hook in `.claude/settings.json`.
Status: asked for by the owner 2026-11-05 ("make sure this does not keep drifting", "a dashboard that keeps itself
up to date"); the owner approves these criteria by merging their PR. Tests: `scripts/test_tracking.py`.

The owner cannot see where the project stands when `STATE.md` drifts from `BACKLOG.md`, or when a checkout lags
behind `main`. So the handoff repeats as little as it can, CI compares what is left, and everything that shows the
state reads it from `main`.

## Handoff
- **TRK-01** `STATE.md` has exactly the sections Now, Next, Waiting on the owner, Blocked, Owner to decide and Good
  to know, at most 70 lines of at most 120 characters, and no version number. CI fails otherwise.
- **TRK-02** `STATE.md` names no BACKLOG task that is done, and *Next* names at least one that is open. CI fails
  otherwise, so finishing a task named there forces the rewrite in the same PR.
- **TRK-03** Every `gate3` task is named under *Waiting on the owner* and every `blocked` task under *Blocked*; a
  task under *Blocked* has status `blocked`. CI fails otherwise.
- **TRK-09** Every BACKLOG id is used by one row. CI fails otherwise (VS8-10 had two rows; one hid the other's `todo`).
- **TRK-04** Every session start prints `STATE.md` as it is on `main`, whatever the checkout is on.

## Dashboard
- **TRK-05** `scripts/dashboard.py` shows `origin/main`, never the working tree. It fetches at least every 30 s and
  the open page follows without a reload or a restart.
- **TRK-06** It serves on 127.0.0.1 only, uses the Python standard library plus `git` and `gh`, and changes nothing.
- **TRK-07** The page shows what waits on the owner first, then Now and Next, progress per backlog section, every
  task with search and filters by status and section, open PRs with their checks, recent merges, releases, the
  workflows on `main` and the version installed on this Mac.
- **TRK-08** A failing `git` or `gh` keeps the last good data on the page, says which source failed and how old its
  data is, and the dashboard keeps running.
