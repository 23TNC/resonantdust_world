# Completed — verification log

_Dated entries: what landed and **how it was checked**. Append-only; authoritative for what's done and why we
believe it. Items live in [`todo.md`](todo.md) with their boxes ticked — this file records the evidence._

_Standing rule for this stream ([I3](issues.md#i3)): an entry here names the **output** that was read — a
`debugReadShadow(cls)` hash / byte population, or a frame time — not a JS field, an input count, or a
screenshot impression. Three verifications this session were false because they stopped at the near end of the
pipeline._

## 2026-07-26 — I3 root-caused and fixed: a carrier prim's position is now written every frame

**What landed.** `carriedLightFor` wrote `PRIM_BASE + prim`'s position *only* inside its
`prim === undefined` allocation branch. Moved that write out into an `else` that refreshes the **position lane
only** (G and both id lanes carried through from the mirror, so the carried-piece slots
`writeCarriedLight` is about to read are untouched). Also threaded `from` — the position a light is leaving —
out of `writeCarriedLight` and into the caller's `markLightDirty`, closing [I1](issues.md#i1) in the same
change, since the caller cannot recover the old position once the map entry is overwritten.

**How it was checked.** In-browser at `?focus=104,55&zoom=1`, reading the OUTPUT:

- **Before.** All three content torches resolve `def = −1` — they carry no sprite, so `definitionFor` returns
  −1 and `buildCasters` `continue`s before `billboardDataFor`, which is the other (working) writer of a
  carrier position. Orbiting them gave `primMoved: true, recordMoved: false`: the prims moved, the light
  records stayed pinned at their allocation position.
- **After.** `lightRecordsMoved: true`, and `debugReadShadow(0)` hashes across three samples were
  **1561431880 → 3834622774 → 4097208919** (non-zero populations 101 297 / 101 481 / 101 161). The map now
  changes every frame, on a large non-zero population.

That hash triple is the acceptance this stream was built around: the same readback returned a **single
constant** hash with the orbit on and off before the fix.

**Corrects [primitive-graph I40](../2026-07-25-primitive-graph/issues.md).** Its diagnosis — `stepOrbit`
writing a discarded per-frame read model — was wrong. `SquareCache.standingPrims()` pushes **references** to
the stored prims, so those writes always landed. The observation was real; the mechanism was invented, and I
had written it into three documents before checking the four-line method that disproves it.

## 2026-07-26 — I2 confirmed: the zoom cliff is a dirty-FRACTION effect

Measured with 3 orbiting torches (reach 16). Full table + the corrected cost model in [I2](issues.md#i2):

| zoom | lod | map (tiles) | dirty/frame | dirty fraction | ms static | ms moving | fps moving |
|---|---|---|---|---|---|---|---|
| 1.0 | 0 | 512 | 512 | **100.0 %** | 8.33 | **43.06** | 23 |
| 0.5 | 1 | 2 048 | 1 363 | 66.6 % | 8.33 | 28.34 | 35 |
| 0.25 | 2 | 8 192 | 1 811 | 22.1 % | 8.33 | 8.33 | **120** |

**The model that fits:** `work ∝ fraction × (slots × slot_texels)` — a **constant** texel budget of which the
dirty fraction is re-baked. Taking zoom 1 as reference, 43.06 × 0.666 = 28.7 predicted vs **28.34 measured
(1.3 %)**. My pre-measurement model (constant per-tile cost, varying tile count) was wrong in both halves,
which cancelled and left the conclusion standing anyway — zooming out bakes 3.5× MORE tiles in 5.2× LESS time,
because per-tile texels fall 16× from lod 0 to lod 2.

**Closes the latched-counter caveat below:** `debugDirtyTiles` reads **0** in every static row, so it is a
genuine per-frame count. The "constant 696" was a stale reading.

## Baseline carried in from the prior session (2026-07-26, pre-P0)

Recorded here so P0's instrumented numbers have something to sit next to. Measured at **zoom 0.25, reach 16
tiles**, 123 lights:

| condition | ms/frame | fps | dirty tiles |
|---|---|---|---|
| static | 0.72 | 120 | 696 |
| 120 of 123 moving | 72.44 | 15.2 | 5 384 |

Caveats that P0 must resolve rather than inherit:
- The **dirty counter reads a constant 696 when static and appears latched** — it may not be a per-frame
  figure at all. Confirm what it reports before any conclusion rests on it.
- Not comparable to the earlier 6.03 ms / 128-light figure: that was **reach 6**, ~7× less area per light.
- The "moving" row predates [I3](issues.md#i3) — those lights moved via the debug array that
  [primitive-graph](../2026-07-25-primitive-graph/README.md) has since deleted, so the number stands as an
  order-of-magnitude signal, not a measurement to diff against.
