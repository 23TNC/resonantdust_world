# Issues — full-resolution lightmap

_Bugs, gotchas, and the findings that shaped the design. Chronological._

---

## I-1 · The averaging loss — why "finer aggregate lightmap" isn't enough {#i1}
**2026-07-24 (design finding, user + assistant).** Today's lightmap pre-sums lights into one irradiance
(att0) + one irradiance-weighted mean **direction** (att1), then applies the normal once in the blit
(`relief = 1 + gain·dot(normal, dir_avg)`). That collapse is lossy: a texel lit by 3 lights from 3
directions keeps only their **average** arrow, so a normal facing light A but away from B is lit by the A/B
mean, not by A alone. `Σ colorᵢ·(N·Lᵢ) ≠ (Σ colorᵢ)·(N·L_avg)` — and **no resolution bump recovers it**;
you must keep the lights separate through the `N·L`. This is why the stream's thesis moved from "finer
aggregate lightmap" to "**bake per-light `N·L` at normal res**" ([forks.md#f2](forks.md#f2)).

## I-2 · Verify the spend on real numbers, not vibes {#i2}
**2026-07-24.** The plan trades VRAM + bake compute for correctness + detail + many lights. Two things must
be measured, not assumed: (1) the **VRAM** at the current 2×-max-zoom window (fine lightmap RTs, cold+hot,
after dropping to 1 attachment, ×2 if HDR) — write the MB; (2) the **bake cost** scaling with light count,
and that **dirty-gating** genuinely means a static many-light scene re-bakes ~nothing per frame (only a
moving light's reach). The A/B (fine per-light vs coarse aggregate on a many-light forest) is the
deliverable that justifies the trade.

## I-3 · Stay CONTIGUOUS — do not make the lightmap a slot atlas {#i3}
**2026-07-24.** The lightmap is addressed by world coordinate (bake + blit). Per the
[map-compatibility](../2026-07-24-map-compatibility/README.md) rule, that is only safe on a **contiguous**
`cols·R × rows·R` toroidal map. "Match the normal res" means bump `R` 16→64 (`TEXTILE_SQUARE`), NOT adopt
the composite's variable `slotPx` `textile_slot` atlas — which would reintroduce the zoom-drift that
reverted shadows-on-prims twice.

## I-4 · The fine-normal-in-the-bake — was feared cross-family, RESOLVED as a frame-indexed atlas read {#i4}
**2026-07-24 — RESOLVED (user), what looked like the crux isn't.** The worry was that the contiguous bake
would have to read the fine normal from the `textile_slot` composite by world coord — the forbidden
cross-family lookup (I-3). The escape: the bake does **per-prim** lighting, so it reads the normal from the
prim's **atlas frame** (`frame_origin + (s,t)·frame_size`), exactly as `casterCover` reads the silhouette.
That's **indexed by frame, not by world coordinate**, so it's in-family and zoom-safe — no world normal map,
no double-bake, no cross-family risk ([F3](forks.md#f3)). The map-compat discipline still holds (never read a
`textile_slot` map by world coord); we simply don't need to — the frame index sidesteps it entirely.
</content>
