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
                      status          : { status: QUEUED,
                                          flags: actions has promote_event ? PROMOTE : 0 },
                      worker_reference: SERVER_REF_NONE,
                      lease_tic       : 0,
                      actions }

// No targets/reads columns. Every reference in `actions` is a u32 whose top byte is the
// server_id that homes it, so the write set is read off the program.
// PROMOTE is latched here, from the program: whoever composes it knows whether it asked.
```

## T=1 — assign, then enqueue

```
// ── assignment is PULL, and this reducer is also the ONLY rescue path ─────────────
// Filtered subscriptions mean an orphaned row is visible to NOBODY. So reclaim lives here,
// at the guaranteed entry point (workers poll it every tic).
worker → event_shard.request_work(self.server_reference, max_batch):
    now = master_tic

    // 1. RECLAIM expired assignments (a worker died mid-flight).
    //    Rewind to THIS PHASE's start, not to the beginning: an event that already declared its
    //    slots must not redo it. The phases exist for exactly this — a worker can die between any
    //    two of them, and reclaim has to know where it was.
    for e in event_log where status.status in (QUEUEING, QUEUE_SUCCESS, RUNNING)
                         and tic::before(e.lease_tic, now):
        e.worker_reference = SERVER_REF_NONE
        e.status.status = (e.status.status == QUEUEING) ? QUEUED : QUEUE_SUCCESS
        // its state_log slots expire on their own lease_*; see data_shard.reap()

    // 2. DROP what can no longer make its tic — the causality guard. In the module, NOT
    //    the worker: a row nobody claimed must still fail, or it rots silently.
    for e in event_log where status.status in (QUEUED, QUEUEING) and tic::at_or_before(e.event_tic, now):
        e.status.flags |= FAILED          // status keeps the phase it died in

    // 3. ASSIGN — bounded batch, ascending, only what is runnable THIS tic.
    n = 0
    for e in event_log where worker_reference == SERVER_REF_NONE order by event_reference:
        if n >= max_batch: break
        if   e.status.status == QUEUED        and tic::at_or_before(e.event_tic, tic::add(now, 2)):
            e.status.status = QUEUEING        // enqueue window
        elif e.status.status == QUEUE_SUCCESS and e.event_tic == tic::add(now, 1):
            e.status.status = RUNNING         // a reclaimed one, already declared: straight to execute
        else: continue
        e.worker_reference = requester
        e.lease_tic        = tic::add(now, LEASE)
        n += 1
    // assigned rows now enter the requester's subscription. It acts on them next pass.
    // Batching by locality here is what keeps the four-slot cap free — one worker holding every
    // event that touches a row takes ONE slot on it, however many events that is. See §Open.

// ── enqueue: stand the slots up, then KEEP the event ──────────────────────────────
for e in worker.event_log where status.status == QUEUEING:
    if tic::at_or_before(e.event_tic, master_tic):                    // missed the window
        worker → event_shard.fail(e.event_reference) ; continue      // flags |= FAILED

    targets = entity references in e.actions            // top byte = the shard that homes it
    ok = true
    for target in targets:
        ok &= worker → home_shard(target).declare_pending(e.event_reference, e.event_tic, target,
                                                          promote: e.actions asks promote_state(target))
    if !ok:
        for target in targets: home_shard(target).withdraw_pending(e.event_reference, target)
        worker → event_shard.fail(e.event_reference) ; continue      // flags |= FAILED

    worker → event_shard.enqueue_done(e.event_reference)
        // status.status = QUEUE_SUCCESS. worker_reference NOT cleared — the SAME worker executes.
        // Releasing here would disarm the subscription exactly when it is needed (below). The
        // state still bumps: if this worker dies, reclaim must know the slots are already declared.

