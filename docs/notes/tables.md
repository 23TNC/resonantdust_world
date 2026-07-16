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

Designed, not yet live — they carry the same hazard the day they are, and each one's filter column
exists *because* of it:

| component | query | against |
|---|---|---|
| orchestrator | `SELECT * FROM event_log WHERE orchestrator_reference = self` | `event_shard` |
| worker | `SELECT * FROM event_log WHERE worker_reference = self` | `event_shard` |
| worker | `SELECT * FROM state_log WHERE worker_reference = self OR observer_reference = self` | `data_shard` — **the `OR` is unverified against SpacetimeDB's grammar; two subscriptions is the fallback** |
| edge (per zone) | `SELECT * FROM state WHERE macro_position_reference = <zone>` | `data_shard` |
| edge (per zone) | `SELECT * FROM event WHERE macro_position_reference = <zone>` | `event_shard` |

Two axes, and they're the whole model: a **worker/orchestrator** subscribes by *its own id* (work is
pushed to it by assignment); the **edge and client** subscribe by *locality* (zone). Nothing
subscribes to a log it doesn't own a row in.

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

## The rebuild's tables

Shapes here, flow in [`../intent/spacetime-again/`](../intent/spacetime-again/README.md), widths from
[`../VARIABLES.md`](../VARIABLES.md).

**`event_reference` and the global total order.** Ascending `event_reference` is what makes
multi-target events deadlock-free — every queue readies in the same order, the classic
acquire-in-a-global-order result, and the one property that must not be broken. An
`entity_reference` is already a u32 (`server_reference:8 | object_reference:24`), so an event's
identity and its ordering key are the same value: the shard mints the low 24 monotonically, and the
server byte on top keeps two event shards' ranges disjoint. Ascending `entity_reference` is therefore
a global total order. Note this is **not** SpacetimeDB's `auto_inc` on the whole column — that
would increment the server byte. The shard composes the reference.

**`payload…` is the one thing the design leaves abstract**, and TABLES.md has to be concrete. The
reference model already says what an object *is* — three orthogonal references: what it is
(`definition_reference`), where it is (`position_reference`), and its state (`data`). So the payload
is those three. This is a reading of the model, not a decision the intent doc made; the old shard's
payload (`kind:u16, zone_id:u32, location:u8, rotation:u8, offset:u8, data0:u64, data1:u64`) is
legacy — `zone_id` is retiring, `offset` was `sub_position` and is gone, and `rotation` now lives
inside `data`.

**`state`'s primary key is `entity_reference` alone**, `tic` a plain column — it's the client-visible
*latest*, and `promote` upserts by entity, replacing. A composite `(entity, tic)` key would accumulate
a row per tic and stop being "latest".

**`actions : Vec<u32>`** — the encoding is [`../ACTIONS.md`](../ACTIONS.md). Narrowed u64 → u32
because an `entity_reference` is a u32: a word carrying a reference needs 32 bits, not 64, and the
old tagged frame couldn't manage it (`op_code:4 + entity_reference:32 = 36`). Leading with the
action instead of tagging every word is what bought the whole u32 back.

### No `targets` / `reads` columns

Both were `Vec<u32>` columns holding the issuer-designated write and read sets. They're duplication:
every reference is already in `actions`, so a column spends a vector per row to restate what the
program says. The worker reads the write set off the program and routes by each reference's top byte.

Collecting operands out of a word stream is *structural* — no game semantics in the spine, which was
the point of having the columns in the first place.

**Both sets fall out of it.** An action's *signature* declares, per operand, written vs read
([`../ACTIONS.md`](../ACTIONS.md)) — so one scan yields both, and the spine never interprets a verb.
That is what closed the `action_reads_actor` question: the same fact, moved from a function that
knows game semantics into a declaration.

### The orchestrator, and what one-worker-per-component buys

**The grouping is the whole design.** An orchestrator unions each tic's events by shared target into
components (union-find, across every event shard, *after* the set is complete) and hands each
component to one worker. One worker per component means no two workers ever touch the same entity in
the same tic — so no lock, no contention, and a cross-entity transaction is just sequential code in
one worker's scratch. Full walkthrough in [`../intent/spacetime-again/`](../intent/spacetime-again/README.md).

Everything below is downstream of that one fact.

### `state_log` — the composition slot

**`uid` is `(entity, tic)`, not a surrogate.** `reserved:16 | entity_reference:32 | tic:16` — so
`find_or_create(entity, tic)` is an exact PK lookup. **Entity-major** (`entity_reference` above `tic`)
because `tic` wraps: a wrapping tic in the high bits sorts by *raw* value, so a `tic <= t` range scan
straddling a wrap misses the oldest rows. Entity-major instead makes one entity's rows contiguous —
which is what the per-entity block check needs — and the tic is filtered with `tic::` serial
comparison within an entity's run. `entity_reference` and `tic` are duplicated out as columns because
a subscription filters on columns and a reducer needs values; a packed field is neither.

