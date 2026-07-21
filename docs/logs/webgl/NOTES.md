# webgl login — edge event log

User `ExpWebgl`, `focus=100,50`, clean env (edge restarted at debug, spacetime data wiped).

**Healthy, complete flow** (see edge.log):
- All 6 upstreams connect in ~8ms: players, event_shard, data_shard, **tile**, **thing** — no cold-shard timeout.
- `session established player_id=1024 name=ExpWebgl`.
- 9 zones subscribed + seeded (100, 82, 114, 83, 116, 98, 84, 115, 99) — the 3×3 reach block ("9 zones").
- World rendered full-screen (good load).

So on a CLEAN env the webgl login works perfectly — identical to what any core-driven client produces.
