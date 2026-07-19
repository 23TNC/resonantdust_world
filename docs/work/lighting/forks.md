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

## F4 · Warm = one round-robin pool; **rt = a separate always-fresh pool** (revised 2026-07-19)

The old game's warm tier is ONE 32-light pool, round-robin: a few get a fresh scattered shadow each frame,
the rest carry a stale `shadow-warm` bit. **Revised:** the converged design ADDS a small **`rt` pool** (4
priority lights) on its own always-fresh `shadow-rt` map — never round-robin, never bumped to the bitfield —
so the cursor has zero staleness. So: 32 warm (round-robin bitfield) + 4 rt (every-frame map). See
[`intent/tiered-lighting.md`](../../components/client/pixijs/intent/tiered-lighting.md).

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

## F7 · Cold shadows = a 32-bit bitfield (lifts the cap 3→32), built by the shared scatter engine (decided)

**Decision.** `shadow-cold` is a **32-bit-per-pixel occlusion bitfield** (one bit per cold light, 32
lights in an RGBA8 texel), built **on dirty** by the same `projectCaster`→scatter→writeback engine the
warm tier uses. The cold bake reads a light's bit (`bf_bit`) and drops its term where occluded. This lifts
the cold shadow-caster cap from **3 (the old RGB `uColdShadow`) to 32** — every cold light in a rect casts
a shadow, which is the whole point of the "dense many-lights" cold tier.

**Considered + rejected: an inline per-fragment sweep** (loop the light texture per pixel, point-in-polygon
per light, no map). It would be *green-field GPU R&D* — the old game does **not** do this (its cold path is
exactly the materialized coverage we're bitfield-ing) — with real per-fragment cost + a hard
geometry-in-shader problem. The bitfield is **proven** (old game `bitfield.ts` + `rectLightBakeShader`),
cheap (bake-time only, one texel/pixel), and gives 32 casters. Not worth reinventing.

**Supersedes** the RGB=3 `shadow-cold` composite (the interim, [D-1](deviations.md)) and the old
architecture table's "CPU 32-bit active map." Full design:
[`intent/tiered-lighting.md`](../../components/client/pixijs/intent/tiered-lighting.md).
