# Blockers — docs-authority

_Things needing the user's input. Convention: [`../../CONVENTIONS.md`](../../CONVENTIONS.md).
Open→resolved (resolved rows archive with a date)._

## Open

- **2026-07-19 · Wire `work-check` into the blocking Stop hook? (the F6 autonomy dial)** — The
  premature-pause detector [`bin/lib/work_check.py`](../../../bin/lib/work_check.py) is **built,
  tested, and runnable** as `rd work-check` (advisory; exit 0). What's deferred is whether it should
  **block** the end of a turn (like `docs-check` does) — i.e. add `work_check.py --enforce` to
  `bin/hooks/stop-docs-check.sh` so an unblocked, unfinished active stream *pushes me to continue*.
  - **Why it needs you:** unlike `docs-check` (bounded, always-safe — it just fixes doc drift), this
    hook drives **open-ended** work (code, commits) with less human-in-loop. How much unsupervised
    continuation you want is a call about *your* oversight, not mine (see [`forks.md`](forks.md) F6).
  - **What I'd do by default** (if you say "just pick"): wire it with the progress guard (two
    no-progress stops → release) + the records-reason escape, blocking-but-bounded. **Turn-up**:
    push through phase boundaries. **Turn-down**: leave it advisory-only (`rd work-check` when you
    want it), never blocking.
  - **Suggested path:** run with `docs-check`-only enforcement for a few sessions; if I still pause
    prematurely, turn `work-check` blocking on. The wire is a one-line add to the Stop hook script.
