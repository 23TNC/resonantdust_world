# Issues — normal frames

_What is broken, suspect, or unverified. Numbered so decisions and commits can cite them._

## I1 — The normal and the light are in different frames {#i1}

The finding itself, restated as the defect. `sceneNormalAt` returns a **camera-space** vector:

```glsl
vec3 n = wcov > 0.5 ? texture(uNormalWarm, uv).rgb : texture(uNormalCold, uv).rgb;
return normalize(n * 2.0 - 1.0);
```

Flat is `(0,0,1)` — out of the image. `design/de-lighting.md` fixes that convention in the format
(*"the normal ships a flat opaque background (#8080FF outside the silhouette)"*) and the generators
match it: laigter and Marigold work from the drawn image, and the design doc calls their depth
*"camera-distance"*.

`ldir` is built in world terms. `dot(nrm, ldir)` across two frames is not a shading term.

**Symptom to expect:** shading that barely tracks a moving light, because every surface is being
treated as though it faced the camera. Partly **masked** by the wrap floor `mix(0.25, 1.0, ndotl)`,
which lifts wrong-dark surfaces toward plausible — so the fix will move apparent **brightness**, not
just direction, and a "looks different" reaction is expected rather than a regression.

## I2 — Mirrored normal maps may have an inverted X — UNVERIFIED {#i2}

West-facing art is *"the east master, mirrored"* (`texture-serving-model`). A mirrored **normal map**
needs its red channel negated; a mirrored albedo does not. `mrtBakeShader`'s normal path contains no
`flipX` handling.

**Not yet proven broken** — the mirror may be applied upstream (where the same problem would exist),
or masters may be per-facing in practice. Worth settling in the same stream because it is the same
class of error and because a half-fix here is hard to see by eye.

**Test:** light a west-facing sprite from due east and from due west and compare which side reads as
lit; then do the same for its east-facing counterpart. If west's lit side is the mirror of east's,
the X is inverted.

## I3 — The art is oblique; a card is not {#i3}

**A scope boundary, stated so it is not mistaken for a bug later.**

This stream treats a billboard as a flat vertical card and rotates its normals into that plane. But
the *art* is drawn in 3/4 oblique (`docs/art-style.md`) — a character's chest genuinely faces
somewhat upward toward the viewer, not due south. So even a perfectly rotated normal map describes a
surface the sprite only approximately has.

The rotation in this stream is still strictly better than none: it removes a systematic 25–65° frame
error and leaves a much smaller art-vs-geometry discrepancy. **Fixing that residue would mean
authoring normals against the card plane rather than the camera** — a content-pipeline change, not a
renderer one, and out of scope here.

## I4 — Detail normals ride the base frame, so they follow for free {#i4}

`mrtBakeShader` composes per-material detail into the base normal with **RNM** (reoriented normal
mapping) — *"rotate the detail into the base normal's frame"*. Because the detail is expressed
relative to the base rather than to the world, rotating the base into world space carries the detail
with it correctly and needs no separate handling.

Recorded so nobody adds a second rotation for detail and double-counts.


## I5 — `world_angle` and the art's obliquity are different kinds of number {#i5}

Recorded because [F1](forks.md#f1) ties them together as a convention and that tie is invisible once
it works.

- **`WORLD_TILT_DEG`** governs **shadow projection** — how a drawn card height becomes a world
  elevation. It is a property of the renderer's geometry.
- **The sprite obliquity** governs **how the art was drawn** — the angle the generator was prompted
  at, baked into every master. It is a property of the content.

They currently agree (`90 − 65 = 25°` matches *"front or sides, not overhead"*), and F1 chooses to
derive one from the other so there is a single dial. **But nothing enforces it.** Regenerate the
corpus at a different obliquity and the normals silently go wrong while `WORLD_TILT_DEG` still reads
65 — the failure would look like a lighting bug, not a content one.

**If they ever diverge, split the constant.** The plan's last item (recording the convention in
`design/de-lighting.md`) exists so the next generator change has somewhere to collide with this.


## I6 — Walls are a third orientation, and nobody has decided it {#i6}

> "I have no clue how walls will function." — user, 2026-08-02

Tiles are parallel to the ground and billboards are perpendicular to it. A wall is **neither in
general**: it is a vertical surface like a billboard, but it does not face the camera — it faces
along its own run, which the autotile cell already knows (`build-walls`: `x = N+2E`, `y = 3−(S+2W)`).

So a wall's correct normal frame depends on **which way that wall faces**, and a single global
"billboard" rotation is wrong for three of its four faces.

`mrtBakeShader` already mentions *"the analytic wall normals"*, so something is being generated for
them — what frame that is in, and whether it is camera-space like the sprite maps or already
world-space, is **unverified**.

**Explicitly out of scope.** This stream implements the two orientations the user named. Walls get
whichever rotation their prims currently classify as, which is very likely wrong for some facings —
recorded here so that is a known gap rather than a surprise, and so the next person starts from
"what frame are the analytic wall normals in" rather than from scratch.


## I7 — The sprite light direction measured NORTH where the frame says south — FIXED {#i7}

> "Light on the back of billboards is causing them to be brighter and on the front of billboards is
> causing them to be darker." — user, 2026-08-02

A plain sign error, found by eye under the single-torch fixture and fixed on the spot in
`lightPass.ts`. The sprite branch declares its frame as *"x right, y up the card, z toward the viewer
(world south)"* and then built the third component as `Puse.y - Lpos.y`, which is **north**. World y
grows southward — the same convention the line below states as `backLit = Lpos.y < Puse.y`.

The sampled normal's flat value decodes to `(0,0,1)`, out of the image and so toward the viewer, so
the mismatched sign negated every front/back term: a light **behind** a card scored `N·L = +1` and
one **in front** clamped to 0. It also inverted the invariant the shadow gate asserts three lines
later — *"back-lit fronts already rest at the N·L wrap floor"* — which was exactly backwards.

**This is not [I1](#i1), and does not shrink it.** I1 is the *frame* error (a camera-space normal
dotted with a world-space direction, no rotation between them); this was a *sign* error inside the
world-space direction itself. Both were present; only this one is now gone. The stream's P0–P4 still
stand in full, and P0's east/west swing measurement is now measuring a correctly-signed baseline
rather than a doubly-wrong one.

**Worth noting for P0:** with the sign inverted, an orbiting light produced shading that tracked it
*backwards* rather than not at all — so any pre-fix capture of the swing is not a usable "before".
