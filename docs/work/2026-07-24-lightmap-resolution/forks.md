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

## F3 · How the bake gets the fine normal (THE crux) {#f3}
**2026-07-24 — OPEN, P0's job.** `LIGHT_FRAG` (contiguous world-space toroidal) must sample the fine normal
per bake texel. The normal today lives in the `SquareCache` composite — a `textile_slot` atlas that is NOT
safe to address by recomputed world coordinate (map-compatibility; the zoom-drift that reverted
shadows-on-prims twice). Options:
- **(a) Bake a contiguous normal map** aligned with the lightmap (a world-space toroidal `textile_square`
  normal target, off the existing normal bake). The lightmap bake samples it safely. Lean — it's the
  in-family, zoom-safe route; costs one more RT + a bake write.
- **(b) Sample the composite from the bake by world coord.** Rejected — the exact fragile cross-family
  lookup the map-compat lesson forbids.
- **(c) Integrate the light bake with the SquareCache per-square bake** so it shares the composite's UV.
  Cleaner in principle, larger restructure; revisit if (a) proves wasteful.
The normal must arrive in the **pitched world frame** `normal-tilt` produces (or be pitched in-bake).

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
