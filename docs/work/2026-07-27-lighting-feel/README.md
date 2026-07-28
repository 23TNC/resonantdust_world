# lighting-feel — colour, glints, and the decay lightmap

_Work stream, opened 2026-07-27. Component: `client/webgl` (`game/viewport/`), plus `bin/art` for
the art-gated phase. Ratified item-by-item with the user (2026-07-27); the numbers below reference
that conversation's list._

## What — the approved appearance features

The lighting is architecturally sound (per-light shadow masks give correct multi-light cross-fill;
performance is under control after [lighting-standing-costs](../2026-07-27-lighting-standing-costs/README.md))
but the world doesn't yet *feel* lit. Approved to build:

- **(5) Colour temperature over falloff** — each light warms at the core, dims + desaturates
  toward its reach edge; fire pools read as fire instead of orange disks. A few ALU in
  `accumulateLights`.
- **(6) Specular glints** — per-light Blinn N·H on things (the live `billboardNormal` is already
  in hand); wet-leaf sparkle near torches.
- **(7) THE DECAY LIGHTMAP** — the flagship, the user's design: a third, coarse, *ephemeral*
  lightmap that decays toward zero every frame; light PARTICLES are splatted into it additively,
  fire-and-forget, and fade out on their own. Flicker/motion then reads in the eye while the
  expensive gather never runs — the torch's base light stays static in the HOT accumulator and
  re-bakes only on REAL movement (under the light-budget allowance, when that lands).
- **(1) AO grounding + (2) emissive** — added back at the user's direction, **art-gated**: the
  corpus surface-G (AO) is believed thin and emissive leaves don't exist yet, so P0 audits decide
  how much `bin/art` work each needs before the blit consumes them.

## Design stance (the decay map)

- **Ephemerality is the whole trick.** The cold/hot lightmaps are EXACT accumulators (quantised
  integer deposits, bit-exact add/subtract) and carry heavy invalidation machinery because they
  must be *undoable*. The decay map is exempt from all of it by construction: it forgets. No
  dirty tracking, no subtraction, no exactness — error decays to zero. Keep it that way; the
  moment something "persistent" is written here, it belongs in cold/hot instead.
- **In-place decay, one draw** ([F2](forks.md#f2)): `blendFunc(ZERO, CONSTANT_COLOR)` +
  `blendColor(k,k,k,1)` computes `dst *= k` with no texture read and no ping-pong. `k` is
  dt-derived (`exp(−dt/τ)`) so fade speed is frame-rate independent.
- **Coarse + bilinear** ([F1](forks.md#f1)): glow is soft by nature — `TEXTILE_UNIT` resolution
  (512×256, RGBA16F, ~1 MB) upsampled bilinearly in the blit. The decay pass touches 131 k texels
  (~0.01–0.02 ms); fine resolution would be ~134 MB of bandwidth per frame for nothing visible.
- **Shadow-STAMPED splats** ([F4](forks.md#f4)): a splat samples its parent light's slot in the
  coarse shadow map at EMIT time and multiplies itself by `(1 − shadow)` — particles inherit their
  light's shadows for free, and flicker cannot bleed light into shadow (the zero-light gameplay
  rule stays honest). Known, accepted limits: the stamp is as-of-emit (particles live ~a second);
  no N·L on particles (they are glow, not lighting); v1 emits only from lights ([F5](forks.md#f5)).
- **Zero light is gameplay.** No shadow-strength caps, no ambient lifts, anywhere in this stream.

## Documented-later + deferred (not built here)

- Intent staged in [`docs/intent/lighting-feel/`](../../intent/lighting-feel/README.md):
  **(3) source halo**, **(8) light shafts**, **(9) tone curve + ambient grading**.
- **(4) ground relief** — arrives free when tile primitives carry normal maps; no code now.
- **SDF-silhouette penumbra** — the standing proposal for shaped soft shadows (one distance-field
  fetch replacing the interval machinery); parked pending the user's call on penumbra, noted in
  the intent doc so it isn't re-derived from scratch a sixth time.

## Acceptance (stream-level)

Torches visibly flicker via particles at full frame rate with the gather submitting ZERO draws on
static frames (`debugClassDraws 0` — flicker must not touch the accumulators); every new pass is
GPU-timed and stays within the standing-costs baseline; A/B screenshots recorded for each visual
change; `bin/rd docs-check` green.
