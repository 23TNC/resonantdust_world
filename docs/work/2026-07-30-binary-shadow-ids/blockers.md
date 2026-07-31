# Blockers — binary occlusion + stored caster ids

## B1 — RESOLVED by [F7](forks.md#f7), 2026-07-31

B1 asked the user to choose a home for the caster id map after MRT on the gather hung twice. It was
premised on keeping 16 lights per tile, which forced the ids outside the 128-bit texel.

**Dropping to 8 lights makes the question disappear**: 8 x u16 = 128 bits = the texel itself. No second
attachment, so the hang cannot recur; no risk for the user to accept; nothing to decide.

The blocker was real given F2, and wrong because F2 was wrong. Kept rather than deleted so the reasoning
chain stays legible — three approaches failed against a constraint I never questioned.
