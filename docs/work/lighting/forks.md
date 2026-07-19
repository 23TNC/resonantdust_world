# Forks — lighting

_Port-time decisions. The **load-bearing constraints** from `tiered_lighting.md` (channel=8b/texel=32,
no-bitfield-blend, identity-survives, gather-has-no-merge, cold-can't-subtract, float-mod ES 1.00) are
carried over as-is — see the [`README`](README.md#the-load-bearing-constraints); they're not re-decided._

---

## F1 · Shadow shape = **earcut silhouette**, not a raster coverage quad (decided, built)

Settled in [art-metadata F1](../art-metadata/forks.md): the scatter projects each caster's **vector
earcut triangulation** (the `outline` sidecar) — filled tris, no per-fragment coverage fetch, holes free
— over a coverage-mask quad. Cheaper on the fragment-bound scatter path, and the reason we ported
`resonantdust-geometry`. The outlines are generated; **P3 consumes them**.

## F2 · Per-light occlusion **inside** the accumulation, never a global mask (decided)

The current renderer's single shadow multiplies the *summed* direct light — a global lit/unlit mask that
breaks with 2+ shadow-casters (B un-shadows A). The port makes occlusion a **per-light gate** in the sum
(`Σ Lᵢ·falloffᵢ·N·L·gate(i)`), so lights can't cross-contaminate. This is the whole reason for the port;
the tiered structure (cold bake + dynamic bitfield) is how we make per-light affordable at scale.

## F3 · The `cold_lightmap` rides the existing `SquareCache` rect tiers (port decision)

**Context.** The cold bake needs a per-rect, world-space, dirty-rebaked RT. The new game already has
exactly that machinery — the `SquareCache` cold tier (toroidal, dirty-driven, world-space, panning-free).

**Decision.** Add `cold_lightmap` (+ the cold `data`/`color`/`active` inputs) as **another channel/target
on the existing cold `SquareCache`**, baked in its `bakeDirty` path with a `lightDirty` trigger — rather
than standing up a parallel rect system. Reuses the proven toroidal/apron/amortization; the light bake is
one more `ChannelSpec`-like pass over the same rects. (Warm-tier cards re-evaluate cold lights on their
own normal via the cold light *textures* — same split as the old game, since the bake uses the ground
normal.)

## F4 · Round-robin freshness is one **dynamic pool**, not a warm/hot split (carried)

Per `tiered_lighting.md`: 32 global dynamic lights, 8 get a fresh scattered shadow each frame, the rest
carry a ≤3-frame-stale bit. "warm"/"hot" are freshness states, not separate tiers or buffers — so the
port builds ONE dynamic pool + ONE `warm_shadowmap` with a round-robin, not two light sets.

## F5 · _(open — decide at P0)_ how `meta.json` reaches the client

`atlas.json` already folds into the content manifest server-side. Presumably `meta.json` folds the same
way (the edge/content path). Confirm the fold picks up the new sidecar + that `outline`/`channel_tints`
survive to the client; if not, that's the P0 serve work. Tracked as [B1](blockers.md).
