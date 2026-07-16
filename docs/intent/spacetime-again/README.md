# spacetime-again — orchestrated event/data shards

> **Status: PSEUDOCODE / intent.** Nothing built. This is the **flow**; the table shapes are in
> [`docs/TABLES.md`](../../TABLES.md), the bit layouts in [`docs/VARIABLES.md`](../../VARIABLES.md),
> the program encoding in [`docs/ACTIONS.md`](../../ACTIONS.md), and the reasoning in
> [`docs/notes/`](../../notes/tables.md). Execution: [`work/spacetime-again/`](../../work/spacetime-again/README.md).
> Revised 2026-07-16.

**What this is for.** Simulate a world across many shards, where an event can touch entities on
different shards (`pawn.mana--` on one, `pawn.health--` on another), without locks, without a
distributed-transaction protocol, and without any single point owning the whole world.

**The one idea.** Group each tic's events by **shared target entity** — transitively — so a whole
conflict-**component** lands on **one worker**. Then:

- **One worker per component** ⇒ no two workers ever touch the same entity in the same tic ⇒ no lock,
  no contention. Cross-entity transactions (`if bob has apple, bob.apples--, alice.apples++`) are just
  sequential code in one worker's scratch.
- **Read only `< tic`.** A worker computes tic T from settled `T-1` values. So a row's final never
  depends on any *other* row's same-tic outcome — each row is independently final the instant it's
  written.
- **Write absolute finals** from that base. A dead worker's work replays identically (same base, same
  events, same order), so re-execution is idempotent — no delta accumulation, no dedup.
- **An orchestrator does the grouping.** One coordinator per tic sweeps every event shard, unions the
  events into components *after the event set is complete*, and hands each component to a worker. That
  is what makes "one worker per component" true by construction.

**Subscriptions are assignments.** A worker subscribes by *its own id*; work reaches it because a
shard stamps its id on a row. The edge and client subscribe by *locality* (zone). Nothing subscribes
to a log it doesn't own a row in.

---

## The pipeline

An event queued at N becomes valid at **N+3**. Using N=0 → valid at N=3:

| tic | what happens |
|---|---|
| **N=0** | edge queues events for N=3 (`event_tic = master + 3`). Each event shard groups its own events by shared target into `event_group`s, and stamps `orchestrator_reference` (the orchestrator that owns tic 3). |
| **N=1** | the N=3 set is **frozen** — no event can be born for N=3 after this (they're all born at N=0). The event shard now serves its N=3 groups to the orchestrator (not before — that's the completeness barrier). The orchestrator unions `event_group`s across all shards into **work-groups** (merge any two that share an entity). |
| **N=2** | partition frozen. An `event_group` still spanning two work-groups is **failed** (too late to merge). Workers assigned to work-groups; the assignment stamps `worker`/`observer` onto event and state rows. Workers begin, computing from state at `≤ N=2`. |
| **N=3** | values valid; promoted rows go live. Workers may still be chewing — a slow component just makes its value *late*, not wrong (see §Running behind). |

---

## Queue (N=0)

```
client.core → server.edge: request
server.edge  validates (authenticated · owns the object · legal verb · rate)
server.edge  → event_shard.queue(actions):
    event_log.insert{ event_reference : shard mints (server_reference:8 | ++counter:24),
                      event_tic       : tic::add(master_tic, 3),
                      status          : { status: QUEUED,
                                          flags: actions has PROMOTE_EVENT ? PROMOTE : 0 },
                      actions,
                      event_group     : (assigned by the shard's grouping, below),
                      orchestrator_reference : orchestrator_for(event_tic),
                      worker_reference : SERVER_REF_NONE }

// event_reference is minted, NOT auto_inc on the column — auto_inc would increment the server byte
// and break the global total order. Ascending event_reference is the composition order (below).
```

**Shard-local grouping.** As events arrive, the shard scans each program's write targets
([`ACTIONS.md`](../../ACTIONS.md) — every reference is a `u32` with its `server_id` on top) and unions
events that share a target into an `event_group`. This is the shard's *local* component; the
orchestrator merges local groups across shards next.

## Group + assign (the orchestrator, N=1 → N=2)

```
// One orchestrator per tic (assignment mechanism — §Open; doubling is safe, see below).
orchestrator for tic T:
    subscribe: SELECT * FROM event_log WHERE orchestrator_reference = self   // on every event shard

    // ── N=1: COMPLETENESS BARRIER, then union ───────────────────────────────────────
    // A shard refuses to serve T's groups until it has read master ≥ T-2, i.e. T's event set is
    // frozen. So every batch the orchestrator receives is a shard's COMPLETE contribution.
    wait until a complete batch has arrived from every event shard in the realm
    work_groups = union-find over all event_groups, merging any two that share an entity
    // Union-find on the COMPLETE set is order-independent: any orchestrator computes the same
    // partition. That is why doubling is safe — see §Why.

    // ── N=2: freeze, fail stragglers, assign ────────────────────────────────────────
    for each event_group still spanning two work_groups:      // arrived too late to merge
        event_shard.fail(event_group)                          // load-shed; NOT delayed — see §Why
    for each work_group WG:
        w = pick a worker (least-loaded)
        for shard, events in WG.events.group_by(event_shard):
            event_shard.assign(events, worker: w)              // stamps worker_reference on the rows
        for shard, entities in WG.targets.group_by(data_shard):
            data_shard.claim(entities, tic: T, worker: w)      // stamps worker + observer, below
```

