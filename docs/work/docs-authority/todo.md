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

Stop hook (`.claude/settings.json` → `bin/hooks/stop-docs-check.sh`, exit-2 block + progress-aware
loop guard) + git pre-commit (`bin/hooks/pre-commit`, symlinked), both tested. Hook behaviour +
install documented in this stream's README. Deferred: an `rd docs install-hooks` to auto-symlink
pre-commit on a fresh clone (low value; the one-line `ln -sf` is documented).

## P4 — Freshness provenance · ✅ DONE (see [`completed.md`](completed.md))

Stamp convention documented in `CONVENTIONS.md` (§ `current/`); the staleness **comparison** is
built (warning-only) and reuses the component README's existing `Path:` field as the code-path map —
no new `code:` line needed. It caught a real 2-day lag on `index/current` on its first run.

## P5 — `bin/rd work-check` (detector) · ✅ DONE · blocking-wire BLOCKED on F6 dial

Detector [`bin/lib/work_check.py`](../../../bin/lib/work_check.py) built + tested (all 3 paths
deterministic). Firing condition, as-built: **active stream** (most-recently-modified `open` stream)
has open executable todo items AND no open blocker AND no `.stop-reason` marker → premature. The
fork check was **dropped** (a fork is self-resolved; only a blocker/stop-reason justifies a pause —
see [`deviations.md`](deviations.md) D5).

Remaining item — **wiring `work-check --enforce` into the blocking Stop hook** — is an open blocker
(needs the F6 autonomy-dial decision): see [`blockers.md`](blockers.md). Advisory `rd work-check`
works today; the progress-guard-vs-block behaviour is settled at the hook layer when the dial is set.
