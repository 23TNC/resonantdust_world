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

## Resolved

### D-1 · ✅ RESOLVED — `type_reference` is `u16`
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
- **Fix**: narrow to `u16` through codec → pipeline → wire → wasm → client. → **Resolution** below.

### D-2 · ✅ RESOLVED — `macro_position` / `micro_position` restored
**2026-07-14 · backfilled · found by user.**
- **Plan**: `position_reference : u32 = macro_position_reference:16 | micro_position_reference:16`.
- **Code/docs**: renamed to `region_zone_reference` / `tile_layer_reference`, and I edited
  `reference-model.md` to match.
- **Why**: misread a one-line cue ("macro_zone smells old"). The literal term `macro_zone` never
  existed in the repo; the real situation was a **pre-existing inconsistency** — the v1 code +
  `spatial-references.md` said `region_zone`, the newer `reference-model.md` said `macro_position`.
  I resolved it toward the **legacy code** and rewrote the design doc to match. Backwards.
- **Fix**: docs reverted (7023e40); code renamed `pack_region_zone` → `pack_macro_position`.
  → **Resolution** below.

### D-3 · ✅ RESOLVED — cold row keys on its header composite (the LIVE BUG is fixed)
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
- **Fix**: divergence **#11** → **Resolution** below.

### D-4 · ✅ RESOLVED — `position_reference:u32` built; the u8 is `tile_reference`
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
- **Fix**: → **Resolution** below.

### D-5 · ✅ RESOLVED — `kind_pos_reference` + `kind_pos_ref_*`
**2026-07-14 · backfilled · found during self-audit.**
- **Plan**: the cold row's `Vec` holds `kind_pos_reference : u32 = kind_reference:16 |
  tile_reference:8 | data:8`.
- **Code**: `pack_cold_entry(..)`, and readers named `kind_ref_*` (`kind_ref_x`, `kind_ref_data`, …)
  — which read an **entry**, not a `kind_reference`, so the name actively misleads.
- **Why**: kept the v1 reader names to avoid touching worker/edge/wasm call sites. Less churn.
- **Layout is correct** — naming only.
- **Fix**: → **Resolution** below.


---

## Resolution (2026-07-14) — all five closed, plan-verbatim

Landed as one conformance re-cut + verified:

- **D-1** `pack_type_reference -> u16`; accessors take `u16`; narrowed through the `cold` column, the
  wire (`ColdObjectsRow`), the wasm `zoneColdPrims`/`objectTypeId` signatures and `client/core`.
  `definition_reference` now **composes from its two u16 halves** (test:
  `definition_reference_is_its_two_u16_halves`).
- **D-2** `pack_macro_position` / `pack_micro_position` (+ accessors); `packed::zone_macro_position`.
- **D-3** `cold` PK is `cold_row_reference:u64 = reserved:28 | macro_position:16 |
  type_reference:16 | layer_id:4`, with `macro_position`/`type_reference`/`layer_id` columns;
  `cold_removed` **1:1** on the same key with `Vec<u8>` `tile_reference` tombstones.
  `find_or_mint` + the edge's interact scan now select `(macro_position, type_id, layer_id)` off the
  target's `cold_reference` then match `tile_reference`.
  **Regression-verified live:** two targets at the *same tile* (7,7) differing only in
  `layer_reference` → the one naming `type_id=2` minted the **tree**, the one naming `type_id=1`
  minted the **ground**. Previously iteration-order luck. (Both have `kind=1` — tree is thing-id 1,
  grass is tile-def 1 — which is exactly why the old log read `kind=1` either way and the bug hid.)
  Unit tests pin the property: rows differing only by layer are distinct keys; `cold_row_of` maps
  the same tile at a different layer/type to a different row.
- **D-4** `position_reference:u32` = `macro:16 | micro:16` as its own type, with `cold_reference`
  sharing the layout as a distinct *type* (aliased packers, so a call site says which it means);
  the u8 primitive is `pack_tile_reference`.
- **D-5** `pack_kind_pos_reference` + `kind_pos_ref_*` readers.

**Found while doing it** (logged as they appeared, per this file's purpose):
- **D-6** the edge's cold **subscription SQL** still filtered `WHERE zone_id` after the column was
  gone — the seed silently never fired (cold empty, no error surfaced). Fixed to
  `WHERE macro_position = …`. Caught by *verifying*, not by compiling: SpacetimeDB's subscription
  SQL is a **string**, so the compiler can't see it. ⚠️ **Lesson: schema changes need a live check,
  not just a green build.**
- **D-7** `docker compose -f shared/compose.yml run --rm check` does **not** compile the wasm's
  `js`-gated code (default features), so `cargo check` passing says nothing about the browser
  surface. `rd build shared` (which uses `--features js`) is the real gate. Three broken call sites
  hid behind that.
