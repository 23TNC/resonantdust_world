# Intent — `index` (what goes in, and why)

_Last updated: 2026-07-15._

## The server tier — live

*Which server owns a player's session.* `servers` (endpoint + heartbeat) and `player_servers` (the
per-player pin the gateway re-uses on reconnect).

- **in** — the **edge** heartbeats itself via `set_server` every 20s against a 60s `SERVER_TTL_MS`.
  A third of the TTL, so a missed beat can't reap a live server. The **gateway** writes the pins
  (`assign_player` / `touch_player` / `release_player`).
- **out** — the **gateway** reads both to route a reconnecting player back to their pinned server.
- **sweep** — `index_gc` reaps a stale server and releases its players; an idle pin expires after
  `PLAYER_TTL_MS`.

`player_servers.player_id` comes from the `players` module — a different database, so it is **not**
FK-enforced.

## The shard tier — vestigial

*Which data shard holds a region.* `region_shards` → `shards`. Written by the operator
(`rd index seed` from `deploy/servers/<env>`), read by nobody: the shard it routed to was deleted
2026-07-15, and its only reader (`edge/src/index.rs`) is `#[allow(dead_code)]`.

A player's shard now rides on their own row (`players.player_shard_reference`) rather than being
derived from geography. The routing chain itself isn't wrong — `zone_id → region → endpoint` is
unchanged — which is why `index.rs` was kept whole rather than deleted and re-derived. Whether
zone→shard routing returns at all is the rebuild's call.

## Why the tiers stay apart

They answer different questions (*where does a player's session live* vs *where does a region's data
live*) and have different writers. They share a database because both are tiny, low-write, and
whole-table subscribed — not because they relate.
