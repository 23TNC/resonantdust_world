# Forks — lighting

_Decision points + options + which we chose + why. Chronological._

---

## F1 · Lightmap, not per-pixel forward {#f1}
**2026-07-23 — DECIDED (user): a baked LIGHTMAP.** Lighting accumulates into a cached, dirty-driven
lightmap RT (`lightmap = ambient + Σ per-light colour·falloff·diffuse·(1−shadow)`); the display
composites `albedo × lightmap` (warm-over-cold intact — [[rt-tiers-cold-warm-hot]]). NOT the per-pixel
forward blit — the lightmap recomputes only dirty regions (reuses the shadow dirty system), matches
the G-buffer map model, and is reusable by post (bloom). The `AlbedoBlitShader` display becomes the
`albedo × lightmap` composite.

## F2 · Falloff curve {#f2}
**2026-07-23 — DECIDED: smoothstep-to-0-at-reach.** `smoothstep(reach, 0, dist)` — 1 at the light, a
smooth Hermite roll-off to 0 exactly at reach (so a light dies at its presence circle with no hard
step). Cheap, reads well for the stylised look ([[art-style]]). Inverse-square + artist curves remain
drop-ins if a more physical falloff is wanted.

## F3 · Normal interpretation (top-down 2D + normal map) {#f3}
**2026-07-23 — DECIDED: horizontal-perturbation relief, flat-neutral.** Not a full N·L (which darkens
flat ground by `L.z`). Instead `relief = 1 + strength · dot(N.xy_world, ldir)` where `N.xy` is the
normal's HORIZONTAL part (flat ground `N.xy≈0` → relief 1, untouched) and `ldir` is the baked
aggregate lit-from direction. A slope toward the dominant light brightens, away darkens. Convention:
normal is +Y-up OpenGL (sprite-north), but world +y is south → flip `N.y` (`vec2(N.x, −N.y)`) into the
world frame `ldir` lives in; `N.x` = world-x already. `strength` default 1.1 (`__relief(n)` tunes live).
Decode `N = enc·2 − 1` matches the bake (flat = 0.5,0.5,1.0). Full 2.5D N·L-with-zdepth stays a later
option if the flat-neutral model reads too subtle.

## F4 · HDR / tonemap {#f4}
**2026-07-23 — DECIDED: LDR clamp.** Irradiance clamps per-channel to [0,1] in the bake. Reads clean +
stylised (a bright light core saturates to its colour — reads as intensity, matches the flat
[[art-style]], not an artifact). A Reinhard `x/(1+x)` on the summed irradiance is the drop-in if
content lights (many overlapping) blow out — revisit when real content lights land, not needed for the
debug rig.

## F5 · Lightmap resolution + where the normal is applied {#f5}
**2026-07-23 — DECIDED: (b) coarse baked lightmap + per-px normal in the blit.** The lightmap is
unit-res (TEXTILE_UNIT, TEXEL-aligned with shadow-cold) and holds the LOW-freq terms: MRT att0 =
irradiance `ambient + Σ colour·intensity·falloff·(1−shadow)`, att1 = the irradiance-weighted mean
horizontal lit-from direction. The HIGH-freq normal is applied PER-PX in the display blit (samples the
normal composite at vUV, dots with att1 — see [F3](#f3)). Per-light directionality is approximated by
the aggregate direction (the dominant/nearest light drives the relief, which is where its irradiance
dominates anyway — reads correctly). Held 120fps (display cap) with 3 lights + 1 moving + soft shadows,
so the coarse lightmap + one blit dot was ample; the px-res option (a) was unnecessary.

## F6 · Shadow ceiling — don't shadow billboards above the shadow's height {#f6}
**2026-07-23 — RAISED (user); DEFERRED to a follow-up (P1–P3 shipped without it).** The lighting pass
now masks by ground shadow-cold, so a tall billboard standing in a low shadow is darkened whole (its
head too) — the artifact this fork fixes. Not built this pass (get flat shadows reading first); it's
the next lighting follow-up. shadow-cold is a GROUND coverage: it says *how
much* light `i` is blocked at tile P, not *up to what height*. So a standing billboard sampling it
flat gets darkened whole — head in the shade even when its head clears the low shadow. The missing
quantity is the **shadow ceiling** `h(P)` = the height up to which the caster blocks light `i` at P
(≈ caster height near the base, → 0 at the shadow tip).

What we already have: **direction** (presence gives the acting lights per tile; shadows are separated
per light, so `normalize(P − L_i)` is known) and the pixel's own depth (G-buffer `zdepth`). What we
threw away: the **caster's height**, because coverage is a scalar max-accumulated over all casters.

**The ceiling is recoverable in the gather for free** — `casterCover` already computes `t` (the
card-height fraction where the grazing ray lands), so it can emit `ceiling = max_casters(caster_height
· f(t))` as a companion channel, **max-accumulated** (idempotent — matches the existing invariant, no
new corridor walk). The lighting pass then masks light `i` by `(pixel_height < ceiling_i)` AND its
coverage: feet shadowed, head lit.

Options:
- **(A) per-light ceiling** *(lean)* — u3–u4 height next to each light's u9 coverage (≈one more set,
  14×u4 = 56 bits, symmetric with the coverage doubling). Correct + consistent with per-light shadows
  (a pawn can be in light-A's shadow while lit by light-B → the ceiling MUST be per-light).
- **(E) single fixed ceiling** — one global height H, zero data; crude, catches the common case, misses
  short-caster nuance.
- **(D) ground-only shadows** — never shadow billboards (RimWorld does this); zero cost, loses "walk
  into shade" entirely.

**Blocker to verify before committing to (A)-cheap:** whether G-buffer `zdepth_world` is the pixel's
HEIGHT above ground or its ground DEPTH (north–south) — different axes in the tilted-world projection.
If it's ground-depth we need one height conversion; that's the only real unknown, the rest is machinery
we own (`t` in the gather, max-accumulate, a companion set like presence lo/hi).
