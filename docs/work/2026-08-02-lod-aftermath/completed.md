# Completed — LOD aftermath

## 2026-08-02 · P0 — flora's colour returns

**Coordination** ([I1](issues.md#i1)): render-performance's I11 landed; its
complete-on-arrival items open and idle ~5 h → custody taken for exactly that bug,
custody note written into their issues (I12), the rest of their charter untouched.

**The fix** ([I2](issues.md#i2) for the mechanism): `ensureCoPack` now records a
PARTIAL pack (a listed map's bytes missing — the transient-404 class that black-baked
flora for a whole session) in a `partialPacks` ledger and schedules a bounded
TIME-DRIVEN retry (3 attempts, 2 s ×2 backoff). Time-driven matters: the first
implementation triggered from `resolve()` and PARKED FOREVER once the bakes drained —
caught in the drill, rebuilt on a timer. The old frame keeps serving during a retry
(no geo flash); a successful re-pack replaces it and emits (targeted re-bake + defs
re-write). `poolStats()` gains `partial`; the HUD shows it.

**Verified live** (driven frames; foreground-tab rule respected): cold cache-less load
→ flora TEXTURED on screen, layers row reads max 255 on the graphics twin, partial 0.
Outage drill (in-page 404 window on flora's layers — the disk-delete form can't 404
because the edge serves its derived cache): partial 1 during the outage with the world
rendering progressively; outage ends → the 2 s timer re-packs → partial 0, layers row
255, no reload. Bound respected: three failed retries stop with the counter raised.
