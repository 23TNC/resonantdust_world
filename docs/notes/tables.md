# Notes — tables

Supporting material for [`../TABLES.md`](../TABLES.md), which is authoritative and carries the
shapes alone. Nothing here defines a schema. If the two disagree, TABLES.md wins.

## The subscription hazard

Subscription queries are **strings**. A schema change that breaks one **compiles clean and fails at
runtime**, against a live DB — no build gate catches it. Every cross-component subscription:

| component | query | against |
|---|---|---|
| edge (per session) | `SELECT * FROM players` | `players` |
| gateway | `SELECT * FROM servers`, `SELECT * FROM player_servers` | `index` |
| edge (startup, global) | `SELECT * FROM region_shards`, `SELECT * FROM shards` | `index` — vestigial, but live: still breaks at runtime |

## Trust and correctness notes

**`chat_messages.sender_player_id` is spoofable.** The chat module has no `players` table to
validate against, so it trusts the caller. The eventual fix is a sidecar or a chat-side mirror of the
session table.

**`chat_messages` is not wired.** The client core's chat subscription is a stub that never fires, so
the feed is empty ([`WorldScene.ts:191`](../../client/pixijs/src/scenes/world/WorldScene.ts)). The
table and reducer are live; only the client link is missing. Locally-parsed slash commands still
work.

**`players.name` uniqueness moved into the schema** when the table flattened. Under the old
history schema `#[unique]` was impossible — a player's own version rows collided on it — so it lived
only in `claim_or_login`'s lookup, and any other writer silently bypassed it. The reducer still
checks first, to fail with a readable message instead of a raw constraint violation.

**`player_servers.player_id` is not FK-enforced.** It comes from the `players` module, a different
database.

**`index` heartbeat margin.** The edge beats `set_server` every 20s against a 60s `SERVER_TTL_MS` — a
third of the TTL, so a missed beat can't reap a live server.

## Vestigial — the shard tier

`region_shards` → `shards` is the geographic `region_id → shard_id → {url, db_name}` chain that
answered "which data shard holds this region". Nothing consumes it: it routed to the deleted shard,
its only reader (`resolve_zone_or_default`) is `#[allow(dead_code)]`, and a player's shard now rides
on their own row (`players.player_shard_reference`) rather than being derived from geography.

The routing itself isn't wrong — `zone_id → region → endpoint` is unchanged by the rebuild, which is
why [`index.rs`](../../server/edge/src/index.rs) was kept whole rather than deleted and re-derived.
Whether zone→shard routing returns at all is the rebuild's call. Written by the operator via
`rd index seed` → `assign_region` / `set_shard`; topology source `content/servers/<env>`.

## History

### `players` was a version-history table — until 2026-07-15

Keyed by `valid_at`, many rows per `player_id`, live one = largest `valid_at` time. The history
bought nothing: a GC sweep reaped every prior version every 10 minutes, nothing ever read one, and
the client never saw the column. Flattened to one row per player, updated in place. Deleted with it:
`players.gc_schedule` (its sweep pruned version rows that no longer exist; the module's `init` moved
to `lib.rs`) and `players.sequence_counter` / `chat.sequence_counter` (both existed only to fill
`valid_at`'s low 16 bits).

### `chat_messages` was keyed by `sent_at` — until 2026-07-15

A packed `[time_ms:48 | sequence:16]` borrowed from the legacy `valid_at` shape, whose low 16 bits
existed only so two same-millisecond sends couldn't collide on the key. `auto_inc` solves that
directly, so the timestamp no longer doubles as an identifier and moved to its own column.

### `players.data_shard` → `player_shard_reference` — 2026-07-15

Same width, but a `realm_server_reference` rather than a bare partition index, so a player's shard is
addressed like any other server and stays unique across realms. `player_profiles.data_shard` was
**not** renamed: it's a different concept — the partition of the auth DB a profile row belongs to,
not the shard serving a player's data.
