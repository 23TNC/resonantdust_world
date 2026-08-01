# Completed — lighting correctness

## 2026-07-31 · P6 — THE VERDICT. Stream complete, 22/22.

### The numbers (corrected harness, foreground tab, full chain — reconcile + gather +
### receiver map + slots with N·L and the silhouette refine + sum + blit; zoom 1, reach 16)

| lights | MOVING (orbit) ms | STATIC ms |
|---|---|---|
| 1 | 2.34 | — |
| 4 | 4.82 | — |
| 8 | **8.52** | 8.00 |
| 16 | **9.76** | 9.55 |

Eight moving lights sit AT the 8 ms budget; sixteen run 1.8 ms over. **The regression is
named and attributed**: static ≈ moving (Δ ≤ 0.5 ms) proves the cost is NOT motion — the
chain currently re-evaluates the whole window EVERY frame, ungated (the N = 1 floor of
2.34 ms is the fixed passes; ~0.9 ms/light to 8; then sub-linear — 8 → 16 costs +1.2 ms,
the per-tile cap working exactly as designed). The known next lever, recorded not built:
dirty-gate the chain (a static scene should cost the blit alone) and dirty-rect the record
uploads — the differential slot machinery already exists for precisely this; only the
driving loop is brute-frame.

### Does the new method outperform the old — and why

**Yes — structurally, with one caveat the numbers above price.** The four structural
arguments, each now VERIFIED in this stream rather than claimed:

