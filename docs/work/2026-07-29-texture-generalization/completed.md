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

## 2026-07-29 · P2 — tiles enter presence (2/2)

**Linked defs from content**: `tileDefinitionFor(kind, stem, lanes)` in coldShadowData —
ONE def per (KIND, lod), cell-free (keyed by kind, not stem: two kinds may share a stem
with different lanes, and the lanes live in the def words). Linked stems: frame = the
WHOLE atlas (`resolve(stem/l, surface)` no-cell = the whole-atlas quadrant), span = the
grid's world tiles, W/H = one cell's window (16 − 2·pad), offset = the pad inset,
TOP-LEFT anchor; plain stems are the degenerate 1×1/pad-0 case; `white` (flat-coloured
ground) mints the lod-0 LOOSE def — pure geometry + lanes, all a flat receiver needs at
P3. Authored lanes ride R/G (type, rotation-as-MODE, cast bit 24, pad, receives). NO
tight box / maxTightHpx bump (tiles author cast=0; wall shadows revisit). **Verified
live** (`__tiledef` probe): wall (102,44) reads span 4 / W·H 14 / pad 1 / offset (1,1) /
rotation 1 / cast 0 / receives 1 / type 1 / lod 9 whole-atlas frame; ground reads loose
lod-0 / receives 2; def keys `tile:6|0` + `tile:6|9` = one def per lod, not 16 per-cell.

**Slot-0 writer**: the bridge SHARES its live tile-kind map by reference
(`setTileKinds` — kind map + stride-6 lane table + stems + type id); `buildCasters`'
window write fills slot 0 per tile via a per-kind per-pass memo (zero allocation).
Billboards re-spec to slots 1..3; all THREE GLSL readers (both walks + receiverAt) get
the new contract — slot 0 may be empty (`c==0 continue`), dense break from slot 1, and a
`set == billboard_data` filter so tile slots are skipped BEFORE any fetch (the cast-flag
cull already precedes record fetches). Also fixed en route: `castSlots` was length-checked
against BILLBOARD_SLOTS but sized by PRIM_SLOTS — reallocating every frame. **Verified
live**: probes over ground/wall/empty read the right words (`40000022` / `20000047` / 0);
the `__tileslots` A/B from a clean post-rebake baseline is BIT-IDENTICAL in both classes
(cold on↔off 0, on↔on 0; hot 0/0; wolf frozen via npc stop). NOTE (observation): a hot
mover freezing leaves ~23 cold words stale until the next rebake touches them — pre-
existing behavior, surfaced by the oracle's settle discipline, not introduced here.

## 2026-07-29 · P3 — the receiver unification (D8) + autotile (2/2)

**receives_shadows modes**: the class pass now forks on the texel's RECEIVE MODE — the drawn
billboard's where one is drawn (billboards remain like-billboard until thing lanes are
authored), else the TILE's mode from its slot-0 presence flags. Mode 0 writes zero and
returns BEFORE the light loop (the flag early-out — unpainted tiles stop computing shadows
entirely); mode 2 is the old flat walk; mode 1 without a billboard (a WALL tile) treats the
TILE as the standing receiver — base at its south edge, climbing its own height, whole-texel
receiver (no silhouette/straddle), and the isThing walk gates (seen-face, light-side) apply
to it. The old unconditional-ground model is exactly this fork with every tile hardwired to
mode 2. The ON-BILLBOARD CUT is RETIRED (both the cold cut and its delta-mode mirror):
billboard texels bake the climbing shadow — never the ground pool — so the cut has no job
left; the prim pass's fine overwrite owns the silhouette edge (same argument that deleted
the edge-refine). **Verified live** (R3′ comparative): world renders correctly, tree
shadows still climb trunks/canopies (capture), no shader errors; PERF — idle 8.33 ms avg /
8.4 p95, full-rebake-EVERY-FRAME 8.33 avg / 8.5 p95 over 240 frames: both vsync-capped at
120 Hz, zero measurable cost from the mode fork + per-texel presence fetch.

**Autotile in-shader**: `tileConnects` + `tileAutoCell` in GATHER_COMMON — the D1 formula
(x = N+2E, y = 3−(S+2W), cell = y·4+x) over the 4 neighbors' slot-0 defs; connected =
rotation-MODE-1 match (the variant/kind gate is future per R4). CPU mirrors
(`tileConnectsMirror`/`tileAutoCellMirror`) read the SAME dataMirror bytes the GPU sees.
Dirty ring: `recordTileKind` → `viewport.tileKindDirty` → a 3×3 both-class rect, riding the
same queue light moves use — neighbors self-heal on the next bake. **Verified live**
(`__autocell` probe): all 11 wall tiles' mirror-computed cells EQUAL the drawn prims' cells
(corners 10/13/0, runs 6/9). The GLSL copy is consumed at P4 (tile lighting samples).

## 2026-07-29 · P4 — tiles lit + the blueprint + the drill (3/3) — STREAM COMPLETE

**Tiles lit with their def's normal**: `tileNormal` in GATHER_COMMON — slot-0 def → the
atlas CELL (in-shader D1 autotile for mode-1 defs, the frame as-is otherwise) → the
pad-inset window stretched across the tile square → the NORMAL quadrant (+side E, −side N,
billboardNormal's own layout). LIGHT_FRAG's normal path: billboard's where drawn, else the
TILE's; `applyNL = dot(pn,pn) > 0` — loose/plain tiles return vec3(0) and keep flat ground
exactly. **Verified**: torch-lit walls at zoom 1 + zoom 2 — the wall run beside the north
torch reads warm cream while the same kind mid-frame sits cool grey and ambient-only walls
darkest (lit-by-proximity demonstrated); wall bevel normals carry the direction (the
normal master has real content: R/G σ 52/57), flat tops read dim under horizontal torch
light — physically consistent; THE USER'S EYES ARE THE FINAL ORACLE on the feel. Ground +
tree shadows unchanged (captures). **Cold bakes only on tile change: 0 cold bakes over 5 s
idle** (hot churns with the wolf as designed).

**Blueprint lit**: the preview FRAG samples the LIVE cold+hot lightmaps + ambient with the
blit's own window mapping (read-only — accumulators untouched by construction); unstreamed
fallback lit too. **Verified mid-drag**: the perimeter preview beside the torch shows the
pool's gradient across it (warm on the torch side, ambient-dark away).

**End-to-end drill**: through the REAL UI — build panel icon → placement mode →
synthesized drag (106,57)→(109,60) with mid-drag captures → release → BUILD_WALL → worker
perimeter SETs → walls painted, autotiled, LIT on the new rails. The zone re-fan also
resurfaced ~66 older server-side drill walls the client had never painted this session —
all 65 in-window walls pass the `__autocell` probe (13 "mismatches" were out-of-window
probe artifacts — the torus slot belongs to another row there). Reload-stable across
multiple navigations (walls persist + relight). **fps: 8.33 ms avg (vsync 120 Hz)** idle
AND under full-rebake-every-frame — no regression budget consumed. Aborting a drag
(right-click) leaves no build; the first mis-calibrated drag was aborted cleanly.
