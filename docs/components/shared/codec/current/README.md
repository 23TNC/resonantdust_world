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

**No open decisions — the reference model is fully settled** (blocker
[B-1](../../../../work/spacetime-rewrite/blockers.md) closed 2026-07-14): taxonomy/naming,
`region_zone` (#4), `hot_reference` re-key (#5), and `server_reference = realm:8 | server_id:8`
(#10) are all decided. What remains is the **codec re-cut** — turning v1 `object.rs`/`refs.rs` into
the model — which is now pure implementation against a fixed target ([`plan/`](../plan/)).
