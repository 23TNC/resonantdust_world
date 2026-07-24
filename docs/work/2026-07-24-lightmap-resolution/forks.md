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
- **Pitch in-shader:** the atlas normal is Laigter-raw (card frame); the bake rotates it to the world frame
  from the data-map tilt (the "tilt in-shader from raw normals" decision) before `N·L`. Macro-vertical +
  fine leaves compose per prim at bake time.
- **Plumbing:** the bake reuses `receiverAt` for the prim + `(s,t)`, reads cached `shadow-cold` per light,
  never re-walks a corridor.
- **Ground:** ground texels have no caster-bucket prim → use flat-up (`ẑ`), no atlas fetch. A detailed
  ground-normal path is a separate later concern; flat-up is correct for the macro and free.
Rejected earlier options (contiguous normal target / sample composite by world coord / merge with SquareCache
bake) are moot — the frame-indexed atlas read is strictly better (no extra RT, no double-bake, no cross-family
risk).

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
