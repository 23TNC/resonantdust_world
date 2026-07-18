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
| `…-index-0` | `index` | the routing directory + the durable tic |
| `…-chat-0` | `chat` | the message feed |
| `…-event-shard-0` | `event_shard` | the event queue + settled client-visible `event`s |
| `…-data-shard-0` | `data_shard` | entity composition slots + client-visible `state` |
| `…-tile-0` | `tile` | cold ground — dense per-zone tiles |
| `…-thing-0` | `thing` | cold scatter — sparse per-zone things |

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

### `cold_shards` — region→cold-shard routing (public)

| column | type | key | notes |
|---|---|---|---|
| `route_reference` | `u16` | PK | `type_id:4 \| region_reference:8` (+ reserved) — the routed unit |
| `type_id` | `u8` | idx | which cold family (`TYPE_BIOME_TILE` / `TYPE_BIOME_THING` / …) |
| `region_reference` | `u8` | idx | the region this row routes (the high byte of `macro_position_reference`) |
| `shard_reference` | `u8` | | the cold shard serving `(type, region)` |
| `url` | `String` | | that shard's endpoint |
| `db_name` | `String` | | its database on that endpoint |

The fresh, **macro/region-keyed** cold router (not the dead `zone_id`-keyed `region_shards`, Vestigial
above). The edge resolves a cold subscription by `position_reference → macro_position → region_reference`,
then `(type, region) →` this table `→` shard endpoint — the **same** lookup routes a cold entity's
`state`. **Explicit region rows** (no wildcard): the master assigns `(type, region) → shard` as regions
come online; today only the region(s) in use (around the origin) are seeded → shard 0. Adding a shard is
one more row, no code change.

writes the master (allocator) · reads edge · sub `SELECT * FROM cold_shards` (edge)

**Status:** not built — [`work/cold-rework`](work/cold-rework/README.md).

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

# The simulation pipeline

