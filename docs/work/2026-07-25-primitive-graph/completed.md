# Primitive graph — completed

_Done + verified. Chronological append; items move here from [`todo.md`](todo.md)._

## P0 — Layouts landed in VARIABLES (2026-07-25)
[`VARIABLES.md`](../../VARIABLES.md) now carries the primitive-graph design as authoritative truth —
code conforms to it from here, not the reverse. Landed:

- **Set table** — `definition_data` (0) / `prim_data` (1) / `light_data` (2) / `light_presence_lo` (3) /
  `billboard_presence` (4, renamed from `caster_buckets`) / `light_presence_hi` (5) / **`billboard_data`
  (6, NEW)**. Set 0 doubles as a prim's "no data carried" sentinel; **in-set id 0 is the global
  sentinel** so commands pad with zeros (every allocator becomes 1-based).
- **`prim_data`** — the composition node: ≤4 carried pieces (`set_a..d` + `id_a..d`), `child` bit
  reinterpreting RED as `parent_id | tile_offset | unit_offset` (root keeps `region|zone|tile|unit`),
  downward-forcing inheritance, one object per layer, **bias-8 signed** offsets.
- **`billboard_data` / `light_data`** — RED = `parent_id` + the **CPU-resolved** position; GREEN keeps
  the **authored** offsets. Light props inline (no light-def band); `emitter_radius` retained.
- **`resolved_zone` on `light_data` only** — `light_presence` is a **reach** relation, so a light sits up
  to `reach` tiles from the fragment reading it and `resolved_tile` pins it only mod 16 tiles (ambiguous
  past ±8; `LIGHT_REACH` = 12). `billboard_presence` is a **containment** relation, so a billboard needs
  no zone. Both documented inline. ([I13](issues.md#i13) — flagged for the user, applied pending reply.)
- **`definition_data`** — `+ u4 type`, `+ inherit_rotation` (per-**kind**, with a `prim_data` override,
  inheriting **one step**), `−` the self-address.
- **Rotation as a reconciliation signal** — the shader always uses the *definition's* rotation; a
  mismatch prompts a CPU definition swap. Written down because it is not inferable from the bits.
- **Presence** — the in-set id leaves the px → **8 slots/tile**: 16 lights + 8 billboards (was 14/7);
  empty standardises on `0`.
- **Command format v3** — fixed **8-px commands** (`R: opcode|set|id₀`, `G/B/A: id₁..id₆`, `px1–7` =
  payloads) = **56 writes/row**; the opcode defines the command so future 8-px operations get their own;
  the trivial index map lets the scatter vertex drop its per-set count scan.

Commit `d0b21e9`. `rd docs-check` green (235 files).

## P1 — Command buffer v3 (transport only) (2026-07-25)
Swapped the transport **without touching a single record layout**, so the change is behaviour-preserving
by construction and independently provable.

- **`SCATTER_VERT` rewritten**: the 16-iteration per-set count scan is **gone**. A point is now
  `p → command p/7, slot p%7`, header at `uCmdBase + cmd·8`, payload at `base + 1 + slot`, target
  `(set << 16) | id[slot]` — read straight from the command header instead of the payload's self-address.
- **`flush()` rewritten**: buckets dirty texels by set, cuts each set into chunks of ≤7, emits fixed
  **8-px commands** (`R = u8 operation | u5 set | u3 count | u16 id₀`, `G/B/A = id₁..id₆`, then 7
  payload px). Batches fill the buffer from the rotating row cursor; the draw issues `batch·7` points and
  the vertex sends `slot ≥ count` off-clip.
- **No sentinel, nothing burned** — `id = 0` stays writable, which the tile-keyed sets require.
- Constants: `CMD_PX`/`IDS_PER_CMD`/`CMDS_PER_ROW`/`MAX_CMDS`/`OP_WRITE_DATA` replace `MAX_PER_SET`;
  the scatter geometry's point buffer sized to the whole buffer (512 commands × 7).

**Verified**: `tsc --noEmit` clean; fresh load renders the scene correctly (trees, soft shadows,
lighting); data confirmed flowing through the new path — 525 billboards live, 3 lights, last flush → set
2 (the orbiting light). Records still self-address in the mirror; the scatter simply ignores it now, and
P2 removes it. Gotcha hit + recorded: [I15](issues.md#i15) (HMR false failure).

## P2a — `definition_data` on the v3 layout (2026-07-25)
Both offsets move into RED, the u16 self-address is dropped, and `layer`/`rotation`/`inherit_rotation`/
`type` are reserved-as-0 until the DSL supplies them (P5). One writer + three decoders
(`receiverCover`/`casterCover`/`billboardNormal`) changed in lockstep. Verified on a fresh load —
pixel-matches the reference. Commit `f83d084`.

## P2b — Presence at 8 slots, 16 lights/tile, `shadow-cold` on u7|u1 (2026-07-25)
The self-address leaves the tile-keyed px, which is what buys back the 8th slot:

- **`tileSlot`** (GLSL, 2 copies) + **`writeTileSet`** (TS) → **8 slots/px**: `R = s0|s1`, `G = s2|s3`,
  `B = s4|s5`, `A = s6|s7`. Presence `_hi` offset 7 → 8; `PRES_SLOTS` 14 → **16**; caster buckets
  7 → **8**; every light loop `slot < 14` → `< 16`; every bucket loop `c < 7` → `c < 8`.
- **`shadow-cold` repacked to u7 | u1 per slot** (user's call — [I16](issues.md#i16)): 16 × 8 = 128 bits
  exactly, so each slot's byte carries its own on-billboard flag and the separate A-lane flag field is
  retired. Decode is `float(b8 >> 1) / 127.0` — the stored `<< 1` **is** the ×2 restore, so full range
  costs nothing.
- Bug found + fixed during verification: a desynced slot-stride literal ([I17](issues.md#i17)).

**Verified** on a fresh load: relief and shadows restored, matches the reference. `tsc` clean.
