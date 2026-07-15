# Current — `shared/codec` (where we are)

_Last updated: 2026-07-14._

The reference layouts + object model are implemented in `shared/codec/src` (`event_word.rs`,
`refs.rs`, `object.rs`, `packed.rs`) and consumed across the stack. Detail + runbook:
[`object-model-status.md`](object-model-status.md).

**Status: the codec matches [`design/reference-model.md`](../design/reference-model.md).** No open
decisions, no open bugs. 28 unit tests pin every roundtrip + field-disjointness, including the D-3
property (a target mints the object it names, not the ground under it).

## What's implemented

- **`refs.rs`** — geographic `server_reference = realm_id:8 | server_id:8` (#10);
  `entity_reference = reserved:10 | reference_id:6 | server_reference:16 | object_reference:32`
  (#5; hot = `REF_HOT | server_reference | hot_reference:32`, cold = `REF_COLD | … |
  cold_reference:32`); the dead functional machinery removed. `event_reference` is `u32`.
- **`object.rs`** — the **definition / position / data** split, tracking the design verbatim:
  - `definition_reference:u32 = type_reference:16 | kind_reference:16`, composed **from its two u16
    halves** (`type_reference` is `u16` precisely so it can be half of a `u32` *and* sit in
    `cold_row_reference` — see [deviations D-1](../../../../work/spacetime-rewrite/deviations.md));
  - `position_reference:u32 = macro_position:16 | micro_position:16` as **its own type**, with
    `cold_reference` sharing the layout as a distinct *type* — a position is *a location*; a
    `cold_reference` denotes *the settled object there*;
  - `pack_tile_reference` for the `u8` `x:4|y:4` primitive (realm/region/zone/tile all reuse it);
  - `pack_kind_pos_reference` + `kind_pos_ref_*` — the cold row's entry
    (`kind_reference:16 | tile_reference:8 | data:8`), and `data:8` decode;
  - `pack_cold_row_reference` (`reserved:28 | macro_position:16 | type_reference:16 | layer_id:4`)
    + `cold_row_of` / `cold_row_selects` — row identity and the **one** selection rule, shared by
    the worker and the edge so the two can't drift.
- **`packed.rs`** — geographic `zone_id = realm:8 | region:8 | zone:8 | reserved:8`; the old-game
  `surface` z-axis is retired.
- **`event_word.rs`** — the event-DSL word (`op_code:4 | reserved:12 | server_reference:16 |
  payload:32`), the `OP_LITERAL/OBJECT/ACTION/ALIAS` set, and the append-only `ACTION_*` palette.
  Consumed by `shared/tick`'s purpose-built VM.

## History worth keeping

- **B-2 was retracted** — it treated the legacy `zone_id`/`surface` as a constraint; geometry is
  realm/region/zone/tile/layer, and `surface` was an old-game concept to retire, not conform to.
- **A `macro_* → region_zone_reference` rename was made here and reverted** — the plan's
  `macro_position_reference` / `micro_position_reference` naming stands (deviation D-2).
- **The conformance re-cut (2026-07-14)** closed deviations **D-1…D-5** and divergence **#11**; the
  cold-row header (`macro_position` + `layer_id`) it restored is what fixed a **live bug** — see
  [D-3](../../../../work/spacetime-rewrite/deviations.md).
