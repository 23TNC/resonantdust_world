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
| `…-data-shard-0` | `data_shard` | entity composition slots + client-visible `state` (the hot catch-all) |
| `…-pawn-0` | `pawn` | hot movers — pawns (`TYPE_PAWN`), same `entity_tables!` shape as `data_shard` |
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
| `flags` | `u32` | | `permissions` bits 8–15; bits 0–1 reserved |

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

### `definitions` — the definition registry (public)

_Work [`definition-registry`](work/2026-08-04-definition-registry/README.md). The corpus describes;
this table numbers. One row per `(type, subType, kind, variant)` tuple the corpus applies to, per
version._

| column | type | key | notes |
|---|---|---|---|
| `id` | `u32` | PK | the packed `definition_reference` — layout in [`VARIABLES.md`](VARIABLES.md), UNCHANGED (F13) |
| `version` | `u32` | | bumped on a SIMULATION-visible change only (F12); art/tint/comments do not bump |
| `type` | `String` | idx¹ | taxonomy — the structural family |
| `sub_type` | `String` | idx¹ | |
| `kind` | `String` | idx¹ | |
| `variant` | `String` | idx¹ | the LABEL (the art tree's folder name, which may be any string); the id carries the u4 SLOT |

¹ one btree over `(type, sub_type, kind, variant)` — resolution is *match the four, take
`max(version)`*. `uniq` over `(type, sub_type, kind, variant, version)`: one row per version of a
tuple.

writes **master only** — the single allocator (F11), via `ensure(…) → id` at content load ·
reads edge (worldgen name→id), client (local string→id after the table is served), npc ·
sub `SELECT * FROM definitions`

**Old rows are never deleted or rewritten.** A version bump inserts a row with a new `id`; existing
entities keep referencing the old one and keep behaving as it describes (F6). Reclaiming a retired
`id` is designed but deliberately NOT built — `entity_state_log` is append-only history, so a
reclaimed id makes replay lie (F7).

### `master_clock` — the simulation tic authority (public)

| column | type | key | notes |
|---|---|---|---|
| `realm` | `u8` | PK | one row per realm |
| `tic` | `u32` | | absolute counter; won't wrap for ~22y at 6 Hz |

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

**Status:** not built — was tracked in `work/cold-rework` (archived out-of-repo 2026-07-19).

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
| `pawn` | `spawn_counter` | single row: the `++counter:24` for minting a pawn's `object_reference` (private) |

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

`data_shard` is **`entity_tables!{data:u8}`** — the hot mover pair, id-addressed, no baseline. Its
`entity_state`/`entity_state_log` follow the generalized
[§`entity_tables!`](#entity_tables--the-primary-pair-entity_stateentity_state_log-generic-t) shape
(`macro`+`micro` split, `definition_reference`, `payload = u8 data`). The columns below are that shape;
the semantic prose (two roles, no lease, base-read-live) is canonical here and applies to **every**
`*_log` the macros emit.

> _**Live names lag:** the deployed tables are still `state`/`state_log` with three flat refs
> (`definition | position | data`); the generalization renames them to `entity_state`/`entity_state_log`
> and promotes `definition`/`macro`/`micro` to first-class columns + a generic `payload`. Not yet built._

### `entity_state_log` — per `(entity, tic)` composition slot

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

### `entity_state` — client-visible latest

| column | type | key | notes |
|---|---|---|---|
| `entity_reference` | `u32` | PK | |
| `macro_position_reference` | `u16` | idx | the zone-subscription key. Duplicates `micro`'s macro half — a subscription filters on columns, not expressions. |
| `tic` | `u16` | | `tic` — the tic this value went live on |
| *payload* | | | `micro_position_reference` / `definition_reference` / `<T>` (§`entity_tables!`) |

reads edge, client · workers never subscribe ·
sub `SELECT * FROM entity_state WHERE macro_position_reference = <zone>` (edge, per subscribed zone).
_Live name: `state` (renamed in the generalization)._

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

## `pawn`

`pawn` is **`entity_tables!{state_hook: payload_follow_state, data:u8}`** — the hot mover pair
for `TYPE_PAWN`, identical shape to [`data_shard`](#data_shard) (distinct database, same table
names) — plus the spawn machinery for [`CREATE`](ACTIONS.md) and the
[`payload` sidecar](#payload_log--payload--the-growable-per-pawn-opcode-stream-slaved-to-state-public):

**The `data` u8 layout** (movement-hardening — `codec::object` owns the pack/unpack):

| bits | field | meaning |
|---|---|---|
| 7–6 | `facing` | `0`=s `1`=e `2`=n `3`=w — stamped per step, e/w winning diagonals; clients read `data >> 6` |
| 5–0 | `trip_serial` | the live movement chain's identity (seed event's `event_reference & 0x3F`) — a `MOVE_STEP` hop whose serial mismatches is superseded and dies (`ACTIONS.md` §Movement) |

### `spawn_log` — replay-idempotent minted ids (public)

| column | type | key | notes |
|---|---|---|---|
| `spawn_uid` | `u64` | PK | `reserved:16 \| event_reference:32 \| index:16` |
| `event_reference` | `u32` | idx | the `CREATE`-carrying event |
| `index` | `u16` | | which `CREATE` within that event's program (composition order) |
| `entity_reference` | `u32` | | the minted pawn id (`TYPE_PAWN` server byte · `spawn_counter`) |

writes `spawn` · reads `spawn` (replay) — the worker's `CREATE` arm calls `spawn`, nothing else.

**`spawn` is the one-transaction spawn** (first-pawns F4): keyed by `(event_reference, index)` —
if the `spawn_log` row exists the call is a replay and returns without minting; else it mints
from `spawn_counter`, records the `spawn_log` row, writes the pawn's first `entity_state_log`
row (absolute, `dirty=false`), and — given the `PROMOTE` bit — upserts `entity_state`, all in
one reducer transaction. A 32-bit `event_reference` can't derive a 24-bit `object_reference`,
which is why the id is recorded, not computed (`ACTIONS.md` §`CREATE`).

`spawn_counter` is module-internal (see [Module-internal](#module-internal)).

### `payload_log` + `payload` — the growable per-pawn opcode stream, SLAVED to state (public)

Human-pawns P0 (user F1/F5): a pawn's open-ended state (part defs, traits, conditions; later
inventory / …) lives in a SIDECAR pair, NOT on the entity rows — movement hops never copy it.
NEEDS live in their own sub-table below (stat-model F2): they churn per sip, so they fan alone.

**Encoding** — a flat `Vec<u32>` of entries, each a header `opcode:16 | count:16` followed by
`count` operand words, entries concatenated (the command-buffer shape). A reader skips unknown
opcodes by `count`. Opcodes (append-only; a retired VALUE is never reused — old rows must read
as unknown, not misread):

| opcode | value | count | operands | meaning |
|---|---|---|---|---|
| `PART` | 1 | 2 | `slot`, `definition_reference` | the FULL def part slot `slot` draws (body = slot 0, head = slot 1; an equip verb later swaps a slot's def) |
| `NEED` | 2 | — | — | **RETIRED** (stat-model I1): needs moved to the `needs` sub-table |
| `CONDITION` | 3 | — | — | **RETIRED** (stat-model I1): the f32-era shape (`def_ref · grant_tic`); replaced by opcode 4 |
| `CONDITION` | 4 | 2 | `remaining_at_write:16 \| kind:12 \| variant:4` · `written_tic:16` | one TIMED condition (stat-model F1/F3): the packed gameplay row — remaining-now = `remaining_at_write − (now − written_tic)`, expiry DERIVED at read, never stored; a re-grant refreshes the row. DERIVED (band) conditions never appear here |
| `TRAIT` | 5 | 1 | `level:16 \| kind:12 \| variant:4` | one trait/skill the pawn carries at `level ≥ 1` (stat-model F1/F5); minted at CREATE from the thing def's bindings (F11), static until the grant/revoke verbs (recorded successor) |

_Reshapes are NOT read-compatible, by policy (stat-model I1): a reshape RETIRES the opcode value
and claims a new one, so pre-reshape rows read as unknown entries and are skipped whole; dev
pawns RE-MINT (the npc's init path). No in-place converter exists._

`payload_log` — the write-history sidecar of `entity_state_log`; one row per payload-carrying
state write (spawn / future equips — NOT movement):

| column | type | key | notes |
|---|---|---|---|
| `uid` | `u64` | PK | `pack_state_uid(entity, tic)` — the state write it rode |
| `entity_reference` | `u32` | idx | |
| `tic` | `u16` | | |
| `payload` | `Vec<u32>` | | the opcode stream |

`payload` — the composed, client-visible sidecar of `entity_state`:

| column | type | key | notes |
|---|---|---|---|
| `entity_reference` | `u32` | PK | |
| `macro_position_reference` | `u16` | idx | the zone-subscription key — SLAVED to the entity's `entity_state` zone |
| `tic` | `u16` | | last payload CONTENT change (a zone re-key keeps it) |
| `payload` | `Vec<u32>` | | the opcode stream |

**The slaving rule (F5):** neither table is ever claimed — the entity's state claim IS the
lock, and every write here rides a state-write transaction: `spawn` inserts both rows (an
EMPTY payload inserts neither), and the `entity_tables!` `state_hook` drags the `payload`
row's zone key along inside every `entity_state` upsert, so a zone-crossing can never leave
the sidecar behind. `payload_log` is currently un-gc'd (volume = spawns + equips — need churn
left with the `needs` sub-table); it joins the gc when the volume warrants.

**Payload-entry verbs:** `GRANT_CONDITION` splices ONE entry — the pawn module's
`grant_condition` reducer reads the current payload, upserts by the row's low 16 bits
(kind|variant) and writes log + projection in one transaction, RE-STAMPING the granted
condition's affected need rows in the same transaction (stat-model F7 — the re-stamp law).
`SET_NEED` targets the `needs` sub-table below. The MODULE composes, the worker relays; the
verb's `obj` is a Write operand so the entity's claim still serialises these with its movement
writes. Idempotent on replay (same tic → same word; the log row upserts by uid).

### `needs` — the per-(pawn, need) row table (public)

Stat-model F2 (user): needs churn on every sip and every re-stamp, so they fan ALONE instead of
dragging the whole payload. One row per `(entity, need kind|variant)`:

| column | type | key | notes |
|---|---|---|---|
| `uid` | `u64` | PK | `entity_reference:32 \| need_key:16` (`need_key` = the packed row's low 16: kind:12\|variant:4) |
| `entity_reference` | `u32` | idx | |
| `macro_position_reference` | `u16` | idx | the zone-subscription key — SLAVED to the entity's zone exactly like `payload` (the state_hook drags it) |
| `need` | `u32` | | the packed gameplay row: `value:16 \| kind:12 \| variant:4` — value is u16 FIXED-POINT on the need's authored domain (stat-model F4) |
| `set_tic` | `u16` | | the write moment; observers compute `satisfaction_at(now)` lazily (never ticked), piecewise across derived condition expiries (F7) |

Rows mint at CREATE from the thing def's `needs` list (full value); `SET_NEED` upserts by `uid`
with the ONE quantization at write; every modifier-set mutation re-stamps `(value, set_tic)`
(F7). No log twin, deliberately: the need's history is reconstructible from `payload_log`'s
grants + the event stream, and the churn is exactly what was evicted from `payload_log`.

### Not shaped yet

| | |
|---|---|
| **orchestrator assignment + liveness** | Which orchestrator owns tic T (doubling is safe, so a good hint suffices — master-assigned is the leaning), and how the master detects a dead orchestrator (heartbeat vs lease). |
| **the world verb palette** | The machine + `promote_*` exist ([`ACTIONS.md`](ACTIONS.md)); no gameplay verb does. Each needs a signature naming, per operand, written vs read. |
| **worker hang-detection** | Death is handled (re-`claim` overwrites the stamp); a *hung* worker needs the orchestrator to notice. In-memory liveness is the leaning, `event_log` lease the fallback. |
| **cold rework** | The cold baseline is built + live but on the *old* shape (no `subtype`, `layer_reference` column). The rework — `subtype`-keyed rows, the `state`/`state_log` overlay, the `cold_shards` router, and the mint/`UNPACK` → fold/`PACK` lifecycle ([`world-storage`](intent/world-storage/README.md)) — is planned in `work/cold-rework` (archived out-of-repo 2026-07-19), all blockers resolved. |

**Resolved and gone:** the four worker slots (→ worker + observer), `dirty`-as-count (→ boolean),
`state_events` (the reverse index — no count to decrement), and B-2 "two events on one `(entity,
tic)`" (dissolved: same-entity events share a component, one worker composes them in `event_reference`
order). See [`notes/tables.md`](notes/tables.md).

---

# Cold storage — the `*_tables!` macros: `entity_state` / `overlay`

Every composing shard is stamped from **table macros**, each emitting one `*_log`/`*` pair (server-side
truth+history / client-visible projection), generic over an **optional payload `<T>`** (monomorphized
at macro-time). `kind_reference` is **always** present (we must know *what* an object is); the payload
is the *extra* (e.g. `u8 data` = `rotation:2|count:6`). Every shard's primary pair is
**`entity_state`/`entity_state_log`**; a cold shard adds an override pair **`overlay`/`overlay_log`**:

| macro | pair | subject | items | addressing |
|---|---|---|---|---|
| **`entity_tables!`** | `entity_state` / `entity_state_log` | `entity_reference` | single row | per-entity (hot movers) |
| **`dense_entity_tables!`** | ″ | `cold_row_reference` (a biome-row) | `Vec<DenseItem<T>>` | **full `ZONE_DIM²`** → `tile_reference` **indexes** |
| **`sparse_entity_tables!`** | ″ | `cold_row_reference` | `Vec<SparseItem<T>>` | occupied cells → `tile_reference` **matches** |
| **`overlay_tables!`** | `overlay` / `overlay_log` | `cold_row_reference` | `Vec<SparseItem<T>>` | a sparse override shadowing `entity_state` |

**The overlay is always sparse** — an override touches a handful of cells, so it's a sparse shadow of
the baseline (`overlay_tables!`), never the entity-addressed hot pair. Shards compose the macros:

| shard | macros | `TYPE` |
|---|---|---|
| **`pawn`** | `entity_tables!{data:u8}` — movers, id-addressed, **no overlay**; + `spawn_log` when `CREATE` lands | `TYPE_PAWN` |
| **`data_shard`** | `entity_tables!{data:u8}` — the hot **catch-all** (the worker's `_ =>` route; retirement a follow-on once nothing lands here — first-pawns F1) | — |
| **`tile`** | `dense_entity_tables!()` + `overlay_tables!()` | `TYPE_BIOME_TILE` |
| **`thing`** | `sparse_entity_tables!{data:u8}` + `overlay_tables!{data:u8}` | `TYPE_BIOME_THING` |

Client renders **`entity_state` ⊕ `overlay`** — an overlay cell shadows its baseline cell (overlay
present ⇒ override wins). Nothing writes any table directly — a worker composes `*_log` and a
[`PROMOTE`](ACTIONS.md) prefix projects it to `*` (see below); GC `PACK`s the settled `overlay` down
into `entity_state_log`. **`PROMOTE` is smart + atomic** — it copies `entity_state` and `overlay` from
their logs where `visible.tic != log.tic`, in one transaction, so a fold's baseline-update +
overlay-clear land together — **no flash, no ordering**. Item layouts + dense-index vs sparse-match:
[`VARIABLES.md § Cold storage`](VARIABLES.md); region routing:
[`cold_shards`](#cold_shards--regioncold-shard-routing-public); lifecycle:
[`world-storage`](intent/world-storage/README.md).

> **Status.** The live shards are still the *pre-generalization* `cold_tile`/`cold_thing` + old
> fixed-shape `state`/`state_log` + the built `SET` overlay + `tic` composite. The
> **`*_tables!` generalization** below — `entity_state`/`overlay`, payload-generic macros,
> `Vec<DenseItem>`/`Vec<SparseItem>`, the smart-atomic `PROMOTE` prefix, and `PACK` — is the
> **2026-07-18 design target, not yet built**. Was tracked in `work/cold-rework` (archived out-of-repo 2026-07-19).

## `entity_tables!` — the primary pair `entity_state`/`entity_state_log` (generic `<T>`)

The hot form is `entity_reference`-addressed (a mover isn't cell-pinned); the cold forms
(`dense_entity_tables!` / `sparse_entity_tables!`) are `cold_row_reference`-addressed biome-rows. All
three emit `entity_state`/`entity_state_log`; the columns below are the hot form (movers). "state" as a
column name is retired — the *table* is `entity_state`, freeing `data` to stay a payload field.

### `entity_state` — client-visible latest (public)

The **hot** form's columns (a mover). The cold forms replace the payload row with `items:
Vec<DenseItem>`/`Vec<SparseItem>` (below).

| column | type | key | notes |
|---|---|---|---|
| `entity_reference` | `u32` | PK | the subject — a mover (hot) |
| `macro_position_reference` | `u16` | idx | the zone — the client's subscription key |
| `micro_position_reference` | `u16` | | the cell within the zone (`tile_reference:8 \| layer_reference:8`) — `macro \| micro` is the full `position_reference` |
| `tic` | `u16` | | the tic this projection is current as of |
| `definition_reference` | `u32` | | the object (`type_reference:16 \| kind_reference:16`) |
| `payload` | `<T>` | | optional per-shard payload (`u8 data` for thing/pawn; absent for tile) |

### `entity_state_log` — the composed truth + history (public)

| column | type | key | notes |
|---|---|---|---|
| `uid` | `u64` | PK | hot: `state_uid` = `reserved:16 \| entity_reference:32 \| tic:16`; cold: `cold_uid` = `…cold_row_reference:32 \| tic:16` |
| `worker_reference` | `u8` | idx | the composing worker (`SERVER_REF_NONE` = none) |
| `observer_reference` | `u8` | idx | the worker reading this as its next-tic base |
| `status` | `u8` | | `state_status` — `flags:4 \| status:4` (`PROMOTE` / `PROMOTED`) |

_(plus the `entity_state` payload columns carried through composition; `dirty` is the boolean settle
flag. A `counterpart_reference` for cross-shard transfers is **deferred** — not needed while mutation
stays in-shard.)_

## `dense_entity_tables!(T)` — a fully-allocated biome-row (generic `<T>`)

The dense-baseline form of `entity_state`/`entity_state_log`. `tile = dense_entity_tables!()` (`T` =
none). `cold_row_reference`-addressed.

### `entity_state` (dense form) — client-visible baseline (public) · _was `cold_tile`_

| column | type | key | notes |
|---|---|---|---|
| `cold_row_reference` | `u32` | PK | `macro:16 \| subtype:12 \| layer_id:4` ([`VARIABLES.md`](VARIABLES.md)) |
| `macro_position_reference` | `u16` | idx | the zone — subscription key |
| `subtype_id` | `u16` | idx | the biome (holds `u12`) |
| `layer_id` | `u8` | | the layer (holds `u4`) |
| `tic` | `u16` | | current-as-of tic (set by `PROMOTE`) |
| `items` | `Vec<DenseItem<T>>` | | **`ZONE_DIM²`** entries, index = `tile_reference`; `DenseItem` = `kind_reference:u16 \| payload:T` (tile: no payload; empty cell = `kind 0`) |

**`entity_state_log`** (dense) — `cold_uid` PK; shared composition columns (`worker_reference`,
`observer_reference`, `status`, `dirty`) + the base columns + `items`. Worker-composed; `PROMOTE` →
`entity_state`; GC `PACK`s the `overlay` into it.

## `sparse_entity_tables!(T)` / `overlay_tables!(T)` — occupied-cells rows (generic `<T>`)

The sparse form. `sparse_entity_tables!` is a **baseline** (`entity_state`/`entity_state_log`, e.g.
`thing`); `overlay_tables!` is the **overlay** (`overlay`/`overlay_log`, on both cold shards). Same
columns as the dense form, but `items: Vec<SparseItem<T>>` holds **only occupied cells** — `SparseItem`
= `tile_reference:u8 \| kind_reference:u16 \| payload:T` (each carries its own cell). A `tile_reference`
**matches** an item, rather than indexing (dense).

- **`overlay`** — the cells overriding the baseline; the client renders `entity_state ⊕ overlay`.
- **`overlay_log`** — mirror of `entity_state_log`, `cold_uid` PK, shared composition columns;
  `PROMOTE` → `overlay`; GC `PACK`s these into `entity_state_log`.

---

# Changing a table

1. Edit this file first, then conform the module.
2. **Check every subscription naming it** — subscription SQL is string-typed; a break compiles clean
   and fails at runtime. See [`notes/tables.md`](notes/tables.md).
3. Regenerate bindings: `rd build spacetime <module>`.
4. Packed layouts change in [`VARIABLES.md`](VARIABLES.md), not here.
