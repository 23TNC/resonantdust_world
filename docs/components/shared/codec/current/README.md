# Current — `shared/codec` (where we are)

_Last updated: 2026-07-14._

The reference layouts + object model are implemented in `shared/codec/src` (`event_word.rs`,
`refs.rs`, `object.rs`) and consumed across the stack. Detail + runbook:
[`object-model-status.md`](object-model-status.md).

**Reference model — DECIDED, code not yet re-cut (2026-07-14).** The object reference model is now
settled: the **definition / position / data** split in
[`design/reference-model.md`](../design/reference-model.md) (all `u32`; `object_reference` →
`definition_reference`; `subkind` dropped; explicit `region_zone`; `data:8` = the cold/hot
pack criterion). **The code (`object.rs`, `refs.rs`) still implements v1** (a `u64
object_reference` with `subkind` + `x/y/data` in the kind half) — the re-cut is a
[`plan/`](../plan/) item.

**Still open (blocker [B-1](../../../../work/spacetime-rewrite/blockers.md)):** the `hot_reference`
u32 re-key (#5, identity side) and geographic `server_reference` (#10, routing side). The
taxonomy/naming + `region_zone` (#4) pieces are now closed by the reference model above.
