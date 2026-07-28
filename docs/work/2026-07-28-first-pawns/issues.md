# Issues — first-pawns

## I1 · Republishing a shard silently orphans connected SDK clients

`rd redeploy` re-published `pawn` (its fresh in-tree `target/` changed the unit's input hash)
while master/orchestrator/worker were connected. The worker then "composed component" cleanly —
into the VOID: the fresh DB never saw the write and nothing errored. Rule: **after any shard
republish, restart its SDK clients** (`bin/sim run …` re-runs the trio). The un-noticed version
of this is exactly what 2026-07-27-sim-self-heal exists to fix (its uplink rebuilds would
reconnect + resubscribe). Also worth a look: whether module input-hashes should exclude
`target/` so a mere build can't trigger a data-wiping republish.
