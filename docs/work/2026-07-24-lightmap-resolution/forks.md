# Forks — full-resolution lightmap

_Decision points + options + which we chose + why. Chronological._

---

## F1 · Cache the lighting vs forward per-pixel every frame {#f1}
**2026-07-24 — DECIDED: cache (bake).** Forward per-light in the blit is correct + cheap for a handful of
lights, but runs every frame × every pixel × light-count. The user's target is **many** lights, so the
**dirty-gated bake** wins: the per-light accumulation runs once per changed tile (static lights never
re-bake), the blit is a single read. Caching is what makes "a lot of lights" affordable.

## F2 · Bake `N·L` per light, or store aggregate + apply normal in the blit {#f2}
**2026-07-24 — DECIDED: per-light `N·L` in the bake.** The aggregate (one summed irradiance + one averaged
direction) is lossy — `Σ colorᵢ·(N·Lᵢ) ≠ (Σ colorᵢ)·(N·L_avg)` ([issues.md#i1](issues.md#i1)), independent
of resolution. Doing the `N·L` per light inside the bake (against the fine normal) stores the correct
accumulated result. Needs the fine normal in the bake (→ [F3](#f3)), and drops the direction/unshadowed
attachments (→ [F5](#f5)).

## F3 · How the bake gets the fine normal — RESOLVED (sample the prim's atlas frame) {#f3}
**2026-07-24 — RESOLVED (user): sample the prim's NORMAL from its ATLAS FRAME in the bake, exactly like the
silhouette.** No world normal map at all. The bake is doing **per-prim** lighting, so at a prim texel it
already finds the prim + `(s,t)` (via `receiverAt`, the same code that samples the surface silhouette in
`casterCover`/`receiverCover`). Reading the **normal** is the identical move on a normal atlas:
`frame_origin + (s,t)·frame_size`, `texelFetch`. That is an **atlas lookup indexed by frame — NOT a
world-coord read of a `textile_slot` composite** — so it is in-family and zoom-safe by construction. The
whole "bake a contiguous world normal map" problem ([old options a–c]) evaporates.
- **Pitch is BAKED at ingest (user, 2026-07-24), NOT in-shader.** The atlas normal is written **already in
  the world frame** — `bin/art` applies the ground-vs-thing rotation at ingest, driven by a **DSL orientation**
  per def. So the bake just `texelFetch`es a world-frame normal and dots it: **no runtime pitch, no runtime
  orientation flag** (both move to ingest). Cost — accepted: the world angle is **bake-committed**; see
  [#f7](#f7).
- **Plumbing:** the bake reuses `receiverAt` for the prim + `(s,t)`, reads cached `shadow-cold` per light,
  never re-walks a corridor.
- **Ground = prims, not a special case (user, 2026-07-24).** Rather than branch "ground texel → flat-up",
  make ground tiles **real prims** carrying a **generic white texture**: white albedo, everything in layer 1,
  an **up-facing** normal (perpendicular to the world ground, not the screen), full-coverage surface. Then
  they bake through the identical atlas path — no geo special-case, forward-compatible (swap the generic frame
  for authored ground textures later, zero pipeline change). Prereq: **author that generic white texture** in
  `bin/art` (today there's only a 1×1 white *fill* for the geo path, not an atlas entry with frames).
- **The one non-uniform bit — a per-def ORIENTATION flag.** "Flat `(0,0,1)`" means different world normals by
  prim: a ground tile's card lies *in* the ground plane (flat → world-up `ẑ`); a thing's card is
  screen-parallel (flat → horizontal, perpendicular-to-ground). Same atlas value, two rotations — so the
  in-shader pitch reads a **1-bit per-def orientation** (`lies-in-ground` vs `stands-perpendicular`) to pick
  which. The atlas/sampling path stays uniform; only the pitch branches. This is the ground-vs-thing rotation
  `normal-tilt` already owns; the flag lives there. The generic-white ground tile carries `orientation =
  ground`.
Rejected earlier options (contiguous normal target / sample composite by world coord / merge with SquareCache
bake) are moot — the frame-indexed atlas read is strictly better (no extra RT, no double-bake, no cross-family
risk).

### F3-A · The frame-origin MECHANISM — parallel normal band (chosen), F6 co-pack consolidates {#f3a}
**2026-07-24 — RESOLVED (build): a per-def NORMAL-frame band in the data texture; F6 co-pack subsumes it later.**
F3 said "sample the atlas frame" but not *how the shader gets the normal frame's origin*. Tracing the code
surfaced the gap: the surface + normal are **separate client atlas pages** (independent `LodPool`s → different
frame origins) with **independent load timing**, and the def texel is **packed full (4×u32, 4 spare bits) and
immutable** — so it can neither carry the normal origin nor be backfilled when the normal resolves a tick after
the surface. Three ways to bridge it:
- **A — parallel normal-frame band (CHOSEN).** Add `NORMAL_DEF_BASE` (set 6, was reserved) = 1 texel/def holding
  the normal frame origin (`G = present<<20 | nfx16<<10 | nfy16`), refreshed on the per-tick caster walk
  (`refreshNormalFrame`). Being a *separate* band from the immutable def band, it rewrites freely — a late normal
  load just backfills it next tick. `primNormal()` reads it + the shared `uNormal` page with the **same frameRel**
  as the silhouette (only the origin swapped). Client-only, rides the existing scatter/upload path, zoom-safe
  (frame-indexed — verified live: normals stay locked to prims across a full zoom sweep, both directions).
- **B — F6 co-pack.** Normal = surface frame + a fixed quadrant offset → no band, no timing issue, but a big
  `bin/art` + resolver + quadtree change. Deferred: it's the eventual consolidation ([#f6](#f6)) that retires A's
  band, sequenced *after* the lighting correctness lands, not before.
- **C — widen the def to 8 words.** Rejected: doubles the def texture AND still has the immutable-backfill problem
  (an 8-word def is still minted once).
Guard: A accepts the normal only when it sits on the shared page AND matches the def's lod (same frame side) —
else `present = 0` and the bake uses a flat up-normal for that prim (same fallback discipline as the surface's
single-page/off-page path). Same multi-page limit as the surface today (C5 lifts both).

## F7 · Bake the normal pitch at ingest → the world angle is bake-committed {#f7}
**2026-07-24 — DECIDED (user): bake the pitch, accept a fixed world angle.** The normal pitch (card→world)
is applied once at `bin/art` ingest, keyed by a DSL orientation (`lies-in-ground` → flat = up `ẑ`;
`stands-perpendicular` → flat = horizontal). Runtime samples the world-frame normal directly — cheaper (no
per-texel rotation), simpler (no runtime orientation flag).

**The cost, stated plainly — it locks the ENTIRE tilt, not just normals.** The light **direction**
(world-space-lighting) is computed in the world frame from the **data-map tilt**; the **normal** is baked in
the world frame at the **ingest** angle. `N·L` is only correct if both share one frame ⟹ the data-map tilt
**must equal** the ingest angle. So `__tilt(deg)` live would desync direction from normal; changing the world
angle means **re-ingesting the corpus**. Fine because: the camera is fixed-angle (pan/zoom, no rotate), 55°
is settled art direction, and the live dial already did its job (finding the angle). After the bake, the
data-map tilt is "the angle the atlas was baked at", not a free knob. (Reverses the earlier "pitch in-shader
so normals follow `__tilt`" lean — the only normal consumer is `N·L`, which wants world-frame, so baking wins.)

## F6 · Co-pack albedo/normal/surface/layers into one atlas (paired optimization) {#f6}
**2026-07-24 — user proposal; do, but sequenceable.** Reserve a slot **one power of 2 larger** than the
sprite (`2N×2N` around `N×N`) and lay the four maps in its quadrants; grab any channel by adding a fixed
`(N,0)/(0,N)/(N,N)` offset to the frame origin — **one atlas binding, one frame lookup + a quadrant shift**,
and albedo/normal/surface for the same texel become **cache-adjacent** (the per-pixel per-light loop reads
all of them). Cost: `bin/art` co-packs at ingest, the resolver learns the quadrant offsets, the quadtree
packer reserves 4× area/sprite. No net VRAM (same data, co-located). NOT required for [F3](#f3) (a standalone
normal atlas also works) — it's an independent atlas win that pairs naturally.

## F4 · Target resolution + family {#f4}
**2026-07-24 — `TEXTILE_SQUARE` = 64/tile, CONTIGUOUS.** Matches the composite's max detail; stays a
contiguous `cols·R × rows·R` toroidal map so the blit + bake address it by world coord safely — NOT the
variable-`slotPx` `textile_slot` atlas ([issues.md#i3](issues.md#i3)). The coarse `shadow-cold` (16/tile) is
**upsampled** per light as the fine bake runs (nearest or a cheap bilinear of the u9 coverage); the shadow
gather itself is untouched.

## F5 · Attachments, format, VRAM {#f5}
**2026-07-24 — fewer + possibly HDR.** Baking `N·L` in means the lightmap is a **single irradiance RGB per
class** (cold/hot) — the old att1 (direction) + att2 (unshadowed) are gone. So ~**1 attachment where we had
3**, which offsets much of the 16× texel growth (64 vs 16/tile). Open: **HDR** — many lights summing can
exceed 1, so `rgba16f` (2×) may be needed vs `rgba8`; decide by whether clamped LDR bands. Write the actual
MB for the 2× max-zoom window in P3, so the "spend the VRAM" call is on real numbers.
</content>
