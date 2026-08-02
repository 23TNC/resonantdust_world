# Forks — normal frames

_Decision points, options, which we chose and why._

## F1 — There is ONE rotation, not two: the ground is drawn top-down {#f1}

**Corrected 2026-08-02, before any code.** My first framing assumed a single camera looking at both
the ground and the sprites, which forced two complementary rotations and a 40° ambiguity about which
surface got which. `design/art-style.md` says that premise is false, in its own most-emphatic section:

> **"This is the single most important and most misunderstood point.**
> The **world grid is viewed top-down**, but **objects and characters are drawn in an oblique
> three-quarter top-down perspective** — they show their **front or sides**, NOT a true bird's-eye
> projection … Do **not** render characters/creatures as if seen from directly overhead."

**Two different projections, by design.** That changes the answer:

| surface | how it is DRAWN | its flat normal `(0,0,1)` | rotation |
|---|---|---|---|
| the **ground** | top-down | already **straight up** — the ground's true normal | **none** |
| a **sprite** | oblique, showing front/sides | tilted up from the card's horizontal by the art's obliquity | pitch **down** by that obliquity |

So there is exactly **one** rotation, it applies to **billboards only**, and it is what the user
described: *"pitched down 90−world_angle so they are parallel with the ground"*.

It also means `lightPass`'s existing ground comment — *"the flat-up fallback decodes to (0,0,1),
reproducing the old overhead/grazing falloff exactly"* — was **already right**, and my plan to rotate
the ground would have broken a working case.

### What is still open, and it is smaller

`90 − world_angle` = **25°** at the documented 65°-from-the-ground reference. That reads as sprites
drawn 25° above eye level — mostly front-on, which is exactly *"they show their front or sides, NOT
a true bird's-eye"*. **Consistent, and it makes the user's formula right as written.**

The residual question is not *which* surface, but whether the sprite's obliquity is **tied** to
`world_angle` at all:

- **(a) Tied** — obliquity = `90 − world_angle`, one dial moves shadow projection and normal
  correction together.
- **(b) Independent** — the obliquity is an art-side fact (how the generator was prompted / how
  masters were drawn) that happens to sit near 25°, and deserves its own constant.

**Recommend (a)** and note it plainly: `world_angle` and the art's obliquity are *not* the same kind
of number — one governs shadow projection, the other governs how a picture was drawn — and this
renderer already keeps two deliberately inconsistent projections. Tying them is a **convention**, not
a derivation. It is the right default because one dial is better than two silently-drifting ones, but
if the art is ever regenerated at a different obliquity, (b) is the honest answer and the constant
should split.

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
