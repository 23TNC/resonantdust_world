# Emitter-based soft shadows (penumbra + umbra) — 2026-07-23

_Component: [`client/webgl`](../../components/client/) · `game/viewport/shadowGather.ts` (the
`shadowCover` / `casterCover` GLSL) + `coldShadowData.ts` (light `emitter_radius`, already stored).
Phases in [`todo.md`](todo.md); the approach fork is [`forks.md#f1`](forks.md#f1)._

Make shadows **soften with the light's emitter size + shadow stretch** — real penumbra, not just
finer edge AA. Each light already carries an `emitter_radius`; today it's unused. The
[14-light u8 shadow](../../VARIABLES.md) gives coverage the precision to represent a gradient; **P0
widens it to u9 (512 levels)** for headroom, then P1 computes the actual soft-shadow coverage.

## The model today (what we're softening)

A caster is a billboard card projected from the light (a **point**) onto the ground → a hard convex
**quad** (`shadowCover` returns 0/1). The silhouette is applied by sampling the sprite's SURFACE
coverage (B) at the inverted card `(s,t)`. So the shadow's real outer edge follows the **sprite
silhouette**, inside the quad. Final `cov = quadMembership × surfaceB`.

## Penumbra geometry (the width)

A real light is an emitter of radius `e`, not a point. An occluder edge casts an umbra→penumbra
gradient whose **half-width grows with emitter size and the occluder-to-ground / light-to-occluder
ratio** — sharp at contact (the card base), soft at the shadow tip. The card-height fraction `t`
(already computed for the silhouette inversion) gives it directly:

```
w_world = emitter_radius · (t·Zt) / (Lz − t·Zt)      // 0 at the base (t=0), e·(k−1) at the tip
```

(`Zt` = card top height, `Lz` = light height, `k` = the top's projection factor — all already in
`shadowCover`.) This is the penumbra half-width in world units at the ground point P.

## Umbra falls out of the same model (darker base → lighter tip)

**Umbra** = the fully-occluded dark core (emitter *entirely* blocked, coverage 1.0). **Penumbra** =
the partial-occlusion soft ring around it. They are one emitter model, not two mechanisms — and the
multi-tap (B) produces both, giving the base-dark → tip-light gradient the design wants:

- Near the base, `t→0 ⟹ w→0`: the kernel is tiny, coverage stays **1.0 (umbra)** — dark, sharp.
- Toward the tip, `w` grows: the kernel averages over more than the (stretched, thinner) silhouette
  feature, so the **peak coverage drops below 1** — the umbra shrinks away, leaving soft, lighter
  penumbra. Wider casters keep an umbra farther (physically correct).

So "implementing umbra" = making the kernel width grow with `t` (it does) so the umbra naturally
recedes with distance. An **optional explicit contact-darkening dial** ([`forks.md#f4`](forks.md#f4))
can strengthen the near-base darkness beyond the physical falloff for art control (RimWorld-style
contact shadows) — default off / physical.

## The fork: WHERE penumbra applies ([`forks.md#f1`](forks.md#f1))

- **(A) Quad-band** — soften the QUAD boundary: signed distance to the quad edge, `smoothstep` over
  `±w_world`. Cheap (no extra samples). BUT the silhouette sits inside the quad and hides the quad
  edge, so this only softens the outer blob, not the shape → **weak penumbra**. Rejected as primary.
- **(B) Silhouette multi-tap (RECOMMENDED)** — soften the actual shadow edge by sampling surface B
  with a small kernel whose radius = `w_world` converted to atlas texels (÷ card width × frame px),
  averaged. Real penumbra: the *shape* edge blurs, wider for bigger emitters / farther shadows.
  Cost: N taps × the existing per-caster B sample (corridor-bounded). Start with a 4–9 tap kernel,
  measure. This is the plan.
- Hybrid possible later: quad-band for the coarse falloff + a cheap 4-tap silhouette blur.

## Shadow coverage — u9 (P0, the "512 range")

Widen each of the 14 slots from u8 → **u9** using 14 of the 16 reserved bits, NO channel straddle:
the low 8 bits stay in their channel (`slot i` → channel `i>>2`, bits `(i&3)·8`), the **9th (high)
bit** goes into A at bit `16+i` (A bits 16–29 = 14 high bits; A bits 30–31 reserved):

```
value_i (0..511) = low8_i | (high1_i << 8)
  low8_i  : channel (i>>2), bits (i&3)·8         (unchanged from u8)
  high1_i : A, bit 16 + i                         (i = 0..13)
```

512 levels is the coverage resolution penumbra gradients need; the format is otherwise the u8 layout
plus the high-bit plane.
