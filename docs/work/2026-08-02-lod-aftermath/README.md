# LOD aftermath — fix forward, don't roll back — 2026-08-02

_Component: [`client/webgl`](../../components/client/webgl/). The user's decision after
the one-resolution P6 diagnosis: **move forward with fixes and mitigations instead of
rolling back** — this stream absorbs
[`one-resolution`](../2026-08-02-one-resolution/README.md) P6's open remainder and
carries each named bug from investigation to fix._

## Why forward, not back (F1 — the decision's evidence)

The in-place A/B against the pre-stream commit FROZE the renderer under the very
tick storms HEAD survives — the old invalidateAll-per-arrival bake stampede that
render-performance's I11 fixed. Rolling back buys that freeze back, resurrects the
ladder's duplicate tiers, and discards the already-restored preview kick. The costs of
the new world are individually named, individually fixable bugs; the cost of the old
world was structural.

## The bug ledger (from one-resolution I4, each with its known depth)

1. **Black flora** — flora's colour IS its layers reconstruction (near-black residual
   by design), and its layers quadrant is EMPTY on both atlas twins while the disk
   bytes are healthy: the independent-per-map pack landed without layers and nothing
   re-packs. ROOT KNOWN. Owned in code by the armed `render-performance` session's
   seam — execution here COORDINATES first (check their tree state at run time; verify
   if landed, implement if abandoned; never edit that seam while their session is
   live on it).
2. **"Clipped" trees = lighting darkness** — unlit A/B proves geometry whole; hard
   black bands across canopies are shadow/N·L interplay (normal-frames' pitch, the
   billboard shadow path, the wrap floor) at high zoom. ROOT NOT YET ISOLATED.
3. **Z-order** — one unlit reproduction (black blob over a canopy); plausibly reduces
   to bug 1 (a colored flora in the same order may read correctly). RE-JUDGE after 1.
4. **Pan at max/min zoom** — unreproduced; pan artifacts are a live-eye class. Needs a
   FOREGROUND drill (the hidden-tab rAF phantom is recorded and must not be
   re-diagnosed as a bug).

## Standing constraints

The foreground-tab rule for anything visual-timing; the packing LAW (one-resolution
F3) and the twin-page split (F4) are settled — fixes build on them, never around them;
`__gather()` occupancy is part of every acceptance (an all-zero shadow buffer hides
from `__lightexact`).
