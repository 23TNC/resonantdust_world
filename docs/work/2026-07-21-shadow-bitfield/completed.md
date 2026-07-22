# Completed — 2026-07-21-shadow-bitfield

_Done + verified. Items move here from [`todo.md`](todo.md) (append-only)._

## P1 (data reshape) · Cold textures → the fixed authoritative layout — 2026-07-21

Reshaped `coldShadowData.ts` to the F6 layout + adapted the fan cast (`shadowCaster.ts`) to read it as a
regression anchor (the fan retires in P4). Typecheck green; browser regression-verify deferred to the
P4 swap.

- `light_data` now **128×33** — column = light; **row 0** = record (position / colour / reach; the old
  `lut_index|lut_count` A-channel freed → reserved), **rows 1–32** = 256× `u16` prim indexes (the LUT
  folded in, `lut_index` implicit = column, **sentinel `0`**-terminated). `writeCaster` packs 8 `u16`/px.
- `prim_data` carries **`u16 definition_index`** in `orient` (bits 6–21); `definition_index` removed from
  the LUT. Prim index allocation starts at **1** (index 0 = sentinel).
- `prim_definition_data` grown to **256×256**. Deleted the `cold_light_prim_data` LUT texture (+ its
  mirror / `lutTexture` / `lightRange` / `debugLut`). `widths` → `[lightW, defW, primW]`.
- Per-light caster **count** kept CPU-side (`casterCount(k)`) — drives the fan's `instanceCount`; the
  gather will sentinel-terminate instead. Debug decoders updated (`debugPrim` gains `def`, `debugCaster`
  reads the folded LUT).

Shadow-cold RT allocation folded into P4 (built with the gather that writes it).

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
