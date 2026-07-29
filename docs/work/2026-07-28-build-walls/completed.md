# Completed — build-walls

_Dated entries, appended as items land: what landed and how it was verified._

## 2026-07-28 · P0 — content + the linked-cell formula (3/3)

`wall_smooth` authored in BOTH corpus files (append rule — no def_id renumbers): the
`:data @define` hook carries the new `tile.build` lane (`"wall &tile.build set`), the
`:visual` hook binds `biome-tile/default/smooth/wall`. Loader grew `tile_build`/`tile_builds`
(the generic data-store read, thing_speed's sibling), wasm grew `tileBuilds`. The cell
formula lives in `game/world/linkedCell.ts` — cell = y·4 + x with x = N+2E, y = 3−(S+2W) —
with the user's 16-row table VERBATIM and `assertLinkedCellTable()` run at BOOT (contentBoot
import; throws on drift — no test runner exists, so the pin is a boot assertion).
`buildMenu.ts`: the category scan (D3) + `blueprintStemFor` (F1: material segment →
"blueprint", convention not authoring). **Verified live** after `rd redeploy --run` (edge +
shared wasm + webgl): `tileBuilds()` = [,,,,,"wall"], names[5] = wall_smooth (def 6), stem
bound, and the CLIENT manifest entry for `…smooth/wall/l` carries grid [4,4] + all four
maps; the boot assertion passed (page loads). The "second kind without client code"
sub-check is structural (the scan reads the table) — not fixture-drilled.

## 2026-07-28 · P1 — linked-tile rendering (1/1)

The Phase-2 the pathway documented: `tilePrimSpec` (ONE spec builder shared by the baseline
row paint and the ground override) routes a linked stem (manifest grid for `<stem>/l`, via
the new `TextureResolver.linkedGridFor`) through `<stem>/l` + the D1 neighbor cell. The
neighbor context is a global `tileKindAt` map (drawn kind per tile, fed by both paint sites
+ tombstones); a kind CHANGE queues the cell and `processReCells` re-picks the 4-neighbor
ring's cells in place (`prim.cell` mutate + the new cold `Viewport.refreshPrim`) — global
map, so row and zone seams need no special casing client-side. **Verified live** (`__bridge`
debug global driving the same primitives the override path uses): a hand-painted L of
wall_smooth (94..98,48)+(94,44..48) crossing the x=96 zone seam — every probed cell EXACT
(corner N+E→15, ends W→4 / S→8, mids W+E→6 / N+S→9), texture `…smooth/wall/l`, and the
capture shows end caps, straight runs, and the corner joined correctly; intact at zoom 0.5.
The SERVER-row seam case (two zones' rows arriving separately) rides the same global map —
exercised for real by P3's event drill.

## 2026-07-29 · P2 — panel + mode + preview (3/3)

`BuildPanel` (`game/panels/build/`): one sub-tab per category from `buildMenuEntries`, icons
= the linked atlas's cell (0,1) as a CSS crop of the hash-addressed master URL (D6 — the
same art the world uses; a bounded retry applies the art when the async manifest lands).
Placement mode lives in WorldScene: icon click → crosshair + suspended selection; LEFT drag
forms the rect with `syncBuildPreview` rebuilding per move; RIGHT click exits (verified:
crosshair on, cleared on right-click, selection functional after). The preview is the new
`BlueprintOverlay` (F2 — a dedicated textured-quad pass, NOT cold-cache prims: zero bake
churn, clearing = dropping the list; unstreamed cells draw flat translucent blue): the
blueprint linked atlas, variant-aware against the preview shape (D1 CPU-side). **Verified
live**: the drag showed a correctly-jointed translucent wall ring (capture); nothing hit the
server until release.

## 2026-07-29 · P3 — the BUILD_WALL event end-to-end (3/3)

Docs FIRST (ACTIONS.md row + §-note; VARIABLES needed no change — operands are generic u32
slots and positions reuse `position_anchor_reference`; docs-check green). `BUILD_WALL = 9`
in codec (`[Imm, Imm, Imm]` — writes NOTHING; ACTIONS documents the queue-events shape),
edge allowlist, core `Command::BuildWall` + `build_wall_program` (bare — no PROMOTE; the
queued SETs carry it), wasm `buildWall`, WasmClient + WorldBridge passthroughs. The worker's
BUILD pass mirrors the movement-continuation pattern: expand the rect PERIMETER, queue one
`PROMOTE SET cold_row(macro,0,0) TYPE_BIOME_TILE tile kind 0` per tile at the next tic —
each SET routes to ITS zone (multi-zone rects safe by construction); best-effort like the
continuations. En route: web.rs's SECOND Command match needed the arm (E0004 caught it).
Redeployed: all modules + edge + shared + webgl (`rd redeploy --run`) + worker/orchestrator
rebuilt + restarted (shard data wiped — the npc re-created its wolves).

## 2026-07-29 · P4 — the drill (1/1) · STREAM DONE 11/11

Icon click → crosshair → drag (102,44)→(106,48) → translucent blueprint ring → release →
**worker log `build_wall expanded x0=102 y0=44 x1=106 y1=48 object=6 queued=16`** → 16
overrides fanned back → every probed perimeter tile kind 6, interior untouched (grass 1) →
the wall ring RENDERED with correct joins → **survived a full reload** (16 overrides
re-delivered from the shard) → **120.2 fps**. Honest bounds: tees-at-overlap reuse the P1-
proven neighbor math but two overlapping rects weren't separately drilled; the multi-zone
rect is safe by construction (per-tile SET routing) but a rect spanning zones wasn't
separately drilled; the user's hands are the final oracle.
