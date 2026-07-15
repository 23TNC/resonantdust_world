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
| worker | `SELECT * FROM event_log WHERE worker_reference = self` | `event_shard` |
| edge (per zone) | `SELECT * FROM state WHERE macro_position_reference = <zone>` | `data_shard` |

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

## The rebuild's tables — what was translated, what is open

The shapes in TABLES.md come from [`../intent/spacetime-again/`](../intent/spacetime-again/README.md),
which was written before this repo's reference model collapsed the object_reference union. The design
is authoritative for *structure*; VARIABLES.md is authoritative for *widths*. Where they disagreed,
widths won:

| design said | TABLES.md says | why |
|---|---|---|
| `worker_reference : u16` | `u8` | `server_reference` is `type_id:4 \| server_id:4` now |
| `targets` / `reads : Vec<u64>` | `Vec<u32>` | an `entity_reference` is a u32 |
| `target_reference : u64` | `u32` | same |
| `payload…` | `definition_reference` + `position_reference` + `data` | see below |

**`event_reference` and the global total order.** The design says `u32 auto_inc` and calls the
resulting ascending order "the one property that must not be broken" — multi-target events are
deadlock-free *because* every queue readies in the same order. Under the current model an
`entity_reference` is already a u32 (`server_reference:8 | object_reference:24`), so an event's
identity and its ordering key are the same value: the shard mints the low 24 monotonically, and the
server byte on top keeps two event shards' ranges disjoint. Ascending `entity_reference` is therefore
still a global total order. Note this is **not** SpacetimeDB's `auto_inc` on the whole column — that
would increment the server byte. The shard composes the reference.

**`payload…` is the one thing the design leaves abstract**, and TABLES.md has to be concrete. The
reference model already says what an object *is* — three orthogonal references: what it is
(`definition_reference`), where it is (`position_reference`), and its state (`data`). So the payload
is those three. This is a reading of the model, not a decision the intent doc made; the old shard's
payload (`kind:u16, zone_id:u32, location:u8, rotation:u8, offset:u8, data0:u64, data1:u64`) is
legacy — `zone_id` is retiring, `offset` was `sub_position` and is gone, and `rotation` now lives
inside `data`.

**`state`'s primary key.** The design line reads `PK target_reference : u64 ; tic : u32 ; payload…`,
which is ambiguous. Taken as PK `target_reference` alone with `tic` a plain column, because the table
is described as "client-visible **latest**" and `promote` does `state.upsert(target_reference, tic,
payload)` — an upsert that replaces. A composite `(target, tic)` key would accumulate a row per tic
and stop being "latest".

**`actions : Vec<u32>` has no word format.** `event_word` was deleted (zero call sites — it belonged
to the old pipeline). The rebuild specifies an RPN program but not its encoding. Narrowed u64 → u32
because an `entity_reference` is a u32 now: whatever the encoding turns out to be, a word carrying a
reference needs 32 bits, not 64.

### `targets` / `reads` dropped — supersedes decision #5

Both were `Vec<u32>` columns holding the issuer-designated write and read sets. Dropped as
duplication: the entity_references are already in `actions`, so storing them again spends a vector
per row to say what the program already says.

**This contradicts the intent doc's decision #5**, which is explicit and reasoned:

> A `reads` set on the row, issuer-designated like `targets`. The store must create READ holds
> *before* the worker computes, and it must not interpret the program to discover them — same
> argument that makes `targets` a column: no game semantics in the spine.

TABLES.md wins on shape; the intent doc is now wrong here and needs a pass. What the removal leaves
open — these are real holes, not paperwork:

