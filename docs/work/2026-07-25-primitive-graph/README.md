# Primitive graph — prims carry data, prims carry prims — 2026-07-25

_Component: [`client/webgl`](../../components/client/) · `game/viewport/` — the unified data texture +
command buffer (`coldShadowData.ts`) and every shader that reads a record (`shadowGather.ts`).
**PREEMPTS the other lighting/shadow streams** — it rewrites the data structure they all read.
Layouts land in [`VARIABLES.md`](../../VARIABLES.md) at P0 (authoritative). Phases in
[`todo.md`](todo.md); decisions in [`forks.md`](forks.md); gaps in [`issues.md`](issues.md); the
calls needing you in [`blockers.md`](blockers.md). Supersedes the record model in
[`2026-07-24-light-prims`](../2026-07-24-light-prims/README.md) (its F1/F2 dissolve — see below)._

## The idea (user, 2026-07-25)
A **prim** stops being "a placed sprite" and becomes a **generic composition node**: a positioned thing
that **carries up to 4 pieces of data**, where a piece may be a **billboard**, a **light**, or **another
prim**. Presentation moves out of the prim into the carried records. That one change generalises every
object we have or plan:

- **torch** = prim → { billboard (the torch sprite), light (the illumination) }
- **pawn** = prim → { billboard (head), billboard (body), prim (left hand), prim (right hand) }
- **hand** = prim → { billboard (hand), prim (held tool → a torch prim) }

Children are placed **relative to their parent**, so moving the pawn moves everything it carries. This
is the general object model — no bespoke "a light is special" path, no bespoke pawn assembly.

## The records (user-specified, 2026-07-25 — all lanes verified to sum to 32)

**`prim_data`** — the composition node. Position + up to 4 carried pieces. No presentation of its own.
```
R  u32   u8 region_position | u8 zone_position | u8 tile_position | u8 unit_position   (each x:4|y:4)
G  u32   u4 reserved | u2 rotation | u1 hot_cold | u1 cast_shadows | u8 z
         | u4 set_a | u4 set_b | u4 set_c | u4 set_d          (set 0 = constants = SENTINEL "no data")
B  u32   u16 id_a | u16 id_b
A  u32   u16 id_c | u16 id_d
```

**`billboard_data`** (NEW set) — the sprite presentation, placed as an offset off its carrier.
```
R  u32   u4 layer | u2 rotation | u1 hot_cold | u1 cast_shadows | u8 z_offset
         | u8 tile_offset | u8 unit_offset                     (offsets x:4|y:4)
G  u32   u16 definition_id | u16 reserved
B  u32   u32 reserved
A  u32   u32 reserved
```

**`light_data`** — the emitter presentation, same offset shape as a billboard.
```
R  u32   u4 layer | u2 rotation | u1 hot_cold | u1 cast_shadows | u8 z_offset
         | u8 tile_offset | u8 unit_offset
G  u32   u8 r | u8 g | u8 b | u8 i
B  u32   u12 reach | u8 radius | u12 reserved
A  u32   u32 reserved
```
Light props are **inline** — there is no light-def band. (This resolves
[light-prims F2](../2026-07-24-light-prims/forks.md#f2) in favour of inline, and F1 dissolves entirely:
a light is never placed directly, it is *carried*.)

**`definition_data`** — unchanged in spirit, plus a `u4 type`, minus the self-address.
```
R  u32   u4 layer | u2 rotation | u2 reserved | u10 offset_x | u10 offset_y | u4 type
G  u32   u9 prim_width | u9 prim_height | u4 frame_span | u10 reserved
B  u32   u10 frame_x | u10 frame_y | u4 frame_page | u4 frame_lod | u2 frame_anchor_x | u2 frame_anchor_y
A  u32   u12 nudge_x | u12 nudge_y | u2 nudge_anchor_x | u2 nudge_anchor_y | u4 reserved
```

## Composition rules (user)
- **Placement.** A **root** prim is placed absolutely by `region/zone/tile/unit`. A **child** prim
  **ignores region + zone** and uses `tile`/`unit` as **offsets off its parent**. Billboards and lights
  likewise carry `tile_offset`/`unit_offset`/`z_offset` off their carrier.
- **`hot_cold` — topmost hot wins.** If a carrier is hot, everything it carries is hot regardless of its
  own bit. (Inheritance is *downward-forcing*, not a lookup.)
- **`cast_shadows` — topmost `!cast_shadows` wins.** If a carrier doesn't cast, nothing it carries casts.
- **`set_a..d` selects the band; `id_a..d` selects the record.** `set 0` (constants) is the **sentinel**
  for "no data here", so a prim carries 0–4 pieces.

## Command buffer — ids move into the command (self-addressing retired)
Dropping the self-address frees a `u16` in every record (that's what buys back the presence slot), but
the scatter no longer knows where to write. So the **command carries the ids** — which is what command
format **v2 originally did** before v2.1 made records self-address (the scheme is still described in
[`coldShadowData.ts:46-50`](../../../client/webgl/src/game/viewport/coldShadowData.ts)). Reinstated as a
flat block per fill:

```
1 header px  +  8 id px  +  64 data px            (one fill = 73 px)
   each id px = 8× u16 in-set ids  →  8 ids × 8 px = 64 ids, one per data px
```
- **Header** becomes **16× u3 counts (u48) + u80 reserved** — counts are in **groups of 8**, because ids
  arrive 8-per-px. (The `u8 opcode` needs a home in the reserved bits — [issues.md#i6](issues.md#i6).
  `u3` caps a set at 7 groups = 56 records/fill, not 64 — [issues.md#i5](issues.md#i5).)
- **8-record granularity** means a 1-record update pads to 8. The scatter is **replay-idempotent**
  (absolute writes), so padding by **repeating the same id + payload** is safe and needs no sentinel
  ([issues.md#i7](issues.md#i7)).
- `SCATTER_VERT` changes: instead of `uint id = px(vPayload).x >> 16` it reads the id from the fill's id
  block at the matching index.

## Presence + buckets regain a slot
With the self-address gone, a tile-keyed px holds **8 u16 slots** instead of 7:
**16 lights/tile** (`light_presence_lo` + `_hi`) and **8 prims/tile** (`caster_buckets`) — up from 14 and 7.
(What the buckets should hold — carrier prims or resolved leaf billboards — is the central fork,
[forks.md#f1](forks.md#f1).)

## What this dissolves
- **[2026-07-24-light-prims](../2026-07-24-light-prims/README.md)**: its whole premise (place a light
  through the prim path) is *subsumed* — lights are never placed, they're carried. F1 (record wiring) and
  F2 (light-def band) dissolve; its **dirty design survives** (two entry points → `pendingRects` →
  `buildDirty`, cold/hot generalised) and moves here as P4.
- The `prim_*` → `billboard_*` rename (2026-07-24) needs **partial reconciliation**: `prim_data` returns
  as the composition node, `billboard_data` is the new leaf, and `billboard_definition_data` should revert
  to `definition_data` (it now carries a `u4 type` and serves more than billboards) —
  [issues.md#i8](issues.md#i8).
