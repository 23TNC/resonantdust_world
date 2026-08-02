# Forks — normal frames

_Decision points, options, which we chose and why._

## F1 — BOTH rotations: ground pitches UP, billboard pitches DOWN (user, 2026-08-02) {#f1}

> "Billboards are perpendicular, I scoped the work for the billboards. You decided to also scope for
> tiles. Fine. Tiles are indeed parallel to the ground. I have no clue how walls will function.
> Implement ground lighting by pitching up and billboard lighting by pitching down."

**Settled: two rotations.** I had corrected this fork *down* to one, on the strength of
`art-style.md`'s *"the world grid is viewed top-down"* — reading that as a straight-down camera for
the ground, which would leave its flat normal already vertical.

**That reading was wrong**, and the user's is the coherent one: *"the world grid is viewed top-down"*
describes the **layout** — a square, axis-aligned grid rather than isometric diamonds — not a second
camera. There is one camera at `world_angle`; the ground is merely drawn **unforeshortened**, which
is a stylistic choice about the grid, not a claim about the view direction. The rest of the codebase
agrees: `P = (tile + inTile) * UPT` is square on both axes, and `2026-08-01-z-positioning` already
found and documented that the renderer does not foreshorten.

So the camera sees both surfaces from the same place, and their flat normals both point at it:

| surface | sits | true normal | rotation |
|---|---|---|---|
| **tile / ground** | parallel to the ground | vertical | pitch **UP** |
| **billboard** | perpendicular to the ground | ground-parallel | pitch **DOWN** |

They are complements: the surfaces are perpendicular to each other, so the two rotations sum to 90°.
Both come from the one `world_angle`.

### Expressed by MEANING, not by a formula that depends on the reference axis

The user's phrasing puts the billboard at `90 − world_angle`. Under the reference this repo already
documents — `world_angle` measured **from the ground plane**, currently 65° — that magnitude is the
**ground's**, and the billboard's is `world_angle`. Under a from-the-vertical reference the two swap.
The pair `{25°, 65°}` is the same either way; only the assignment moves, and picking wrong misses by
**40°** on every normal.

**So the code does not encode either formula.** It encodes the geometric requirement:

- a billboard's flat normal must come out **ground-parallel** (vertical component 0)
- a tile's flat normal must come out **vertical**

Both are asserted. Under the documented from-the-ground reference those resolve to 65° down and 25°
up; under the other reading the assertions fail loudly instead of shading 40° wrong in silence. That
is the point of asserting rather than trusting the arithmetic.

### Walls are NOT covered

*"I have no clue how walls will function."* Neither is settled here — see [I6](issues.md#i6). A wall
is a third orientation and this stream deliberately does not guess at it.

## F2 — Rotate the NORMAL into world, not the light into the surface {#f2}

Today the shader keeps the normal as-is and builds `ldir` in a different frame per receiver class:

```glsl
if (recvIdx != 0u) { vec3 ldir = ...; }   // "sprite frame: x right, y up the card, z toward viewer"
else               { vec3 ldir = ...; }   // "ground frame: x right, y north, z up"
```

- (a) Keep branching the light's frame; add the tilt to each branch.
- **(b) Rotate the sampled normal once into WORLD space; build `ldir` in world once.**

**Chosen: (b).** The light direction is already world-space after
[z-positioning P2](../2026-08-01-z-positioning/completed.md) — it is the normal that is in the wrong
frame, so that is what should move. (a) spreads one geometric fact across two branches that must then
be kept in step, which is the exact shape of bug this project has paid for four times (a writer and a
reader disagreeing about one rule).

It also **collapses the branch**: with the normal in world space there is one `ldir`, and the
receiver-ness test survives only where it genuinely belongs — choosing *which rotation* the normal
takes, which is a property of the surface, not of the light.

## F3 — The rotation lives in `worldTilt.ts`, TS and GLSL {#f3}

- (a) Inline the matrix in `lightPass`.
- **(b) Put it beside the existing tilt conversions, in the one file that owns the angle.**

**Chosen: (b).** `worldTilt.ts` already exists for exactly this, already interpolates its constants
into GLSL so the two halves cannot drift, and already documents the reference axis. A second place
that knows the world angle is how the first one goes stale.

## F4 — Mirrored facings: the normal's X, unverified {#f4}

West-facing art is served as **the east master, mirrored** (`texture-serving-model`). Mirroring an
albedo is free; mirroring a **normal map is not** — the red channel encodes left/right slope, so a
mirrored normal map must have its X **negated** or every west-facing sprite is lit as though its
relief ran the other way.

No `flipX` handling appears in `mrtBakeShader`'s normal path. That is *suspicious, not proven* — the
mirroring may happen upstream in the atlas, where it would have the same problem, or the master may
be authored per-facing after all.

**Not decided here.** [I2](issues.md#i2) is to find out, and the fix (if needed) is one sign flip
keyed off the same `flipX`/rotation the albedo already uses. Recorded now because it is the same
class of error as the main finding — a map consumed in a frame it was not authored in — and because
fixing the pitch while leaving a mirrored X would half-fix the problem in a way that is hard to see.