```
data_shard.claim(entities, tic, worker):
    for E in entities:
        row = state_log.find_or_create(state_uid(E, tic))      // dirty = true on create
        row.worker_reference = worker
        prev = most-recent state_log row for E with tic::before(prev.tic, tic)   // always exists — latest is never GC'd
        prev.observer_reference = worker
        // worker now sees, over its subscription: the row it will write (worker), and the
        // previous row it reads as base (observer). Two roles, one per direction.
        // NO lease on the row. If worker dies, the orchestrator re-assigns the component and
        // re-stamps worker_reference here — which fences the old worker out (write checks it).
```

## Execute (the worker)

```
worker subscribes:
    SELECT * FROM event_log  WHERE worker_reference = self                      // its programs
    SELECT * FROM state_log  WHERE worker_reference = self OR observer_reference = self   // its rows + their bases

for WG in my assigned work_groups (identifiable by shared worker_reference):
    // Compose the whole component in local scratch, from T-1 values. One worker owns it, so this is
    // ordinary sequential code — no lock, no coordination.
    order WG.events by event_reference ascending          // the global composition order
    scratch = { E: base(E)  for E in WG.targets }         // base = prev row's payload

    // BLOCK — the correctness requirement, not an optimization. A local shard fence CANNOT enforce
    // this: `A += B` reads B, which may be on another shard, so A's shard can't see B's dirtiness.
    // The worker CAN — it is subscribed to every target it reads. If any target's prev is dirty,
    // defer the whole component and revisit next pass. Never spin, never partial-write.
    if any E in WG.targets has prev(E).dirty:  defer WG ; continue

    for e in WG.events:                                   // sequential, in event_reference order
        apply e.actions to scratch                        // reads other scratch entries freely — same tic, in-worker
    // scratch now holds each target's ABSOLUTE final for tic T.

    for shard, group in scratch.group_by(data_shard):
        data_shard.write(WG.events.refs, tic: T, group)   // one call per shard; absolute values
    event_shard.complete(WG.events)
```

```
data_shard.write(event_refs, tic, results):               // idempotent: absolute values from immutable T-1
    for r in results:
        row = state_log[state_uid(r.entity, tic)]
        require(caller == row.worker_reference)            // only the assigned worker writes
        if !row.dirty:  continue                           // already written (a replay) — skip
        row.payload = r.payload
        row.dirty   = false                                // this row is now final and unblocks its observer
        if (row.status.flags & PROMOTE) and row.status.status != PROMOTED:
            state.upsert(entity_reference: r.entity,
                         macro_position_reference: macro_of(r.payload.position_reference),
                         tic: tic, payload: r.payload)
            row.status.status = PROMOTED
```

Note `write` clears `dirty` **per row**, even if the event's *other* targets aren't done — a row's
value is final regardless of them (reads are `< tic`), so its observer may proceed. The event isn't
`COMPLETE` until every row is written; if the worker dies mid-way, replay finds the written rows clean
(skipped) and the rest dirty (redone) — same finals either way.

## Master (each tic, right after `bump`)

```
master → bump(master_tic + 1) on every shard in lockstep     // the tic advances everywhere at once
// No state_log reap: worker liveness is the orchestrator's, and it reclaims by re-assigning the
// component (re-stamping worker_reference). The master reaps only the ORCHESTRATOR (it owns that
// assignment) — a dead orchestrator's tic gets a fresh one, which recomputes and re-assigns.
master → data_shard.gc(t):
    delete state_log rows that are !dirty, not the latest for their entity, older than the horizon
    // NEVER the latest per entity — that is every base. horizon must stay << TIC_WINDOW.
master → event_shard.settle(t):
    for e in event_log where (COMPLETE or FAILED) and tic::at_or_before(e.event_tic, t):
        if e.status.flags & PROMOTE:
            for zone in distinct macro_position of e's targets:    // one event row PER ZONE it touched
                event.insert{ uid: event_uid(zone, e.event_tic, e.event_reference), ... }
        event_log.delete(e)                                    // the queue stays small forever
```

**Promotion is opt-in.** `state` and `event` are written only where a program ran `PROMOTE_STATE` /
`PROMOTE_EVENT`. A program that promotes nothing composes in `state_log`, settles in `event_log`, and
no client ever sees it — the simulation can run entirely server-side.

## Running behind

Execution isn't tic-gated after assignment — a worker may still be composing N=3 at N=4, N=5, because
its base (N=2 and earlier) persists. Per entity the chain is serial (N+1 blocks on N's row being
clean); across entities it's parallel, and different tics can be different workers. So a stalled
component holds up *its* entity's future, not the world's. Once it clears, downstream workers zip
through the unblocked chain and catch up.

