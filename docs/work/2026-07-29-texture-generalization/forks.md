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

## D9 · Presence = OCCUPANCY; the overhang moves into the walk (user challenge, 2026-07-29)

The R1′ capacity worry came from TODAY's semantics: extent bucketing (a caster registered in
every tile its tilted card spans — the I-7 fix), where overlapping extents exceed 3 in a
forest. The user's model — one slot per OCCUPANT (tile/thing/pawn, ≤3 by construction) — is
the better fit for D7, with the walk (and the receiver scan) checking `ceil(TILT ·
maxCardHeight)` rows SOUTH of each visited tile instead: the only tiles whose occupants can
lean over it. maxCardHeight is content-derived (conifer span 2 → +1 row). The trade prices
in D7's favor (cheap flagged u32 reads vs record fetches; incremental fill). The occupancy
probe is retired — capacity is bounded by construction.

## D9 refined · Base-LINE registration, not single-tile (user, 2026-07-29)

A caster registers in its anchor row × EVERY column its base occupies (a 2×3 tree in both
base tiles). Single-tile registration would silently drop casts from the wide half: the
walk's ±1 dilation is perpendicular to the RAY's dominant axis — a horizontal ray dilates
vertically and never looks a column sideways — so width coverage cannot ride the walk.
Width stays FILL-time (base line); only the vertical tilt overhang moves to WALK-time
(southern dilation). Capacity unaffected: base occupancy is exclusive per tile.
