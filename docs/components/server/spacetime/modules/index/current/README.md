# Current — `index`

_Last updated: 2026-07-15._

**Live.** The server tier works: the edge registers + heartbeats, the gateway routes on it.

The **shard tier is vestigial** — `region_shards` / `shards` have no consumer since the shard was
deleted, but the edge **still subscribes** to them at startup and logs the row counts. That
subscription is inert but real: it still breaks at runtime if those tables change. Recorded in
[`TABLES.md` §Vestigial](../../../../../../TABLES.md).

`region_shards.region_id` documented itself as `region_x:8 | region_y:8 | surface:8 | reserved:8`
until 2026-07-15 — the pre-0.2.3 layout, including the retired `surface` z-axis. Comment-only (the
code takes the mask from the codec), now corrected to `realm:8 | region:8 | reserved:16`.
