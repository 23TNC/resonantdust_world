# Lighting — illuminate the world from the 3 lights — 2026-07-23

_Component: [`client/webgl`](../../components/client/) · `game/viewport/` — a new lighting pass at
the display seam (replaces/augments `albedoBlitShader`'s UNLIT blit), consuming the G-buffer +
`coldShadowData` lights + `shadow-cold`. Phases in [`todo.md`](todo.md)._

> **STATUS — P1–P4 DELIVERED + verified 2026-07-23** (baked lightmap: emission + per-px normal relief +
> per-light shadows, 120 fps display cap). See [`completed.md`](completed.md). Remaining: **F6 shadow
> ceiling** (P3b, deferred — [`todo.md`](todo.md)).

Stand up the **lighting pass** (deleted 2026-07-19 in the lighting/shadow nuke — the G-buffer + the
whole shadow system have since been rebuilt; this re-lights the world on top of them). The pieces are
all in place now:

| input | where | holds |
|---|---|---|
| **albedo** | G-buffer (SquareCache `albedo-cold`/`-warm`) | base pigment (material-jittered, no light) |
| **normal** | G-buffer `normal-*` | surface relief (+Y-up, OpenGL; [[marigold-delight]]) |
| **surface** | G-buffer `surface-*` | R presence · G ao · B alpha ([[coverage-surface-model]]) |
| **lights** | data texture set 2 (`coldShadowData`) | pos · colour · reach · emitter · intensity |
| **presence** | data texture sets 3/5 | the ≤14 lights acting on each tile |
| **shadow-cold** | the gather RT | per-light u9 coverage (14 slots) — [[lighting-rebuild-complete]] |

## Architecture: a baked LIGHTMAP that combines with the maps (user, 2026-07-23)

Lighting writes into a **lightmap** — a cached, dirty-driven map (like the G-buffer channels + the
per-rect light texture of [[lighting-direction]]), NOT per-pixel-every-frame forward shading. It
accumulates each acting light PER-LIGHT (shadow + normal fold in BEFORE the sum, since each is
per-light); the display composites it into albedo:

```
lightmap = ambient + Σ_i  colour_i · intensity_i · falloff_i · diffuse(N, L_i) · (1 − shadow_i)
out      = albedo × lightmap      // material (albedo/normal) × illumination (lightmap)
```

**Cached + dirty-driven is the point**: the lightmap recomputes only where lights/shadows changed —
reusing the shadow dirty system (a static light over static trees bakes once; a mover re-bakes its
reach box). **Resolution is the P2 fork** ([`forks.md#f5`](forks.md#f5)): falloff + shadow are
LOW-frequency (a coarse unit-res lightmap like shadow-cold is cheap + enough), but normal relief is
HIGH-frequency (per-px) — so either a px-res lightmap (full detail, 16× the light math) or a coarse
lightmap + per-px normal at composite (cheaper, approximate).

## The phased plan (user's order)

Built up in phases; each writes the lightmap more completely, the display composite stays
`albedo × lightmap`:

### P1 · Emission — lights illuminate the world
Flat accumulation, no normal, no shadow. Per pixel: `light = ambient + Σ_i colour_i · intensity_i ·
falloff(dist_i, reach_i)`; `out = albedo · light`. Proves the presence → light-record → falloff →
sum path renders (a bright halo around each light, over the flat albedo). Falloff curve is a fork
([`forks.md#f2`](forks.md#f2)).

### P2 · Albedo + normal — shaded lighting
Add **Lambert diffuse** from the normal map: per light, `diffuse = max(0, N · L̂)` with `L̂` the
direction to the light (in-plane `light.xy − P`, combined with the normal's up component for the
top-down 3/4 look). `out = albedo · (ambient + Σ_i colour_i · intensity_i · falloff_i · diffuse_i)`.
Now relief reads (lit side bright, away side dark). Normal convention is a fork
([`forks.md#f3`](forks.md#f3)).

### P3 · Shadows — lights respect their shadow-cold coverage
Mask each light's contribution by its shadow: `… · (1 − shadowCoverage_i)`, reading slot `i`'s u9
coverage from `shadow-cold` at the pixel's unit-texel (presence slot `i` ↔ shadow slot `i`, same
light). A shadowed pixel stops receiving that light — the penumbra/umbra we built modulate it
smoothly. This is what the whole shadow stack was for.

## Where the pass runs

The Viewport currently blits UNLIT albedo (warm-over-cold) via `AlbedoBlitShader`. The lighting pass
is that display draw, now sampling albedo + normal + surface + presence + shadow-cold and outputting
lit colour ([`forks.md#f1`](forks.md#f1): extend the blit vs a separate composite RT; warm-over-cold
compositing must stay — [[rt-tiers-cold-warm-hot]]).

## Not sun/ambient — dense many-lights
Per [[lighting-direction]]: this is DENSE point lights (the DSL emits them), not a global sun. Ambient
is a small floor, not the main term. The 3 debug lights are the first consumers; content lights follow.