- **`declare_pending` has no input.** T=1 enqueue does `for target in e.targets:
  home_shard(target).declare_pending(...)`. With no column, the write set has to be recovered from
  `actions` — by the worker (which can interpret) or the store (which decision #5 says must not).
- **`acquire(WRITE)` / `acquire(READ)` likewise**, at T=2.
- **Telling writes from reads is the hard half.** Collecting an `OP_OBJECT`-style operand out of a
  word stream is structural — arguably not "game semantics" and fine in the spine. Knowing which
  operands a verb *writes* versus *reads* is not: that's `action_reads_actor`, and the design's own
  §Open already names it — *"Who computes `reads`? … it must agree with the VM about which operands
  a verb reads."* Dropping the column doesn't answer that question, it makes it load-bearing.
- **Partition policy** (§Open) proposed assigning work by `home_shard(targets[0])`. No `targets`, no
  `targets[0]`.

None of this blocks the schema — the columns are gone and the shape is smaller. It blocks the
enqueue flow, which isn't built.

### `state_log` — composite `uid`, `flags`, and the events split

**`uid` is not a surrogate.** `reserved:16 | entity_reference:32 | tic:16` — it *is* `(entity, tic)`,
so `find_or_create(entity, tic)` is an exact PK lookup and `entity_reference` / `tic` are read off it
rather than stored again. It replaces the design's `idx (target_reference, tic)`.

**Entity-major, because `tic` wraps.** Putting `tic` high would look like it buys `promote`'s
`where tic <= t` a range scan. It doesn't: the table would sort by *raw* tic, and a wrapping tic makes
raw order ≠ temporal order. With rows spanning a wrap (65530…65535, 0…5) and `t = 3`, a numeric range
scan returns 0…3 and **misses 65530…65535** — the oldest rows, the ones most needing promotion. No key
ordering can give a sound tic range while the tic is a ring, so `promote` scans and serial-compares
regardless of layout, and `tic` costs nothing in the low bits.

`entity_reference` doesn't wrap, so entity-major locality is real: one entity's slots are contiguous,
which is what the read rule needs (below). Within an entity the rows still sort by raw tic — filter
them with `tic::` serial comparison, not `<`. Cheap, because an entity's slot count is bounded by the
GC horizon.

**The read rule is why.** `refresh_ready`'s READ branch asks *"no `state_log` row `(entity, t)` with
`t <= h.tic` and not settled"* — a per-entity question, and `refresh_ready` is the scheduler's hot
path (every `apply`, every `acquire`). Entity-major gives it a range scan. `promote` runs once per tic
from the master and has to walk the table anyway to find settled-and-not-promoted rows.

**Why `entity_reference` and not `object_reference`.** The slot's 32 bits are an `entity_reference`
(`server_reference:8 | object_reference:24`), not a bare `object_reference` (u24). The server half is
load-bearing: it says *which* object exactly. A shard could in principle imply it — `declare_pending`
routes to `home_shard(target)`, so every row is homed locally — but the reference is then no longer
self-describing, and a bare handle only means something next to the server that minted it.

**`state.macro_position_reference` is deliberate duplication.** It's already inside the payload's
`position_reference` (high half, `region:8 | zone:8`), but a subscription filters on **columns** —
`WHERE (position_reference >> 16) = X` isn't expressible. The edge scopes a client's world to the
zones it subscribes, so the zone key has to be its own indexed column or there is no per-zone
subscription. The old shard carried the same duplication for the same reason
(`SELECT * FROM cold WHERE macro_position = …`).

Only `state` needs it. `state_log` is keyed by entity and no one subscribes to it per-zone; workers
subscribe to nothing but their holds.

Writers must keep the two halves in step — the column is a projection of the payload, and nothing in
the schema enforces that. A row whose `macro_position_reference` disagrees with its
`position_reference` is invisible in the zone it's actually in, and visible in one it isn't.

**`flags : u8` replaces `settled` + `promoted`.** Two bools were two bytes in practice. Bit 0
`SETTLED`, bit 1 `PROMOTED`, six spare.

**`state_events` is 1:1 with `state_log`, split by access pattern.** `state_log` is read and written
every tic by `refresh_ready` / `apply`; the event vector changes only when the event set does. Keeping
a `Vec<u32>` inline made every composition slot variable-width for a field most passes don't touch. A
slot with no pending events has no row, which also makes "settled" checkable by absence.

**`state.target_reference` → `entity_reference`** for the same vocabulary, though only `state_log` was
named in the instruction — the two address the same thing and diverging names would be worse than the
edit.

### `failed` dropped — folded into `status`

Pure redundancy: every site that set `failed = true` also set `status = QUEUE_FAILED`
(`e.status = QUEUE_FAILED ; e.failed = true`), and the terminal-state query already reads
`status in (COMPLETE, QUEUE_FAILED)`. The bool never carried information the status didn't.

`FAILED` was added alongside `QUEUE_FAILED` because the design's enum has no terminal value for the
**execute**-phase failure its T=2 `event_shard.fail(e.event_reference)` triggers — the enum only
covers the enqueue path. Keeping the two distinct preserves which phase failed, which the old
`status` carried and a single flag would have lost.

**Still open in the design itself** (not translation gaps): partition policy, read-set derivation,
`base` copy cost, and where cold `find-or-mint` lives. See the intent doc's §Open.

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
