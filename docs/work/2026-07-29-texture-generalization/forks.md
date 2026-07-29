# Forks — texture-generalization

_Decisions resolved during execution, with reasoning. Pre-registered: R4 (the connect rule —
rotation-match per the user's design vs def-id match if cross-material junctions read
wrong), R5 (the authored internal_padding value once checked against the master), and the
slot-0 writer's exact rebuild cadence._

## Pre-resolved by the user's second pass (2026-07-29)

R4 → cross-material junctions accepted; the FUTURE variant model (≤16 variants per kind,
connection gated by a variant/kind check so wall never joins fence/rock) is deferred to its
own planning. R5 → not a conflict: the manifest's `pad` is atlas-EXTERNAL (correctly 0);
`internal_padding` is the between-cell inset inside the linked atlas and is what prevents
neighbor bleed. R1/R2/R3 → superseded by D7 (presence 4×u32 `flags|set|index`, self-
describing slots, flag early-outs) and D8 (receives_shadows modes; the ground special case
and the cut RETIRE). Open measurement gate: R1′ — the 4-slot caster capacity, probed before
the reshape ships.
