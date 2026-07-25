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
