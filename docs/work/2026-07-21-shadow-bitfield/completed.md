# Completed — 2026-07-21-shadow-bitfield

_Done + verified. Items move here from [`todo.md`](todo.md) (append-only)._

## P1 (data reshape) · Cold textures → the fixed authoritative layout — 2026-07-21

Reshaped `coldShadowData.ts` to the F6 layout + adapted the fan cast (`shadowCaster.ts`) to read it as a
regression anchor (the fan retires in P4). Typecheck green; browser regression-verify deferred to the
P4 swap.

- `light_data` now **128×33** — column = light; **row 0** = record (position / colour / reach; the old
  `lut_index|lut_count` A-channel freed → reserved), **rows 1–32** = 256× `u16` prim indexes (the LUT
  folded in, `lut_index` implicit = column, **sentinel `0`**-terminated). `writeCaster` packs 8 `u16`/px.
- `billboard_data` carries **`u16 definition_index`** in `orient` (bits 6–21); `definition_index` removed from
  the LUT. Prim index allocation starts at **1** (index 0 = sentinel).
- `billboard_definition_data` grown to **256×256**. Deleted the `cold_light_billboard_data` LUT texture (+ its
  mirror / `lutTexture` / `lightRange` / `debugLut`). `widths` → `[lightW, defW, primW]`.
- Per-light caster **count** kept CPU-side (`casterCount(k)`) — drives the fan's `instanceCount`; the
  gather will sentinel-terminate instead. Debug decoders updated (`debugPrim` gains `def`, `debugCaster`
  reads the folded LUT).

Shadow-cold RT allocation folded into P4 (built with the gather that writes it).

## No-skew · Perspective-correct UV decode (inverse projection) — 2026-07-21

