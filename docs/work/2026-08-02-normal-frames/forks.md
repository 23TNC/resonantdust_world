# Forks — normal frames

_Decision points, options, which we chose and why._

## F1 — Which rotation is `90 − world_angle`, and from which axis? {#f1}

**The one thing that needs the user.** Both readings below produce a renderer that runs; they differ
by **40°** on every normal in the scene, so this cannot be settled by taste.

The user wrote: *"normals need to be pitched down 90−world_angle so they are parallel with the
ground. This means the normal map is perpendicular to the ground."*

Those two halves point at different surfaces:

- *"parallel with the ground"* / *"the normal map is perpendicular to the ground"* describes a
  **billboard card** — it stands perpendicular to the ground, so its face normal is horizontal.
- **`90 − world_angle` = 25°** is, under the reference axis this project already documents, the
  **ground's** correction, not the card's.

`worldTilt.ts` defines `WORLD_TILT_DEG = 65` **measured from the ground plane**, and says so
explicitly. Under that reference the camera's out-of-screen axis sits 65° above the ground, so:

| surface | rotation from camera space | magnitude at 65° |
|---|---|---|
| billboard card → normal horizontal | pitch **down** by `world_angle` | **65°** |
| ground → normal vertical | pitch **up** by `90 − world_angle` | **25°** |

- **(a) The reference is the GROUND plane** (as documented). The user's `90 − world_angle` is the
  **ground** correction and is exactly right; the card takes `world_angle` = 65°. The sentence and
  the formula describe the two different cases.
- **(b) The reference is the VERTICAL.** Then the camera sits 25° above the horizon, the card's
  correction *is* `90 − world_angle` = 25°, and the sentence and formula agree — but
  `worldTilt.ts`'s documented axis is wrong and a 65°-from-vertical camera is a nearly horizontal
  view, which does not match the art.

**Recommend (a)**, because the reference axis is already written down, 65° from the ground is a
plausible 3/4 view where 65° from vertical is not, and (a) makes both halves of the user's message
true at once rather than one of them. **But it is their geometry**, and P0a of the previous stream
warned in writing that this exact number means two different cameras depending on the axis — so it
is asked, not assumed.

**Either way the plan is the same shape**: one angle, two derived rotations, one place. Only the
constant changes, so P1–P3 can be built before this is settled and P4 pins it.

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
