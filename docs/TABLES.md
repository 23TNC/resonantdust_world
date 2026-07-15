# Tables — the cross-component reference

> **AUTHORITATIVE** for every cross-component table: its **columns, types, keys, and indexes**,
> and who reads/writes it. Established 2026-07-15. Where anything disagrees with this file — a
> module, a doc, or a comment — **this file wins and the other is the bug**.

_Peer to [`VARIABLES.md`](VARIABLES.md), which owns the **variables** these columns are made of. A
column typed `valid_at` or `region_id` means exactly what VARIABLES.md says it means; this file
never redefines a layout, it cites one. Rationale for a table's shape belongs in its component's
`design/`._

**Scope — what's in here.** A table is *cross-component* when **two or more components** read or
write it. Module-internal tables (counters, schedules, sequences) are **out of scope** and listed at
the bottom, so their absence reads as deliberate rather than missed.

**Reading convention.** `PK` = primary key, `idx` = btree index. Types are Rust/SpacetimeDB types;
where a column carries a packed layout, its VARIABLES.md name is given and **that** is the shape.

**Verified against the modules** at `server/spacetime/server/modules/{chat,index,players}` on
2026-07-15.

---

## Databases

One database per module, named `resonantdust-<env>-<module>-<idx>` (`rd_db_for`; env ∈ dev, claude,
test, alpha). All three below are `-0` today.

| DB | module | holds | scale posture |
|---|---|---|---|
| `…-players-0` | `players` | accounts, login, identity → shard routing | low-write; one auth DB serves everyone |
| `…-index-0` | `index` | the routing directory (zones→shards, servers, session pins) | tiny, low-write, whole-table subscriptions |
| `…-chat-0` | `chat` | the message feed | append-heavy, retention-swept |

---

## `players` DB

### `players` — public

The player identity row. **Version history**: multiple rows per `player_id`; the live one is the
row with the largest `valid_at` *time* component. Kept narrow — per-player private state belongs in
`player_profiles`.

| column | type | key | meaning |
|---|---|---|---|
| `valid_at` | `u64` | **PK** | [`valid_at`](VARIABLES.md#valid_at--u64--the-bitemporal-row-key) — `time_ms:48 \| sequence:16`. The sequence half is why two same-ms writes don't collide. |
| `data_shard` | `u16` | | the card-shard partition holding this player's cards/souls. `0` today. |
| `player_id` | `u32` | `idx` | logical id. `< 1024` reserved for system/pseudo-players; real players start at `FIRST_PLAYER_ID = 1024`. `DEVELOPER_ID = 512`. |
| `name` | `String` | `idx` | display name, **case-sensitive** ("Alice" ≠ "alice"), ≤ `MAX_PLAYER_NAME_LEN` (64). |
| `last_login_secs` | `u32` | | unix seconds of last `set_last_login`; `0` until the first login round-trip. Drives the chat catch-up window. |
| `flags` | `u32` | | per-player flag bits (faction subfield drives the object-texture pack picker). |

> **Uniqueness of `name` is reducer-enforced, not schema-enforced.** The history schema can't use
> `#[unique]` — multiple version rows per player would collide — so `claim_or_login` enforces it by
> lookup. Anything writing this table directly bypasses that check.

| | |
|---|---|
| **writes** | `players` module — `claim_or_login`, `set_last_login` |
| **reads** | **edge** (`ws.rs` login read-back, polls the cache for the row the reducer just wrote) |
| **subscribed** | edge, per client session: `SELECT * FROM players` |

### `player_profiles` — public

Per-player private state. Flat (no `valid_at` history — the machinery would be deadweight). Created
in `claim_or_login`'s new-player branch.

| column | type | key | meaning |
|---|---|---|---|
| `player_id` | `u32` | **PK** | |
| `data_shard` | `u16` | | the partition this row belongs to (`DATA_SHARD`, `0` today) |

| | |
|---|---|
| **writes** | `players` module — `claim_or_login` |
| **reads** | *none today* — public and ready, no consumer yet |

---

## `index` DB — the routing directory

Two independent tiers that happen to share a database:
- **shard tier** (`region_shards` → `shards`): *which data shard holds a region, and where it lives.*
- **server tier** (`player_servers` → `servers`): *which game server owns a player's session.*

Each is a two-step lookup; nothing joins across the tiers.

### `region_shards` — public

Step one of the shard tier: which shard holds a region. One row per **assigned** region — an
unrouted region is absent, and the edge falls back to the env's default shard DB (single-shard
deployments seed no rows, so absence is the common path).