// ── acquire EARLY: allowed as soon as the worker holds a declared event ──────────
// This is what not releasing buys. The worker claims next tic's rows during THIS tic, so the
// data has already arrived over the subscription when the execute tic comes.
for e in worker.event_log where status.status in (QUEUE_SUCCESS, RUNNING):
    for shard, group in targets(e).group_by(home_shard):
        worker → shard.request_state(self.server_reference, group)

data_shard.declare_pending(event_reference, tic, entity_reference, promote) -> bool:
    uid = state_uid(entity_reference, tic)
    row = state_log.find_or_create(uid)     // payload seeded from the resolved value at < tic
    if event_reference not already holding row:
        row.dirty += 1                                       // dirty = events holding this slot
        state_events[event_reference].uid.push(uid)          // the reverse index; internal
    if promote: row.status.flags |= PROMOTE   // sticky: any event asking is enough
    return true

data_shard.request_state(worker, entities) -> bool:
    for entity in entities:
        // EVERY row for that entity, not just the one at event_tic — the worker needs the
        // entity's history to know what is settled and what it is blocked behind.
        for row in state_log where entity_reference == entity:
            slot = row's worker_a..worker_d already == worker ? that one : first free
            if none free: return false                       // the four-worker cap
            row.worker_<slot> = worker
            row.lease_<slot>  = tic::add(master_tic, LEASE)
    return true
    // the rows now enter the worker's subscription — a tic BEFORE it executes them.
    // A slot is a worker ADDRESS, not a per-event lock: one worker holding a thousand events
    // against this row occupies one slot. The cap is on distinct workers, so it only binds when
    // five servers want the same entity in the same window — which locality-batched assignment
    // is what avoids.
```

## T=2 — execute

```
for e in worker.event_log where status.status == RUNNING:
    if tic::at_or_before(e.event_tic, master_tic):    // the tic is sealed — too late to write it
        worker → event_shard.fail(e.event_reference) ; continue      // flags |= FAILED
    if e.event_tic != tic::add(master_tic, 1): continue        // execute exactly one tic early

    // 1. BLOCKED = an earlier tic for one of my targets is still dirty. The worker can see
    //    this: it holds a slot on EVERY row for each target, so it has the history.
    //    Defer, never block — leave the row and revisit next pass.
    for target in targets(e):
        if any state_log row (target, t) with tic::before(t, e.event_tic) and dirty > 0:
            continue outer

    // 2. compute — from the payload on the rows already in the subscription.
    //    base = the newest settled row for the target at or before event_tic. No table
    //    reads, no base copy: they arrived at T=1.
    scratch = {}
    for target in targets(e):
        base = newest state_log row (target, t) with tic::at_or_before(t, e.event_tic) and dirty == 0
        scratch[target] = vm.run(e.actions, base: base.payload)

    // 3. commit — ONE call per data shard (atomic there), never one per target.
    for shard, group in scratch.group_by(home_shard):
        ok = worker → shard.apply(e.event_reference, e.event_tic, group as Vec<TargetState>)
        if !ok: break                          // convergent: re-drive replays; apply is idempotent
    if all ok: worker → event_shard.complete(e.event_reference)

data_shard.apply(event_reference, tic, results: Vec<TargetState>) -> bool:
    for r in results:
        row = state_log[state_uid(r.entity_reference, tic)]
        // The store re-checks the block: a worker may have computed from a base that a
        // faster worker has since dirtied at an earlier tic.
        require(no state_log row (r.entity_reference, t) with tic::before(t, tic) and dirty > 0)
        require(event_reference is holding row and caller occupies one of row.worker_a..d)
        row.payload = r.payload
        row.dirty  -= 1
        state_events[event_reference].uid.remove(row.uid)

        // Visibility is opt-in: the program asked, and the slot has no events left.
        if (row.status.flags & PROMOTE) and row.dirty == 0 and row.status.status != PROMOTED:
            state.upsert(entity_reference          : r.entity_reference,
                         macro_position_reference  : macro_of(r.payload.position_reference),
                         tic                       : tic,
                         payload                   : r.payload)
            row.status.status = PROMOTED
    release_slots(event_reference)             // free its worker_a..d slots + leases
    return true

