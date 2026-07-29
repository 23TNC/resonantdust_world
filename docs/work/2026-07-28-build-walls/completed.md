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