| column | type | key | meaning |
|---|---|---|---|
| `region_id` | `u32` | **PK** | [`region_id`](VARIABLES.md#legacy--retiring-do-not-build-on) = `realm:8 \| region:8 \| reserved:16` — a `zone_id` masked by `REGION_ID_MASK` (`0xFFFF_0000`). **Legacy shape**, retiring with `zone_id`. |
| `shard_id` | `u16` | | keys into `shards` |

### `shards` — public

Step two: where a shard physically lives.

| column | type | key | meaning |
|---|---|---|---|
| `shard_id` | `u16` | **PK** | |
| `url` | `String` | | SpacetimeDB server URL to connect to for this shard |
| `db_name` | `String` | | database name on that server holding the shard's regions |

| | |
|---|---|
| **writes** | operator, via `rd index seed` → `assign_region` / `unassign_region` / `set_shard` / `remove_shard`. Topology source: `content/servers/<env>`. |
| **reads** | **edge** — `index.rs` resolves `zone_id → region → endpoint` against the subscribed cache (no per-lookup round trip). *Currently unconsumed: the shard it routed to was deleted; the chain itself is intact.* |
| **subscribed** | edge, server-global, once at startup: `SELECT * FROM region_shards`, `SELECT * FROM shards` |

### `servers` — public

A state-authoritative game server's endpoint + liveness. Reaped by `index_gc` once `last_seen_ms`
goes stale (`SERVER_TTL_MS` = 60s); reaping a server also releases its players.

| column | type | key | meaning |
|---|---|---|---|
| `server_id` | `u16` | **PK** | |
| `url` | `String` | | endpoint clients connect to (a process URL — no `db_name`) |
| `last_seen_ms` | `u64` | | last heartbeat, server-resolved ms |

### `player_servers` — public

Which server owns a player's live session — the pin the gateway re-uses on reconnect. `index_gc`
releases an idle pin after `PLAYER_TTL_MS`.

| column | type | key | meaning |
|---|---|---|---|
| `player_id` | `u32` | **PK** | from the `players` module — **not** FK-enforced across DBs |
| `server_id` | `u16` | | keys into `servers` |
| `last_seen_ms` | `u64` | | last player activity |

| | |
|---|---|
| **writes** | **edge** → `set_server` (heartbeat every `HEARTBEAT_INTERVAL` = 20s, a third of the TTL so a missed beat can't reap a live server). **gateway** → `assign_player`, `touch_player`, `release_player`. |
| **reads** | **gateway** — `directory.rs` routes a reconnecting player back to their pinned server |
| **subscribed** | gateway: `SELECT * FROM servers`, `SELECT * FROM player_servers` |

---

## `chat` DB

### `chat_messages` — public

The message feed. `general` is the only channel today (no channel column — see the chat design doc
for how a `local` feed would land).

| column | type | key | meaning |
|---|---|---|---|
| `sent_at` | `u64` | **PK** | [`valid_at`](VARIABLES.md#valid_at--u64--the-bitemporal-row-key) shape — `time_ms:48 \| sequence:16`. Sorting by it gives chronological order in one pass. |
| `sender_player_id` | `u32` | `idx` | **caller-supplied and unvalidated** — see below |
| `sender_name` | `String` | | display name **frozen at send time**; a later rename does not rewrite history |
| `body` | `String` | | trimmed + validated server-side |

> **Trust boundary.** This module has no `players` table to validate against, so it **trusts the
> caller** on `sender_player_id`. Spoofable today. The eventual fix is a sidecar or a chat-side
> mirror of the session table.

| | |
|---|---|
| **writes** | `send_chat_message` (client-facing) |
| **reads** | **pixijs** `ChatPanel`, via the wasm client core |
| **status** | ⚠️ **not wired.** The client core's chat subscription is a stub that never fires, so the feed is empty ([`WorldScene.ts:191`](../client/pixijs/src/scenes/world/WorldScene.ts)). Locally-parsed slash commands still work. The table and reducer are live; only the client link is missing. |

---

## Not here yet — the rebuild's tables

The shard and tick pipeline were deleted 2026-07-15, taking their tables with them (`event_log`,
`state_log`, `state`, `tic_meta`, `cold`, `cold_removed`, `applied_foreign`). Their replacements —
`event_log` / `event` on an event shard, `state_log` / `state` / `state_hold` on a data shard — are
**designed but not built**: [`docs/intent/spacetime-again/`](intent/spacetime-again/README.md),
status PSEUDOCODE.

**They land in this file when they exist**, not before. This file documents what we read and write
today; the intent doc is where the shape is still being argued.

---

## Out of scope — module-internal tables

Private, single-module, no external reader or writer. Listed so their absence above is legibly
deliberate:

| DB | table | role |
|---|---|---|
| `players` | `player_id_counter` | id allocation |
| `players` | `sequence_counter` | the `valid_at` low-16 tie-breaker |
| `players` | `gc_schedule` | scheduled version-history pruning |
| `chat` | `sequence_counter` | same tie-breaker |
| `chat` | `chat_retention` | scheduled retention sweep |
| `index` | `gc_schedule` | scheduled stale server/pin reaping |

---

## Changing a table

1. **Edit this file first**, then conform the module.
2. **Check every subscription that names it.** Subscription queries are **strings** — a schema
   change that breaks one compiles clean and fails at runtime, against a live DB. The build gates
   cannot catch it. Every cross-component subscription is listed above; today they are:

   | component | query | against |
   |---|---|---|
   | edge (per session) | `SELECT * FROM players` | `players` |
   | edge (startup, global) | `SELECT * FROM region_shards`, `SELECT * FROM shards` | `index` |
   | gateway | `SELECT * FROM servers`, `SELECT * FROM player_servers` | `index` |

3. **Regenerate bindings** for every consumer (`rd build spacetime <module>` emits
   `server/<c>/src/bindings/<module>/`).
4. A column carrying a packed layout changes in [`VARIABLES.md`](VARIABLES.md) — not here.
