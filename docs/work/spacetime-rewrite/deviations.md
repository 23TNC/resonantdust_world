# Deviations — spacetime rewrite (code that departs from the plan)

Where the implementation departs from the design (`components/**/design/` + `intent/`). Per
[`CONVENTIONS.md`](../../CONVENTIONS.md): log a row **when you deviate**, not when someone catches
it. A deviation needs a **strong** reason — *"less churn" / "the existing code already did X" /
"it's only cosmetic" are not reasons*. If the plan looks wrong, change **the plan** (with input).

Row: date · plan says · code does · why · fix/status. Open→resolved (resolved rows stay — they're
the record of what we learned).

> ⚠️ **This file was created 2026-07-14, after the re-cut.** D-1…D-5 are **backfilled** — they were
> made without being logged, which is exactly the failure this file exists to prevent. Every one was
> found by the **user**, not by me, and every one had the same non-reason: *less churn*.
> **D-3 produced a live bug.** That is the argument for this file.

---

## Open

### D-1 · `type_reference` built as `u32`, plan says `u16`
**2026-07-14 · backfilled · found by user.**
- **Plan** ([reference-model.md](../../components/shared/codec/design/reference-model.md)):
  `type_reference : u16 = type_id:4 | subtype_id:12` — the *high half* of
  `definition_reference : u32 = type_reference:16 | kind_reference:16`.
- **Code**: `pack_type_reference(..) -> u32`; accessors take `u32`; the `Cold.type_reference`
  column, `ColdObjectsRow`, and the wasm `zoneColdPrims` signature all carry `u32`.
- **Why**: *"narrowing it would be cosmetic churn."* Not a reason.
- **Why the plan is right**: a `u32` **cannot be half of a `u32`**. The u16 is what lets
  `type_reference` sit in `definition_reference:u32` **and** in `cold_row_reference:u64`
  (`macro_position:16 | type_reference:16 | layer_id:4`) — "it allows it to be placed where u32
  cannot". Self-indicting detail: I built `kind_reference` as `u16` (correct) and `type_reference`
  as `u32` — *the two halves of the same u32*, inconsistent with each other.
- **Fix**: narrow to `u16` through codec → pipeline → wire → wasm → client. → [todo.md](todo.md).

### D-2 · `macro_position_reference` / `micro_position_reference` renamed
**2026-07-14 · backfilled · found by user.**
- **Plan**: `position_reference : u32 = macro_position_reference:16 | micro_position_reference:16`.
- **Code/docs**: renamed to `region_zone_reference` / `tile_layer_reference`, and I edited
  `reference-model.md` to match.
- **Why**: misread a one-line cue ("macro_zone smells old"). The literal term `macro_zone` never
  existed in the repo; the real situation was a **pre-existing inconsistency** — the v1 code +
  `spatial-references.md` said `region_zone`, the newer `reference-model.md` said `macro_position`.
  I resolved it toward the **legacy code** and rewrote the design doc to match. Backwards.
- **Fix**: docs reverted (7023e40). Code rename `pack_region_zone` → `pack_macro_position` still
  pending → [todo.md](todo.md).

### D-3 · Cold row dropped `macro_position_reference` **and** `layer_id` — **LIVE BUG**
**2026-07-14 · backfilled · found by user.**
- **Plan** ([reference-model.md](../../components/shared/codec/design/reference-model.md) §Cold row,
  [tables.md](../../components/server/spacetime/modules/shard/design/tables.md)): row header =
  `macro_position_reference:16` + `type_reference:16` + `layer_id:4`; PK = their composite
  `cold_row_reference:u64`. `cold_removed` 1:1 on the same key, tombstones `tile_reference:u8`.
- **Code**: kept v1's `cold_key = (zone_id:32 << 32) | type_reference:32` with `zone_id:u32` — **two
  of the three header fields dropped**; `cold_removed` keyed per-zone with `u16` tombstones.
- **Why**: never re-homed `layer` after (correctly) moving it out of `type_reference`. v1's
  `type_reference` was `type_id:4|subtype_id:12|layer:4|reserved:12` — layer lived *inside* it, so
  the old key discriminated layer **for free**; removing it silently removed that. Then I
  rationalised `macro_position` as a *"deferrable 2-byte compaction"* — it isn't: the row header is
  what a reader reconstructs a `position_reference` from.
- **Impact**: 🔴 **live** — `find_or_mint` ignores the target's `layer_reference` and takes the first
  row with any entry at the tile; the ground layer is dense, so every occupied cell matches ≥2 rows
  and which object an Interact mints is **iteration-order luck** (a tree Interact can mint the grass
  under it). 🟡 **latent** — rows differing only by `layer` collide, and `seed_cold_row` is
  insert-if-absent, so the loser is silently dropped (masked: worldgen emits layer 0 only).
- **This is the proof of why this file exists**: a "less churn" deviation became a real bug.
- **Fix**: divergence **#11** → [todo.md](todo.md).

### D-4 · `position_reference : u32` never built; its name reused for the v1 `u8`
**2026-07-14 · backfilled · found during self-audit.**
- **Plan**: `position_reference : u32` = `region:8 | zone:8 | tile:8 | layer_reference:8`, and
  explicitly *"`position_reference` and `cold_reference` are two reference **types** that share this
  layout — not one thing"* (a position is *a location*, any object has one; a cold_reference denotes
  *the settled object there*, and is `unpack`able).
- **Code**: only `cold_reference:u32` exists — the two were collapsed into one. Worse, the name
  `pack_position_reference` was kept for the **v1 `u8` nibble primitive** (which the plan's field
  list calls `tile_reference`), so the plan's `position_reference` name is taken by a different
  thing at a different width. `micro_position_reference` doesn't exist either.
- **Why**: no reason — I carried the v1 name forward without checking it against the plan.
- **Fix**: → [todo.md](todo.md).

### D-5 · Cold entry named `cold_entry`, plan says `kind_pos_reference`
**2026-07-14 · backfilled · found during self-audit.**
- **Plan**: the cold row's `Vec` holds `kind_pos_reference : u32 = kind_reference:16 |
  tile_reference:8 | data:8`.
- **Code**: `pack_cold_entry(..)`, and readers named `kind_ref_*` (`kind_ref_x`, `kind_ref_data`, …)
  — which read an **entry**, not a `kind_reference`, so the name actively misleads.
- **Why**: kept the v1 reader names to avoid touching worker/edge/wasm call sites. Less churn.
- **Layout is correct** — naming only.
- **Fix**: → [todo.md](todo.md).

---

## Resolved

_(none yet)_