It's an `entity_reference` (`server_reference:8 | object_reference:24`), not a bare `object_reference`,
because the server half says *which* object exactly.

**Two roles, not four slots.** `worker_reference` (writes the row) + `observer_reference` (the *next*
tic's worker, reading this row as its base) — one column each, **no lease**. One worker owns a
component, so exactly one writes a row; the per-entity chain has exactly one next, so exactly one
reads it. An earlier sketch had four slots (`worker_a..d`) for a world where several workers touched
one row — the orchestrator makes that impossible, so it collapses to write-role + read-role. The
subscription is `WHERE worker_reference = self OR observer_reference = self` (two subscriptions if the
grammar rejects the `OR`).

**No lease on the row.** A worker is evicted by *event*, not by row: the orchestrator owns its pool,
tracks worker liveness, and reclaims by re-assigning the component — which overwrites
`worker_reference`, and since `write` fences on `caller == worker_reference`, that fences the old
worker out. A lease was a per-*component* deadline; putting it on `state_log` replicated one value onto
every entity's row. It lives with the orchestrator (in memory, regenerated on takeover; `event_log` if
a durable home is wanted). The data shard has no `reap`.

**`dirty` is a boolean, not a count.** One worker owns the component, so a row is *pending* or
*settled*. No count to increment/decrement, so the old `state_events` reverse index is gone too.

**The base is read live, and the block is the worker's job.** A worker computes `(E, T)` from `(E,
prev)`'s payload at execution, once `prev` is `!dirty`. Checking the *immediate previous* suffices:
rows are created in tic order (nothing earlier appears late) and clean propagates by induction. The
block **cannot** be a store fence — `A += B` reads B, possibly on another shard, and A's shard sees
only A's chain. So the worker must block (it's subscribed to its read targets); the store fences only
`caller == worker_reference`. The most-recent row per entity is **never GC'd**, so `prev` always
exists in `state_log` — the base never comes from `state`.

**Writes are absolute, so replay is free.** The worker sees all of an entity's tic-T events and writes
the *final* value; a dead worker's replay recomputes the identical value from the immutable `T-1` and
`write` skips already-`!dirty` rows. No deltas, no dedup. (Spawns are the exception: an insert isn't
idempotent by value, so a spawned `entity_reference` must be a pure function of `(event_reference,
index)`, not a mutable counter.)

**`state.macro_position_reference` is deliberate duplication.** It's already in the payload's
`position_reference` (high half), but a subscription filters on **columns** — `WHERE
(position_reference >> 16) = X` isn't expressible — so the zone key must be its own indexed column or
there's no per-zone subscription. Only `state` needs it (`state_log` is entity-keyed, no one
subscribes to it per-zone). Writers must keep the two in step; nothing in the schema does.

### `status : u8` = `flags:4 | status:4`

`status` is *where it is*, `flags` is *what was asked / happened*. `event_status.status`:
`QUEUED → GROUPED → ASSIGNED → RUNNING → COMPLETE` (the phase). `flags`: `FAILED` (the terminal
signal, keeping the phase it died in — one flag beside the phase replaces a `QUEUE_FAILED` vs `FAILED`
pair), `PROMOTE` (latched from the program).

`state_status.status`: `OPEN → PROMOTED`. `flags`: `PROMOTE`. **`PROMOTE` is a request, `PROMOTED` a
fact** — one is latched at `claim`, the other set by `write` once the row settles, keeping promotion
idempotent. Composition-settled stays `dirty == false`, not duplicated into `status`.

### Promotion is an action, not a sweep

`state` and `event` are opt-in projections, written only where a program ran `PROMOTE_STATE` /
`PROMOTE_EVENT`. There is no `promote(t)` sweep. So **a program that promotes nothing runs entirely
server-side** — `state_log` composes, `event_log` settles, and no client sees a thing. Visibility is
something a program asks for, not a tax the spine charges.

### `event_reference` is the composition order

Within a component, a worker applies events in ascending `event_reference`. It's already a global
total order (the server byte keeps shards disjoint, the counter orders within), so it needs no
orchestrator-assigned order field and survives failover trivially — it's a property of the events.
Mint it (`server_reference:8 | ++counter:24`), **not** column `auto_inc`, which would increment the
server byte and break the order.

### No `targets` / `reads` columns

Every reference is already in `actions`, so a column would restate the program. The event shard scans
it to group (write targets) and the worker scans it to execute; an action's *signature* declares per
operand written-vs-read ([`../ACTIONS.md`](../ACTIONS.md)), so both sets fall out of one scan with no
game semantics in the spine.

**Still open** (intent §Open): orchestrator assignment (master-hint leaning), the world verb palette,
worker eviction, `POP`'d targets, and cold `find-or-mint`.

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
