# Forks — lighting standing costs

_Decision points, options, which we chose and why._

## F1 — how to draw only the dirty tiles {#f1}

- **(a) Keep the fullscreen draw + per-texel dirty gate** (status quo). Simple; pays the discard
  tax on every clean texel — 6.6 M of 8.4 M fragments at the reach-4 measurement.
- **(b) Scissored draws, one per merged rect.** Exact, but N draw calls for N rects.
- **(c) Instanced tile/rect quads in ONE draw**, built from the dirty mirror the CPU already owns.

**Chosen: (c), with (a)'s texel gate kept during the transition.** One draw preserves the current
submission shape (no per-rect state churn), the vertex stream is tiny (the mirror is ≤ 512 slots),
and rects are tile-aligned so coverage is exact by construction. The `uDirty` fetch stays in the
shaders as belt-and-braces until identity is proven on the rect path, then retires — a silent
coverage bug would otherwise present as a stale-but-plausible tile, the hardest class to see.

## F2 — the receiver map's dirty channel {#f2}

The receiver map changes when BILLBOARDS change, not when lights move. Options:

- **(a) Reuse the existing class rects** (`cls 2` covers caster changes). Wrong: light-move rects
  (`cls 0/1`) would re-bake receiver texels whose value cannot have changed — which is the exact
  waste this phase exists to remove.
- **(b) A separate receiver-dirty mirror** fed only by `markPrimDirty`/`markBillboardDirty`.

**Chosen: (b).** The whole win is that light motion stops touching receiver work; that requires a
channel light motion cannot reach. Costs one more R8UI mirror + upload, trivial beside the scan it
deletes.

## F3 — separate stream, not more light-budget phases {#f3}

These items could have been appended to [light-budget](../2026-07-27-light-budget/README.md).
Kept separate because the budget stream is a STABILITY fix (bound the worst draw) and this is a
THROUGHPUT fix (shrink every draw) — different acceptance, different risk. The one coupling is
measurement order: the budget's allowance should be sized against THIS stream's improved constants,
so the final phase here re-runs the budget's P0 sweep. Recorded in both READMEs.
