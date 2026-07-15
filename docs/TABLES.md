# Tables — the cross-component reference

> **AUTHORITATIVE** for every cross-component table: its **columns, types, keys, and indexes**,
> and who reads/writes it. Established 2026-07-15. Where anything disagrees with this file — a
> module, a doc, or a comment — **this file wins and the other is the bug**.

_Peer to [`VARIABLES.md`](VARIABLES.md), which owns the **variables** these columns are made of. A
column typed `region_id` or `entity_reference` means exactly what VARIABLES.md says it means; this
file never redefines a layout, it cites one. Rationale for a table's shape belongs in its
component's `design/`._

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

The player identity row. **One row per player**, flat. Kept narrow — per-player private state
belongs in `player_profiles`.

| column | type | key | meaning |
|---|---|---|---|
| `player_id` | `u32` | **PK** | logical id. `< 1024` reserved for system/pseudo-players; real players start at `FIRST_PLAYER_ID = 1024`. `DEVELOPER_ID = 512`. |
| `player_shard_reference` | `u16` | | [`player_shard_reference`](VARIABLES.md#server_reference--u16-and-its-roles) — a `server_reference` (`realm_id:8 \| server_id:8`) naming the shard that serves this player's cards/souls. `0` today (realm 0, server 0). |
| `name` | `String` | **unique** | display name, **case-sensitive** ("Alice" ≠ "alice"), ≤ `MAX_PLAYER_NAME_LEN` (64). |
| `last_login_secs` | `u32` | | unix seconds of last `set_last_login`; `0` until the first login round-trip. Drives the chat catch-up window. |
| `flags` | `u32` | | per-player flag bits (faction subfield drives the object-texture pack picker). |

> **Was a `valid_at`-keyed version-history table** until 2026-07-15: many rows per `player_id`, live
> one = largest `valid_at` time. The history bought nothing — a GC sweep reaped every prior version
> every 10 minutes and nothing read one — so it went with `valid_at` itself. Mutations now update in
> place.
>
> **`name` uniqueness is now schema-enforced.** Under the history schema `#[unique]` was impossible
> (a player's own version rows collided on it), so it lived only in `claim_or_login`'s lookup and any
> other writer silently bypassed it. Flattening removed the obstacle. The reducer still checks first,
> to fail with a readable message instead of a raw constraint violation.
>
> **`player_shard_reference` replaced `data_shard`** — same width, but a `server_reference` rather
> than a bare partition index, so a player's shard is addressed like any other server and stays
> unique across realms. It is the routing that survives here: with a player's shard on the player's
> own row, the geographic `region_shards → shards` chain is not what finds it.

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

The **server tier** (`player_servers` → `servers`): *which game server owns a player's session.* A
two-step lookup, read by the gateway.

> **`region_shards` and `shards` are not documented here.** They are the *shard tier* — the
> geographic `region_id → shard_id → {url, db_name}` chain that answered "which data shard holds
> this region." Nothing consumes it: the shard it routed to was deleted, `resolve_zone_or_default`
> is `#[allow(dead_code)]`, and a player's shard now rides on their own row
> (`players.player_shard_reference`) rather than being derived from geography. The tables still
> exist in the `index` module and the edge still subscribes to them — see
> [Vestigial](#vestigial--exists-but-nothing-consumes-it) below. They earn a place here again only
> if the rebuild defines zone→shard routing that something actually reads.

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
| `message_id` | `u64` | **PK**, `auto_inc` | monotonic — ascending `message_id` **is** chronological order, in one pass, no tie-break |
| `sent_at_ms` | `u64` | `idx` | unix ms, server-stamped. The retention sweep's age filter selects on it; also available for display. |
| `sender_player_id` | `u32` | `idx` | **caller-supplied and unvalidated** — see below |
| `sender_name` | `String` | | display name **frozen at send time**; a later rename does not rewrite history |
| `body` | `String` | | trimmed + validated server-side |

> **Was keyed by `sent_at : u64`** until 2026-07-15 — a packed `[time_ms:48 | sequence:16]`
> borrowed from the legacy `valid_at` shape, whose low 16 bits existed only so two same-millisecond
> sends couldn't collide on the key. `auto_inc` solves that directly, so the timestamp no longer
> doubles as an identifier and moved to its own column.

> **Trust boundary.** This module has no `players` table to validate against, so it **trusts the
> caller** on `sender_player_id`. Spoofable today. The eventual fix is a sidecar or a chat-side
> mirror of the session table.

| | |
|---|---|
| **writes** | `send_chat_message` (client-facing) |
| **reads** | **pixijs** `ChatPanel`, via the wasm client core |
| **status** | ⚠️ **not wired.** The client core's chat subscription is a stub that never fires, so the feed is empty ([`WorldScene.ts:191`](../client/pixijs/src/scenes/world/WorldScene.ts)). Locally-parsed slash commands still work. The table and reducer are live; only the client link is missing. |

---

## Vestigial — exists but nothing consumes it

Live in a module, and in one case still subscribed, but with no consumer of the result. Recorded
rather than silently omitted: the code is there, so a reader who greps will find it and deserves to
know it's inert.

| DB | table | state |
|---|---|---|
| `index` | `region_shards` | The geographic shard tier: `region_id → shard_id`, and `shard_id → {url, db_name}`. Written by the operator (`rd index seed` → `assign_region` / `set_shard`); read only by the edge's `resolve_zone_or_default`, which is `#[allow(dead_code)]` — it routed to the deleted shard. The edge **still subscribes** at startup (`SELECT * FROM region_shards`, `SELECT * FROM shards`) and logs the row counts; the resolution feeds nothing. |
| `index` | `shards` | ↑ same chain. |

The routing itself isn't wrong — `zone_id → region → endpoint` is unchanged by the rebuild, which is
why [`index.rs`](../server/edge/src/index.rs) was kept whole rather than deleted and re-derived. But
a player's shard is no longer found this way: it rides on `players.player_shard_reference`. Whether
zone→shard routing returns at all is the rebuild's call.

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
| `chat` | `chat_retention` | scheduled retention sweep |
| `index` | `gc_schedule` | scheduled stale server/pin reaping |

Deleted 2026-07-15 with `valid_at`: `players.sequence_counter` and `chat.sequence_counter` (existed
only to fill `valid_at`'s low 16 bits) and `players.gc_schedule` (its sweep pruned version rows that
no longer exist; the module's `init` moved to `lib.rs`).

---

## Changing a table

1. **Edit this file first**, then conform the module.
2. **Check every subscription that names it.** Subscription queries are **strings** — a schema
   change that breaks one compiles clean and fails at runtime, against a live DB. The build gates
   cannot catch it. Every cross-component subscription is listed above; today they are:

   | component | query | against |
   |---|---|---|
   | edge (per session) | `SELECT * FROM players` | `players` |
   | gateway | `SELECT * FROM servers`, `SELECT * FROM player_servers` | `index` |
   | edge (startup, global) | `SELECT * FROM region_shards`, `SELECT * FROM shards` | `index` — **vestigial**, but a live subscription: it still breaks at runtime if those tables change |

3. **Regenerate bindings** for every consumer (`rd build spacetime <module>` emits
   `server/<c>/src/bindings/<module>/`).
4. A column carrying a packed layout changes in [`VARIABLES.md`](VARIABLES.md) — not here.