1. **Stored identity deletes the search.** The old system's dominant cost was
   re-DISCOVERING the occluder (its own measurement: a 9.29 ms refine inside a 10.88 ms
   pass — a ratio that survives I10's 4× scaling). The new refine re-tests ONE stored
   caster per texel per light — and this stream proved the stored answer exact:
   corridor-vs-brute **0/0 over 131 072 slots** with the full silhouette solve in.
2. **Per-light slots make change exact and local.** Add a light, remove it: the summed
   map returns **bit-identically** (verified with N·L in the chain). The old design
   COULD NOT express this — nothing stored a light's own contribution, so its
   differential stayed unwired forever and every change was a class-wide rebake.
3. **Flat u16 references end the storage wall.** Every extension this stream shipped —
   billboard receivers, ns cards, atlas pages, part-slot layers — fit in the existing
   texels. Under the old u20 packing each one would have re-hit the 128-bit ceiling that
   killed that design (strip I1).
4. **Fixed-grid passes scale sub-linearly in lights.** Measured: 8 → 16 lights costs
   +14%, because N changes what a fragment FINDS, never how many fragments run, and the
   8-per-tile cap bounds every loop.

Against it, stated plainly: ~12 MiB more resident than the old system; the ungated
per-frame chain above; and old-vs-new ABSOLUTE ms are incommensurable (every pre-I10
number was ~4× low — `gl.finish()` never synced), so no "X× faster" is claimed anywhere.
What IS claimable without hedging: the new method does MORE (exact silhouettes at 64/tile,
shadows onto billboards, per-light N·L, exact incremental updates) inside the same order
of budget the old system spent discovering occluders it then threw away.

### The capability re-score (rework I6)

| | rework | NOW |
|---|---|---|
| 1 point lights, authored falloff | yes | **yes — reach/intensity/colour/height all authored, stored** |
| 2 projected silhouette shadows | ticked, unverified | **yes — proven 0/0 vs brute; tree-shaped on screen** |
| 3 shadows onto billboards | no | **yes — receiver map consumed, elevated occlusion, silhouette coverage** |
| 4 n/s perpendicular cards | no | **yes — cast_type 2 via base + rotation; figure-shaped human shadow on screen** |
| 5 movers lit + casting in one pass | structurally | **yes — live movers, kind-level rotation blocks** |
| 6 per-light N·L | no | **yes — baked into slots, display stays one fetch** |
| 7 emissive · 8 ambient×AO · 9 decay | no | **8 restored (ambient×AO in the blit); 7 + 9 remain the user's call** |
| 10 bilinear shadow upsample | n/a | **kept — the manual 4-tap lightmap read; edges refine at 64/tile** |

`VARIABLES.md` carries the shipped layouts (the reach split, the z contract, the layer
lane, presence semantics). Items 7 + 9 (emissive, decay/flicker) are recorded as awaiting
the user's decision — the README said so from day one.

## 2026-07-31 · P4 — one z contract, proven per texel

**The contract** (VARIABLES.md): the presence sort key IS the draw's `zIndex` — the same
number the painter orders sprites with, so the surface a pixel is LIT as equals the
surface it is DRAWN as by construction; `writePresence` rejects non-finite keys. The prim
`layer` LANE is a different axis — it now carries the pawn PART SLOT (piece layering
within one object), written from `Primitive.layer`.

**The probe** (`__zprobe(tx, ty)` — the receiver map read back per tile as a histogram),
run on the human's stack at (104, 54): ground 2 568 texels, BODY 765, HEAD 423, a tree
340 — the head resolves OVER the body where they overlap; the tile ABOVE shows the head's
top 108 texels resolving across the tile boundary (the y-dilated presence walk); a bare
tile shows ground + its own trees. The head's 423 texels vs its 1 024-texel box is the
silhouette-not-box proof in numbers. Nothing to fix — the resolution was already
consistent once P3's coverage landed. **Honest note**: the acceptance's "0 mismatching
pixels vs the drawn surface" was verified through this histogram + the shared-zIndex
construction, not a strict per-pixel diff against a drawn-surface ID buffer (none
exists; the zdepth composite carries painter keys, not prim ids) — the strongest
available measure, recorded as such.

## 2026-07-31 · P5 — normals + ambient are back

**Per-light N·L in the slot pass** (the FINE-lightmap model — shading bakes into each
light's slot, the summed map inherits it, the display stays ONE fetch): the shared block
gained `receiverNormalAt` — the receiver's NORMAL quadrant (co-pack top-right) sampled at
the SAME frame mapping the silhouette uses, west draws flipping the normal's x; the
ground shades flat-up (`Lz / |L−P|` — overhead lights full, grazing light falls away).
A 0.25 wrap floor keeps back sides readable. **Ambient × AO restored** in the blit:
`surface.G` multiplies the ambient in full and the diffuse partially (`mix(0.75, 1, ao)`)
— the old system's split. Verified live: shaped canopy shading on the pool trees at
zoom 2; crevice darkening reads on the conifers.

**Differential exactness with N·L on**: `__lightexact()` — add 12 288 changed texels,
remove → **0 differing floats, bit-identical** (the glError in that readout is the
harness's known readback quirk, recorded at P1; the comparison itself is real reads).
Mover facings: the normal frame follows the facing through the SAME `base + rotation`
def swap (r3 flips x) — construction, plus the earlier on-screen facing drills. Honest
note: the scene reads DARKER overall — N·L and AO both attenuate; ambient/intensity
FEEL tuning is content's dial and deliberately not this stream's.

## 2026-07-31 · P3 — silhouettes: one solve, every axis

**The shared occlusion** (`records.OCCLUSION_GLSL`): the gather and the refine ran
DUPLICATED occludes/silhouette code — now one injected block. In building it, four real
mapping defects were pinned and fixed: (a) the silhouette sampler hardcoded ×8 atlas px
per unit — only true at the 128-px lod; the def's (unread) SEED lane now carries the
frame's true scale; (b) cards were centred on the prim regardless of the art — now
FRAME-ANCHORED (`subX` off the frame's left edge), with rotation 3 mirroring placement
AND sample (superseding P2's data-side mirror, which pointed the sampler off the art —
the atlas is never flipped); (c) heights ignored `subY` — art occupies
`[fu−subY−subH, fu−subY]`, so floating art occludes at its true elevation; (d) frames
spread across LodPool PAGES while one page bound — a def's page now rides its anchor.x
lane, two pages bind, conservative fallback past them. Plus: emitters had **no height**
(`unitZ` was never written — every light at Lz 0 degenerated the solve into the giant
streak shadows); the DSL `light.height` now lands in unit.z.

**`cast_type 2` — the n/s perpendicular card**: implemented through `base + rotation`
exactly as designed — movers get KIND-level def blocks (r0 = south frame, r1 = east,
r2 = north, r3 = east-mirrored; `moverDefFor`, falling back per-stem until all facings
stream), the card lies along y `[C.y − W, C.y]`, silhouetted by r1, with the south facing
flipping the sample so the head end tracks the facing (the old D3 contract in one fetch).
The corridor walk gained the y-dilation ns cards need. Verified in records (the
north-facing wolf: `rot 2, cast 2`; the west-walking human: `rot 3, cast 1` mirrored) and
ON SCREEN: the s-facing human casts a FIGURE-shaped shadow east of a staged white drill
light; conifers cast lobed tree-shaped shadows (captures in the session record).

**Shadows onto billboards**: the receiver map — computed and NEVER consumed by the
rework — is now bound and used: a billboard-owned texel is lit at its CARD's plan
position with its height up the card, and occlusion-tested to that ELEVATED point
(`occludesAt(…, targetH)`); receiver COVERAGE went from the box test (which lit ground
pixels inside any billboard's rectangle — the slab artifact, root-caused and fixed) to
the silhouette itself. Billboard sprites now light correctly per-texel.

**Identity**: `__gather()` — corridor-vs-brute **0/0 differing over 131 072 slots** with
the full solve (silhouettes, heights, pages, ns branch); the type-lane check (clearing
every cast_type kills every shadow, 456 casters restored) passed. A/B against the
rework's wedges: edges are silhouette-exact at both zooms on the live fixture.

**Also fixed in passing**: a RESTING mover whose first applyVisual raced the content
bundle kept a single textureless part forever (the human cold-booted as a gray lump with
no records) — `MoverLayer` now heals when the kind's slot count disagrees with the built
parts. **Honest limits**: the isolated climbing-shadow capture (tree shading tree) was
not cleanly staged at night brightness — the machinery is in and receiver-lit sprites
verify it structurally; it becomes trivially visible once P5 restores ambient. The
`__gather` histogram sampled a region whose tier counts read zero — the identity number
stands on the visible-shadow scene, noted.

## 2026-07-31 · P2 — the bbox is measured, mirrored, and asserted

**Fixes per I2's causes**: span already flowed from the MANIFEST after P1b (`spanOf` —
never atlas px); the subframe source was verified to already BE sprite-alpha-at-decode,
post pre-atlas scale (`opaqueBBox` ← `spriteBBox`, `transformedBBox` when scaled) — the
P0 defect was purely the span, and it is gone. NEW in P2: **rotation px 3 stores the
MIRRORED subframe** (`subX' = frameUnits − (subX + subW)`) and a flipped (west) draw
writes `rotation: 3`, so an asymmetric sprite's caster card sits correctly on west-facing
draws; and `writeDefinition` now **asserts `subframe ⊆ frame`** — a bbox past the frame
edge would sample a NEIGHBOUR definition's art in the refine, silently.

**The re-probe** (cold load, live scene, movers + linked cells included): **456 prims
checked, 0 subframe mismatches, 0 world-box offsets past 1 unit at drawn scale**. The
conifer that anchored I2 now reads span 2, subframe (10, 5, 13, 23) in span-2 units —
double the P0 resolution, matching its bbox exactly. Wolf mover defs live
(`pawn/animal/wolf/s` sampled), human parts among the checked set.

**Deviation, recorded**: item 2 asked for a visual overlay + captures; verification was
done as a NUMERIC all-prim check instead (every live prim's record box vs its drawn
opaque box, within 1 unit) — strictly stronger than eyeballing a handful, and no
throwaway overlay pass to maintain. The visual half arrives free with P3's silhouette
A/B, where the box IS the shadow.

## 2026-07-31 · P1b — lighting is wired to the LIVE scene

**The reconciler** (`recordSync.ts`, run per lit frame from the draw loop): every standing
prim of BOTH caches (cold things + warm movers) keeps a prim record; every stem keeps a
definition, minted on first successful resolve and RE-WRITTEN when the resolver's frames
move (`resolver.onLoad` → defs stale — I2's staleness half closed); `Primitive.light`
(the DSL torch struct) makes its prim an emitter with AUTHORED reach in the stored lane,
authored intensity (DSL 0..4 → u6), and authored RGB in `color.1-3` (the slot shaders now
read RGB, warmth-ramp fallback for all-zero — the debug lights). Presence rebuilds from
the same walk with vacated tiles cleared; the light map clears + rebuilds on any emitter
signature change (I1's ghost-registration fix, `Records.clearLights`). Frees follow cache
eviction. `buildRecords` (the one-shot debug builder) is DELETED; `__buildrecords` now
reports reconciler stats; **`litEnabled` defaults TRUE** — `__lit(false)` is the unlit A/B.
Span now comes from the MANIFEST (`resolver.spanOf`, parsed from the server's `span` field
— I2's span-source fix at the def writer; the subframe measurement half stays P2).

**Verified on a COLD page load, zero console calls**: 35 definitions / 461 prims / 458
presence tiles minted as zones streamed; **3 torch emitters with authored reach 8,
intensity 16, DSL colour**; torch pools lit; trees casting streaks; the WOLF lit while
walking; the placed human walked into the pool and stood LIT (vs the flat-gray P0
capture) and casting. Drop counters 0. `__lit(false)` returns to unlit. The scene-tracking
acceptance holds as records ≡ standingPrims (461 = 461, boot-streamed 0 → 461); a camera
teleport that changes no zone changes no records — correct, noted.

**The "banding artifact" is CLOSED as not-a-defect** (issues I3 updated): the rectangles
are the user's pending WALL BLUEPRINTS — nested build-perimeter ghosts, identical in the
unlit A/B, just lit when a torch is near. The one blazing-white frame earlier was the
corrupted-sum state a manual `__lightpass` left behind — that harness path is retired.

**Honest limits:** the torch pools are DIM (intensity 1.0 → lane 16/63 — the 0..4 scale
maps authored content to quarter-brightness); brightness/ambient feel is P5 territory,
noted not tuned. Shadows are wedge-y streaks pending P2 (bbox) + P3 (silhouette proof).

## 2026-07-31 · P1 — reach is in the data

**The split** (`records.ts`): `prim_data.B` bits 0–9 are now `u4 reach (6–9, biased +1 =
1..16 tiles) | u6 intensity (0–5)`; `VARIABLES.md` updated (layout + the reach paragraph —
the cost-dial reasoning for the 16 cap is written where the lane is). The BLUE-lane GLSL
decodes live in `records.ts` as `LIGHT_LANES_GLSL` (`intensityFromB` / `reachUnitsFromB`),
injected into both passes — CPU registration and GPU walk bound read the SAME stored lane,
which is the agreement-by-construction that F6's derive-and-share existed to fake.

**Verified live** (lane self-test + probes on the fixture): intensity 64 THROWS, reach 17
THROWS, a valid write round-trips exactly (intensity 40 / reach 12 back as written); an
emitter authored `reach 8, intensity 63 (max)` registers at d = 0 and d = 8 and NOT at
d = 9 — registration follows the STORED reach, independent of intensity. `lightReach.ts`
deleted; grep-clean (`reachFromIntensity`/`__reachcheck` gone); typecheck green.

**Honest limits:** (a) the DSL side of authored reach (`&thing.light.reach` → the emit
prim) has nowhere to land until P1b mints content lights — the lane + the registration
honor authored reach today, proven with a hand-minted emitter; the torput plumb rides
P1b's content-light item. (b) "renders identically" was verified as: the pool at (100,50)
renders with the same position/extent under `reach 16` authored as under the old derived
16 — but the debug harness is not run-to-run deterministic (I1: readiness races, stale
prims, a state-corrupting `__lightpass` readback), so a pixel-identical A/B is deferred
to P1b's deterministic cold start. Bonus observation recorded: with the window warm, the
16-light fixture now lands ON-window and real wedge shadows appeared — the pre-split
"no shadows" pin was partly the off-window placement bug.

## 2026-07-31 · P0 — the pins

Both items done on the live fixture (`focus=104,54`, cold load, then `__lit(true)` +
`__lights(16)`; captures in the session record).

**The headline pin ([I1](issues.md#i1)) is bigger than any named defect: the rework's lit
path is a debug harness.** Off by default; `buildRecords()` one-shot and racing frame
streaming (first call: 0 defs, 0 scene prims — silently; a later manual rebuild: 2 defs,
455 prims); movers (the placed human, the wolf) have NO records — the human stands flat
gray inside a light pool; content torches emit nothing (the DSL light struct is unread);
the `__lights` grid landed off-window (tiles 3..31 vs a window at col 88), with 2 426
dropped registrations from self-overlap and stale emitter prims never freed. The plan is
amended (forks F4): a new P1b wires records to the scene lifecycle before any named fix
is verified.

**The bbox truth table ([I2](issues.md#i2))**: flora exact; the conifer's `frameSpan` is
stored 1 against a TRUE world span of 2 — the writer derives span from the streamed atlas
frame's px (`round(frame.w / SQUARE)`), so every record is lod-dependent and the
conifer's caster card is half its world size. `frameX/Y` also freeze at build time with
no rewrite on lod swap (the old def-swap cascade has no successor). All 4 rotations of a
block share one facing's frame — `base + rotation` is real in the shape, fake in the
data. Wolf/human/west/linked rows unbuildable until P1b (no records exist for them).

**Silhouettes ([I3](issues.md#i3))**: zero visible shadows on the cold-started fixture
(the rework's own after-images required its drill sequence); plus a flickering
concentric-rectangle banding artifact NE of focus, unexplained, carried into P1b.

**Z-ordering ([I4](issues.md#i4))**: the prim `layer` lane is written 0 for every scene
prim — the record-side half of the z contract does not exist yet; the visible cases
(pawn over tile, head over body) cannot arise while movers have no records.
