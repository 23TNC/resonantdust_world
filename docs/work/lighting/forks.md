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

**Decision.** Add `cold_lightmap` (+ its light-data-texture input; occlusion is inline, [F7](#f7), not a
separate `active` map) as **another target on the existing cold `SquareCache`**, baked in its `bakeDirty`
path with a `lightDirty` trigger — rather than standing up a parallel rect system. Reuses the proven
toroidal/apron/amortization; the light bake is one more `ChannelSpec`-like pass over the same rects.
(Warm-tier cards re-evaluate cold lights on their own normal via the cold light *textures* — same split
as the old game, since the bake uses the ground normal.)

## F4 · Round-robin freshness is one **dynamic pool**, not a warm/hot split (carried)

Per `tiered_lighting.md`: 32 global dynamic lights, 8 get a fresh scattered shadow each frame, the rest
carry a ≤3-frame-stale bit. "warm"/"hot" are freshness states, not separate tiers or buffers — so the
port builds ONE dynamic pool + ONE `warm_shadowmap` with a round-robin, not two light sets.

## F5 · _(open — decide at P0)_ how `meta.json` reaches the client

`atlas.json` already folds into the content manifest server-side. Presumably `meta.json` folds the same
way (the edge/content path). Confirm the fold picks up the new sidecar + that `outline`/`channel_tints`
survive to the client; if not, that's the P0 serve work. Tracked as [B1](blockers.md).

## F6 · `cold_lightmap` = a derived `SquareCache` composite baked from the normal (P1 integration)

Refines [F3](#f3). The cold lightmap is **not** a prim-baked channel (albedo/normal/…) — it's a
post-process over the already-baked **normal** composite: `ambient + Σ cold·N·L·falloff²`. So it's added
as a **derived composite** in the `SquareCache` — baked per square, LAST in the per-square pass (so the
normal slot is ready), from a screen-quad reading the normal slot + the cold-light uniforms, into its
own ping-pong composite. It reuses the cache's toroidal layout + apron + dirty machinery; a `lightDirty`
trigger (a cold light in range changed) rebakes a rect even when its geometry didn't. Exposed via
`displayComposite("cold-lightmap")`; the display shader samples it into the albedo multiply. Least new
machinery; amortization + wrap-apron for free. Port `rectLightBakeShader.ts`'s math.

## F7 · Cold shadows are tested **inline in the bake**, NOT rasterized to a map (decided — corrects a miss)

**This fork should have existed from the start; it didn't, and its absence produced the wrong build.**
The architecture table + F1 above frame *all* shadows as "rasterize each light's silhouette into a channel
of an RGBA map." That is right for the **dynamic** tier (per-frame, whole viewport — a per-pixel light
sweep is too expensive there). It is **wrong for cold**, and treating it as a given (never a fork) is how
the first cold-shadow build ([D-1](deviations.md)) inherited a materialized `shadow-cold` map with a
**3-lights-per-square cap** — the exact limit the cold tier exists to avoid.

**Decision.** The cold bake computes occlusion **inline**, per pixel, per light, inside the accumulation
(the [Cold strategy](README.md#cold-lighting-strategy--the-inline-sweep) sweep): loop the light-data
texture, cull by radius, test the light's projected silhouettes against this pixel, add the light if lit.
**No shadow map is materialized.**

**Why this is the right call for cold specifically:**
- **Unbounded lights.** A gather (sum-in-one-invocation) has no merge wall (README constraint 4). Looping
  a light-data *texture* means the count is limited by texture size + ALU, not by 3–4 RGBA channels.
- **Cold can afford it.** The bake is amortized — only dirty rects, budgeted per frame — so the per-pixel
  ×per-light ×per-caster cost the dynamic path can't pay, cold can.
- **"Dense many-lights" is the cold tier's entire reason to exist.** A 3-light cap defeats the point.

**Cost / open piece.** Getting the geometry into the fragment shader for the inline test (a data texture
of projected tris/contours) + simplifying the shadow silhouette hard enough for a per-fragment
point-in-polygon. Pin down against `../resonantdust`'s working version before building — [B3](blockers.md#b3).

**Supersedes** the cold-column of the architecture table's original "CPU 32-bit active map" and the
`shadow-cold` materialized composite built under D-1 (to be removed in the P4 rework).
