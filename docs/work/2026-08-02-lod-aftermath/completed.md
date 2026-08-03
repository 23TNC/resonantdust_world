# Completed — LOD aftermath

## 2026-08-03 · tree clipping reopened and fixed — the variant rides the STEM (I3)

The user's eyes reopened the clip: every conifer drew the CANONICAL master cropped by
VARIANT 0's sapling rect (the tightest in the set), scale-to-fit enlarged — clipped
edges, oversized, and all nine variants identical. Cause chain in
[I3](issues.md#i3): `thingTexture` spoke the grid dialect (canonical stem + cell),
`resolve()` drops the cell on a non-grid entry, ingest reads `#0` alone — while
variants became stem-addressable FOLDERS (human-pawns P1).

Fix mirrors `moverSlotTexture`: `thingTexture` routes `<base>/<v>/e` via a `has`
probe; `registerThingSubframes` keys each mastered variant's rect on its own stem at
`#0` (linked/grid kinds keep the cell dialect, probed by the new `resolver.gridOf`);
`onTexturesChanged` re-registers and repaints thing rows when a late manifest changes
the routing signature.

**Verified live** (foreground tab, `focus=100,50&zoom=1`): 455 flora/conifer prims
across 22 VARIANT stems (canonical folders on the bare stem, `conifer/1/e` correctly
absent); 9 conifer subframe keys all at `#0`, each its own rect, anchor `(0.5, 1)`;
screenshot + zoom show varied, WHOLE trees — canopy tips, side branches, trunk flares
intact, shadows matching each variant's silhouette; pool 1 page / partial 0 (the boot
warnings were P0's bounded retry healing a cold edge cache, ledger empty after).
Typecheck clean.

## 2026-08-03 · P1–P2 — shadows dim instead of delete; the one depth key lands

**P1, isolated then fixed**: new drill toggles `__shadows(on)` / `__ndotl(on)`;
shadows-off removed EVERY dark band at zoom 2 — the "clipped canopies" were 100% the
occlusion term rendering shadowed texels at ZERO contribution (with one dominant torch,
that is bare ambient ≈ black). Fix: `SHADOW_KEEP = 0.35` — an occluded texel keeps 35%
of the light, so shadows GRADE instead of voiding; the after-capture shows soft
silhouette shadows and whole canopies at every tree. Tunable by eye in one constant.

**P2, the ONE depth key (F2)**: `groundContactY/Row` exported from SquareCache and
consumed by the record anchor (recordSync) and the zdepth painter's lane; the bake/slot
paint orders are bound by the contract note (they encode the same row at creation).
The BACK-CASTER compare landed in the refine: a billboard receiver skips any caster
whose ground-contact row is north of its own (that shadow belongs to the never-drawn
back face). The unlit overdraw case CLOSED as reduced-to-P0 — textured flora renders
correctly behind canopies; the black fill had erased the depth cues.

**Full-footprint presence**: prims register in every tile their drawn box overlaps
(1050 presence tiles at the fixture vs ~460 base-row entries), which let BOTH dilation
loops collapse to zero — 15→1 presence fetches per walk step, 3→1 per receiver texel —
with `walk = brute` still exact at dilation 0, `droppedReceivers` 0, occupancy 355,
`__lightexact` bit-identical, and an unchanged render.

## 2026-08-02 · P0 — flora's colour returns

**Coordination** ([I1](issues.md#i1)): render-performance's I11 landed; its
complete-on-arrival items open and idle ~5 h → custody taken for exactly that bug,
custody note written into their issues (I12), the rest of their charter untouched.

**The fix** ([I2](issues.md#i2) for the mechanism): `ensureCoPack` now records a
PARTIAL pack (a listed map's bytes missing — the transient-404 class that black-baked
flora for a whole session) in a `partialPacks` ledger and schedules a bounded
TIME-DRIVEN retry (3 attempts, 2 s ×2 backoff). Time-driven matters: the first
implementation triggered from `resolve()` and PARKED FOREVER once the bakes drained —
caught in the drill, rebuilt on a timer. The old frame keeps serving during a retry
(no geo flash); a successful re-pack replaces it and emits (targeted re-bake + defs
re-write). `poolStats()` gains `partial`; the HUD shows it.

**Verified live** (driven frames; foreground-tab rule respected): cold cache-less load
→ flora TEXTURED on screen, layers row reads max 255 on the graphics twin, partial 0.
Outage drill (in-page 404 window on flora's layers — the disk-delete form can't 404
because the edge serves its derived cache): partial 1 during the outage with the world
rendering progressively; outage ends → the 2 s timer re-packs → partial 0, layers row
255, no reload. Bound respected: three failed retries stop with the counter raised.
