# Blockers — binary occlusion + stored caster ids

## B1 — The id map has no viable home on this driver; every remaining route changes the design

**What blocks.** [F2](forks.md#f2) chose "two extra attachments on the shadow RT" as the caster id
map's home, and P2–P4 all assume the winner id can ride along with the coverage write. The bisect in
[I6b](issues.md) proves that **any second colour attachment on the gather hangs the renderer** — one
attachment, constant write, zero accumulators hung identically to two attachments with eight
accumulators. Register pressure, write bandwidth and GLSL validity are all ruled out; what is left is
that MRT on this pass is pathological on the ANGLE/D3D11 path this machine uses.

So F2 is closed, and with it the premise of the phase.

**Why it needs the user, not me.**

1. **Every remaining route is a different design from the one they specified.** The user's proposal was
   explicit — "hold a shadow data texture that holds 1px per slot", scaling to N prims by adding px.
   That is exactly what cannot be built here. Choosing a substitute is choosing a different feature.
2. **It means touching the gather again, and that pass has now locked their machine twice.** The cost of
   a wrong guess is not a failed build, it is a forced reboot of their session. That is their risk to
   accept, not mine to spend on their behalf.
3. **The stream's value is already banked.** P0/P1 delivered the headline (15 -> 16 lights, 8.30 -> 7.11
   ms) and are committed and verified. Continuing is upside, not rescue — which is precisely the
   situation where guessing is least justified.

**Options.**

| | route | cost | risk |
|---|---|---|---|
| **A** | **Pack a tile-LOCAL id into the 6 bits binary freed** (which of the tile's 8 caster slots + a small tile offset), no new attachment | a re-derivation of the id scheme; caps what can be addressed | touches the gather again — the pass that hung twice |
| B | Widen the attachment | — | **dead end**: `rgba32uint` is 128 bits and WebGL2 has nothing wider |
| C | Second gather pass writing ids to its own RT | a full extra corridor walk | **self-defeating** — re-walking IS the search P2 exists to delete |
| D | Stop the stream here | none | none; P1's win stands, P3/P4 never happen |

**Recommendation: A, but only if the user accepts the machine risk.** A is the only route that keeps the
4x-finer-edge payoff, and a tile-local id plausibly fits 6 bits where a global prim id
([F3](forks.md#f3)'s objection) cannot — the id only has to be resolvable from the same texel that
stored it, so it never needs to be globally unique.

If the appetite for another hang is low, **D is a clean close**: the stream would end at 10/26 having
already moved the number it was created to move, with I6/I6b recording exactly why the rest is not
buildable on this hardware — which is a real result for whoever revisits it on a different driver.
