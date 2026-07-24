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

## I-4 · The fine-normal-in-the-bake is the risk, and it's a cross-family read {#i4}
**2026-07-24.** [F3](forks.md#f3) is the one genuinely hard part: the contiguous bake needs the fine normal,
which currently lives in the `textile_slot` composite. Sampling that atlas from the bake by world coord is
exactly the forbidden cross-family lookup (I-3). The safe route is a **contiguous** fine normal map for the
bake to read — but that means the normal is produced twice (composite for display, contiguous for lighting)
unless the bakes are unified. P0 must resolve this before any lighting code, or it silently re-introduces
the zoom bug. (Same discipline as world-geometry / map-compat: don't build on an unverified cross-map read.)
</content>
