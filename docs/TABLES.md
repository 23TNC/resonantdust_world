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

Shapes to build **to**. No module implements them yet. The flow that uses them is
[`intent/spacetime-again/`](intent/spacetime-again/README.md); the reasoning is
[`notes/tables.md`](notes/tables.md).

## `event_shard`

### `event_log` — the queue (in flight only)

| column | type | key | notes |
|---|---|---|---|
| `event_reference` | `u32` | PK | `entity_reference`, `type_id` = `TYPE_EVENT`. Ascending = composition order. |
| `worker_reference` | `u8` | idx | subscription key. `SERVER_REF_NONE` = unassigned |
| `event_tic` | `u16` | idx | `tic` — wraps |
| `status` | `u8` | idx | `event_status` — `flags:4 \| status:4`. Phase in `status`, `FAILED` / `PROMOTE` in `flags`. |
| `actions` | `Vec<u32>` | | the RPN program; **word format undefined** |
| `lease_tic` | `u16` | | `tic` — assignment expiry; the reclaim clock |

sub `SELECT * FROM event_log WHERE worker_reference = self` (worker)

The write set is **not** a column — it comes out of `actions`, routed by each reference's top byte.
See [`notes/tables.md`](notes/tables.md).

### `event` — client-visible settled events

| column | type | key | notes |
|---|---|---|---|
| `event_reference` | `u32` | PK | |
| `macro_position_reference` | `u16` | idx | the zone-subscription key |
| `event_tic` | `u16` | | `tic` |
| `status` | `u8` | | `event_status`, frozen — terminal only (`COMPLETE`, or `FAILED` on the phase it died in) |
| `actions` | `Vec<u32>` | | frozen |

reads edge, client · workers never subscribe ·
sub `SELECT * FROM event WHERE macro_position_reference = <zone>` (edge, per subscribed zone)

The counterpart of `state`: zone-scoped and client-visible, so a client can see the events in its
locality, not just their outcome. `event_log`'s queue mechanics (`worker_reference`, `lease_tic`) do
not come across — they are in-flight state, and this row is settled.

A row lands here **only** when a program says so, via a `promote_event` action. See
[`notes/tables.md`](notes/tables.md).

## `data_shard`

Payload = the reference model's three orthogonal references, carried by both `state_log` and
`state`:

| column | type | notes |
|---|---|---|
| `definition_reference` | `u32` | what it is |
| `position_reference` | `u32` | where it is |
| `data` | `u8` | its state |

### `state_log` — per `(entity, tic)` composition slot

| column | type | key | notes |
|---|---|---|---|
| `uid` | `u64` | PK | `state_uid` — `reserved:16 \| entity_reference:32 \| tic:16` |
| `entity_reference` | `u32` | idx | also in `uid`; a column because packed fields can't be filtered or worked with |
| `tic` | `u16` | idx | same |
| `worker_a` | `u8` | idx | `server_reference` acting on this row this tic. `SERVER_REF_NONE` = free |
| `worker_b` | `u8` | idx | |
| `worker_c` | `u8` | idx | |
| `worker_d` | `u8` | idx | |
| `lease_a` | `u16` | | `tic` — `worker_a`'s expiry |
| `lease_b` | `u16` | | |
| `lease_c` | `u16` | | |
| `lease_d` | `u16` | | |
| `dirty` | `u8` | | count of events holding this slot. `0` = settled |
| *payload* | | | composed so far; seeded from the resolved value at `< tic` |
| `status` | `u8` | | `state_status` — `flags:4 \| status:4` |

sub `SELECT * FROM state_log WHERE worker_a = <self> OR worker_b = <self> OR worker_c = <self> OR
worker_d = <self>` (worker) — the worker's window into a data shard.

**At most four servers may act on one row in one tic.** That bound is what makes the subscription a
fixed-width disjunction instead of a per-target subscription, and it lets the worker read the payload
off the row — so nothing carries a `base` copy. A worker claims a slot by calling the shard with the
entity ids it intends to work; the shard allocates a free `worker_*` and stamps the matching `lease_*`.

`uid` is entity-major; `entity_reference` and `tic` are duplicated out of it because a subscription
filters on columns and a reducer needs them as values.

### `state_events` — an event's slots on this shard

| column | type | key | notes |
|---|---|---|---|
| `event_reference` | `u32` | PK | |
| `uid` | `Vec<u64>` | | the `state_log` slots this event holds **here** |

**Internal** — never subscribed. The reverse index: add/remove an event and this gives its `uid`s
directly, so `dirty` can be incremented and decremented without a search.

### `state` — client-visible latest

| column | type | key | notes |
|---|---|---|---|
| `entity_reference` | `u32` | PK | |
| `macro_position_reference` | `u16` | idx | the zone-subscription key. Duplicates `position_reference`'s high half — a subscription filters on columns, not expressions. |
| `tic` | `u16` | | `tic` — the tic this value went live on |
| *payload* | | | |

reads edge, client · workers never subscribe ·
sub `SELECT * FROM state WHERE macro_position_reference = <zone>` (edge, per subscribed zone)

A row lands here **only** when a program says so, via a `promote_state` action.

### Not shaped yet

| | |
|---|---|
| **the composition head** | The lowest pending `event_reference` for a slot — `apply`'s order fence, and how a worker knows it's its turn. `dirty` is a count; it answers *how many*, not *which is next*. Decision #4 ("the one property that must not be broken") rests on it. |
| **the read set** | Writes are recoverable from `actions` (each reference carries its `server_id`). Reads are not — telling which operands a verb *reads* needs `action_reads_actor`, which the design's §Open already flags. |
| **`promote_state` / `promote_event`** | Named as actions, but the verb palette (`ACTION_MOVE` and friends) died with `event_word`. They land wherever the word format does. |

See [`notes/tables.md`](notes/tables.md) and the intent doc.

---

## Changing a table

1. Edit this file first, then conform the module.
2. **Check every subscription naming it** — subscription SQL is string-typed; a break compiles clean
   and fails at runtime. See [`notes/tables.md`](notes/tables.md).
3. Regenerate bindings: `rd build spacetime <module>`.
4. Packed layouts change in [`VARIABLES.md`](VARIABLES.md), not here.
