# Continuation hooks — forks

_Decision points, options, which we chose and why. Convention:
[`../../CONVENTIONS.md`](../../CONVENTIONS.md)._

## F1 — Where does item state live? · resolved 2026-07-25

**Checkboxes in the markdown**, not a generated sidecar or an issue-tracker schema. See
[`issues.md` I1](issues.md#i1) for the options. The deciding argument: any *second* store of item state
will drift from the prose, and we already have a documented instance of that failure class
(`current/` lagging code → the phantom-project trap). One marker the human writes anyway is the only
option with no drift surface.

## F2 — New stream vs. more `docs-authority` phases · resolved 2026-07-25

Split into this stream. `docs-authority` owns the *audit* (`docs-check`, the front door, the invariants)
and is largely delivered; the continuation half is substantial, multi-phase, and has a different
failure model (fails open and silent, vs. the audit which fails loud). Keeping them together would
bury this plan under a mostly-done stream. Cross-linked both ways so they can't drift.

## F3 — Does the granularity check block? · resolved 2026-07-25

**Warning only.** 67 items in the corpus would trip it today, so making it an ERROR would either
block the tree on day one or force a mass rewrite of plans that are otherwise fine. Its job is to
prompt decomposition at the moment a phase is picked up, which a warning does.

## F4 — Autonomy: does the hook push through *phase* boundaries? · resolved 2026-07-25

Yes — unchanged from [`docs-authority` F6](../docs-authority/forks.md) (dial = default,
blocking-but-bounded, retuned to `WORK_CHECK_MAX_NUDGES=3`). This stream does **not** turn the dial up
further; it makes the existing setting actually work. The escapes (blocker row, `.stop-reason`,
`SKIP_WORK_CHECK=1`) stay untouched, and P2 *narrows* firing (execution-intent gate) rather than
widening it.
