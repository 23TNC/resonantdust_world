# Light prims — todo

_Phases toward: a light is a placed world prim, allocated + dirtied through the prim pipeline; the
`this.lights[]` + manual-flag scaffold is deleted. **Write-up state (2026-07-24): NOT greenlit to
build** — P0 ratifies two layout-touching forks (see [`blockers.md`](blockers.md)); building starts on
an explicit "build it". File:line anchors are current-code references for the executor._

## P0 — Ratify the model + layouts (VARIABLES first, no code)
- Resolve [F1](forks.md#f1) (light-record wiring: dual-record vs derived vs full-merge) and
  [F2](forks.md#f2) (where a light's static props live — dedicated light-def / reuse
  `prim_definition_data` / inline). Both touch [`VARIABLES.md`](../../VARIABLES.md) bands → the user's
  call ([`blockers.md`](blockers.md)).
- Resolve [F3](forks.md#f3) (delivery list: parallel `lightPrims()` vs a light-flagged standing prim).
- Land any layout change in `VARIABLES.md` **first**, then conform code (VARIABLES is authoritative).
- Deliverable: the ratified record + def + delivery model written into `VARIABLES.md` + `forks.md`.

## P1 — Light prim source + per-frame delivery
- Give a light a world-object home + a delivery path mirroring the thing path
  (`WorldBridge.onColdThings → viewport.addPrim → SquareCache.prims → standingPrims()`,
  `WorldBridge.ts:397-443` / `SquareCache.ts:350-354` / `Viewport.ts:350`). Per [F3](forks.md#f3):
  a `SquareCache.lightPrims()` selector (or a light flag on `Primitive`), fed to
  `ShadowGather.tick(...)` alongside `standing` (`shadowGather.ts:1349`).
- No world content authoring yet — a debug placement path may seed the light-prims (through
  `addPrim`, NOT `this.lights`), so P1 is testable before P5.

## P2 — Allocate light records from light-prims (retire `buildLights(this.lights)`)
- Replace `buildLights(lights: ColdLight[])` (`coldShadowData.ts:417`) with a builder that, per
  light-prim, allocates/patches its record(s) using the **same free-list + compare-write + `mark`**
  the caster path uses (`primDataFor` `coldShadowData.ts:360-391`: `primFreeList`, compare-write,
  `mark(PRIM_BASE+idx)`). Position is the placed prim's anchor (single source of truth); the light's
  radiometric props derive from its light-def per [F1](forks.md#f1)/[F2](forks.md#f2).
- Keep the gather's read path unchanged if F1 = dual-record/derived (it still `fetchLin`s `light_data`);
  only a full-merge (F1-c) touches `GATHER_FRAG`/`LIGHT_FRAG`.

## P3 — Automatic dirty (generalise the prim cascade; delete the manual flags)
- Route light **placement / move / removal** through the existing scoped-rect cascade
  `markPrimChange → markLightMove → pendingRects → buildDirty`
  (`shadowGather.ts:1195-1207` / `:1177-1183` / `:1246-1256`) — the one path that already turns a prim
  change into scoped dirty without a force-all. A light move already uses `markLightMove` (`tick:1374`);
  extend it to placement (first sight) and removal (queue the light's last reach box **before** freeing).
- Replace the two **force-all fallbacks** (`tick:1396`): caster **removal** (`removed`) and a light change
  with **no scoped rects** (re-seed / prop change) — both become scoped cascades (a freed prim/light
  queues its last-known reach box; a light-def prop change queues that light's cast region).
- Retire the manual light-dirty flags: `coldDirty` (`:881`), `lightsVer` (`:903`), `lastCasterCount`
  as a light-change proxy, and the light-placement uses of `forceColdDirty`/`forceHotDirty` (`:925-926`).
  Presence rebuild (`buildPresence` `lightsVer`-sig gate, `:1071-1072`) keys off the light-prim set
  version instead. **Keep** the tuning-knob re-bake for genuinely-global changes (`__tilt`/`__pitchnormal`/
  `__worldlight`/… set `forceColdDirty` because a *shader constant* changed, not a light) — fold those
  into ONE explicit `rebakeAll()`, distinct from placement dirtying ([issues.md#i4](issues.md#i4)).

## P4 — Delete the scaffold
- Remove `this.lights: Light[]` (`shadowGather.ts` `:39` `EXTRA_LIGHT_TILES`, the `Light` interface
  `:109-118`, `seed()` `:1051`, `__manylights` `:1020`) once P1–P3 supply lights through the prim path.
- Remove the hand-set dirty flags from the debug hooks (or re-point the hooks at the placement path).
- `grep` clean: no `this.lights.push`, no bare `this.coldDirty = true` for light placement.

## P5 — Authoring (content) — may be deferred
- A **light kind** the world can place: a `<light>` object layer (or a light aspect on `<thing>` in
  `content/data/things.rd`) → worldgen / gameplay places it → `WorldBridge` expands it to a light-prim.
  Lets an emissive prim (torch) carry a co-located light. Deferrable behind a debug placement path (P1).

## P6 — Verify
- Place N lights **through the pipeline** (not `this.lights`), confirm they bake + light the world and
  that add/move/remove dirties only the correct rects (scoped, no force-all).
- Re-run the **50-light perf test** through the placement path (this session's test never propagated —
  [issues.md#i1](issues.md#i1)); record fps at `?focus=100,50&zoom=0.25`.
- **Zoom sweep** (the recurring drift class) + **corridor↔brute identity** intact (unchanged bake math).
- Note per-tile coverage is nearest-≤14 lights/tile by design ([issues.md#i2](issues.md#i2)).
