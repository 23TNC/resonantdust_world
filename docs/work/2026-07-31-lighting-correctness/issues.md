# Issues — lighting correctness

## I1 — THE root defect: lighting is a debug harness, not a system {#i1}

Pinned on the live fixture (`focus=104,54`, cold page load):

- **Off by default**: `uLit` stays 0 until `__lit(true)` — the game renders unlit, always.
- **One-shot + readiness race**: `__lit(true)` runs `buildRecords()` ONCE. On this session's
  first call it minted **0 definitions and 0 scene prims** — `resolver.resolve()` had no
  frames streamed yet, and the builder silently skips unresolved stems. A later manual
  `__buildrecords()` on the warm atlas minted 2 defs / 455 prims. Nothing re-runs it:
  no hook on frame streaming, window movement, content swap, or scene churn.
- **Movers have no records at all**: the builder iterates `map.standingPrims()` — the COLD
  cache. The wolf and the placed human are warm prims: never lit, never casting. On screen:
  the human stands flat gray INSIDE a light pool (capture in the session record).
- **Content lights don't exist**: the torch's DSL light struct (`&thing.light.*`) is unread;
  the only emitters are debug prims (`buildRecords`' one at (100,50) + `__lights(n)`).
- **`__lights` placed its grid off-window**: recorded light tiles (3..31, 4..11) against a
  window at (88,46) — the `map.window` it read was not the drawn window at call time. The
  16-light fixture lights nothing visible; `droppedLights` 2426 from self-overlap.
- **Stale emitters accumulate**: `placeLights` resets `liveLights` but never frees the prior
  batch's prims, and `buildLights` never clears tiles it no longer covers.

Everything below is subordinate to this: bbox/silhouette/z verification against a harness
that only lights hand-minted snapshots is not verification of lighting. The plan gains a
LIVE-INTEGRATION phase (P1b, forks F4).

## I2 — bbox: `frameSpan` derives from streamed atlas px, not the def's world span {#i2}

The def→(stored, actual) table, live (`rec.debugDefinition` vs `resolver.opaqueBBox`):

| block | stem | stored frameSpan | TRUE world span | stored sub (x,y,w,h) | derived-from-bbox @span1 |
|---|---|---|---|---|---|
| 1 | flora/e | 1 | 1 ✓ | (1,3,14,12) | (1,3,14,12) ✓ |
| 2 | conifer/e | **1** | **2** (DSL `&thing.span 2`) | (5,3,6,11) | (5,3,6,11) — self-consistent but in units of the WRONG span |

The writer computes `spanTiles = round(frame.w / SQUARE)` — the STREAMED atlas frame's px
over world px/tile. At the 128-px lod every frame reads span 1; the conifer's authored span
is 2, so its caster card and subframe are **half size in world units**. Structurally wrong
twice over: (a) span must come from the def/manifest (`span` field), never atlas px; (b)
`frameX/Y` are captured from the frame's atlas position at build time and NOTHING rewrites
them on a lod swap — the old system's def-swap cascade has no successor, so records go
stale the moment the resolver upgrades a stem.

Also pinned: **all 4 rotations of a block share ONE facing's frame + subframe** (the stem
already encodes the facing; the builder writes the same fields to r0..r3), so
`base + rotation` addressing exists in the record shape and is FAKE in the data. And the
truth table's wolf/human/west/linked rows CANNOT be built — movers and linked cells never
get defs (I1).

## I3 — silhouettes: absent on the live fixture; plus a rectangular banding artifact {#i3}

With 455 prims / 2 defs live and the debug light at (100,50): the two conifers inside the
pool cast **no visible shadow at all** (zoomed capture in the session record). The
after-images in the rework's record show wedge/tree shadows under its own drill sequence —
on a cold-started fixture the chain produces none. Not yet root-caused (candidates: the
gather finding no casters for the un-rebuilt window; `surface` quadrant sampling against
the wrong span from I2 — a 2× short card can miss every crossing).

Separate artifact, reproduced across rebuilds and captures: **concentric rectangular
banding** NE of focus (~tiles 105–107 × 52–55), flickering between frames — square hard
edges where only radial falloff should exist. Unexplained; candidates: refine reading
def-index garbage, slot-boundary bleed, or the un-cleared first ping-pong frame.

## I4 — z-ordering: the prim `layer` lane is never written {#i4}

`buildRecords` writes scene prims with `layer` defaulted to 0 (only the synthetic
oversubscription test authors layers). `presence` sorting uses the JS-side `zIndex` for
ORDER but the record lane every shader could read is uniformly 0 — so any per-pixel
receiver resolution that consults `layer` sees a flat world, and the one-z-contract (P4)
has nothing real to verify against yet. Movers being absent (I1) also means the cases
where z-ordering visibly matters (pawn over tile, head over body, tree over pawn) cannot
even arise in the current records.
