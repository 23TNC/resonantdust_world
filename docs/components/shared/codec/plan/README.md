# Plan — `shared/codec` (current → design)

_Last updated: 2026-07-14._

1. **Re-cut `object.rs` / `refs.rs` to the reference model** (2026-07-14 decision,
   [`../design/reference-model.md`](../design/reference-model.md)) — a code change, not yet done:
   `definition_reference : u32` (type+kind, drop `subkind`), `position_reference : u32`
   (macro+micro), the new cold row (`type_reference` + `macro` + `layer_id` header + `Vec<kind |
   tile | data>`), and `data:8` decode. Rename `object_reference` → `definition_reference` across
   consumers. Then the shard's `region_zone` cold keying (#4) follows directly.
2. **Still needs a decision** (blocker
   [B-1](../../../../work/spacetime-rewrite/blockers.md)): the `hot_reference` u32 re-key (#5) and
   geographic `server_reference` (#10). Once decided, the remaining shard re-keys are mechanical.
