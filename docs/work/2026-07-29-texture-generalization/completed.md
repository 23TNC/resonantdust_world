# Completed — texture-generalization

_Dated entries, appended as items land: what landed and how it was verified._

## 2026-07-29 · P0 — docs + DSL lanes (3/3)

VARIABLES rewritten FIRST: `prim_presence` (renamed; set-4 listing + the region-torus section
now define the 4×u32 slot — flags 31–28 (cast 31, receives 30–29, spare 28), set 27–24,
index 15–0, 8 reserved — the slot roster (tile/primary/secondary/pawn), the D9 base-line
OCCUPANCY relation + walk-time southern dilation, and the early-out re-spec); definition R
bit 24 = cast_shadows, G's reserved lane = internal_padding (u4, units) + receives_shadows
(u2); rotation documented as the per-type tile MODE; the stale duplicate `billboard_presence
(was caster_buckets)` textile-map paragraph REPLACED with a pointer to the one authority
(delete-don't-deprecate). DSL: every ground tile authors `cast_shadow 0 / receives 2`;
wall_smooth authors `linked 4×4, padding 1, rotation 1, cast 0, receives 1`; loader + wasm
expose `tileLightingLanes` (stride-6). `maxCardTiles` derives from content in refreshStems
(max thing size + tile height → 2 today) → `ShadowGather.setMaxCardTiles` +
`dilationRows()` = ceil(TILT·max). **Verified live**: the lanes probe reads ground
[0,0,0,0,0,2] / wall [4,4,1,1,0,1]; the constant probes 2→2 rows and a taller fixture (4)
derives 3; docs-check green. EN ROUTE: the host/docker restarted mid-phase — spacetime +
gateway containers died (edge exited at boot: DNS for `start:3000` unresolvable), the npc's
restart policy did NOT retry after its exit; recovered with `rd up spacetime` + `rd up
gateway` + **`rd deploy gateway`** (the gateway container is a dev shell like the edge —
`docker start` alone runs nothing) + `rd deploy edge` + `FORCE=1 sim run npc`; shard data
survived (the wolf resumed trips at tic 53k). The vite dev server also died — restarted.

## 2026-07-29 · P1 — the presence reshape (2/2)

The 4×u32 slot reshape + base-line occupancy + southern dilation is VERIFIED bit-identical.
The long-running I1 residual (~250 words) turned out to be an artifact of measuring on a
page whose texture pipeline was silently wedged (previewCache IndexedDB hang — fixed en
route, `76e4bdb9`, along with the wolf's all-zero surface maps regenerated and the false
B1 art blocker retracted). On a healthy settled page the full oracle battery reads ZERO
diffs: cold — settle 0, round-trip 0, extent(+uDilateS 0, old-exact)↔base-line 0,
brute↔corridor 0; hot (npc stopped, wolf frozen; 9,980 nz words) — settle 0, extent↔base
0. The receiver-map diff also showed extent mode DROPPING registrations via 4-slot
overflow (2 full tiles; 1,908 fine receiver words) where base-line stays ≤3 — the new
model is a strict improvement, not just an equivalent. `__fillmode` kept as the A/B
re-verification lever for P2.
