# Blockers — binary occlusion + stored caster ids

## B2 — P3/P4 need more than the id; the storage question reopens with a real number

**What blocks.** [I7](issues.md): a caster's position is stored **mod 16 tiles** relative to the bucket
tile the walk was reading (`resolvedTilePos`). The id map records *which* caster, never *where it was
found* — so an incumbent recovered from a texel cannot be positioned. Substituting the receiver's own
tile is unsafe at reach 16, where a caster can sit beyond the ±8-tile wrap tolerance, and the failure
mode is a **false positive**: a mis-decoded caster lands somewhere plausible, may occlude, and paints a
phantom shadow. Both P3 (early-out) and P4 (fine placement) rest on this.

**Why it needs the user.** The fix is more bits per slot — ~25 rather than 15 — which means a **second
texel**, the exact thing [F7](forks.md#f7) removed to make the hang go away. That is their call twice
over: it re-opens a decision they made, and the last two attempts at extra storage locked their machine.

**Options.**

| | route | cost |
|---|---|---|
| **A** | Store a **receiver-relative** dx,dy alongside the id (the texel knows its own tile, so no `ref` needed) | ~25 bits/slot → a second texel; but a 2x-wide RT is now the known-safe way to get one (no MRT) |
| B | Widen the caster record's position field so no `ref` is needed | touches the data-texture layout, which `VARIABLES.md` owns |
| C | Restrict the early-out to short reaches where ±8 tiles is provably enough | narrow benefit, silent when it stops applying |
| D | **Close the stream on P1+P2** | P1 delivered 15→16 lights, 8.30→7.11 ms; P2 the id map, free |

**Recommendation: D, then A as its own stream.** The banked wins are real and independent. A is a
genuine design with a now-concrete bit budget, and it deserves planning from a clean start rather than
being bolted onto a phase whose premise it invalidates — especially since the 2x-wide-RT route was never
tested and is the one storage option that has not hung anything.
