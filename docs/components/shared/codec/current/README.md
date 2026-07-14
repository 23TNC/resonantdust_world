# Current — `shared/codec` (where we are)

_Last updated: 2026-07-14._

The reference layouts + object model are implemented in `shared/codec/src` (`event_word.rs`,
`refs.rs`, `object.rs`) and consumed across the stack. Detail + runbook:
[`object-model-status.md`](object-model-status.md).

**Open decisions (gate downstream work).** The object-model taxonomy has open design decisions
(the `hot_reference` u32 re-key / D3, `region_zone` keying, geographic `server_reference`). Until
settled they **block** the shard's representation re-keys — see
[`work/spacetime-rewrite/blockers.md`](../../../../work/spacetime-rewrite/blockers.md) **B-1**.
Settling them here (in `design/object-model.md`) is what unblocks that work.
