# Incremental presence — light↔tile index (deferred CPU optimization) — 2026-07-23

_Component: [`client/webgl`](../../components/client/) · `game/viewport/shadowGather.ts`
(`buildPresence` / `buildCasters` / the dirty walk). Phases in [`todo.md`](todo.md)._

**Deferred by the user 2026-07-23** — working well in the 3-light case; document, build later.

## The problem (why it exists)

`buildPresence`/`buildCasters` rebuild-and-diff every frame: clear the whole window scratch → refill
from every light's reach box → **write EVERY in-window tile** (~2000) so compare-write can find the
1–3 that changed. The **GPU** is already dirty-only (compare-write ⇒ only changed tiles scatter — a
mover's reach boundary, 1–3/frame). The **CPU** wastes an O(window ≈ 2000) scan to discover changes
it already knows: `markLightMove` pushed the mover's reach box into `pendingRects` and `buildDirty`
marked exactly those tiles — but `buildPresence` re-derives it from scratch instead of consuming it.

`buildCasters` is worse: **no gate at all** — it re-scans all ~2000 every frame even when only a
light moved and no caster changed (`buildPresence` at least gates on `presSig`).

## The design (user-proposed)

A **bidirectional light↔tile index**, so a moving light only touches its own tiles:

- **`light → tiles`**: each light remembers its current reach-box tile set (the old list on move).
- **`tile → lights`** (PERSISTENT): the CPU mirror of presence — stop rebuilding it; mutate in place.

The move handler piggybacks on the dirty walk (they visit the same reach box):

```
for tile in newReachBox:            markDirty(tile);  upsert(light, tile)   // leading + interior (idempotent)
for tile in oldTiles − newReachBox: remove(light, tile);  markDirty(tile)   // trailing
lightTiles[light] = newReachBox
// changed tiles compare-write to the GPU as today
```

Static lights cost **zero**. Per-mover CPU drops from O(window ≈ 2000) → O(reach box ≈ 729 + a thin
trailing crescent). No separate leading/trailing computation — it falls out of "upsert new, prune
old − new."

## Wrinkles to handle honestly

1. **Window pan.** Incremental covers lights moving; a camera pan exposes new window tiles that need
   presence from ALL covering lights. Cleanest scope: keep incremental for light motion, **fall back
   to the full rebuild on window change** (pan is far rarer than per-frame light motion).
2. **Nearest-N rank churn.** With ≤14 lights/tile it never bites. If >14 lights overlap a tile, a
   mover entering can evict another, and removing a mover should PROMOTE the previously-15th — which
   the persistent index doesn't track. On remove-from-overfull, re-scan the lights covering the tile.
   Deferred while light counts are low; recorded so it's not a silent correctness hole.