The failure surface is **latency, not correctness**. If health hits 0 at N=3 and a heal was valid at
N=4, the heal fails — and it fails whether computed on time or late, because validity is per *logical*
tic, not wall-clock. Actions on an object that's logically dead get dropped; correct, occasionally
surprising. Overloaded regions lag and self-heal; they don't corrupt.

---

## Why it is shaped this way

1. **One worker per component dissolves the hard problem.** Same-entity contention, cross-entity
   transactions, and conditionals were three faces of one thing: no single agent saw the whole
   conflict set. The orchestrator gives one agent the whole component, so a transaction is just code,
   and there is nothing to serialize because there is no second writer.
2. **The completeness barrier is the load-bearing correctness piece.** Union-find on a *complete* set
   is deterministic, so any orchestrator computes the same partition — which is why **exactly-one
   orchestrator is not required**: two that both wait for completeness produce the same work-groups,
   and a double worker-assignment is safe (idempotent writes). What is *not* safe is assigning on a
   partial view — split `{a,b}` and `{c,d}`, then a late `(b,c)` reveals one component across two
   workers, and they compute `a` from different subsets → corruption. So: **never assign before the
   set is complete**; everything else may double.
3. **Composition order is `event_reference`.** It is already a global total order (the server byte
   keeps shards disjoint, the counter orders within). A worker sorts its component's events by it.
   Deterministic, needs no orchestrator, survives failover trivially — it's a property of the events.
4. **The block is a worker requirement, not a fence.** `A += B` reads B; B may be on another shard; so
   A's shard cannot enforce "B settled." The worker can — it is subscribed to its read targets. Skip
   the block and you read a stale B, write a wrong-but-clean A, and next tic composes on corruption.
   (The store-enforceable alternative is two-phase — write tentative, verify all settled, then clear
   dirty — more round trips. We take "block correctly" while workers are our own code.)
5. **Absolute finals ⇒ idempotent replay.** One worker sees all of an entity's tic-T events, so it
   writes the final value, not a delta. Replay recomputes the identical value from the immutable
   `T-1`. No applied-event set, no dedup. Deltas were only ever needed when several workers each
   contributed a piece to one row — the orchestrator removes that.
6. **Spawns need deterministic ids.** An insert isn't idempotent by value. A spawned `entity_reference`
   must be a pure function of `(event_reference, index)`, not a mutable counter, or a replay inserts a
   duplicate. (Events use the counter; spawns must not.)
7. **Fail, don't delay, on a missed grouping.** Spanning-at-N=2 only fires under orchestrator
   overload. Fail sheds load; delay *re-injects* it (the event re-enters a future tic's grouping),
   which spirals under sustained overload. Delay also reorders order-dependent actions and can't be
   proven to terminate. Surface the drop to the player at the edge; don't hide it as a delay.
8. **The queue is not the log.** `event_log` holds only in-flight work; `settle` moves terminal rows
   to `event` and deletes them. That keeps every subscription (worker's, orchestrator's) small
   forever instead of growing with history.
9. **Every tic comparison is `tic::` serial arithmetic, never `<`.** `tic` is a wrapping u16 ring; `<`
   inverts across the wrap. Ordering holds only within `TIC_WINDOW` (32767), which bounds the GC
   horizon.

## Open

- **Orchestrator assignment + liveness.** Which orchestrator owns tic T, and who notices it died.
  Doubling is safe (§Why 2), so assignment needs a good deterministic hint, not consensus — leaning:
  the **master** assigns per tic (singular, lockstep, guaranteed to run) and reaps a dead orchestrator
  by handing its tic to a fresh one, which recomputes (the orchestrator stays **stateless** — a pure
  function of the shard-durable event set). The master↔orchestrator liveness signal (heartbeat vs
  lease, where) is unshaped.
- **Worker eviction is the orchestrator's**, not the store's. The orchestrator owns its pool, so it
  tracks worker liveness (in memory — regenerated on takeover, since a new orchestrator re-assigns
  everything) and reclaims by re-assigning the component, which re-stamps `worker_reference` and
  fences the old worker out. There is **no lease on `state_log`**. What's unshaped is the exact
  hang-detection (a durable lease on `event_log` is the fallback if in-memory proves insufficient).
- **`POP`'d targets.** A written `entity_reference` must be statically known at grouping, but a
  `POP`'d operand has no value until run time. Default: a written target must be literal; `POP` is for
  numbers and read operands. See [`notes/actions.md`](../../notes/actions.md).
- **The world verb palette.** The machine + `PROMOTE_*` exist ([`ACTIONS.md`](../../ACTIONS.md)); no
  gameplay verb does. Each needs a signature naming, per operand, written vs read.
- **Cold.** No cold tier yet — no table, no `find-or-mint`. Where a settled object is resolved to a
  live one is unshaped.
