# Completed — lighting rebuild

_Done **and** verified on `/overlayRT shadow-cold`. Items move here from [`todo.md`](todo.md).
Append-only history; authoritative for what's done._

---

## P0 · Strip the mess — 2026-07-22 ✓ (archive deferred)

- Removed all debug scaffolding from `shadowGather.ts`: the reject-bit classification overlay (back
  to plain coverage tint), the `__setLight` / `__only` / `__kept` hooks + `debugOnly` field + the
  `tick()` caster filter. The forced-`0` overrides fell out with the pure-quad rewrite (penumbra,
  base-pad `f`, `cover()` gone); `SHADOW_LIFT = 0` remains as a real dial.
- Corridor march + `u/v` inversion already replaced by the geometric quad + brute-force walk.
- **Deferred:** archiving `2026-07-22-shadow-corridor` out of the repo — held to the end of the
  rebuild so the cross-links (README/issues reference it) don't break mid-stream. Still in
  [`todo.md`](todo.md).

## P1 · Data textures — 2026-07-22 ✓

- Light data (position, colour, reach), per-tile **presence** (≤8 lights), per-tile **caster
  buckets** (≤8 casters) all live (carried from the prior gather shell). One light seeded at tile
  **(54,21)**, **z = 40 units**, reach 12 tiles. Gizmo draws the light dot + reach ring.

## P2 · Geometric quad shadows (brute force, units) — 2026-07-22 ✓

- Occlusion is **point-in-convex-quad** (`shadowCover`, four `cross2` edge signs) — texture-agnostic,
  all in **units**. Caster card = **2 tiles / 32 units** tall, tilted 65° north + elevated; light
  `z = 40 units`. Per texel: presence → walk every tile in reach → test each caster's projected quad;
  occluded → write the light's colour. Overlay displays by light colour.
- **Zoom-independence FIXED + verified.** `ColdShadowData.definitionFor` no longer gates on the
  surface LOD resolving (pure quad needs only prim W/H) — the zoom-dependent dropout
  ([`issues.md#zoom`](issues.md)) is gone. Verified quads render identically at zoom **0.5, 1, 2**.
- Still brute-force (walk all in-reach tiles); the corridor returns in P6 validated against this.

## P3 · Tight opaque-bbox quads + P1.5 sweep fix — 2026-07-23 ✓

- Shadow quad = the sprite's **opaque bbox**. Size (W/H) + **offset** (opaque-bbox base-centre −
  game anchor) live in the **def** (per-sprite, shared); `prim_data` keeps the **true game anchor**;
  the gather applies the offset. Bbox pre-computed on the CPU at decode (surface B-channel fractions,
  LOD-safe) — no mid-render GPU readback. Removed the per-prim version/upgrade machinery.
- **THE all-day bug — sweep loop miscompile** ([`issues.md#reach-walk`](issues.md)): a body-modified
  `cov` in the sweep's for-condition made GLSL silently skip iterations → the walk never reached the
  caster's tile → no shadows, despite every value being correct end-to-end. Rewrote the sweep as a
  slot-space, **constant-bound** loop (no body-dependent condition, no break). **Shadows render,
  verified at zoom 0.5 / 1 / 2.** Conforms to [`map-model.md`](map-model.md).

## P4 · Apply the shape (silhouette from SURFACE) — 2026-07-23 ✓

- Shadows now carry the **sprite's silhouette**: inside the proven point-in-projected-quad test the
  gather inverts `P` back to the card's `(s,t)` — linear in `t`, one division, **in-range by
  construction** (no `u/v`-out-of-range class) — and samples the SURFACE coverage (B) at the def's
  **silhouette frame** (the opaque sub-rect of the surface frame, atlas px; F2 → SURFACE, decided).
- Def layout: B = `frame_x|frame_y`, A = `frame_w|frame_h` (u16 halves); `frame_w = 0` → solid-quad
  fallback (surface not yet decoded / frame off the one shared page — C5 limitation, warns once).
  Defs are **compare-written** each pass, so LOD upgrades move the frame and auto re-dirty.
- W-facing (`rotation = 3`) mirrors both the anchor `offset_x` and the sample `s` (+ the CPU bucket
  extent) — the flipped sprite's silhouette casts mirrored.
- Also: maps grounded in the **textile resolutions** (`TEXTILE_TILE/UNIT/SQUARE` + `UNIT` in
  `squareMath.ts`); dead `depthUnits`/`defPending`/`u8` deleted; `debugDef` decodes the real layout;
  `docs/VARIABLES.md` §Cold shadow data conformed to the bucket-era layouts (LUT-era section was
  stale). **Verified in browser at zoom 0.5 / 1 / 2** — conifer silhouettes taper away from the
  light, shrubs cast crescents, all radial directions correct; console clean.

## P5 · Moving light — scoped dirty routing — 2026-07-23 ✓

- A light MOVE now dirties only the tiles in its **old ∪ new reach box** (+1 margin) via queued
  world-tile rects applied in `buildDirty` — no more `forceDirty` full-window recompute on move.
  Correct by construction: a moved light can only change presence/shadow inside old ∪ new reach;
  every other tile keeps its persistent texel. Force-all remains for re-seed / caster-set changes /
  def upgrades.
- **Verified in browser** (orbit on → off, zoom 0.5): the whole field re-points correctly as the
  light orbits (the conifer streak hops between trees), **no stale shadows** linger outside the
  swept boxes, and freezing the orbit settles a clean field.

## P6 · Corridor rebuilt as a pure optimization — 2026-07-23 ✓

- **Why the segment suffices** (the proof the old corridor lacked): P occluded by a caster ⟹
  `P = L.xy + f·(C.xy − L.xy)` for a card point `C`, `f ≥ 1` ⟹ `C.xy = L.xy + (P − L.xy)/f` lies
  **on the segment light→P** — and `C.xy` is inside the caster's ground rect, which is **exactly
  what `buildCasters` buckets** (I-7). So every occluding caster has a bucketed tile on the segment.
- Walk: sample the segment at ≤1-tile steps (`nsteps = ⌈|dx|⌉+⌈|dy|⌉+1`) + a **±1 cross pad** per
  sample (covers corner crossings + float edges). Constant loop bound (48), only the step index in
  the loop condition (the miscompile foot-gun), same `pmod` slot mapping as the buckets. None of the
  old failure modes: no 1-tile march (wedges), no test cap (3-tile shadows), no basis mismatch.
- **Accumulation made idempotent** — `cov = max(cov, cover)` in BOTH paths (was `min(cov+…, 1)`):
  a caster bucketed in several visited tiles is tested a different number of times per path, so any
  additive accumulator breaks bit-identity on fractional silhouette edges. Max is also the honest
  hard-shadow union.
- Brute force kept in-shader behind `uCorridor` (`__corridor(false)`), + `__gather.debugReadShadow()`
  RGBA32UI readback for the diff. **Acceptance: bit-identical** — 2 097 152 words diffed at two
  light positions (seed + post-orbit), **0 mismatches**, 4 892 / 5 631 nonzero texels. Both paths
  display-capped at 121 fps in this scene (1 light; the win is structural: ~125 vs ~729 bucket
  reads per texel·light at reach 12). Corridor is now the default path.

## P0 (deferred item) · shadow-corridor stream archived — 2026-07-23 ✓

- `docs/work/2026-07-22-shadow-corridor/` moved out of the repo → `~/archive/shadow-corridor/`
  (rebuild complete, cross-links resolved: work index row removed, README references de-linked).
