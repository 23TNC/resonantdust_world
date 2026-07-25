# Primitive graph — prims carry data, prims carry prims — 2026-07-25

_Component: [`client/webgl`](../../components/client/) · `game/viewport/` — the unified data texture +
command buffer (`coldShadowData.ts`) and every shader that reads a record (`shadowGather.ts`).
**PREEMPTS the other lighting/shadow streams** — it rewrites the records they all read.
Layouts land in [`VARIABLES.md`](../../VARIABLES.md) at P0 (authoritative). Phases in
[`todo.md`](todo.md); decisions in [`forks.md`](forks.md); gaps in [`issues.md`](issues.md); the calls
needing you in [`blockers.md`](blockers.md). Supersedes the record model in
[`2026-07-24-light-prims`](../2026-07-24-light-prims/README.md)._

## The idea (user, 2026-07-25)
A **prim** stops being "a placed sprite" and becomes a **generic composition node**: a positioned thing
that **carries up to 4 pieces of data**, where a piece may be a **billboard**, a **light**, or **another
prim**. Presentation moves out of the prim into the carried records:

- **torch** = prim → { billboard (sprite), light (illumination) }
- **pawn** = prim → { billboard (head), billboard (body), prim (left hand), prim (right hand) }
- **hand** = prim → { billboard (hand), prim (held tool → a torch prim) }

Children are placed **relative to their carrier**, so moving the pawn moves everything it carries.

## The records (user-specified; revised 2026-07-25 after review — all lanes sum to 32)

**`prim_data`** — the composition node. `child` reinterprets the top half of RED: a root is placed
absolutely, a child points at its carrier and carries only offsets.
```
R  u32   ROOT  (child=0):  u8 region_position | u8 zone_position | u8 tile_position | u8 unit_position
         CHILD (child=1):  u16 parent_id                        | u8 tile_offset    | u8 unit_offset
G  u32   u2 reserved | u1 inherit_rotation | u1 child | u2 rotation | u1 hot_cold | u1 cast_shadows | u8 z
         | u4 set_a | u4 set_b | u4 set_c | u4 set_d      (set 0 = constants = SENTINEL "no data")
B  u32   u16 id_a | u16 id_b
A  u32   u16 id_c | u16 id_d
```
No `layer` on a prim — **one object per layer**, so a prim cannot carry two pieces on the same layer;
the layer lives on the carried record. (Positions/offsets are **bias-8 signed** per nibble, −8..+7.)

**`billboard_data`** — the sprite presentation. RED is entirely **parent + CPU-resolved position**.
```
R  u32   u16 parent_id | u8 resolved_tile | u8 resolved_unit
G  u32   u4 layer | u2 rotation | u1 hot_cold | u1 cast_shadows | u8 z_offset
         | u8 tile_offset | u8 unit_offset                      (the AUTHORED offsets)
B  u32   u16 definition_id | u16 reserved
A  u32   u8 resolved_zone | u24 reserved            ⚠ zone needed — see issues.md#i13
```

**`light_data`** — the emitter presentation. Props inline; no light-def band.
```
R  u32   u16 parent_id | u8 resolved_tile | u8 resolved_unit
G  u32   u4 layer | u2 rotation | u1 hot_cold | u1 cast_shadows | u8 z_offset
         | u8 tile_offset | u8 unit_offset                      (the AUTHORED offsets)
B  u32   u8 r | u8 g | u8 b | u8 i
A  u32   u12 reach | u8 radius | u8 resolved_zone | u4 reserved   (`radius` restored ✅)
```
**Authored vs resolved.** `tile_offset`/`unit_offset` (GREEN) are the **authored** placement relative to
the carrier — the durable truth you edit. `resolved_*` (RED/ALPHA) is the **CPU-computed absolute
position**: start at the root's tile/unit, apply each child prim's offsets down the chain, land at the
leaf. The GPU reads only the resolved fields, so it never walks the graph in the hot loop.

**`definition_data`** — plus a `u4 type`, an `inherit_rotation` bit, minus the self-address.
```
R  u32   u4 layer | u2 rotation | u1 inherit_rotation | u1 reserved
         | u10 offset_x | u10 offset_y | u4 type
G  u32   u9 prim_width | u9 prim_height | u4 frame_span | u10 reserved
B  u32   u10 frame_x | u10 frame_y | u4 frame_page | u4 frame_lod | u2 frame_anchor_x | u2 frame_anchor_y
A  u32   u12 nudge_x | u12 nudge_y | u2 nudge_anchor_x | u2 nudge_anchor_y | u4 reserved
```

