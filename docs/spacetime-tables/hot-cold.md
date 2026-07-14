# Hot and cold — one system the worker doesn't have to think about

The whole point of the split is that **the worker resolves work uniformly**, whether the
object it's touching is settled (cold) or live (hot). Cold↔hot is not a special path in
the worker; it's just more actions in the log. This is the idea the code missed by writing
`pack`/`unpack` as standalone reducers — see [divergences.md](divergences.md).

## The two states ✅

| | **cold** | **hot** |
|---|---|---|
| what | settled, packed, static | minted, live, ticking |
| lives in | `cold` table | `state` / `state_log` |
| addressed by | `cold_reference : u32` (place: region·zone·position·layer·type) | `hot_reference : u32` (a per-server mint handle) |
| ticks? | never | every tic it has events |
| identity source | its geography | a per-server mint counter |

Both references are `u32` on purpose: an event slot holds "a reference" and the **action**
says whether that slot is cold or hot (no discriminator bit). See
[../references/hot-cold-references.md](../references/hot-cold-references.md) for the bit
layouts.

## Shard modules — one per shape, each hot **and** cold ✅

A data shard is the generic engine instantiated **per payload/shape**: **tiles**, **pawns**,
**things** are three modules (three payloads). **Each carries both a hot and a cold table** — a
tile is usually cold but can go hot (an active cell), a thing is cold at rest / hot when carried
or thrown, a pawn is usually hot. That's why `pack`/mint stay *within one module*: promotion
never leaves the shape's shard.

| module (shape) | payload | mostly | but also |
|---|---|---|---|
| **tiles** | tile cell | cold (dense terrain) | hot (active cell) |
| **pawns** | mobile agent | hot | cold (settled/despawned) |
| **things** | object | both | — |

This refines the earlier "tile shard (cold) vs object shard (hot)" two-class framing: it's not
cold-vs-hot *shards*, it's **per-shape modules that each hold both tiers**.

- ✅ **Shape-sharded, not geographic** — a pawn stays in its realm's pawn module as it walks
  around. **Within-realm movement never changes servers.** Only a *realm* crossing does
  (open decision #3 in `object-model.md`).
- ✅ **A shard belongs to exactly one realm.** `realm_reference` is a property of the DB,
  not stored per row. `server_reference : u16 = realm_reference : u8 + server_id : u8`.
- ✅ **All the same generic module** (`decl_tick_pipeline!`); the shape is chosen by the payload
  it's invoked with, the role by deploy (`set_shard_id` + which DB it publishes to).

> **This is the cold/hot axis, not the event/data axis.** Orthogonally, the `event_log` +
> lifecycle live on **event shards** and the game data (`state`/`state_log`/`cold` +
> holder table) on **data shards** — see [lifecycle.md](lifecycle.md). A given deployment
> mixes these axes (e.g. an object shard is a hot data shard).

## The bridge — mint is absorbed into enqueue; `pack` is the settle ✅

Moving an object cold→hot is **not a separate logged action**. It falls out of the enqueue
phase: a `cold_reference` has no `state_log` row, so *standing up work* for a cold target **is**
minting it hot ([lifecycle.md](lifecycle.md) Phase 1). The action just names the object by its
`cold_reference`; enqueue promotes it and rebinds the target to the `hot_reference`.

```
  enqueue a row whose target is a cold_reference:
       find-or-mint the hot entity at that location   ← idempotent by location
         (append x:4|y:4|layer:4|type_id:4 to the zone's cold_removed delta   — NOT a cold-row rewrite;
          seed the hot state_log row = the pending-work row we needed)
       target := hot_reference
   ⇒ no `unpack`/`mint_hot` action, no execute-time `GET` — the target is hot before execute.
```

The mint records a tiny **removal tombstone** in `cold_removed` rather than mutating the big
`cold` `Vec` (which would re-transmit the whole zone to every subscriber). Clients render `cold`
minus `cold_removed`; GC compacts. See [tables.md](tables.md) §`cold_removed`.

## `PACK` — settle hot → cold, as a worker action (not GC) ✅

The reverse — settling an idle hot object back into cold, and compacting `cold_removed` — is a
**worker-resolved `PACK` action**, *not* a GC job. That keeps all correctness on workers
([lifecycle.md](lifecycle.md) §GC).

```
  PACK (enqueued against a cold row / zone):
     for each hot object at rest here with NO holders (no pending read/write):
         append its object_kind_reference into the cold row
         delete its hot state / state_log
     apply cold_removed tombstones; clear the delta
     stamp the cold row with the tic it became cold
   ⇒ objects that "caught" new activity (holders != 0) are skipped — they stay hot.
```

- **Refcount-gated.** Only pack an object with **zero holders** — one that's genuinely at rest.
  If it caught new pending actions, don't write it to cold; leave it hot.
- **The check-and-write is one atomic transaction** (single shard), so an object can't be grabbed
  in the gap between "decided idle" and "written cold."
- **Re-unpack race (packed out from under an event).** If an event needs a hot object that was
  just packed, it finds it cold, **verifies the cold data is consistent**, then tombstones it
  (`cold_removed`) and re-mints hot — the same `find-or-mint` path.
- ❓ **What triggers a `PACK`** (edge / master / a periodic sweep over zones with settled hot
  objects or accumulated `cold_removed`) is a small open — it just needs an owner.

Why the enqueue-side (mint) matters:

- **The worker has no cold-specific branch at execute.** By the time an action runs, its target
  is already hot (minted at enqueue). The interpreter only ever sees hot refs.
- **Idempotent by location.** `find-or-mint` means a re-drive, or a second event targeting the
  same cold object, reuses the one hot entity — never a duplicate.
- **Ordering & fencing come for free** — the mint happens inside the fenced, recoverable enqueue
  phase, not an out-of-band reducer racing the tick loop.
- ✅ **"To touch a cold object, promote it first" is now automatic** — the edge names the cold
  target by `cold_reference` (all it knows); enqueue does the promotion (`object-model.md`
  §Plans).

## Reference rebinding ✅ (decided — at enqueue, by location)

There is **no execute-time rebind and no `GET` verb**. When a row targets a `cold_reference`,
enqueue's `find-or-mint` promotes it and sets the target to the resulting `hot_reference`
*before* execute — so by the time the action runs, its target is already hot. `find-or-mint` is
**by location** (realm · region · zone · position · layer · type_id — the `cold_reference`'s own
fields, realm implied by the shard), so a re-drive or a second event targeting the same cold
object reuses the one hot entity — **no duplicate, no binding table**. (Closes `object-model.md`
open #1.) The hot `state` must therefore be queryable by location (an index on those fields) —
see [lifecycle.md](lifecycle.md) Phase 1 and implementation S3/S7.
