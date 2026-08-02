# Normal frames — the maps are camera-space, the lighting is world-space — 2026-08-02

_Component: [`client/webgl`](../../components/client/webgl/). Plan in [`todo.md`](todo.md); decisions
in [`forks.md`](forks.md); what is already suspect in [`issues.md`](issues.md)._

## The user's finding

> "I believe we have not handled world geometry when handling our normal maps. Normals need to be
> pitched down 90−world_angle so they are parallel with the ground. This means the normal map is
> perpendicular to the ground."

Correct, and verified before planning. **Normal maps in this project are authored in CAMERA space
and consumed as if they were WORLD space, with no rotation between the two.**

The evidence is in the format itself. `design/de-lighting.md`: *"the normal ships a flat opaque
background (#8080FF outside the silhouette)"*. `#8080FF` decodes to `(0, 0, 1)` — **out of the
image**, which is out of the *screen*, not up out of the ground and not out of a billboard's face.
The generators agree: laigter and Marigold both work from the drawn image, and the design doc already
notes their depth is *"camera-distance"*.

The shader then does this, in `lightPass`:

```glsl
vec3 nrm = sceneNormalAt(P);            // camera space, flat = (0,0,1) out of the screen
...
ndotl = clamp(dot(nrm, ldir), 0.0, 1.0);   // ldir is built in WORLD terms
```

A dot product between two vectors expressed in different frames is not a shading term. Every lit
surface is currently shaded as though the camera were looking straight down its own normal.

## Two surfaces, two rotations, one angle

The fix is not one global pitch, because the two surface classes are oriented differently in the
world and the camera sees both:

| surface | how it sits | its true normal | from camera space |
|---|---|---|---|
| a **billboard card** | perpendicular to the ground | **parallel** to the ground (horizontal) | pitch **down** |
| the **ground** | parallel to the ground | **perpendicular** to it (vertical) | pitch **up** |

Both rotations are about the same axis (screen-x, world east) and both derive from the one
`WORLD_TILT_DEG` that `worldTilt.ts` already owns. The two magnitudes are complements — they sum to
90° — which is why the user's sentence and their formula each describe one of them
([F1](forks.md#f1) works out which is which, and flags the one thing that needs settling).

## What exists today

Verified, not recalled:

| piece | state |
|---|---|
| normal maps authored camera-space (`#8080FF` flat) | **yes** — the format is the evidence |
| any camera→world rotation of the normal | **none** — `sceneNormalAt` returns `normalize(n*2-1)` and stops |
| the light direction built in world terms | **yes**, since `2026-08-01-z-positioning` P2 |
| one normal, two different `ldir` frames | **yes** — the shader branches the LIGHT's frame per receiver-ness |
| a wrap floor hiding some of the error | **yes** — `mix(0.25, 1.0, ndotl)` |

That last row matters for expectations: the fix will change apparent **brightness**, not only the
direction of shading, because the wrap floor is currently propping up surfaces whose `N·L` is wrong.

## Why it is worth doing now

`2026-08-01-z-positioning` put the light direction into world space and left the normal in camera
space, so the mismatch is now the *only* frame error left in the shading term — and it is a clean,
isolated one. It is also the reason a light circling a billboard does not read as circling it: the
shading barely tracks the light, because the surface is being treated as camera-facing no matter
where the light is.

## The number this stream moves

**A light orbiting a billboard produces shading that tracks it** — bright on the lit side, dark on
the far side — measured as the swing in `N·L` between a light due east and due west of the same
card. Today that swing is wrong by the frame error; it should follow the geometry. Secondary: no
measurable frame cost (this is a matrix applied to a value already fetched).
