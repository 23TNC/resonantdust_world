# spacetime-again — event/data shard split + per-worker subscriptions

> **Status: PSEUDOCODE / intent.** Nothing built. This is the flow; the table shapes are in
> [`docs/TABLES.md`](../../TABLES.md) and the bit layouts in [`docs/VARIABLES.md`](../../VARIABLES.md).
> Terminology: [reference-model.md](../../components/shared/codec/design/reference-model.md).
> Revised 2026-07-15; the model it replaces is
> [shard/intent/lifecycle.md](../../components/server/spacetime/modules/shard/intent/lifecycle.md).

**What this is for.** Two problems the old pipeline had:
1. **Every worker mirrored the whole shard** — 6 blanket `SELECT *` subscriptions. A "pool" where
   every member held every row and they raced on `claim`.
2. **The queue and the log were the same table** — `event_log` was both, so there was no narrow
   relation a worker could subscribe to instead.

**The ideas that fix them.**
- **A subscription is an assignment.** A worker subscribes by *its own id*, on exactly two
  relations. Work reaches it because a shard stamps its id on a row. The edge and client subscribe
  by *locality* instead. Nothing subscribes to a log it doesn't own a row in.
- **The queue is not the log.** `event_log` holds only work in flight; settled rows leave.
- **At most four servers may touch one `state_log` row per tic.** That cap is what lets a worker
  subscribe to `state_log` *itself* (a fixed-width `OR` over `worker_a..worker_d`) rather than to a
  projection of it. So the worker **reads the payload off the row it is assigned** — no hold table,
  no `base` copy, no scheduler mirror.
- **Visibility is opt-in.** `state` and `event` are projections written only by a `promote_state` /
  `promote_event` action. A program that promotes nothing runs entirely server-side.

---

## T=init — a worker's entire world

```
for shard in spacetime.event_shard.served_by(self):
    subscribe: SELECT * FROM event_log WHERE worker_reference = self.server_reference

for shard in spacetime.data_shard.served_by(self):
    subscribe: SELECT * FROM state_log WHERE worker_a = self.server_reference
                                          OR worker_b = self.server_reference
                                          OR worker_c = self.server_reference
                                          OR worker_d = self.server_reference

// and NOTHING else. No state, no event, no state_events, no cold.
// Both subscriptions are keyed by the worker's own id, so a shard hands work over by
// writing that id onto a row. The state_log rows it is assigned carry the payload it
// computes from — there is nothing to copy and nothing else to read.
```

## T=0 — queue

```
client.core → server.edge: request
server.edge  validates (authenticated · owns the object · legal verb · rate)
server.edge  → event_shard.queue(actions):
    event_log.insert{ event_reference : shard mints (server_reference:8 | ++counter:24),
                      event_tic       : tic::add(master_tic, TIC_GAP)          // +3
                      status          : QUEUED,
                      worker_reference: SERVER_REF_NONE,
                      lease_tic       : 0,
                      actions }

// No targets/reads columns. Every reference in `actions` is a u32 whose top byte is the
// server_id that homes it, so the write set is read off the program.
```

## T=1 — assign, then enqueue

