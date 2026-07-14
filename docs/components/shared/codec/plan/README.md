# Plan — `shared/codec` (current → design)

_Last updated: 2026-07-14._

1. **Re-cut `object.rs` / `refs.rs` to the reference model** (2026-07-14 decision,
   [`../design/reference-model.md`](../design/reference-model.md)) — a code change, not yet done:
   `definition_reference : u32` (type+kind, drop `subkind`), `position_reference` / `cold_reference`
   (positional u32, distinct types), the new cold row (`type_reference` + `macro` + `layer_id`
   header + `Vec<kind | tile | data>`) + `data:8` decode, the `object_reference` union, and
   `entity_reference = reserved:10 | reference_id:6 | server_reference:16 | object_reference:32`
   (was `entity_type | entity_id | mint_server`). This carries the shard's `region_zone` (#4) and
   `hot_reference` (#5) re-keys directly (they're now defined, not decisions).
2. **Still needs a decision** (blocker
   [B-1](../../../../work/spacetime-rewrite/blockers.md)): the geographic-vs-functional
   `server_reference` internal layout (#10) — the last open interior of `server_reference`.
