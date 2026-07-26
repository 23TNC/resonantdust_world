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
the 3584×1536 reference — and lod selects *world coverage* only.

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

**Fix:** `(SLOTS_X + 2) × SQUARE` — **4352×2304** on the final 32×16 grid. Note the apron is
`2 · (SQUARE >> lod)`, so it SHRINKS with lod; the buffer is sized for the largest case (lod 0) and higher
lods leave a margin unused rather than resizing. Cost: **38.3 MiB** per RGBA8 channel instead of 32.
Whether the apron is still needed at all is [I9](#i9) — this issue only establishes that the WRITES need
the space, which is a weaker claim.

**Rule this implies:** when replacing a derived size with a fixed one, re-derive what every term in the old
expression was for. A divisor is not necessarily a safety margin.

### I11 — the reach fix is correct but INERT: the client honours only the FIRST anchor push (2026-07-26)
[I8](#i8) fixed `radii()` to size the subscription from the tile window, and that computation is right. It
does not currently help, because of a **pre-existing** client-side limitation.

**Measured at lod 2** (window 128×64 tiles = 8×4 zones): exactly **48 of 128 columns** carry zone data, and
it is stable, not still streaming. 48 tiles = **3 zones**. Content is bounded to a **3×3 = 9-zone** square
regardless of what reach is requested.

**Why 9.** At load the camera is at zoom 1 → lod 0 → `windowTiles = 32` → `active = 32/2 + 2 = 18` tiles
≈ 1.1 zones radius ⇒ 3×3 zones. Every later push (zooming out raises `active` to 66) is sent — `setAnchor`
re-pushes whenever the radii change — but the subscription never grows.

**This is already a known open item**, independently observed before this stream:
[webgl-engine](../webgl-engine/README.md) W4g — *"with the grid on, a GOOD load shows content bounded to ~9
zones and panning doesn't seem to acquire more — confirm world-bounds vs under-subscription (**only the
initial anchor's reach is handled**)"*. This stream did not cause it; it made it **visible**, because the
window at lod 2 is 8×4 zones and finally exceeds the 9-zone ceiling that lod 0 never did.

**Consequence for [B-1](blockers.md).** Raising `ZOOM_MIN` to 0.25 does not by itself give a full world at
maximum zoom-out — the ceiling is the anchor handling, not the zone count or bandwidth. The loading-priority
system ([I10](#i10)) is also downstream of this: there is no point prioritising a fetch order for zones the
client will not request.

**Next step** (not attempted here — it is Rust in `client/core`, outside this stream's component): confirm
whether `Client::set_anchor` ignores a reach change for an existing anchor id, and make a reach increase
re-evaluate the zone set. Cheap to verify by pushing a second anchor with a larger radius and counting
subscriptions.

### I10 — a LOADING-PRIORITY system is the real unlock for deep zoom-out (open, user 2026-07-26)
`ZOOM_MIN` moved to 0.25 to sidestep lod 3's ~450-zone window ([B-1](blockers.md)), but the user named the
actual fix: **"in all cases we will likely want a priority system so the system can prioritize what it
updates."**

Note the client already has HALF of this. Bake priority is solved — `prio()` is `band + ring`, so empty
tiles drain before stale ones and the screen centre fills first ([P5](todo.md)). What has no priority at all
is **zone FETCHING**: the subscription asks for the whole window at once and takes whatever order it gets,
so at deep zoom-out the centre of the screen is no more urgent than the far corner.

**Shape it probably wants**, mirroring what already works for bakes: order zone requests by ring distance
from the anchor, cap in-flight requests, and let the outer ring arrive late. That alone would make lod 3
pleasant without any wire-format change, because the perceived problem is ORDER, not total bytes — the world
currently grows outward from a small square only because arrival order is arbitrary.

A reduced-detail payload for distant zones (B-1's option 3) is a separate, larger lever and should not be
conflated with this one.

### I9 — is the wrap-apron still needed? UNRESOLVED — kept, with the test that would settle it (2026-07-26)
Kept the apron (texture = `SLOTS + 2` slots per axis, ~6 MiB per channel, ~50 MiB total). Recording the
uncertainty honestly rather than claiming it resolved, because the reasoning went both ways during design.

**What IS verified:** every channel texture is **NEAREST** — `texture.ts:68` only selects `LINEAR` when
`nearest === false` is passed, and `RenderTarget` never passes it. `fillDisplay` emits one quad per tile
with UVs spanning exactly that slot's interior. So the display path cannot bleed across a slot edge.

**What is NOT verified:** that *no* consumer samples across a slot boundary. The apron's purpose is the
toroidal straddle — `bakeSquare` mirrors an edge slot to the opposite border so a square adjacent across the
wrap has physically adjacent texels.

**A wrong argument to not repeat:** "removing the apron space truncated every tall sprite, therefore the
apron is needed" ([I7](#i7)). That only proves the apron *writes* need the space — not that the writes are
needed. Different claim.

**Kept because** the pow2 win F5 wanted is already secured by `SLOTS = 32×16` (the wrap is a bitmask
whatever the texture dims), a non-pow2 texture has no WebGL2 penalty at NEAREST/CLAMP, and ~50 MiB is cheap
against intermittent edge seams that are hard to spot and easy to ship.

**Test that settles it:** drop the `(sx + 1)` offset and the `ax`/`ay` mirror writes, size the buffer at
`SLOTS · SQUARE` (4096×2048), then pan the window across a wrap boundary at each lod and diff the composite
against the apron build. Identical ⇒ the apron is dead weight and also saves 3 of every 4 blits per baked
square per channel. Cheap to run, independent of everything else in this stream.

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