```
// ── assignment is PULL, and this reducer is also the ONLY rescue path ─────────────
// Filtered subscriptions mean an orphaned row is visible to NOBODY. So reclaim lives here,
// at the guaranteed entry point (workers poll it every tic).
worker → event_shard.request_work(self.server_reference, max_batch):
    now = master_tic

    // 1. RECLAIM expired assignments (a worker died mid-flight).
    for e in event_log where status in (QUEUEING, RUNNING) and tic::before(e.lease_tic, now):
        e.worker_reference = SERVER_REF_NONE
        e.status = QUEUED                    // rewind to the start: one worker owns the whole flow
        // its state_log slots expire on their own lease_*; see data_shard.reap()

    // 2. DROP what can no longer make its tic — the causality guard. In the module, NOT
    //    the worker: a row nobody claimed must still fail, or it rots silently.
    for e in event_log where status in (QUEUED, QUEUEING) and tic::at_or_before(e.event_tic, now):
        e.status = QUEUE_FAILED

    // 3. ASSIGN — bounded batch, ascending, only what is runnable THIS tic.
    n = 0
    for e in event_log where worker_reference == SERVER_REF_NONE order by event_reference:
        if n >= max_batch: break
        if e.status == QUEUED and tic::at_or_before(e.event_tic, tic::add(now, 2)):
            e.status           = QUEUEING
            e.worker_reference = requester
            e.lease_tic        = tic::add(now, LEASE)
            n += 1
    // assigned rows now enter the requester's subscription. It acts on them next pass.

// ── enqueue: stand the slots up, then KEEP the event ──────────────────────────────
for e in worker.event_log where status == QUEUEING:
    if tic::at_or_before(e.event_tic, master_tic):                    // missed the window
        worker → event_shard.fail(e.event_reference, QUEUE_FAILED) ; continue

    targets = entity references in e.actions            // top byte = the shard that homes it
    ok = true
    for target in targets:
        ok &= worker → home_shard(target).declare_pending(e.event_reference, e.event_tic, target)
    if !ok:
        for target in targets: home_shard(target).withdraw_pending(e.event_reference, target)
        worker → event_shard.fail(e.event_reference, QUEUE_FAILED) ; continue

    worker → event_shard.running(e.event_reference)
        // status = RUNNING. worker_reference is NOT cleared — the SAME worker executes.
        // Releasing here would disarm the subscription exactly when it is needed (below).

// ── acquire EARLY: allowed any time the worker holds a RUNNING event ─────────────
// This is why the worker is not released. It claims next tic's rows during THIS tic, so
// the data has already arrived over the subscription when the execute tic comes.
for e in worker.event_log where status == RUNNING:
    for shard, group in targets(e).group_by(home_shard):
        worker → shard.request_state(self.server_reference, e.event_tic, group)

data_shard.declare_pending(event_reference, tic, entity_reference) -> bool:
    uid = state_uid(entity_reference, tic)
    row = state_log.find_or_create(uid)     // payload seeded from the resolved value at < tic
    if event_reference not already holding row:
        row.dirty += 1                                       // dirty = events holding this slot
        state_events[event_reference].uid.push(uid)          // the reverse index; internal
    return true

data_shard.request_state(worker, tic, entities) -> bool:
    for entity in entities:
        row  = state_log[state_uid(entity, tic)]
        slot = row's worker_a..worker_d already == worker ? that one : first free
        if none free: return false                           // the four-worker cap
        row.worker_<slot> = worker
        row.lease_<slot>  = low8(tic::add(master_tic, LEASE))
    return true
    // the rows now enter the worker's subscription — a tic BEFORE it executes them.
```

## T=2 — execute

```
for e in worker.event_log where status == RUNNING:
    if tic::at_or_before(e.event_tic, master_tic):    // the tic is sealed — too late to write it
        worker → event_shard.fail(e.event_reference, FAILED) ; continue
    if e.event_tic != tic::add(master_tic, 1): continue        // execute exactly one tic early

    // 1. ORDER: only the head event of a slot may compose. Defer, never block —
    //    leave the row and revisit next pass.
    if any slot of e is not headed by e: continue              // [OPEN — see below]

    // 2. compute — from the payload on the rows already in the subscription.
    //    No table reads. No base copy. They arrived at T=1.
    scratch = {}
    for target in targets(e):
        scratch[target] = vm.run(e.actions, base: state_log[state_uid(target, e.event_tic)].payload)

    // 3. commit — ONE call per data shard (atomic there), never one per target.
    for shard, group in scratch.group_by(home_shard):
        ok = worker → shard.apply(e.event_reference, e.event_tic, group as Vec<TargetState>)
        if !ok: break                          // convergent: re-drive replays; apply is idempotent
    if all ok: worker → event_shard.complete(e.event_reference)

data_shard.apply(event_reference, tic, results: Vec<TargetState>) -> bool:
    for r in results:
        row = state_log[state_uid(r.entity_reference, tic)]
        require(head(row) == event_reference)         // ORDER FENCE: only the head may write [OPEN]
        require(event_reference is holding row and caller occupies one of row.worker_a..d)
        row.payload = r.payload
        row.dirty  -= 1
        state_events[event_reference].uid.remove(row.uid)

        // Visibility is opt-in: the program asked, and the slot has no events left.
        if r.promote_state and row.dirty == 0 and !(row.flags & PROMOTED):
            state.upsert(entity_reference          : r.entity_reference,
                         macro_position_reference  : macro_of(r.payload.position_reference),
                         tic                       : tic,
                         payload                   : r.payload)
            row.flags |= PROMOTED
    release_slots(event_reference)             // free its worker_a..d slots + leases
    return true

data_shard.reap():                             // called by the master
    for row in state_log, for slot in a..d:
        if row.worker_<slot> != SERVER_REF_NONE
           and tic8::before(row.lease_<slot>, low8(master_tic)):
            row.worker_<slot> = SERVER_REF_NONE          // a dead worker's slot returns
```

## T=3 — settle (the master, right after `bump`)

```
// The master is the metronome — it is GUARANTEED to run. So liveness lives here, not in a
// worker that may never ask.
master → data_shard.reap()
master → data_shard.gc(t):
    delete state_log rows with dirty == 0, slot-free, not the latest for their entity,
    and older than the horizon        // tic::before, and the horizon must stay << TIC_WINDOW

master → event_shard.settle(t):
    for e in event_log where status in (COMPLETE, QUEUE_FAILED, FAILED)
        and tic::at_or_before(e.event_tic, t):
        if e asked promote_event:
            event.insert{ event_reference, macro_position_reference, event_tic, status, actions }
        event_log.delete(e)          // THE QUEUE stays small — this is what keeps the
                                     // worker's subscription cheap, permanently.
```

