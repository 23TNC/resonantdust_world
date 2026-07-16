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
| worker | `SELECT * FROM state_log WHERE worker_a = <self> OR worker_b = <self> OR worker_c = <self> OR worker_d = <self>` | `data_shard` — **the 4-way OR is unverified against SpacetimeDB's subscription grammar** |
| edge (per zone) | `SELECT * FROM state WHERE macro_position_reference = <zone>` | `data_shard` |
| edge (per zone) | `SELECT * FROM event WHERE macro_position_reference = <zone>` | `event_shard` |

Two axes, and they're the whole model: a **worker** subscribes by *its own id* (work is pushed to it
by assignment); the **edge and client** subscribe by *locality* (zone). Nothing subscribes to a log
it doesn't own a row in.

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

**`state`'s primary key.** The design line reads `PK target_reference : u64 ; tic : u32 ; payload…`,
which is ambiguous. Taken as PK `target_reference` alone with `tic` a plain column, because the table
is described as "client-visible **latest**" and `promote` does `state.upsert(target_reference, tic,
payload)` — an upsert that replaces. A composite `(target, tic)` key would accumulate a row per tic
and stop being "latest".

**`actions : Vec<u32>` has no word format.** `event_word` was deleted (zero call sites — it belonged
to the old pipeline). The rebuild specifies an RPN program but not its encoding. Narrowed u64 → u32
because an `entity_reference` is a u32 now: whatever the encoding turns out to be, a word carrying a
reference needs 32 bits, not 64.

### No `targets` / `reads` columns

Both were `Vec<u32>` columns holding the issuer-designated write and read sets. They're duplication:
every reference is already in `actions`, so a column spends a vector per row to restate what the
program says. The worker reads the write set off the program and routes by each reference's top byte.

Collecting operands out of a word stream is *structural* — no game semantics in the spine, which was
the point of having the columns in the first place.

**It only works for writes.** Telling which operands a verb *reads* rather than *writes* is game
semantics, and the two look identical in the stream. That's `action_reads_actor`-shaped knowledge and
it's still open — probably answered by the word format, since "how do I find the read set" is really
"what does a word look like".

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

`entity_reference` doesn't wrap, so entity-major locality is real: one entity's slots are contiguous.
Within an entity the rows still sort by raw tic — filter with `tic::` serial comparison, not `<`.
Cheap, because an entity's slot count is bounded by the GC horizon.

**The blocking rule is why.** Everything hot is per-entity-across-tics: *"is any earlier tic for this
entity still dirty?"* (the worker's block check, and `apply`'s re-check), and `request_state` claims a
slot on **every** row for a target. Entity-major makes each a range scan. `promote` walks the table
regardless — no key order can give a wrapping tic a sound range.

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

### The four-worker rule, and why `base` is gone

**Only four servers may act on one `state_log` row in one tic.** `worker_a..worker_d` hold their
`server_reference`s, and a worker subscribes with a fixed-width disjunction:

```sql
SELECT * FROM state_log WHERE worker_a = <self> OR worker_b = <self> OR worker_c = <self> OR worker_d = <self>
```

That bound is what makes the subscription possible at all. The three shapes it beats:

| | why not |
|---|---|
| a single `worker_reference` on the row | multiple events touch one row — one column can't name them all |
| subscribe by target | a bazillion subscriptions |
| subscribe by event | multiple events touch the same rows in one tic |

**The payoff: the worker sees the row, so a hold needs no `base`.** The design's `state_hold` carried
a payload copy *because* the worker couldn't see `state_log` — §"the consequence that drives
everything below", and the §Open cost concern (*"every hold carries a payload copy … measure"*). Both
dissolve: the worker reads the payload off the row it's subscribed to. That is what this rule buys,
and it's why it's worth a hard cap.

