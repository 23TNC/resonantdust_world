# Tiered lighting — the cold inline sweep (durable intent)

**Status (2026-07-19).** The target lighting architecture, kept here so it survives the
[`docs/work/lighting`](../../../../work/lighting/README.md) work docs being archived. The current
renderer is the 0.1 forward pass (`design/lighting.md`, "Where we are"); this is what it must grow into.
The **cold strategy below is the load-bearing, easily-lost insight** — it has already been built the
wrong way once (a materialized, capped shadow map). Do not lose it again.

## The frame — dense many-lights, split by update rate

This is a **dense many-lights** world (point lights authored on DSL prims; darkness + light pools is the
aesthetic — ambient/sun are a faint floor, never the model). The dominating cost is rebuilding **shadows**,
which is *per-light*, so lights are split by how often they change:

- **Cold** — static lights (the authored bulk, hundreds). Baked once into `lightmap-cold`, rebaked only
  when a dirty rect's geometry or an in-range cold light changes. Amortized. **← this doc's subject.**
- **Dynamic** (warm/hot) — moving lights (torches on pawns, the cursor). A live pool re-evaluated every
  frame at display, with round-robin scatter-map shadows. See the work docs; not repeated here.

Both must give **per-light occlusion** (each light lit *with its own shadow*, then summed) so a second
light can't "un-shadow" a spot the first shadowed — the thing the current single global shadow can't do.

## Cold strategy — an inline per-pixel sweep, NO shadow map

The cold lightmap is baked by a **fragment shader over each dirty square**. Per pixel:

1. Read this square's baked **normal** → `N`; seed the sum with **ambient**.
2. **Loop the cold lights** from a **light-data texture** (rgba8, N texels/light: world `xy`, height `z`,
   colour, intensity, radius). It is a *texture*, not a uniform array — that is what makes the light count
   **unbounded**.
3. Per light: **cull by radius** (cheap distance test; most fail). For a survivor compute `falloff` +
   half-Lambert `N·L`.
4. **Test occlusion inline** — right here, decide whether this pixel can see this light: do any of the
   light's nearby casters' projected billboard silhouettes (the earcut `outline` sidecars, sheared through
   the light) cover this pixel? If shadowed, the light adds nothing; else add `colour·intensity·N·L·falloff`.
5. Write the sum (opaque) into `lightmap-cold`; the display reads that one texture.

**Why it must be this way — and why NOT a shadow map:**

- **Unbounded shadow-casting lights.** A *gather* (summing lights in one shader invocation) has no
  merge wall. A materialized shadow map does: the GPU can't merge per-light masks by blend, so each
  casting light needs its own texture channel → a hard **3–4 lights** cap. That cap is the entire failure
  mode — it kills "dense many-lights," which is the only reason the cold tier exists.
- **Cold can afford the sweep.** The bake is amortized (only dirty rects, a few per frame), so the
  per-pixel × per-light × per-caster cost the *dynamic* (whole-viewport, every-frame) path can't pay,
  cold can. This is exactly why the two tiers build shadows **oppositely**: dynamic rasterizes to a map,
  cold sweeps inline.
- **Rect boundaries are free.** Baking each square in world space over all casters within a light's reach
  (not per-rect clipping) means a caster outside the square still throws its shadow into the square.

**The one hard sub-problem** (why it isn't built yet): the inline test needs each light's projected
caster geometry addressable *per fragment* — encode the radius-culled projected tris/contours into a
**data texture** the bake indexes, and decimate the shadow silhouette far below the ~180-pt render
outline (too heavy for a per-fragment point-in-polygon). Match `../resonantdust`'s working inline sweep
before building. Tracked in [work `blockers.md` B3](../../../../work/lighting/blockers.md).

## Anti-goals (the traps)

- **Do NOT materialize a cold shadow map** (a `shadow-cold` RGBA composite with per-light lanes). It
  reintroduces the 3-light cap. An interim build did exactly this — see
  [work `forks.md` F7](../../../../work/lighting/forks.md) + [`deviations.md` D-1](../../../../work/lighting/deviations.md);
  it stands only until the inline sweep replaces it.
- **Do NOT put cold lights in a uniform array.** A fixed `uLightData[N]` caps the count; the light-data
  texture is the point.
- **Do NOT reuse the dynamic tier's channel/bitfield constraints for cold.** Those govern the *rasterized
  scatter* path; cold is a gather and inherits only "a baked sum can't subtract one term" (so a leaving
  cold light makes its rect light-dirty → rebuild).

## References

- Work (in-flight, will archive): [`docs/work/lighting/`](../../../../work/lighting/README.md) — the
  phased port + the interim materialized build + open blockers.
- Current renderer + design directions: [`design/lighting.md`](../design/lighting.md).
- Prior working design: `../resonantdust/docs/tiered_lighting.md` + `../resonantdust/view/src/game/lighting`.
  (Note: that doc frames shadows as materialized scatter/bitfield — right for **dynamic**, not for
  **cold**; the cold inline sweep is the correction this doc preserves.)