**There is no `promote(t)` sweep.** `state` and `event` are written only where a program asked, in
`apply` and `settle`. A program that promotes nothing composes in `state_log`, settles in
`event_log`, and no client sees a thing.

---

## Why it is shaped this way

1. **`status` and `worker_reference` are separate fields.** An early sketch assigned `"queueing"`
   *to* `worker_reference`, so `WHERE worker_reference = self` could never match and the worker would
   never receive the work it just requested. Assignment sets **both**.
2. **The worker is never released mid-flow.** One worker takes an event from `QUEUED` to
   `COMPLETE`. That is what allows early acquisition (T=1), which is the whole reason the execute tic
   has its data ready. A release-and-reassign between phases would disarm the subscription at exactly
   the wrong moment, and open a window where nobody owns the event.
3. **Four servers per row per tic, and the worker reads the row.** Every alternative fails: one
   `worker_reference` column can't name the several events touching a row; subscribing per target is
   a bazillion subscriptions; subscribing per event collides when two events share a row in a tic.
   The cap makes a fixed-width `OR` possible, and *that* is what deletes the hold table, `ready`,
   and `base` — the worker computes from the row it is subscribed to. Cost: load, not logic, can
   fail an event (the fifth server has no slot).
4. **Composition = serialize per `(entity, tic)` by ascending `event_reference`**, enforced by an
   order fence in `apply`. **Deadlock-free** for multi-target events *because* `event_reference` is a
   global total order — every queue readies in the same order, the classic acquire-in-a-global-order
   result. `event_reference` is an `entity_reference`, so the server byte on top keeps two event
   shards' ranges disjoint and the order total. **This is the one property that must not be broken.**
5. **The write set comes from the program.** Every reference in `actions` is a `u32` carrying its
   `server_id`, so the worker knows which shards to call without a `targets` column. No game
   semantics in the spine — collecting operands is structural.
6. **Reclaim + drop live in the module, not the worker.** Filtered subscriptions make an orphaned
   row invisible to every other worker, so `claim`-on-lease-expiry is impossible. `request_work`
   reclaims; the master's `reap` frees dead slots. A row nobody claims still fails.
7. **Commit batches per data shard** (`Vec<TargetState>`), not per target. Per-target calls let a
   row half-land; a row is atomic. Across shards it stays convergent (idempotent re-drive).
8. **Execute has a late guard** (`event_tic <= master_tic → fail`), mirroring enqueue's. Without it
   a late worker writes into a tic that was already promoted.
9. **`event_log` = queue, `event` = log.** What keeps the worker's subscription small forever
   instead of growing with history.
10. **Execute at `event_tic - 1`, promote at `event_tic`.** A tic is sealed once no new event can
    land on it: appends go to `master + 3`, so tic N is sealed at `master = N-2`. Executing at N-1 is
    therefore safe and makes the value go live exactly on its tic.
11. **Every tic comparison is `tic::` serial arithmetic, never `<`.** `tic` is a wrapping u16 ring —
    `0` is *after* `65535`. `<` inverts across the wrap, which would stop the causality guard firing
    and let promote miss its oldest rows. Ordering holds only within `TIC_WINDOW` (32767), which
    bounds the GC horizon.

## Open

- **The composition head.** `dirty` counts the events holding a slot; it does not order them.
  Decision #4 needs *which event is next* — `head(row)` above has no source. `state_events` is the
  transpose (event → slots) and carries no tic. This is the last unshaped piece of the data shard.
- **Read set derivation.** Writes fall out of `actions`; reads do not. Telling which operands a verb
  *reads* rather than *writes* needs `action_reads_actor`-style knowledge, and the references look
  identical in the word stream. Probably resolves with the word format.
- **The word format.** `actions : Vec<u32>` has no encoding — `event_word` was deleted with the old
  pipeline. `promote_state` / `promote_event` are named as verbs but the palette went with it.
- **Does SpacetimeDB's subscription grammar accept a 4-way `OR`?** The whole `state_log` shape rests
  on that one query and has no fallback. Verifiable only against a live DB.
- **Partition policy.** `request_work` hands out an ascending batch, so two workers get disjoint
  rows — but nothing makes a worker's rows *local* to the data shards it serves.
- **Cold.** The worker can't see `cold`, so `find-or-mint` (resolving a settled object to a live one
  at enqueue) needs a home: probably `declare_pending`, since the store *can* see cold. Touches
  [hot-cold.md](../../components/server/spacetime/modules/shard/intent/hot-cold.md).
