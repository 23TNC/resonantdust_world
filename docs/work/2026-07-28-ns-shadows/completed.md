# Completed — ns-shadows

_Dated entries, appended as items land: what landed and how it was verified._

## 2026-07-28 · P0 — record plumbing (3/3)

`billboard_data.A` claimed in VARIABLES.md FIRST (caster_definition_id 16–31, caster_flip 15,
caster_valid 14; docs-check green), then `billboardDataFor` grew an optional resolver param
(both callers pass it) and stamps A for rotations 0/2: the side def resolves by swapping the
texture name's facing segment for `e` through the SAME immutable `definitionFor` path (cached
by key; a missing side frame degrades to the loose lod-0 def naturally). Fast path compares
the A mirror word. **Verified live**: the n-facing wolf's record carries drawnDef 68 (`/n`)
AND casterDef 69 (`/e`), valid=1, flip=0; e/w rotations carry A=0; a zoom 1→0.25→1 transition
swapped casterDef 69→106→69 with the dirty counter firing (+20). EN ROUTE (I1): `addPrim`
never copied `rotation` — the pawn-render P4 explicit-copy gotcha AGAIN — so a resting n/s
mover derived the e/w regime until its first move; fixed in the same cut (field added to the
explicit copy), verified by the same probe (resting wolf rot=2).