## Composition rules (user)
- **Placement.** A root prim is absolute (`region/zone/tile/unit`). A child prim and every carried
  billboard/light are **offsets off the carrier** (`tile_offset`/`unit_offset`/`z_offset`).
- **`hot_cold` — topmost hot wins.** A hot carrier forces everything it carries hot.
- **`cast_shadows` — topmost `!cast_shadows` wins.** A non-casting carrier forces its subtree non-casting.
- **`layer` — one object per layer**, so a prim's carried pieces occupy distinct layers.
- **`parent_id` everywhere** (leaves + child prims) means the graph is navigable **upward**, so either
  side can resolve — the CPU normally does, the GPU *can* ([F1](forks.md#f1) resolved).

## Rotation is a CPU reconciliation signal, NOT a render input (user — important)
**The shader always uses the DEFINITION's rotation**, because the definition *is* the sprite currently
active. The `rotation` stored on prims/billboards is the **desired facing**; when it disagrees with the
active definition's rotation, the **CPU detects the mismatch and swaps the definition**. So a pawn told
to face E while its definition is still S **renders S** until the swap lands — the stored rotation is a
*dirty signal*, not a transform.

**`inherit_rotation` lives on the DEFINITION, not the instance** (user, revised): all objects of a kind
behave the same, so there's no reason to store the choice per object — `definition_data` RED carries the
bit, telling the CPU that pieces using this definition should take their **parent's** rotation.
`prim_data` GREEN carries its own `inherit_rotation` for the case where a particular prim should act
independently. **Inheritance is ONE STEP** — a child takes its parent's rotation, not a chain walk. The
leaves (`billboard_data`/`light_data`) carry no inherit bit at all; **as far as the GPU is concerned the
rotation is the bottom-most rotation**, already reconciled by the CPU.

⇒ There is **no render-time rotation precedence to define** — that concern dissolves
([issues.md#i4](issues.md#i4)).

## Presence: leaves, not carriers — two maps
`light_presence` carries **`light_data` ids**; **`billboard_presence`** (renamed from the caster
buckets / `prim_presence`) carries **`billboard_data` ids**. Bucketing *carriers* instead would make the
per-tile billboard count **unbounded** (a carrier fans out to ≤4, recursively) — bucketing leaves keeps
the gather's hot loop exactly as cheap as today. With self-addressing gone each tile px holds **8 u16
slots**: **16 lights/tile** (`_lo` + `_hi`) and **8 billboards/tile**.

## Command buffer v3 — fixed 8-px commands (user)
Self-addressing is retired (that's what buys back the presence slot), so the command carries the ids.
Rather than counts-in-groups, **every command is exactly 8 px (128 B)**, so 8 commands fit a 64-px row:

```
px 0 = header   u8 operation (0x01 = write-data) | u5 set | u3 count | 7× u16 target ids
px 1..7         up to 7 payload records, written to (set, id[0..count-1])
```
Concretely: `R = operation | set | count | id0`, `G = id1|id2`, `B = id3|id4`, `A = id5|id6`.
**56 record-writes per row**, 512 commands per 64×64 buffer. Benefits over the counts scheme:
- No per-set count field, no `u3`/`u4` sizing problem, no group padding
  ([issues.md#i5](issues.md#i5) dissolves).
- The scatter vertex **drops its 16-iteration count scan**: record `p` → command `p/7`, slot `p%7`,
  target `(set << 16) | id[slot]`. Strictly cheaper than today.
- The leading opcode byte is a real **extension point** — future 8-px operations (presence writes, bulk
  clears) get their own opcodes, which is what the old reserved-opcode field was for
  ([issues.md#i6](issues.md#i6) dissolves).
- One command writes to **one set**, and the header's **`u3 count`** says how many of its 7 ids are
  live — so a partial command needs **no sentinel** and **`id = 0` stays a usable id**. That matters:
  the tile-keyed sets address by `foldTile`, whose range is **0..65535 exhaustively**, so fold 0 is a
  real tile with no spare id to bias into ([issues.md#i14](issues.md#i14)). The scatter issues 7 points
  per command and drops those with `slot ≥ count`, keeping the trivial `p/7`, `p%7` map.

## What this dissolves
- **[light-prims](../2026-07-24-light-prims/README.md)** is subsumed — lights are never *placed*, they're
  *carried*; F1/F2 dissolve. Its **dirty design survives** (two `markDirty` entry points →
  `pendingRects` → `buildDirty`, cold/hot generalised) and moves here as P4.
- Naming reconciles as `prim_data` (node) / `billboard_data` (leaf, NEW) / `light_data` (leaf) /
  `definition_data` (reverting `billboard_definition_data`) ([issues.md#i8](issues.md#i8)).
