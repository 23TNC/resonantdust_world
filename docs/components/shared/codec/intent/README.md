# Intent — `shared/codec` (the why)

_Last updated: 2026-07-14._

Why the reference shapes are what they are: every `*_reference` is a fixed-width packed integer so
references **compose** — a `u32 hot`/`cold`/`event_reference` drops into a `u64 action_reference`
(`server_reference:16 | payload:32`), so an action carrying any reference is passed identically
(no per-kind special-casing). `server_reference` per operand makes cross-shard addressing free.
(See the shard's [`event-reference.md`](../../../server/spacetime/modules/shard/intent/event-reference.md)
for the worked example.)