**The `lease_*` are plain `tic`s** — u16, same as every other tic, compared with the same `tic::`
serial arithmetic. A u8 would only hold `tic & 0xFF`, which would work (a lease is ~4 tics, far
inside a u8's 127-tic window) but would put the ring math at two widths: `tic.rs` is u16-only, so the
u8 comparison would be hand-rolled at four call sites. That is how nibble orders get reversed. Twelve
bytes of slot state per row buys one ring.

**`dirty : u8`** is the count of events holding a slot; `0` = settled, so `SETTLED` leaves `flags`
(only `PROMOTED` remains).

### The worker keeps its assignment through queueing → running

The design released the worker between phases — `enqueue_done(ok) → status = QUEUE_SUCCESS,
worker_reference = 0` (*"released; re-assigned for execute"*). It no longer does: the worker that
stands the `state_log` rows up and pushes `QUEUEING → RUNNING` keeps the row.

That is what makes early acquisition work. A worker can claim `state_log` slots any time after it
holds the event in `RUNNING`, so it acquires the rows during the enqueue tic and the data is already
in its subscription when the execute tic arrives — one worker, one flow, no re-assignment churn and
no window where nobody owns the event. The subscription is the delivery mechanism, so it has to be
armed a tic early; releasing and re-assigning would disarm it exactly when it's needed.

### Promotion is an action, not a sweep

`state` and `event` are **projections, and opt-in ones**. A row reaches them only when a program
executes `promote_state` / `promote_event`; the master does not sweep settled rows into them.

The consequence is the point: **a program that promotes nothing runs entirely server-side.**
`state_log` composes, `event_log` settles, and `state` / `event` never see a single change — so no
client ever observes it. Visibility becomes something a program asks for rather than something the
spine does by default.

This replaces the design's `promote(t)`, which promoted every settled row automatically:

> `for row in state_log where tic <= t and settled and !promoted: state.upsert(…)`

`flags.PROMOTED` survives to mark what has already been projected, so promotion stays idempotent. `state_events` (`event_reference → Vec<uid>`) is the reverse index that
makes increment/decrement a direct lookup rather than a search — internal, never subscribed, so the
per-event keying that sank the last attempt is fine here: nothing asks it a per-slot question.

**`entity_reference` and `tic` are columns despite being inside `uid`.** A subscription filters on
columns, and a reducer needs them as values — a packed field is neither. Same reason
`state.macro_position_reference` is duplicated out of `position_reference`. The `uid` stays the key;
the columns are for working with.

**A slot is a worker address, not a per-event lock.** One worker holding a thousand events against a
row occupies one slot — the event shard can hand a worker a zillion inspections of a shared object
and they cost that row one address. So the cap binds only when five *distinct workers* want one
entity in one window, which is a partitioning question, not a contention one. With a single worker it
is unreachable.

**If the subscription grammar rejects the 4-way `OR`**, use four subscriptions — one per slot
column. Same rows, same cost, no redesign.

**`state.target_reference` → `entity_reference`** for the same vocabulary, though only `state_log` was
named in the instruction — the two address the same thing and diverging names would be worse than the
edit.

### Ordering — by tic, not by a per-slot head

An earlier sketch carried the design's `events : Vec<event_reference>` per slot, so the store could
answer *"which event composes next here"*. That relation is gone and isn't needed. The rule is:

> A row at tic T may not be written while any earlier tic for that entity is still dirty. The
> entity's settled value is the newest row with `dirty == 0`.

The worker evaluates that itself, because `request_state` gives it a slot on **every** `state_log`
row for its targets — it holds the entity's history, not one slot. `apply` re-checks, since a worker
may have computed from a base another worker dirtied underneath it in the meantime.

That covers ordering *across* tics. Two events at the *same* `event_tic` on the same entity are a
separate question — see the intent doc's §Open.

### `status : u8` = `flags:4 | status:4`

Both logs carry one. `status` is *where it is*; `flags` is *what was asked of it or happened to it*.
The split does real work rather than just packing:

**`QUEUE_FAILED` and `FAILED` collapse into one `FAILED` flag.** They were two terminal states whose
only difference was *which phase* died. With the phase already in `status`, the flag says failed and
`status` says where — `QUEUEING|FAILED` is the old `QUEUE_FAILED`, `RUNNING|FAILED` the old `FAILED`.
An earlier `failed : bool` beside a `QUEUE_FAILED` status was the same information twice; this is it
once, and it keeps the phase, which a lone bool lost.

`event_status.status` is then just the phase — `QUEUED → QUEUEING → RUNNING → COMPLETE` — four
values in a nibble that holds sixteen.

**`PROMOTE` is a flag, `PROMOTED` a status.** One is a request, the other a fact. `PROMOTE` is
latched from the program at `queue` (event) and at `declare_pending` (each slot, sticky — one event
asking is enough). `state_status.status = PROMOTED` is set by `apply` once the slot settles, which
keeps promotion idempotent.

**Settled stays `dirty == 0`** — not duplicated into `status`. One source.

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
