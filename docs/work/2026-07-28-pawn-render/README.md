# pawn-render — depth-correct wolves, lit and shadowed through the HOT maps

_Work stream, opened 2026-07-28. Component: `client/webgl` (`game/viewport/` — Viewport,
SquareCache, shadowGather, coldShadowData, albedoBlitShader, mrtBakeShader; `game/world/`
MoverLayer/WorldBridge/thingPlacement). User's brief: a graphics + functionality pass on the
wolf pawns — they render on top of albedo and behind shadow-cold; they need proper DEPTH
(loaded into the mover-tier albedo/normal/etc. maps with per-pixel front/behind against
trees), then SHADOWS applied to the wolf billboards (n/s billboards differ from the e/w ones
already built), all landing in HOT maps so cold light is never recalculated, and their NORMAL
maps applied (expected free once lighting works — stated explicitly anyway)._

## The user's observations, confirmed in code (2026-07-28 map)

Every reported symptom has a located mechanism:

- **"On top of albedo"** — the display blit composites warm-over-cold by COVERAGE only
  (`albedoBlitShader.ts:83-90`: `wcov = surface-warm.B; alb = mix(cold, warm, wcov)`). Both
  tiers already BAKE a `zdepth-world` front/behind key (`0x80 | baseRow & 0x7f`,
  `Viewport.ts:199-205`) — but its B lane is **read by nothing**. A wolf behind a tree paints
  over the trunk inside its own silhouette.
- **"Behind shadow-cold" / no lighting / no normals** — the wolf IS multiplied by the
  lightmap, but by the texel baked for the GROUND (or overlapped tree) at its world position:
  the lightmap bake only ever sees cold prims (`Viewport.ts:398` passes
  `this.map.standingPrims()` — the cold cache). No mover receiver entry, no mover N·L, no
  mover atlas-normal sample. So the wolf wears whatever the terrain under it wears — the
  ground's shadow term darkens it wholesale, and its own normal map is never consulted.
- **No cast shadows** — movers are absent from the caster buckets for the same reason
  (`shadowGather.ts:1650` buckets cold standing prims only).
- **Emissive works** — correct: the blit reads `uDepthWarm.r` for the emissive mask
  (`albedoBlitShader.ts:123`); it is the ONE warm lane consumed today.
- **"N/S billboards cast e/w"** — the shadow system has no n/s concept at all: caster records
  hard-code `rotation = flipX ? 3 : 1` ("both E/W regime for now",
  `coldShadowData.ts:475`), and `casterCover`/`sampleCard`/`billboardNormal` know only the
  e/w card + mirror. An n/s-facing wolf casts its side-profile silhouette.

## The tier rule (user, restated 2026-07-28 — ALREADY ratified in [hot-shadows](../hot-shadows/README.md))

| light | prim | writes to |
|---|---|---|
| cold | cold | `*-cold` map (baked, rarely) |
| cold | hot | `*-hot` map (per frame) |
| hot | cold | `*-hot` map (per frame) |
| hot | hot | `*-hot` map (per frame) |

**If the light OR the prim is hot, the result lands in a HOT map.** Cold×cold is the only
baked cell — moving a wolf must never dirty `lightmap-cold`. The machinery half-exists: the
gather/light pass already runs as TWO class passes with separate RTs
(`coldLightRT`/`hotLightRT`, `shadowGather.ts:2161-2168`) and the blit already sums
`ambient + coldLight + hotLight`; what's missing is prims (not just lights) carrying
temperature — the mover tier participating in the HOT pass as receivers, casters, and
N·L surfaces.

## Design sketches (ratified as forks)

- **Depth ([F1](forks.md#f1))**: the blit compares the two zdepth B lanes it already binds —
  wrap-aware row compare (rows are mod-128; the viewport spans far less) — and the winner
  contributes albedo/surface/light. No new channel, no new bake: the key has been baked on
  both tiers since the warm cache landed.
- **Mover light/shadow data by UNIFORM ([F2](forks.md#f2))**: warm billboard records in the
  SAME packed format as the cold texture records (the hot-shadows design) — movers are few,
  a uniform array beats per-frame texture uploads, and the shader decode is shared.
- **Blit light-select ([F3](forks.md#f3))**: a warm-winning pixel takes
  `ambient + hotLight` ONLY — its hot-map texel was computed against the WOLF (all lights,
  cold + hot, wolf normal, wolf receiver mask); a cold pixel takes
  `ambient + coldLight + hotLight` exactly as today. This one selection implements the whole
  matrix with no double-count and no cold re-bake.
- **N/S casting ([F5](forks.md#f5))**: the card GEOMETRY stays the standard billboard; what
  changes per facing is the FRAME (the n/s art's silhouette) + the rotation code (0=s, 2=n)
  wired through records, `sampleCard`, `casterCover`, and `billboardNormal`. Deeper
  perpendicular-card modeling is explicitly deferred until it visibly matters.

## Relationship to open lighting streams

- [hot-shadows](../hot-shadows/README.md) (blocked): this stream implements its
  uniform-hot-data + tier-matrix core for the MOVER subset; the work-item budgeting system
  stays that stream's scope.
- [2026-07-24-shadows-onto-prims](../2026-07-24-shadows-onto-prims/README.md) (open, 0/8):
  billboard RECEIVER machinery (receiver maps + the climb `zElev = uElevK·(baseY − y)` +
  cone culls) is meanwhile LIVE in `shadowGather.ts` (built by the lightmap-resolution /
  lighting-feel line) — that folder is STALE; the wrap phase reconciles it. This stream
  extends the live receiver machinery to warm prims; it does not re-derive it.
- The hard-won guard rails apply throughout: NEVER read a `textile_slot` composite by
  recomputed world coordinate from a light/shadow pass (the attempt-#2 revert); NEAREST for
  encoded maps; zoom stability is an acceptance test at ≥3 zooms, not an afterthought.

## Non-goals

Ambient occlusion changes, the work-item budget system (hot-shadows), per-mover materials
(the wolf authors none — flat reconstruction is correct), pathfinding-aware anything, and
perpendicular n/s card geometry. Wolf emissive already works — verified as a regression
check, not rebuilt.

## Verification surface

Browser `:5174/?user=Claude&focus=100,50&zoom=1&cb=area1` with the tab VISIBLE; the rd-npc
wolf soak provides continuous motion. Debug: `__movers`/`__moverLayer`, `__shownormal`,
`__zoom(z)`, cold-dirty counters (the cold-map-never-rebakes acceptance), screenshots at ≥3
zooms per phase.