data_shard.reap():                             // called by the master
    for row in state_log, for slot in a..d:
        if row.worker_<slot> != SERVER_REF_NONE and tic::before(row.lease_<slot>, master_tic):
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
    for e in event_log where (status.status == COMPLETE or status.flags & FAILED)
        and tic::at_or_before(e.event_tic, t):
        if e.status.flags & PROMOTE:
            // ONE ROW PER ZONE the event's targets occupy — so a client subscribed to any one
            // of them sees the event, even though it reached across several.
            for zone in distinct macro_position_reference of e's targets:
                event.insert{ uid: event_uid(zone, e.event_tic, e.event_reference),
                              macro_position_reference: zone,
                              event_tic, event_reference, status, actions }
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
2. **The worker is not released mid-flow, but the phases still bump.** One worker carries an event
   from `QUEUED` to `COMPLETE`, which is what allows early acquisition (T=1) — releasing between
   phases would disarm the subscription exactly when it is needed. The phases remain because a worker
   *can* die: reclaim rewinds to the phase's start, and only the status says which that is. Not
   releasing removes the ownership gap in the happy path; it does not remove the edge case, and the
   edge case is what the states are for.
3. **Four servers per row, and the worker reads the row.** Every alternative fails: one
   `worker_reference` column can't name the several workers touching a row; subscribing per target is
   a bazillion subscriptions; subscribing per event collides when two events share a row in a tic.
   The cap makes a fixed-width disjunction possible, and *that* is what deletes the hold table,
   `ready`, and `base` — the worker computes from the row it is subscribed to. A slot is a worker
   **address**, not a per-event lock: one worker holding a thousand events against a row takes one
   slot, so the cap binds only on distinct workers, which is a partitioning question (§Open).
4. **Ordering is by tic, and `dirty` is the signal.** A row at tic T may not be written while any
   earlier tic for that entity is still dirty; the newest row with `dirty == 0` is the entity's
   settled value. The worker can evaluate that itself because `request_state` gives it a slot on
   **every** row for its targets — it holds the history, not a snapshot. `apply` re-checks, since a
   worker may have computed from a base another worker has since dirtied underneath it.
   (Within a single tic, see §Open.)
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

- **Two events on one `(entity, tic)`.** Ordering *across* tics is settled: an earlier dirty tic
  blocks a later one, and the worker can see it because it holds a slot on every row for the target.
  *Within* one tic it isn't: two events with the same `event_tic` targeting the same entity are both
  unblocked the moment the previous tic settles, and `dirty` counts them without ordering them.
  Whether that can happen — and if so whether ascending `event_reference` breaks the tie — decides
  whether anything more than `dirty` is needed.
- **Read set derivation.** Writes fall out of `actions`; reads do not. Telling which operands a verb
  *reads* rather than *writes* needs `action_reads_actor`-style knowledge, and the references look
  identical in the word stream. Probably resolves with the word format.
- **The word format.** `actions : Vec<u32>` has no encoding — `event_word` was deleted with the old
  pipeline. `promote_state` / `promote_event` are named as verbs but the palette went with it.
- **Partition policy.** `request_work` hands out an ascending batch, so two workers get disjoint
  rows — but nothing makes a worker's rows *local*. This is what keeps the four-slot cap free:
  batching every event that touches an entity onto one worker costs that entity one slot. Ungoverned,
  five workers can want the same entity and the fifth fails on load rather than logic.
- **Cold.** The worker can't see `cold`, so `find-or-mint` (resolving a settled object to a live one
  at enqueue) needs a home: probably `declare_pending`, since the store *can* see cold. Touches
  [hot-cold.md](../../components/server/spacetime/modules/shard/intent/hot-cold.md).
