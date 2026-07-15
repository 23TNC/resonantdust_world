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

**The `lease_*` are `tic` low bytes, not tics.** A `tic` is a u16; a `u8` lease can only hold
`tic & 0xFF`, compared with **u8** serial arithmetic — a 127-tic window instead of 32767. That is
ample for a lease (`CLAIM_LEASE_TICS` is 4, ~2s at 2 Hz) and it hard-caps one at 127 tics, which is
a bound worth knowing rather than discovering. It also means the ring math exists at two widths:
`tic.rs` is u16-only today and needs u8 variants, or the comparison gets hand-rolled at each of the
four call sites — which is exactly how nibble orders get reversed.

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

**Two things to check before building on this:**

1. **Does SpacetimeDB's subscription grammar accept a 4-way `OR`?** Subscription SQL is a subset, and
   this design has no fallback if the answer is no — the whole shape rests on that one query.
   Verifiable only against a live DB (see §The subscription hazard).
2. **What happens to the fifth server?** The cap is a real bound on concurrency, not just a schema
   convenience: a row that five servers want has no slot for the fifth. Failing its event is
   correct-ish (the causality guard already fails events that miss their tic) but it means load, not
   logic, can fail an event.

### The per-event hold — tried, deleted (2026-07-15)

An earlier `state_events` — `event_reference → worker_reference | lease_tic | Vec<entity_reference>`
— tried to be *both* the write set and the worker's subscription in one per-event row. Deleted. The
name came back for a different thing (above: internal, never subscribed), so keep the two apart. Two
facts killed the per-event hold, and they still constrain anything that carries a hold:

**`base` is per-target; the row was per-event.** The worker can't see `state_log`, so the value to
compute from has to ride its hold (`vm.run(actions, base: hold.base, …)`). A row keyed by event, with
a vector of entities, would need one `base` per element of that vector — the only field that varies
*within* the vector. Every field that is genuinely per-event (`worker_reference`, `lease_tic`) fit
fine; `base` is the one that doesn't. **Whatever holds `base` must be keyed per (target, tic).**

**The composition head is a per-slot question.** `refresh_ready` needs `row.events.first()` — the
lowest pending `event_reference` for `(entity, tic)` — and `apply` fences on the same value ("only
the head may write"). A per-event table answers the transpose ("which entities does X touch?"), so
the head would need a scan, and it still couldn't finish: no `tic` on the row, so an event landing on
`(E, T)` is indistinguishable from one on `(E, T+1)`, and `event_tic` lives on the *event* shard,
which a data shard cannot see. **Whatever answers the head must be keyed per (entity, tic) and know
the tic.**

Both point the same way: the data shard's per-slot relation is the load-bearing one. The per-event
write set is a convenience for `withdraw_pending` / `release_holds` and can be derived from it.

Decision #4 calls this ordering "the one property that must not be broken" — it's what makes
multi-target events deadlock-free. So the head lookup needs a home. Three shapes that would give it
one:

| | |
|---|---|
| add `tic:u16` to `state_events` | Answers "does X touch E at T", still by scan. Cheapest edit, worst lookup. |
| key it `(entity, tic)` again | i.e. the previous `state_uid`-keyed `events: Vec<u32>`. Answers the head directly; loses the per-event write set that `withdraw_pending` / `release_holds` want. |
| keep both | The write set *is* the transpose of the pending lists. Two relations, one truth — they must not drift. |

Nothing is built, so this costs a doc edit today and a scheduler bug later.

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
