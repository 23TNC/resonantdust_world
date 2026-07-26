# TEXTILE_SLOT — issues

_Traps found while designing this, and how they resolved. Chronological append._

### I1 — LOD does not affect sharpness; the `2^k` cancels (2026-07-26)
The intuition that "lod 0 spans a 128px→64px screen range, downsampling" does not hold on a fixed grid, and
the reason is worth keeping because it is counter-intuitive.

At lod k a tile occupies `128/2^k` texels and lands on screen at `σ · 128/2^k` px. Texels per screen pixel:
```
(128/2^k) / (σ · 128/2^k)  =  1/σ
```
The lod term **cancels**. Display sharpness is governed entirely by `σ` — that is, by the viewport against
the 2560×1536 reference — and lod selects *world coverage* only.

Two consequences:
1. **Downsample-only is unaffordable, not merely unchosen.** A 2:1 minifying band per lod needs the visible
   grid to hold the zoomed-out end of the band: 40×24 visible slots, 4× the texels ([F2](forks.md#f2)).
2. **The two sampling stages must be reasoned about separately.** Bake (art → slot) is always exactly 1:1 and
   never up- or downsamples. Display (slot → screen) is `1/σ`. Conflating them is what produced the wrong
   intuition.

### I2 — per-light `last_lod` is insufficient; it must be `coarsest_lod` (2026-07-26)
Storing lod state **per light** rather than per tile is right — a tile can hold contributions deposited at
four different lods at once, and each light independently knows the grid it must be undone on, so a zoom
costs nothing at update time. But *which* lod to store is not obvious.

**A down-then-up round trip is lossy.** Worked through, with L deposited on the lod-32 grid:
- zoom out → decimate to 16: texel `(c,d)` keeps `v(2c, 2d)`; the other three values are destroyed
- zoom in → replicate back to 32: texel `(a,b)` now holds `v(2·(a>>1), 2·(b>>1))`

L's `last_lod` still reads 32 and current lod is 32, so the naive rule evaluates L at `(a,b)` → `v(a,b)`.
The stored value is `v(2·(a>>1), 2·(b>>1))`. They differ, and the difference **leaks light permanently**.

**Resolution — store the COARSEST lod since the light was last cast.** Information is only ever destroyed by
decimation and never returns; later upscales merely replicate. So the stored contribution is always L
evaluated on the coarsest grid it has passed through, and evaluating there reproduces it exactly. The field
is monotone (`coarsest = max(coarsest, new_lod)` on every zoom) and resets to the current lod whenever the
light is genuinely re-cast. Composition holds: 64→32→16→32 stores at 16, so evaluate at 16.

Same 2 bits as `last_lod`. Note the asymmetry with billboards, which legitimately want `last_lod` — they are
re-baked rather than accumulated, so there is no invertibility requirement.

### I3 — without a light refresh queue, zoom-out is permanent (2026-07-26)
Falls out of [I2](#i2): `coarsest_lod` is monotone, so nothing ever restores a light's resolution on its own.
A light that has been zoomed out and back in stays blocky **forever** until something else happens to dirty
it. The stale queue is therefore not an optimisation — lights whose `coarsest_lod ≠ current` must be
re-cast in the background, prioritised below empty tiles.

### I4 — reprojection cannot be done in place (2026-07-26)
The toroidal wrap modulus is the slot count, so when the lod changes every tile's slot address **permutes**
— it is a scale *and* a shuffle. Source and destination overlap arbitrarily, so the reproject must go through
a scratch target and then swap. One instanced per-slot blit.

### I5 — `ensurePartition` clears every channel, and says so (2026-07-26)
The flashing on zoom has a precise address:
```
// Re-partition: clear buffers + re-bake the whole window (no reproject — simplified W4c).
for (const ch of this.channels) ch.buf!.clear(0, 0, 0, 1);
```
A documented shortcut, not an oversight — [webgl-engine](../webgl-engine/README.md) W4c shipped a single
buffer per channel as the floor and W4h carries the restore as an open item. Note `SIZE_STEP = 4` quantises
the window, so the flash is stepped rather than continuous.

### I7 — the composite is `cols + 2` slots wide: the WRAP-APRON is not slack (2026-07-26)
Sizing the channel buffers at `SLOTS · SQUARE` truncated every tall sprite: conifers rendered their top
and then smeared a solid vertical band downward, with stray horizontal streaks. No console errors — writes
simply fell outside the texture and reads came back clamped.

`bakeSquare` writes the interior at `(sx + 1) · slotPx` and mirrors edge slots to the **opposite border**
at `(cols + 1) · slotPx`, so the composite carries a **one-slot apron on every side** and is `cols + 2`
slots wide. That is precisely what the retired `slotPx = fixedCW / (cols + 2)` divisor was expressing —
the `+2` looked like padding slack and was load-bearing.

**Fix:** `(SLOTS_X + 2) × SQUARE` = 3328×2304. Note the apron is `2 · (SQUARE >> lod)`, so it SHRINKS with
lod; the buffer is sized for the largest case (lod 0) and higher lods leave a margin unused rather than
resizing. Cost: 29.3 MiB per RGBA8 channel instead of 24.

**Rule this implies:** when replacing a derived size with a fixed one, re-derive what every term in the old
expression was for. A divisor is not necessarily a safety margin.

### I8 — the ZONE SUBSCRIPTION reach was a screen estimate, and `SQUARE` 64→128 halved it (2026-07-26)
Reported as "zooming doesn't work correctly": at lod ≥ 1 the window edges rendered black.

**Not a map-read bug.** The window edges baked to *nothing* because there was no tile data there:
`WorldBridge.radii()` sized the zone subscription from a **screen estimate**
```
screenTiles = max(innerWidth, innerHeight) / (SQUARE · zoom)
```
which **halves** when `SQUARE` goes 64 → 128. The renderer's window still spanned zones 4–7 while the
reach only fetched 5–6, so the outer zones were never subscribed and their tiles baked empty.

**Diagnosis worth keeping — the measurement contradicted the obvious suspect.** The empty columns were the
window's two EDGES (world tiles 76–79 and 112–123) with the carried middle intact, which looks exactly like
a reproject failure. It is not: a **clean load** at the same lod, with no reproject in play, showed the same
emptiness. That one control ruled out the entire reproject path.

**Fix:** derive the reach from `SLOTS << lodForZoom(zoom)` — the window the cache is about to adopt.
Deliberately computed from the ZOOM rather than read off `viewport.window`, because `zoomTo` sets the zoom
and calls `radii()` in the same turn while the cache only re-partitions next tick — reading the live window
would size the subscription from the OUTGOING lod and lag a frame behind every zoom.

**Verified:** 0 empty columns at lod 0, 2 and 3 including transitions (was 4 at lod 1 on a clean load, 16
after a lod 0→1 step). Note lod 3 needs ~225 zones, so it streams in over a second or two — the world is
briefly a small square before filling.

**Rule:** anything sized to "how much world will we draw" must derive from the tile window, not from screen
px. `SQUARE` is now a rendering constant that no longer implies a screen extent.

### I6 — verify 128px masters exist before relying on lod 0 — RESOLVED 2026-07-26, none missing
`SQUARE` 64 → 128 means lod 0 wants natively 128px art, and any kind lacking it would upscale at maximum
zoom. Checked against the live texture manifest rather than assumed: **16 stems, `maxSize` histogram
`{128: 8, 256: 4, 320: 1, 512: 3}`, zero under 128.** Every stem can serve lod 0 natively, and the server
derives the smaller LODs on demand, so the whole ladder is covered.