**Built and live end-to-end** — the [`spacetime-again`](intent/spacetime-again/README.md) rebuild
(client → edge queue → orchestrator group → worker compose → promote → `state`/`event` → client).
The reasoning is [`notes/tables.md`](notes/tables.md); the open refinements (not tables) are under
[Not shaped yet](#not-shaped-yet).

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
| **cold rework** | The cold baseline is built + live but on the *old* shape (no `subtype`, `layer_reference` column). The rework — `subtype`-keyed rows, the `state`/`state_log` overlay, the `cold_shards` router, and the mint/`UNPACK` → fold/`PACK` lifecycle ([`world-storage`](intent/world-storage/README.md)) — is planned in [`work/cold-rework`](work/cold-rework/README.md), all blockers resolved. |

**Resolved and gone:** the four worker slots (→ worker + observer), `dirty`-as-count (→ boolean),
`state_events` (the reverse index — no count to decrement), and B-2 "two events on one `(entity,
tic)`" (dissolved: same-entity events share a component, one worker composes them in `event_reference`
order). See [`notes/tables.md`](notes/tables.md).

---

# Cold storage

A cold shard is **an immutable baseline + a hot-format overlay**, both public:

- **Baseline** — `cold_tile` / `cold_thing`: a zone-layer's terrain, position-addressed, **written
  only at `init`/`seed` + `fold`** (a cold row *is* a compressed `state` row, so it carries a `tic`).
  The `type_id` is the shard (the `tile` module *is* `TYPE_BIOME_TILE`, `thing` *is*
  `TYPE_BIOME_THING`), so the `u32 cold_row_reference` carries `macro:16 | subtype:12 | layer_id:4` and
  a multi-biome zone is one row per `subtype`.
- **Overlay** — `state` / `state_log`, **the same shape as `data_shard`'s** ([above](#data_shard)). A
  cold cell is *never* rewritten in place; a change mints a `state_log` row (the source of truth) that
  augments the baseline, and `fold` eventually folds settled `state` back into the baseline (bumping
  its `tic`). The client renders **`cold_tile`/`cold_thing` ⊕ `state`**, ordered by `tic` — **the more
  recent of the baseline row and the override wins** for a cell, so a cold-row/overlay arrival race
  (e.g. a `fold` that bumps the baseline past its now-stale override) resolves deterministically. So
  cold rides the exact hot machinery — same `state_uid`, same worker composition, same `promote_state`.

Design + lifecycle (mint/`UNPACK` → `state_log` → GC-fold/`PACK`): [`world-storage`](intent/world-storage/README.md).
Which shard a position lives on is resolved through `index` ([cold_shards](#cold_shards--regioncold-shard-routing-public)) by **region**.

> **Status.** Baseline + `state`/`state_log` overlay + `tic`-ordered composite are **built + live**
> (seed → mint/`set_tile`·`set_thing` → edge relay → `baseline ⊕ state` render → `fold`-to-baseline,
> browser-verified). The **event-driven `UNPACK` routing** (mutate through edge → worker rather than
> the direct reducer) + core two-phase remain. Tracked in [`work/cold-rework`](work/cold-rework/README.md).

## `tile` — the cold ground shard (`TYPE_BIOME_TILE`)

### `cold_tile` — one zone-layer-biome's dense ground (public)

| column | type | key | notes |
|---|---|---|---|
| `cold_row_reference` | `u32` | PK | `macro_position:16 \| subtype_id:12 \| layer_id:4` ([`VARIABLES.md`](VARIABLES.md)) |
| `macro_position_reference` | `u16` | idx | the zone — the client's subscription key |
| `subtype_id` | `u16` | idx | the biome (holds `u12`); searchable |
| `layer_id` | `u8` | | the layer (holds `u4`) |
| `tic` | `u16` | | the tic this baseline is current as of (set at `seed`, bumped by `fold`); orders it against a `state` override — most recent wins |
| `tiles` | `Vec<u16>` | | **exactly 256** `kind_reference`s, index = `tile_reference`; cells outside this biome are `0` (dense; no per-entry tile/data) |

writes `seed(macro, subtype_id, layer_id, tiles)` (trusted worldgen; `type_id` is the module) · reads
edge, client · sub `SELECT * FROM cold_tile WHERE macro_position_reference = <zone>` (edge, per zone —
returns every biome-row for the zone)

## `thing` — the cold scatter shard (`TYPE_BIOME_THING`)

### `cold_thing` — one zone-layer-biome's sparse scatter (public)

| column | type | key | notes |
|---|---|---|---|
| `cold_row_reference` | `u32` | PK | `macro_position:16 \| subtype_id:12 \| layer_id:4` |
| `macro_position_reference` | `u16` | idx | the zone — subscription key |
| `subtype_id` | `u16` | idx | the biome (holds `u12`) |
| `layer_id` | `u8` | | the layer (holds `u4`) |
| `tic` | `u16` | | current-as-of tic (set at `seed`, bumped by `fold`); orders the baseline against a `state` override |
| `things` | `Vec<u32>` | | **sparse** `kind_pos_reference`s (`kind:16 \| tile:8 \| data:8`), one per occupied cell |

writes `seed` · reads edge, client · sub `SELECT * FROM cold_thing WHERE macro_position_reference =
<zone>`. Same shape as `cold_tile` but each entry is a full `u32` — a thing carries *where* + *data*
(`data = rotation:2 | count:6`, universal), a tile carries only *kind* (its position is its dense index).

Each cold shard also holds the `state` / `state_log` overlay pair — see [`data_shard`](#data_shard)
for their columns (identical), and [`world-storage`](intent/world-storage/README.md) for how a cold
cell becomes a `state_log` row and folds back.

---

# Changing a table

1. Edit this file first, then conform the module.
2. **Check every subscription naming it** — subscription SQL is string-typed; a break compiles clean
   and fails at runtime. See [`notes/tables.md`](notes/tables.md).
3. Regenerate bindings: `rd build spacetime <module>`.
4. Packed layouts change in [`VARIABLES.md`](VARIABLES.md), not here.
