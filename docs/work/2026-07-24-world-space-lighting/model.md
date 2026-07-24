# The world-space lighting model — derivation (P0)

_The single source of truth for the world-3D light→point vector. Derived from the ratified world-geometry
(`z = sin65·Δ`, ground at 65° to the view). **The N–S factor is DERIVED here but must be
simulation-confirmed by the user before it's trusted** — this is the `2·tan65`-class risk (see
[issues.md#i1](issues.md#i1)). Implemented behind a toggle with the factor itself live-tunable so the eye/sim
settles it._

## The frame
Orthographic oblique projection (no perspective — billboards are drawn parallel to the view as rectangles,
so foreshortening is uniform, not distance-dependent). Ground-aligned world axes:
- **ê** — east. In the view plane; **1:1 with screen x** (the ground and view plane share this axis; the
  ground tilts about it).
- **n̂** — north, along the ground (the tilt / steepest-ascent direction).
- **ẑ** — up, perpendicular to the ground.

Dihedral angle between the ground and the view plane is **65°** (a billboard ∥ view makes 65° with the
ground ⟹ ground makes 65° with the view plane).

## North foreshortening — DERIVED (the factor is `1/cos65`, by identity)
Don't reason in screen space; go to true 3D and let it simplify. A ground point at screen `(sx, sy)`:
the ground's north direction is `n̂ = cos65·v̂ + sin65·ŵ` (rotate view-vertical by the 65° dihedral;
v̂ = screen-vertical, ŵ = out-of-screen), so the screen shows `sy = w·cos65` ⟹ the point is at 3D
`(sx, sy, sy·tan65)`. The light sits height `Z` above the ground along the ground normal
`ẑ = (0, −sin65, cos65)`, i.e. at 3D `(Lx, Ly − Z·sin65, Ly·tan65 + Z·cos65)`.

Form the light→point vector and square its length (`dx = Lx−Px`, `dy = Ly−Py`):
```
Δ    = ( dx,  dy − Z·sin65,  dy·tan65 + Z·cos65 )
|Δ|² =  dx² + (dy − Z·sin65)² + (dy·tan65 + Z·cos65)²
```
The `Z·sin65` cross-terms are `−2·dy·Z·sin65` and `+2·dy·tan65·Z·cos65 = +2·dy·Z·sin65` — **they cancel
exactly** — leaving:
```
|Δ|² = dx² + dy²·sec²65 + Z²  =  dx² + (dy / cos65)² + Z²
```

> **N–S factor = `1/cos65` (≈ 2.366), light height enters as `+Z²`. Settled by algebra, not eyeball.**

**Why `sin65` is NOT the answer here (the resolved worry):** `sin65` is the *fictional-height* coefficient —
how a *vertical* extent projects to screen (`z = sin65·Δy`), a different projection. `cos` governs distance
*along* the ground; `sin` governs height *off* it. They live in different slots; they never competed for the
falloff. (This was the initial conflation; the cancellation above is the proof.)

**What's still an eyeball question — a DIFFERENT one:** not `cos` vs `sin`, but *is the world actually 65°?*
The factor is `cos θ` for whatever the true art tilt `θ` is; nominal 65° is right only if the sprites were
authored for a 65° ground. `__tilt(deg)` sweeps `θ` live (data-map) so the ground foreshortening can be
matched to the art — then the falloff (and shadows, elevation) follow automatically.

## The true light→point vector
Light: screen ground position `(Lx, Ly)`, height `Lz`. Point: **use the point's own ground footprint's
screen-y, not its drawn y** — for a ground texel that's `Py`; for a **billboard** texel it's the base's
screen-y `baseY` (the texel stands *above* its base, so its ground footprint is at the base), with elevation
`Pz` (the fictional height `zElev` the gather already computes). Same identity both cases:

```
// GROUND texel:            Δy = Py − Ly,     Pz = 0
// BILLBOARD texel:         Δy = baseY − Ly,  Pz = zElev
Δ_east  = Px − Lx                     // 1:1
Δ_north = Δy / cos65                  // un-foreshorten the screen N–S (θ from the data-map tilt)
Δ_up    = Pz − Lz                     // heights (light above the ground; billboard point elevated)
vec3 toLight = -vec3(Δ_east, Δ_north, Δ_up)
dist = length(toLight)                // → FALLOFF (ellipse + height); billboard: (Lz − Pz)² in the up-term
dir  = normalize(toLight)             // → N·L against the world-frame normal (normal-tilt)
```

- **Falloff** uses `dist`. `Δ_north / cos65` makes N–S count ~2.37× more → the reach is an **oval flattened
  N–S** (semi-axis ratio `cos65`); `Δ_up` means a point under the light is at distance `Lz`, not 0.
- **Direction** uses `dir` — the operand the relief `N·L` was missing (it dotted the world normal against a
  screen-space dir). A billboard turned away from the light gets `N·L < 0` for free (subsumes `prim.y >
  light.y`). **This is the P2 half, and it only reads right once [`normal-tilt`](../2026-07-24-normal-tilt/README.md)
  has the normal in the same world frame** — that coordination, not the trig, is what still gates P2.

## Phasing note
- **P1** uses `Pz = 0` for every texel (ground assumption) — fixes the oval + light height, the dominant
  effect, with no dependency on the receiver elevation in `LIGHT_FRAG`.
- **P2/P3** thread the billboard `Pz = zElev` (the point's own elevation) into both `dist` and `dir` for
  standing prims, and switch the relief to consume `dir`.
</content>
