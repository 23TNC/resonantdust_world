# Shadow strategy — projected billboard silhouettes (durable intent)

**Status (2026-07-19).** The *why* behind the [`design/shadows.md`](../design/shadows.md) shape, and
how it feeds [tiered lighting](tiered-lighting.md). The geometry converged in the interactive sandbox
([`bin/shadow-projection-sandbox.html`](../../../../../bin/shadow-projection-sandbox.html)); this doc
records the decisions so they survive the sandbox and the session.

## What a shadow is, and what it is not

A caster's ground shadow is its **billboard silhouette, projected radially from the point light onto
the ground** — a handful of textured triangles, alpha sampled as the mask. Rejected alternatives:

- **Blob shadows** (a soft ellipse per object) — cheap but art-blind; every object casts the same
  smear, breaking the RimWorld/Prison-Architect flat-but-legible look.
- **Height-field / self-shadow march** — captures ground relief (it's how surface AO works), but a
  standing billboard's ground shadow is a *projected footprint*, not a height feature. Wrong tool.

Projecting the actual silhouette keeps the shadow **faithful to the sprite** and **per-caster,
per-light**, which is the whole point of a dense many-lights world (each light throws its own shadow).

## Why the geometry is shaped the way it is

- **Facing split (E/W vs N/S).** A side-on sprite (E/W) stands on its foot and leans away, so its
  shadow roots on the foot edge. A front/back sprite (N/S) has no meaningful side profile, so we
  roll it edge-on and root on its centerline — otherwise it would cast a shadow as if it were a flat
  card facing the wrong way. Same fan, two rootings.
- **Per-corner depth, not a single thickness.** Real objects aren't zero-thickness cards; the shadow
  base needs a little spread to read as a footprint, not a knife-edge. Splitting into `dA`/`dB` (one
  per base corner) lets an asymmetric object splay unevenly — a lopsided sprite throws a lopsided base.
- **Depth measured from the art, with a DSL override.** Rather than hand-tune a thickness per sprite,
  derive it: the shadow base half-width ≈ the object's opaque footprint, so `depth = ½ · average
  opaque extent` of the relevant half of the sprite. This makes new art "just work," and the DSL
  override is the escape hatch when a stylized caster wants a fatter/thinner shadow than its pixels imply.
- **Alpha as the mask, sampled through UVs.** We sample the sprite's *alpha* (silhouette), not its
  colour, and interpolate it across each flat triangle — which is precisely what a fragment shader
  does. So the sandbox's picture *is* the shader's picture, and the UVs port 1:1. (The red/blue/oval
  test textures only verify the UVs land where expected.)

## How it feeds tiered lighting

The projected silhouette is the **shared primitive** the two lighting tiers consume oppositely (see
[`tiered-lighting.md`](tiered-lighting.md)): the **cold** inline sweep tests "does any nearby caster's
projected silhouette cover this pixel for this light?" per fragment; the **dynamic** tier rasterizes
the same silhouette into a scatter map. Per-light occlusion means each caster projects once *per
light in range*.

## Cost — cheap per shadow; the multiplier is caster × light

- **Runtime math per shadow is negligible** — project 4 corners (one divide each), derive the base
  points, emit 5 triangles. Tens of thousands per frame is fine on the CPU; 5 tris/caster is nothing
  on the GPU.
- **The depth analysis is a bake, not a frame cost.** The presence scan runs once per sprite-facing at
  asset time and stores 4 scalars on the def; it never touches the render loop.
- **What actually scales is `casters × lights_in_range`**, because occlusion is per-light. This is why
  the tier split exists: **static caster under static light bakes into the cold lightmap once**; only
  movers and dynamic lights pay per frame. The budget to watch is that product, plus **fill/overdraw**
  (the `±depth` flaps double-cover near the base) — not the per-shadow triangle count.

## References

- Shape + formulas: [`design/shadows.md`](../design/shadows.md).
- The tier machinery this plugs into: [`tiered-lighting.md`](tiered-lighting.md) and
  [`design/lighting.md` §D](../design/lighting.md).
- Interactive model + parameter tuning:
  [`bin/shadow-projection-sandbox.html`](../../../../../bin/shadow-projection-sandbox.html) (live
  artifact `claude.ai/code/artifact/89cfbfe2-2922-4c3d-ac91-123f3e2506ee`).