The user flagged the PixiJS-era skew: interpolating UVs affinely across the projected trapezoid shears the
sprite to one side (they'd worked around it with a 3-triangle fan). Since our gather is **per-fragment**
(not vertex-rasterized), we do the exact fix instead of porting the hack: treat the caster as a flat 3D
**card** and **invert the projection** — for ground point P, solve the card param `(u,v)` whose shadow is P
(v is linear in P.y; then s, then u), and sample the sprite at `(u, v·(1−f))`. Exact for a projected planar
quad → **zero skew**, and it *replaces* the two triangles + `bary` + `projGround` (all deleted) with a
direct decode. Derivation checked (base-centre→v=1, top-centre→v=0). Browser-verified: clean conifer
silhouettes, no lean.

## Base-gap · Lift the shadow base to the sprite's opaque base — 2026-07-21

Per the user: the sprite has transparent padding along its base, so the shadow (a) started at the frame
bottom, below the opaque sprite, and (b) projected that alpha outward — two gaps. Fix: pre-compute
**`base_pad`** = transparent rows below the bottom-most opaque pixel (from the surface-B readback already
in `depthUnits`), stored in the def's repurposed `dA` slot (bits 14–21, atlas px; VARIABLES updated). The
gather lifts the quad **base** up by `f·H` (world Y, on the ground; `f = base_pad/frame_h`) **and** the
base **UV** to `1−f`, so the shadow emanates from the opaque base and doesn't project the alpha. `dA`/`dB`
(fan-only) retired from `inShadow`. Browser-verified (conifer `base_pad`=2 → ~4px world lift; shadows meet
the trunks).

## Debug-viz · 1 light + light/radius gizmos — 2026-07-21

Per the user, for shadow debugging: `MAX_LIGHTS = 1` (a single light **centred** on the seed tile — all
shadows now one colour), plus a **light gizmo** drawn with the shadow overlay (`/overlayRT shadow-cold`):
a filled **dot** at the light and a **ring** at its radius, both screen-constant thickness (`GIZMO`
program + unit quad, per-light draw in `drawOverlay`). Browser-verified. Makes the radial projection +
radius clip legible.

## P4-rect · Switch the fan → a 4-corner projected quad — 2026-07-21

Per the user (diagonal-line artifacts from the fan): `inShadow` now builds a **standard 4-corner
projected billboard rect** (bottom edge on the ground line, top edge projected away from L; 2 triangles,
plain quad UVs) instead of the 5-triangle fan; `dA`/`dB` dropped (deviation D-2). Silhouette mask kept.
**Browser-verified:** diagonals gone, clean tree-shaped shadows (a thin horizontal ground-contact line
where bottom edges align across a row — flagged). All P1–P3 machinery unchanged.

## P4-silhouette · Mask the fan region by the sprite outline — 2026-07-21

The gather predicate now masks by the **sprite silhouette**, not the solid fan trapezoid: for P inside
one of the 5 fan triangles it computes barycentric weights, interpolates the fan's per-role sprite UVs
(the retired fan's `SUV`/`TRI`, verbatim), and samples the shared surface page `.B` coverage — the bit
is set only where covered. Added `uSurface` (bound from `ColdShadowData.surfacePage`, self-heals via the
resolver-`onLoad` → `coldDirty` → force-all-dirty path). **Browser-verified:** shadows now read as the
**conifer shape** (projected tree silhouettes), still per-light hues, overlaps still combine. No errors.

## P3 · Dirty-tile gating (`shadow_dirty`) + toroidal persistence — 2026-07-21

Replaced the full-recompute-each-frame with a `discard`-gated single pass. `shadow_dirty` (`R8UI`
cols×rows, added `r8uint` to the `Texture` class + tight `UNPACK_ALIGNMENT`); the gather samples its tile
and `discard`s clean tiles so `shadow-cold` **persists** (no per-frame clear, no ping-pong). CPU dirties a
slot when its **world-tile owner changes** (pan/resize — per-slot `ownerCol/Row/valid` tracking à la
`SquareCache.markStale`) or on a cold rebuild (sticky `forceDirty`, survives a window-not-ready frame).
RT cleared once on alloc. **Browser-verified:** static picture identical to P2 (gating is a visual no-op);
**panning keeps shadows world-locked** — they track their casters with no tearing / stale bits / smear
(toroidal persistence correct). No console errors. Closes deviation D-1.

## P2 · `light_presence_cold` per-tile light cull — 2026-07-21

`ShadowGather` now builds `light_presence_cold` (cols×rows `RGBA32UI`, one tile/px, bit L = light L's
radius box covers the tile), CPU-rebuilt only on a light/window-signature change; the gather reads its
tile's presence px and **skips lights whose bit is clear**. **Browser-verified** — shadows now **clip to
each light's radius box** (visible straight cut-offs at the box edges). Correcting the D-1 note: presence
is a **semantic** cull (a light only needs shadow bits where it illuminates), so it **does** change the
debug picture (clips to radius) — not invisible; that clipping is correct. No console errors.

## P4-core · The gather + shadow-cold RT + bit-decode overlay — 2026-07-21

New `shadowGather.ts` (`ShadowGather`) replaces the retired fan (`shadowCaster.ts` **deleted**).
**Browser-verified 2026-07-21** (`/overlayRT shadow-cold` at focus 100,50): projected-fan shadows
render in per-light hues; **overlapping shadows combine to white** (multiple bits set → hues summed) —
the design's per-bit decode + overlap-combine. No console errors. (First run showed nothing → caught
`SQUARE = 64` world-px/tile, not 16; the gather/overlay GLSL had hardcoded `16.0`, 4×-compressing world
positions so the region test missed everywhere. Fixed by injecting `SQUARE`/`UNIT` as GLSL literals.)

- **shadow-cold** = a world-space toroidal `RGBA32UI` `RenderTarget` (`cols·16 × rows·16`, `SHADOW_SLOT`
  = 16 texels/tile), resized when the cold cache's tile window changes. `SquareCache.window` now exposes
  `winCol/winRow/cols/rows/slotPx` for the alignment.
- **The gather** = one fullscreen pass; each fragment maps its texel → world tile (toroidal inverse) →
  world units, loops the lights, walks each light's sentinel-terminated caster run, and runs the **exact
  fan region** (roles 0–6 → 5-triangle union, a direct port of the fan's projection) as the point-in-
  shadow predicate; sets `1u<<lightIndex` and writes the full `uvec4`. **No** ping-pong.
- **Overlay** (`/overlayRT shadow-cold`): a world-space window quad samples shadow-cold (`usampler2D`) and
  decodes all 128 bits → per-bit hue colours (overlap sums). Wired into `Viewport` as its own decode path.
- **Deviation D-1 in force:** P4-core is full-recompute, all-lights, no silhouette mask — presence cull
  (P2), dirty gating (P3), and the mask land next as invisible optimisations.
