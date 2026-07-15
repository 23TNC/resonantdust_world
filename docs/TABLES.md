# Tables

> **AUTHORITATIVE** for every cross-component table's columns, types, keys and indexes. Anything
> that disagrees — a module, a doc, a comment — is the bug.
> Rationale, hazards, history: [`notes/tables.md`](notes/tables.md).

Cross-component = two or more components read or write it. Module-internal tables are listed at the
bottom. Column layouts are cited from [`VARIABLES.md`](VARIABLES.md), never redefined here.

`PK` = primary key. `idx` = btree index. `uniq` = unique constraint.

---

## Databases

`resonantdust-<env>-<module>-<idx>`, env ∈ dev, claude, test, alpha. All `-0` today.

| DB | module | holds |
|---|---|---|
| `…-players-0` | `players` | accounts, login, player→shard routing |
| `…-index-0` | `index` | the routing directory |
| `…-chat-0` | `chat` | the message feed |

---

## `players`

### `players` — public

| column | type | key | notes |
|---|---|---|---|
| `player_id` | `u32` | PK | `<1024` reserved; real players from `FIRST_PLAYER_ID`=1024; `DEVELOPER_ID`=512 |
| `player_shard_reference` | `u16` | | `realm_server_reference` |
| `name` | `String` | uniq | case-sensitive, ≤ `MAX_PLAYER_NAME_LEN` (64) |
| `last_login_secs` | `u32` | | unix seconds; 0 until first login |
| `flags` | `u32` | | `faction` bits 0–1 (deprecated); `permissions` bits 8–15 |

writes `claim_or_login`, `set_last_login` · reads edge · sub `SELECT * FROM players` (edge, per session)

### `player_profiles` — public

| column | type | key | notes |
|---|---|---|---|
| `player_id` | `u32` | PK | |
| `data_shard` | `u16` | | auth-DB partition (`DATA_SHARD`); not yet reference vocabulary |

writes `claim_or_login` · reads none

---

## `index`

### `servers` — public

| column | type | key | notes |
|---|---|---|---|
| `server_id` | `u16` | PK | |
| `url` | `String` | | process endpoint |
| `last_seen_ms` | `u64` | | heartbeat; `SERVER_TTL_MS` = 60s |

### `player_servers` — public

| column | type | key | notes |
|---|---|---|---|
| `player_id` | `u32` | PK | not FK-enforced across DBs |
| `server_id` | `u16` | | → `servers` |
| `last_seen_ms` | `u64` | | `PLAYER_TTL_MS` |

writes edge `set_server` (20s beat) · gateway `assign_player` / `touch_player` / `release_player` ·
reads gateway · sub `SELECT * FROM servers`, `SELECT * FROM player_servers` (gateway)

---

## `chat`

### `chat_messages` — public

| column | type | key | notes |
|---|---|---|---|
| `message_id` | `u64` | PK, `auto_inc` | monotonic = chronological |
| `sent_at_ms` | `u64` | idx | unix ms, server-stamped; retention filter |
| `sender_player_id` | `u32` | idx | caller-supplied, **unvalidated** |
| `sender_name` | `String` | | frozen at send |
| `body` | `String` | | validated server-side |

writes `send_chat_message` · reads pixijs (via wasm core) — **not wired**, see notes

---

## Module-internal

| DB | table | role |
|---|---|---|
| `players` | `player_id_counter` | id allocation |
| `chat` | `chat_retention` | scheduled retention sweep |
| `index` | `gc_schedule` | scheduled stale server/pin reaping |

## Vestigial

| DB | table | state |
|---|---|---|
| `index` | `region_shards` | `region_id`→`shard_id`. No consumer; edge still subscribes. |
| `index` | `shards` | `shard_id`→`{url, db_name}`. Same chain. |

---

# Not built — the rebuild

Shapes to build **to**. Designed in [`intent/spacetime-again/`](intent/spacetime-again/README.md);
no module implements them yet. Widths are this repo's current reference model, which the design
predates — see [`notes/tables.md`](notes/tables.md) for what was translated and what is still open.

## `event_shard`

### `event_log` — the queue (in flight only)

| column | type | key | notes |
|---|---|---|---|
| `event_reference` | `u32` | PK | `entity_reference`, `type_id` = `TYPE_EVENT`. Ascending = composition order. |
| `worker_reference` | `u8` | idx | subscription key. `SERVER_REF_NONE` = unassigned |
| `event_tic` | `u16` | idx | `tic` — wraps |
| `status` | `u8` | idx | `QUEUED` `QUEUEING` `QUEUE_SUCCESS` `RUNNING` `COMPLETE` `QUEUE_FAILED` |
| `actions` | `Vec<u64>` | | the RPN program; **word format undefined** |
| `targets` | `Vec<u32>` | | `entity_reference` — issuer-designated write set |
| `reads` | `Vec<u32>` | | `entity_reference` — issuer-designated read set |
| `lease_tic` | `u16` | | `tic` — assignment expiry; the reclaim clock |
| `failed` | `bool` | | |

sub `SELECT * FROM event_log WHERE worker_reference = self` (worker)

### `event` — the log (settled history / audit / replay)

`event_log`'s columns, frozen, PK `event_reference`. Never subscribed by workers. `settle(t)` moves
rows here and deletes them from the queue.

## `data_shard`

Payload = the reference model's three orthogonal references, carried by both `state_log` and
`state`:

| column | type | notes |
|---|---|---|
| `definition_reference` | `u32` | what it is |
| `position_reference` | `u32` | where it is |
| `data` | `u8` | its state |

### `state_log` — per `(target, tic)` composition slot

| column | type | key | notes |
|---|---|---|---|
| `uid` | `u64` | PK | surrogate |
| `target_reference` | `u32` | idx `(target_reference, tic)` | `entity_reference` |
| `tic` | `u16` | idx | `tic` — wraps |
| *payload* | | | composed so far; seeded from the resolved value at `< tic` |
| `events` | `Vec<u32>` | | `event_reference`s still to apply, **ascending** |
| `settled` | `bool` | | `events` empty → eligible to promote |
| `promoted` | `bool` | | |

### `state` — client-visible latest

| column | type | key | notes |
|---|---|---|---|
| `target_reference` | `u32` | PK | `entity_reference` |
| `tic` | `u16` | | `tic` — the tic this value went live on |
| *payload* | | | |

reads edge · workers never subscribe

### `state_hold` — not added

The worker's only window into a data shard (`worker_reference`, `ready`, `base`, `lease_tic`). Part
of the same design and load-bearing in it; omitted here because it wasn't asked for. See the intent
doc.

---

## Changing a table

1. Edit this file first, then conform the module.
2. **Check every subscription naming it** — subscription SQL is string-typed; a break compiles clean
   and fails at runtime. See [`notes/tables.md`](notes/tables.md).
3. Regenerate bindings: `rd build spacetime <module>`.
4. Packed layouts change in [`VARIABLES.md`](VARIABLES.md), not here.
