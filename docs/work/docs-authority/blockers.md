# Blockers — docs-authority

_Things needing the user's input. Convention: [`../../CONVENTIONS.md`](../../CONVENTIONS.md).
Open→resolved (resolved rows archive with a date)._

## Open

None open.

## Resolved

- **2026-07-19 · Wire `work-check` into the blocking Stop hook? (the F6 autonomy dial)** —
  **Resolved 2026-07-19:** user said "wire with your default." Done — `work-check --enforce` is now
  stage 2 of [`bin/hooks/stop-check.sh`](../../../bin/hooks/stop-check.sh), blocking-but-bounded: a
  premature pause blocks the turn, a progress guard (no new `completed.md` entry since the last
  nudge) releases so it can't loop, and a recency window (`WORK_CHECK_WINDOW_MIN`, default 180) plus
  the `.stop-reason` / blocker escapes keep it from mis-firing. Dial position recorded in
  [`forks.md`](forks.md) F6. Turn it off any time with `SKIP_WORK_CHECK=1` or by removing stage 2.
