# Work — caster-lut (shadow casters + per-light range lists in textures)

_Opened 2026-07-20. Builds on the verified [`light-data-texture`](../light-data-texture/README.md) (lights
in a texture) and the [`shadow-tiered`](../shadow-tiered/README.md) cast. Component:
[`client/pixijs`](../../components/client/pixijs/). Moves the **shadow casters** and the **which-prims-does-
each-light-see** cull into textures, so the whole cast can eventually run on the GPU (light → LUT → prim)
instead of the JS `Math.hypot` cull + `Graphics` billboard cast in `castScreen`._

## Why — the cast's inputs must live where the GPU can read them

`light-data-texture` proved lights can live in a texture. The cast also needs the **casters** (their world
rect + silhouette frame + depth) and, per light, the **list of casters in range**. Uniforms don't scale to
thousands of casters; the standard answer is the same as for lights — **data textures** — plus a **LUT** (a
flat index array) that gives each light a contiguous run of caster indices. That's the CSR / offset-count
indirection used by tiled/clustered light culling. With all three in textures, a shader can do the entire
cast: read light `k` → walk its LUT run → fetch each caster → project its wedge.

## The three textures — all `1024×12` RGBA32F

`1024` is the **fixed, cross-platform-safe width** (max texture size varies by device; 1024 is a floor we
can rely on). Height grows in **3-row bands** as capacity needs it; **12 (= 4 bands) is the standard
start**. Each is 12,288 px = 49,152 f32 = **192 KB**. A "definition" (light or caster) is **3 px = 12 f32**;
4 bands × 1024 = **4,096 def slots** per def-texture.

### Prim (caster) data — 4,096 caster defs

| px | R | G | B | A |
|----|-------|-------|-------|-------|
| 0 | world_x | world_y | world_z | facing |
| 1 | width | height | depth1 | depth2 |
| 2 | frame_x | frame_y | frame_width | frame_height |

Maintained with the most recent caster definitions (the resolved standing prims — [`standingPrims()`](../../../client/pixijs/src/game/viewport/SquareCache.ts)).
`facing` selects the billboard orientation; `depth1/depth2` feed the wedge projection; `frame_*` is the
silhouette sub-frame in the atlas.

### Light data — extends the current 2 px to 3 px

| px | R | G | B | A |
|----|-------|-------|-------|-------|
| 0 | world_x | world_y | world_z | radius |
| 1 | red | green | blue | alpha |
| 2 | **start_index** | **count** | reserved | reserved |

`start_index`/`count` point into the LUT: this light's in-range casters are LUT entries
`[start_index, start_index + count)`.

### LUT — 49,152 caster-index entries

Flat array; each px holds **4 caster indices** (into the prim texture). `1024×12 = 12,288 px × 4 =
49,152` entries. LUT entry `i` → px `i >> 2`, channel `i & 3`. A light seeing 256 casters occupies 64 px.
Total used = `Σ count` over all lights (a caster in range of 3 lights appears 3× — see [F2](forks.md#f2)).

## The lookup

For each of N lights: read light `k` → `(start, count)` → for `j` in `[0,count)`: caster index =
LUT[`start + j`] → read caster `idx` from the prim texture → project its shadow. Two indirections, both
texture fetches. Values are integers stored as `f32` — exact below 2²⁴, and all indices (caster ≤ 4,096;
LUT ≤ 49,152) are far under, so `floor(texel + 0.5)` recovers them ([I-1](issues.md#i-1)).

## Maintenance

- **Prim texture:** near-static; patch a caster's 3 px in place with `texSubImage2D` when it moves/spawns
  ([I-2](issues.md#i-2)), not a full re-upload.
- **LUT + light `(start,count)`:** rebuilt when the cull changes (a light or caster moved) — tie to the
  dirty-light path `shadow-tiered` already has, uploading only the used prefix `[0, Σcount)`.

## Capacity notes

- **4,096 caster slots** is fewer than the RT's **7,440 tiles at full zoom-out** (see [the tile-count
  measurement](../../components/client/pixijs/)), so a dense scene can exceed it. Expand by adding 3-row
  bands (height 24 → 8,192), width stays 1024 ([F1](forks.md#f1)).
- **LUT overflow** (`Σ count > 49,152`) is the scaling limit under many overlapping large-radius lights;
  `log()` any truncated run so silent drop-off doesn't read as "no shadow" ([I-4](issues.md#i-4)).
- Light texture is over-provisioned (4,096 slots, 5 used) — harmless; could shrink to `1024×3`
  independently if uniformity isn't worth 192 KB ([F1](forks.md#f1)).

## Alignment with the durable design

This is the data plumbing that lets the **cast move onto the GPU** — the deferred `light-data-texture` L5
and the real [`shadows`](../shadows/README.md) engine. The CPU's job shrinks to maintaining the cull (which
it already does in `castScreen`); the projection becomes a shader reading these three textures.

## State

Phased in [`todo.md`](todo.md); the sizing/strategy calls in [`forks.md`](forks.md); the ES-1.00 /
precision / upload gotchas in [`issues.md`](issues.md). All textures `RGBA32F`, `nearest`; **alpha never a
bitfield here** (these are data/colour textures, not the shadow bitfield).
