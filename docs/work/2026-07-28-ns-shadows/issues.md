# Issues — ns-shadows

_Defects found during execution land here. Known input: the P4-era conservative fine receiver
classification treats HOT billboards specially — if the rotated caster changes which texels
carry the mover's shadow, re-verify the no-self-squares result from hot-sync P4._

## I1 · `addPrim` dropped `rotation` — a RESTING n/s mover cast as e/w (found at P0 probe)

The P0 acceptance probe showed the resting n-facing wolf with `rotation: undefined`: the
field was declared on `Primitive` (pawn-render P4) and set on the MUTATION path, but
`SquareCache.addPrim`'s EXPLICIT field copy never gained it — the exact gotcha the pawn-render
memory warns about (`hot` hit it first). Every mover that had moved was masked (the mutation
path stamps it); only never-moved movers regressed. Fixed in the P0 cut; the probe now shows
rot=2 on the resting wolf. Rule stands: a new `Primitive` field MUST be added to `addPrim`.
