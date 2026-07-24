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

## North foreshortening (the oval factor)
The ground's north direction, in the view frame (v̂ = screen-vertical, ŵ = view normal / out of screen), is
`n̂ = cos65·v̂ + sin65·ŵ` (rotate the view-vertical by the 65° dihedral into the ground). The **screen** shows
only the view-plane (v̂) component, so a true north displacement `w` along the ground projects to screen
`Δy = w·cos65`. Therefore:

> **true N–S ground distance = screen Δy / cos65**  (cos65 ≈ 0.4226, so ×2.366)

This is the standard result: a plane at dihedral angle θ to the image plane foreshortens its steepest
direction by `cos θ` (θ=0 → factor 1, θ=90° edge-on → factor 0). Here θ=65° → `cos65`. E–W is unaffected.

**The caution / tension (flag for the sim check):** the shadow model's *fictional height* uses `sin65`
(`z = sin65·Δy`), and 65° vs its complement 25° (`sin65 = cos25`) is exactly where a factor can flip. The
fictional height is the shadow model's own construct; the falloff wants the *physical* distance, for which
the ground-recession derivation gives **cos65**. Confidence is high but NOT sim-verified — hence the tunable.

## The true light→point vector
Light: screen ground position `(Lx, Ly)`, height `Lz` above the ground (the `z` byte of the light record,
units). Point: screen `(Px, Py)`, with world elevation `Pz` (0 for a ground texel; the fictional height
`zElev` for a billboard texel — same value the gather already computes). In the ground-aligned frame:

```
Δ_east  = Px − Lx                     // 1:1
Δ_north = (Py − Ly) / cos65           // un-foreshorten the screen N–S
Δ_up    = Pz − Lz                     // heights (light above the ground; billboard point elevated)
vec3 toLight_world = -vec3(Δ_east, Δ_north, Δ_up)   // point → light
dist   = length(toLight_world)        // → FALLOFF (the ellipse + height)
dir    = normalize(toLight_world)     // → N·L against the world-frame normal (normal-tilt)
```

- **Falloff** uses `dist`. Because `Δ_north` is divided by `cos65 < 1`, N–S counts ~2.37× more → the reach
  is an **oval flattened N–S** (semi-axis ratio `cos65`), and `Δ_up` means a point directly under the light
  is at distance `Lz`, not 0.
- **Direction** uses `dir` — the operand the relief `N·L` was missing (it dotted the world normal against a
  screen-space dir). A billboard turned away from the light now gets `N·L < 0` for free (subsumes the
  `prim.y > light.y` idea).

## Phasing note
- **P1** uses `Pz = 0` for every texel (ground assumption) — fixes the oval + light height, the dominant
  effect, with no dependency on the receiver elevation in `LIGHT_FRAG`.
- **P2/P3** thread the billboard `Pz = zElev` (the point's own elevation) into both `dist` and `dir` for
  standing prims, and switch the relief to consume `dir`.
</content>
