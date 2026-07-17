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

### `master_clock` — the simulation tic authority (public)

| column | type | key | notes |
|---|---|---|---|
| `realm` | `u8` | PK | one row per realm |
| `tic` | `u32` | | absolute counter; won't wrap for ~68y at 2 Hz |

The **one** durable tic for a realm. The **master** advances it (`bump_tic`) and holds no counter of
its own, so a restart resumes from here — the tic survives any shard reset. Every SDK-client server
(master, orchestrator, worker, edge) subscribes to its realm's row and reads the tic straight from
here; the SpacetimeDB *modules* (event/data shards) can't subscribe cross-DB, so the master alone
copies its low 16 bits into their `clock` mirrors. See [`the clock`](#clock--every-shards-tic-mirror).

writes master `bump_tic` · reads every SDK-client server ·
sub `SELECT * FROM master_clock WHERE realm = self`

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
| `event_shard` | `event_counter` | single row: the `++counter:24` for minting `event_reference` (private) |

## `clock` — every shard's tic (mirror)

Each shard module (`event_shard`, `data_shard`) owns a single-row `clock` table:
`{ id:u8 PK, master_tic:u16 }`. It is a **mirror** of `index.master_clock`, not an authority — a
module can't subscribe cross-DB, so the **master** copies the tic's low 16 bits into it each tic
(`bump`). The shard's reducers (`queue`, `settle`, `gc`) read it locally to compute `event_tic =
master+3`, the sealed tic, and the GC horizon. Wiped on redeploy and re-stamped by the master's next
fan-out (the durable value lives in `index.master_clock`). SDK-client servers (orchestrator, worker)
read the tic from `index.master_clock`, **not** from here.

## Vestigial

| DB | table | state |
|---|---|---|
| `index` | `region_shards` | `region_id`→`shard_id`. **Dead** — the edge no longer subscribes or resolves against it (coord-purge C removed the router). Left inert in the module; a fresh macro-keyed router replaces it when multi-shard is a real need. |
| `index` | `shards` | `shard_id`→`{url, db_name}`. Same — inert, no edge consumer. |

---

# Not built — the rebuild

Shapes to build **to**. No module implements them yet. The flow that uses them is
[`intent/spacetime-again/`](intent/spacetime-again/README.md); the reasoning is
[`notes/tables.md`](notes/tables.md).

**The model in one breath.** Events are grouped per tic by shared target entity — transitively, so a
whole conflict-**component** lands on **one worker**. An **orchestrator** does that grouping (union-find
across every event shard, after the event set is complete) and hands each component to a worker. One
worker per component means no lock, no contention, and cross-entity transactions are just sequential
code. Writes are absolute finals from the `< tic` base, so a dead worker's work replays identically.
Nothing composes on data from its own tic — every read is `< tic`.

## `event_shard`

### `orchestrator` — this shard's assigned orchestrator (public)

| column | type | key | notes |
|---|---|---|---|
| `id` | `u8` | PK | always `0` — single row |
| `orchestrator_reference` | `u8` | | `SERVER_REF_NONE` until the master assigns |

The **master** stamps it at standup (`set_orchestrator`). `queue` reads it onto every event row, and
**rejects** if it is `SERVER_REF_NONE` — an event shard with no orchestrator would only accumulate
work nothing groups. Public so the **edge** (W7) routes `queue` only to shards that have one.

writes master `set_orchestrator` · reads `queue` (internal), edge (routing, future)

### `event_log` — the queue (in flight only)

| column | type | key | notes |
|---|---|---|---|
| `event_reference` | `u32` | PK | `entity_reference`, `type_id` = `TYPE_EVENT`. Ascending = composition order. |
| `event_tic` | `u16` | idx | `tic` — wraps |
| `status` | `u8` | idx | `event_status` — `flags:4 \| status:4` |
| `actions` | `Vec<u32>` | | the event program — [`ACTIONS.md`](ACTIONS.md) |
| `event_group` | `u32` | idx | the shard-local group: events sharing a target entity. The orchestrator merges these across shards into work-groups. Singleton (`= event_reference`) until real local grouping lands. |
| `orchestrator_reference` | `u8` | idx | the orchestrator that owns this tic (subscription key) |
| `worker_reference` | `u8` | idx | the worker the orchestrator assigned (subscription key). `SERVER_REF_NONE` = unassigned |
| `zones` | `Vec<u16>` | | the `macro_position_reference`s the event's targets occupy — the worker supplies them at `complete` (it holds the targets' positions), and `settle` fans `event` out one row per zone from them. Empty until `complete`. |

subs: `SELECT * FROM event_log WHERE orchestrator_reference = self` (orchestrator) ·
`SELECT * FROM event_log WHERE worker_reference = self` (worker)

The write set is **not** a column — it comes out of `actions`, routed by each reference's top byte.
See [`notes/tables.md`](notes/tables.md).

### `event` — client-visible settled events

| column | type | key | notes |
|---|---|---|---|
| `uid` | `u64` | PK | `event_uid` — `macro_position_reference:16 \| event_tic:16 \| event_reference:32` |
| `macro_position_reference` | `u16` | idx | the zone-subscription key |
| `event_tic` | `u16` | | `tic` |
| `event_reference` | `u32` | idx | the event; **not** unique here |
| `status` | `u8` | | `event_status`, frozen — terminal only |
| `actions` | `Vec<u32>` | | frozen — [`ACTIONS.md`](ACTIONS.md) |

**One row per zone the event's targets occupy** — an event touching three zones writes three rows.
That is what lets a client subscribed to one zone see a **cross-zone** event that reaches into it,
rather than only events wholly inside it. `event_reference` is therefore not the key and not unique;
`uid` is.

reads edge, client · workers never subscribe ·
sub `SELECT * FROM event WHERE macro_position_reference = <zone>` (edge, per subscribed zone)

The queue mechanics (`event_group`, `orchestrator_reference`, `worker_reference`) do not come across —
they are in-flight state and this row is settled. A row lands here **only** when a program says so, via
a `promote_event` action.

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
| `worker_reference` | `u8` | idx | the worker that **writes** this row (its component's owner). `SERVER_REF_NONE` = none |
| `observer_reference` | `u8` | idx | the worker that **reads** this row as the base for its next-tic work (the `(E, next)` component's owner) |
| `dirty` | `bool` | | `true` = work pending; `false` = settled. One worker owns the component, so it's binary, not a count. |
| *payload* | | | the composed value; written once, absolute |
| `status` | `u8` | | `state_status` — `flags:4 \| status:4` (`PROMOTE` / `PROMOTED`) |

sub `SELECT * FROM state_log WHERE worker_reference = self OR observer_reference = self` (worker) —
the worker's window: rows it writes, and the previous rows it reads as base.

**Two roles, not four slots.** One worker owns a whole component, so exactly one worker writes a row —
`worker_reference`, a single column. The `observer` is the worker of the entity's *next* tic, which
reads this row as its base; the per-entity chain has exactly one next, so one observer. The old
four-slot scheme was for a world where several workers touched one row — the orchestrator makes that
impossible, so it collapses to write-role + read-role.

**No lease here.** A worker is evicted by **event**, not by data-shard row: the orchestrator owns its
worker pool, so it tracks each worker's liveness and, on a hang or death, re-assigns the whole
component — which overwrites `worker_reference` (the write-fence), fencing the old worker out. So the
row needs no lease; a per-component deadline replicated onto every entity's row would be N copies of
one value. The lease lives with the orchestrator (in memory, regenerated on takeover; `event_log` if a
durable home is ever wanted). The data shard never reaps roles.

**The base is read live, not seeded.** A worker computes `(E, T)` from `(E, prev)`'s payload at
execution — `prev` is the most-recent row `< T` for E, which it holds as observer. It computes only
once `prev` is `dirty == false`; **the worker must block on that** (see the correctness note below).
The most-recent row per entity is **never GC'd**, so `prev` always exists in `state_log` — the base is
never fetched from `state`.

`uid` is entity-major; `entity_reference` and `tic` are duplicated out of it because a subscription
filters on columns and a reducer needs them as values.

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

### The block is a worker requirement, not a fence

`A += B` writes A and reads B. Its correctness depends on **B being settled** (`dirty == false` on B's
latest row) — but B may live on a different shard than A, so **A's shard cannot enforce it.** A local
fence checks A's *own* previous row; it can't see B. A worker that skips the block reads a stale B,
writes a wrong-but-clean A, and the next tic composes on the corruption.

So the worker **must** block on every read target's dirtiness. It can, because it is subscribed to
every target it touches — write targets and the read targets' rows both. The block is a correctness
invariant of the worker (our own code), the one place the store cannot cover. (The alternative is
two-phase: write all tentative, verify all settled, then clear dirty — more round trips, enforceable
without trusting the worker. We take "block correctly" while workers are our code.)

### Not shaped yet

| | |
|---|---|
| **orchestrator assignment + liveness** | Which orchestrator owns tic T (doubling is safe, so a good hint suffices — master-assigned is the leaning), and how the master detects a dead orchestrator (heartbeat vs lease). |
| **the world verb palette** | The machine + `promote_*` exist ([`ACTIONS.md`](ACTIONS.md)); no gameplay verb does. Each needs a signature naming, per operand, written vs read. |
| **worker hang-detection** | Death is handled (re-`claim` overwrites the stamp); a *hung* worker needs the orchestrator to notice. In-memory liveness is the leaning, `event_log` lease the fallback. |
| **cold** | Being built — the `tile`/`thing` cold shards ([`intent/world-storage/`](intent/world-storage/README.md)). `tile` exists (below). |

**Resolved and gone:** the four worker slots (→ worker + observer), `dirty`-as-count (→ boolean),
`state_events` (the reverse index — no count to decrement), and B-2 "two events on one `(entity,
tic)`" (dissolved: same-entity events share a component, one worker composes them in `event_reference`
order). See [`notes/tables.md`](notes/tables.md).

---

## `tile` — the cold ground shard

Plan: [`intent/world-storage/`](intent/world-storage/README.md). One cold shard per type per zone
(server = the module), so the key drops the server and is a `u32`.

### `cold_tile` — one zone-layer's dense ground (public)

| column | type | key | notes |
|---|---|---|---|
| `cold_row_reference` | `u32` | PK | `macro_position:16 \| layer_reference:8` ([`VARIABLES.md`](VARIABLES.md)) |
| `macro_position_reference` | `u16` | idx | the zone — the client's subscription key |
| `layer_reference` | `u8` | | `type_id:4 \| layer_id:4` |
| `tiles` | `Vec<u16>` | | **exactly 256** `kind_reference`s, index = `tile_reference` (dense; no per-entry tile/data) |

writes `seed` (trusted worldgen generation, not a player mutation) · reads edge, client ·
sub `SELECT * FROM cold_tile WHERE macro_position_reference = <zone>` (edge, per subscribed zone)

Static-ish: seeded, subscribed, rendered as the ground. Mutation comes later via `UNPACK` → hot →
`PACK` (with `cold_removed` tombstones); not built yet.

## `thing` — the cold scatter shard

### `cold_thing` — one zone-layer's sparse scatter (public)

| column | type | key | notes |
|---|---|---|---|
| `cold_row_reference` | `u32` | PK | `macro_position:16 \| layer_reference:8` |
| `macro_position_reference` | `u16` | idx | the zone — subscription key |
| `layer_reference` | `u8` | | `type_id:4 \| layer_id:4` |
| `things` | `Vec<u32>` | | **sparse** `kind_pos_reference`s (`kind:16 \| tile:8 \| data:8`), one per occupied cell |

writes `seed` · reads edge, client · sub `SELECT * FROM cold_thing WHERE macro_position_reference =
<zone>`. Same shape as `cold_tile` but the entry is a full `u32` — a thing carries *where* + *data*,
a tile carries only *kind* (its position is its dense index).

---

## Changing a table

1. Edit this file first, then conform the module.
2. **Check every subscription naming it** — subscription SQL is string-typed; a break compiles clean
   and fails at runtime. See [`notes/tables.md`](notes/tables.md).
3. Regenerate bindings: `rd build spacetime <module>`.
4. Packed layouts change in [`VARIABLES.md`](VARIABLES.md), not here.
