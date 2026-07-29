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

## 2026-07-28 · P1 — the perpendicular card in the walk (4/4) + P2 guards (1/2)

The arm landed as `casterCoverNS` (GATHER_COMMON, before `casterCover`): centre ray L→Q
intersects the vertical plane x = centerline once; card coords s = n–s offset across the side
frame's tight width centered on the anchor (D2), t = height; caster_flip rides sampleCard's
proven rot==3 mirror; penumbra = the SAME interval arithmetic with du/dλ LINEARIZED at the
centre ray (u(λ) is rational on this card — same approximation class as the main card holding
t fixed). `casterCover` branches to it for rot 0/2 + caster_valid; lod<4 returns the solid
interval fraction (rotated-quad fallback). `casterOne`: gate (1) — the seen-face north/south
band — is SKIPPED for perpendicular casters (F1: an e/w-throwing card has no n/s seen-face
asymmetry to protect; gates 0 and 2 stand). `buildCasters` buckets the rotated ground trace
(centerline col ±1 × side-width rows centered on the anchor row). **Verified live** (synthetic
hot rot-2 caster — `warmAddPrim` with conifer/n drawn (loose) + conifer/e side frames — beside
a boosted `__torch`): westward side-profile shadow with the caster WEST of the torch, FLIPS
east when moved east of it (no special casing — the ray–plane solve), penumbra widens with
distance; **corridor↔brute 0 mismatches / 524 288 words** at rot 2 AND rot 0 (flip=1 stamped);
zooms 1 / 0.5 / 0.25 keep the shadow live with the caster def following the lod (52→129→92,
valid=1 throughout); **120.2 fps** at zoom 1 with the npc walking. HONEST GAPS: the s-facing
mirror was verified structurally (flip bit + identity), not visually (a conifer profile is
near-symmetric); the wolf-eyes drill is BLOCKED (see blockers — the concurrent art-128
session's in-flight wolf texture rewrite broke the wolf sprite render entirely; bisect proved
it environmental: identical on 9bd328e-era code).
