# Issues — first-pawns

## I1 · Republishing a shard silently orphans connected SDK clients

`rd redeploy` re-published `pawn` (its fresh in-tree `target/` changed the unit's input hash)
while master/orchestrator/worker were connected. The worker then "composed component" cleanly —
into the VOID: the fresh DB never saw the write and nothing errored. Rule: **after any shard
republish, restart its SDK clients** (`bin/sim run …` re-runs the trio). The un-noticed version
of this is exactly what 2026-07-27-sim-self-heal exists to fix (its uplink rebuilds would
reconnect + resubscribe). Also worth a look: whether module input-hashes should exclude
`target/` so a mere build can't trigger a data-wiping republish.

## I2 · The `event` table has no retention — subscribes replay ALL history

A fresh zone subscription's snapshot delivers every settled intent ever fanned to that zone
(3 ancient intents on one page load), and re-subscribes (anchor hysteresis) re-deliver them.
Client-side guards added in `MoverLayer.onMoveIntent` (no-clock ⇒ ignore; finished-long-ago ⇒
ignore; serially-older-than-last-armed ⇒ ignore — `lastIntentTic` per mover). The server-side
fix is an `event` retention sweep (like chat's) — not built; the table is 6 rows today.

## I3 · Intent delivery at the edge is FLAKY (sometimes absent, sometimes doubled)

Across four live moves: one delivered the intent cleanly (speculation glided 2 tiles/s,
landed e=0.01), one delivered it TWICE 2 s apart, one delivered only stale replays, one
delivered NOTHING (the pawn fell back to seed→final snapping — F8 degrading gracefully, so
correctness held). Server side is proven clean (exactly one `event` row per move; worker logs
show single chains at exact 3-tic spacing). Suspected: SDK snapshot-vs-live-insert callback
semantics on the edge's per-zone `event` subscriptions (a row landing via a subscription
snapshot fires no `on_insert` — the same delivery guarantee the cold baselines and the pawn
sub already solve with an `on_applied` replay; the event sub has NO such replay and ALSO no
dedup). Fix candidates: an `on_applied` replay + edge-side seen-set, or client-side only (the
`lastIntentTic` guard already dedups). Reproduce + fix with the npc soak (P4) — it issues
moves continuously.
