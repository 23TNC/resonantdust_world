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
