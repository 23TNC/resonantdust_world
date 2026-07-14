# Plan — `shared/codec` (current → design)

_Last updated: 2026-07-14._

The crate is implemented; the open work is **deciding** the object-model's open questions (D3
`hot_reference` u32, `region_zone` keying, geographic `server_reference`) — a human-input step,
tracked as [`work/spacetime-rewrite/blockers.md`](../../../../work/spacetime-rewrite/blockers.md)
**B-1**. Once decided in [`../design/object-model.md`](../design/object-model.md), the shard's
representation re-keys become mechanical.
