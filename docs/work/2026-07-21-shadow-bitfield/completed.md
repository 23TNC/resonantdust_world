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
