# Todo — docs-authority

_Executes: [`docs/CONVENTIONS.md`](../../CONVENTIONS.md). Listed in **execution order** (phases),
not newest-first — the whole plan was authored 2026-07-19 in one pass; date-ordering starts once
items are added later. Items move to `completed.md` as they land._

The audit's invariants (P2) are the spine — everything else exists to make them enforceable and
to bring the tree into a state where they pass. Read the invariant list first; the rest is scaffolding.

---

## P0 — Compliance sweep · ✅ DONE (see [`completed.md`](completed.md))

Brought the tree to green; invariant refinements discovered here logged in
[`deviations.md`](deviations.md) (D1–D3) and folded into P2 below.

## P1 — The front door · ✅ DONE (see [`completed.md`](completed.md))
## P2 — The audit `bin/rd docs-check` · ✅ DONE (see [`completed.md`](completed.md))

Six invariants live in [`bin/lib/docs_check.py`](../../../bin/lib/docs_check.py); tree is green.
Precision refinements from the first run in [`deviations.md`](deviations.md) D4.

## P3 — Wire the automatic hook · ✅ DONE (see [`completed.md`](completed.md))

Stop hook (`.claude/settings.json` → `bin/hooks/stop-check.sh`, exit-2 block + progress-aware
loop guard) + git pre-commit (`bin/hooks/pre-commit`, symlinked), both tested. Hook behaviour +
install documented in this stream's README. Deferred: an `rd docs install-hooks` to auto-symlink
pre-commit on a fresh clone (low value; the one-line `ln -sf` is documented).

## P4 — Freshness provenance · ✅ DONE (see [`completed.md`](completed.md))

Stamp convention documented in `CONVENTIONS.md` (§ `current/`); the staleness **comparison** is
built (warning-only) and reuses the component README's existing `Path:` field as the code-path map —
no new `code:` line needed. It caught a real 2-day lag on `index/current` on its first run.

## P5 — `bin/rd work-check` + continuation hook · ✅ DONE (see [`completed.md`](completed.md))

Detector [`bin/lib/work_check.py`](../../../bin/lib/work_check.py) built + tested; **wired into the
Stop hook** (stage 2, `--enforce`, blocking-but-bounded — F6 dial resolved to default, see
[`forks.md`](forks.md) / [`blockers.md`](blockers.md)). Firing condition, as-built: active stream
(most-recently-modified `open` stream, within the recency window) has open executable todo items AND
no open blocker AND no `.stop-reason` → premature. Fork check dropped (D5).

## P6 — Make the continuation hook actually fire · ✅ DONE (see [`completed.md`](completed.md))

Audited on the user's report that turns still end after every task. The hook was silently dead
(status-filtered stream selection → stale fallback → "nothing to check"). Fixed: session→stream
binding via a PostToolUse hook (answers "was it *this* session's stream?"), status-agnostic
selection, header-based blocker detection, a widened progress guard (`WORK_CHECK_MAX_NUDGES`,
default 3), and an actionable nudge naming the next items + the four legitimate exits.
