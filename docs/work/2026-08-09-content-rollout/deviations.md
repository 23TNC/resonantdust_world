# Deviations — content-rollout

_Where the code departs from the plan (`docs/components/<c>/{design,intent}` and this stream's
[`forks.md`](forks.md)). Log a row **at the moment you deviate**, not when someone catches it — a
departure from a decision we spent real thought on is how bugs get in, so it must carry a strong
reason. "Less churn", "the existing code already did X" and "it's only cosmetic" are the absence of
a reason. If the plan looks wrong, change the plan with input._

Rows: date · what the plan says · what the code does · why · fix/status.

**None yet — the stream has written no code.**

One departure is already anticipated and pre-authorized, so it does not become a surprise row:

| date | plan says | code will do | why | status |
|---|---|---|---|---|
| 2026-08-09 | [registry F6](../2026-08-04-definition-registry/forks.md#f6): "old objects keep old ids FOREVER; no migration sweep" | `ROLLOUT_DEF` rebinds a live entity's `definition_reference` to a newer version | The user asked for exactly this lane on 2026-08-09 ("force all bunnies from version 1 to version 2 across shards"), superseding the earlier no-sweep call for the ACTIVE case. F6's reasoning survives intact for the PASSIVE default: rollout is opt-in and operator-issued, never automatic on a corpus change ([F5](forks.md#f5)/[F10](forks.md#f10)) | authorized — registry F6 gains a pointer here when P0 lands |
