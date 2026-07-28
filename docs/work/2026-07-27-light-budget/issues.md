# Issues — light budget

_Problems hit, candidate solutions, which we chose and why._

## I1 — the lighting path is the unbudgeted one, and I had it backwards

Worth recording because I asserted the opposite earlier today and built a fix on it.

In [plane-intersection I5](../2026-07-27-plane-intersection/issues.md) I claimed the G-buffer bake was
unbounded and wrote a per-frame budget for the shadow gather on that basis. `SquareCache.bakeDirty` takes
a `budget: number` and always has — one `grep` of the signature would have killed the theory. That work
was reverted.

**The truth is the mirror image.** The G-buffer is capped:

    this.warm.bakeDirty(BAKE_BUDGET);                                              // 128
    this.map.bakeDirty(Math.max(BAKE_BUDGET - this.warm.lastBaked, COLD_BAKE_FLOOR));  // >= 64

and the **lighting gather has no cap at all** — `buildDirty` marks every dirty slot and the gather is one
fullscreen draw over all of them.

So the budget being built here is the right change, arrived at for the right reason the second time. The
reverted `pendingWork` machinery is close to what P2 needs; it was not wrong code so much as code built
on an unverified mechanism.

**The lesson is the same one this session keeps teaching:** a confirmed symptom (the GPU dies) plus an
invented mechanism (unbounded G-buffer bake) reads as settled and is more dangerous than an open
question. Read the signature before theorising about the callee.

## I2 — the cost model, and where its constants came from

The numbers this stream's budget is sized from, so they can be re-derived rather than trusted:

| reach | dirty tiles / frame | ms | ms per pair |
|---|---|---|---|
| 4 | 110 | 0.170 | 0.00155 |
| 8 | 263 | 0.394 | 0.00150 |
| 12 | 404 | 0.692 | 0.00171 |

Measured on the analytic-interval build with the calibrated orbit harness (one moving light, zoom 1, 240
frames, cold draw discriminated by render target). Flatness of the last column is what licenses treating
a tile-light pair as a constant unit — **and P0 must confirm it still holds with several lights on one
tile**, since every figure above is single-light and the whole budget rests on that constant.

Ceiling for context: 32 x 16 = 512 slots in the window, up to 16 lights per tile, so ~8 200 pairs is the
theoretical worst frame — around 14 ms in ONE draw.
