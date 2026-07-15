# spacetime-again — event/data shard split + per-worker subscriptions

> **Status: PSEUDOCODE / intent.** Revised 2026-07-15 from the original sketch (commit `7529597`
> — read that diff for what changed and why). Nothing built. Terminology:
> [reference-model.md](../../components/shared/codec/design/reference-model.md); the model it
> replaces: [shard/intent/lifecycle.md](../../components/server/spacetime/modules/shard/intent/lifecycle.md).

> ⚠️ **The table shapes below are superseded by [`docs/TABLES.md`](../../TABLES.md)**, which is
> authoritative for shape. This doc remains authoritative for *flow* — the phases, the scheduler, the
> ordering argument. Known divergences (2026-07-15):
> - **`targets` and `reads` are not columns.** Dropped as duplication of `actions`. This voids
>   **decision #5** and leaves `declare_pending` / `acquire` without an input — the write and read
>   sets must now be recovered from the program, and *who* does that is unresolved. §Open's partition
>   policy (`home_shard(targets[0])`) loses its input too.
> - **`failed` is gone**, folded into `status` (it was pure redundancy with `QUEUE_FAILED`).
> - **Widths**: `tic` is a wrapping `u16`; `worker_reference` `u8`; entity keys and
>   `target_reference` `u32`; `actions` `Vec<u32>`.
>
> The pseudocode below still reads `e.targets` / `e.reads` / `e.failed`. Treat those as the open
> question, not as the shape.

**What this is for.** Two problems the current pipeline has:
1. **Every worker mirrors the whole shard** — 6 blanket `SELECT *` subscriptions. It's a "pool"
   where every member holds every row and they race on `claim`.
2. **The queue and the log are the same table** — `event_log` is both, so there is no narrow
   relation a worker could subscribe to instead.

**The two ideas that fix them.**
- **A worker's subscription is its work.** `WHERE worker_reference = self` on exactly two tables.
  A worker sees **nothing else** — no `state`, no `state_log`, no `cold`.
- **The queue is not the log.** `event_log` holds only work *in flight*; settled rows move to
  `event`. The queue stays small, so subscribing to it is cheap.

**The consequence that drives everything below.** If the worker can't see `state_log`, the hold
must carry *both* the go-signal **and** the value to compute from. That's why `state_hold` gained
`ready` + `base` and lost `dirty_count` (a count can't answer either question — see §Decisions).

---

## Tables (shape only; bit layouts per the reference model)

```
event_shard
  event_log            THE QUEUE — in-flight only
    PK  event_reference : u32  auto_inc      also the composition order (ascending)
    idx worker_reference: u16                ← the subscription key. 0 = unassigned
    idx event_tic       : u32
    idx status          : u8                 queued|queueing|queue_success|running|complete|queue_failed
        actions         : Vec<u64>           the RPN program
        targets         : Vec<u64>           issuer-designated write set (entity_keys)
        reads           : Vec<u64>           issuer-designated READ set (entity_keys)   ← NEW, see §Decisions
        lease_tic       : u32                assignment expiry — the reclaim clock
        failed          : bool

  event                THE LOG — settled history/audit/replay. Never subscribed by workers.
    PK  event_reference : u32 ; + the above, frozen

data_shard
  state_log            per (target, tic) composition slot
    PK  uid            : u64
    idx (target_reference, tic)
    idx tic
        payload…                             composed SO FAR (seeded from tic-1)
        events         : Vec<u32>            event_references still to apply, ASCENDING
        settled        : bool                events empty → eligible to promote
        promoted       : bool

  state                client-visible latest. Workers never subscribe; the EDGE does.
    PK  target_reference : u64 ; tic : u32 ; payload…

  state_hold           the worker's ONLY window into this shard
    PK  uid              : u64
    idx worker_reference : u16               ← the subscription key
        target_reference : u64
        tic              : u32
        event_reference  : u32
        kind             : u8                WRITE | READ
        ready            : bool              ← the module's answer to "is it my turn?"  (= !blocked)
        base             : payload…          ← what to compute FROM (the worker can't read state_log)
        lease_tic        : u32
```

---

## T=init — a worker's entire world

```
for shard in spacetime.event_shard.served_by(self):
    subscribe: SELECT * FROM event_log  WHERE worker_reference = self.server_reference
for shard in spacetime.data_shard.served_by(self):
    subscribe: SELECT * FROM state_hold WHERE worker_reference = self.server_reference
// and NOTHING else. No state, no state_log, no cold, no event.
// Everything the worker needs to compute arrives on a hold.
```

## T=0 — queue

```
client.core  → server.edge: request
server.edge  validates (authenticated · owns the object · legal verb · rate)
server.edge  → event_shard.queue(actions, targets, reads):
    event_log.insert{ event_reference : auto,
                      event_tic       : master_tic + TIC_GAP(3),
                      status          : QUEUED,
                      worker_reference: 0, lease_tic: 0, failed: false,
                      actions, targets, reads }
```

## T=1 — assign, then enqueue

```
// ── assignment is PULL, and this one reducer is also the ONLY rescue path ──────────
// Filtered subscriptions mean an orphaned row is visible to NOBODY. So reclaim lives here,
// at the guaranteed entry point (workers poll it every tic).
worker → event_shard.request_work(self.server_reference, max_batch):
    now = master_tic

    // 1. RECLAIM expired assignments (a worker died mid-flight).
    for e in event_log where status in (QUEUEING, RUNNING) and e.lease_tic < now:
        e.worker_reference = 0
        e.status = (e.status == QUEUEING) ? QUEUED : QUEUE_SUCCESS   // rewind to its phase's start
        // its holds expire the same way — see data_shard.reap() below

    // 2. DROP what can no longer make its tic — the causality guard. In the module, NOT
    //    the worker: a row nobody claimed must still fail, or it rots silently.
    for e in event_log where status in (QUEUED, QUEUEING) and e.event_tic <= now:
        e.status = QUEUE_FAILED ; e.failed = true

    // 3. ASSIGN — bounded batch, ascending, only what is runnable THIS tic.
    n = 0
    for e in event_log where worker_reference == 0 order by event_reference:
        if n >= max_batch: break
        if   e.status == QUEUED        and e.event_tic <= now + 2:   // enqueue window
            e.status = QUEUEING ; e.worker_reference = requester ; e.lease_tic = now + LEASE ; n += 1
        elif e.status == QUEUE_SUCCESS and e.event_tic == now + 1:   // execute exactly one tic early
            e.status = RUNNING  ; e.worker_reference = requester ; e.lease_tic = now + LEASE ; n += 1
    // assigned rows now enter the requester's subscription. It acts on them next pass.

// ── enqueue: declare the write set, so the data shard can order it ────────────────
for e in worker.event_log where status == QUEUEING:
    if e.event_tic <= master_tic:                       // missed the window
        worker → event_shard.enqueue_done(e.event_reference, ok: false) ; continue
    ok = true
    for target in e.targets:
        ok &= worker → home_shard(target).declare_pending(e.event_reference, e.event_tic, target)
    worker → event_shard.enqueue_done(e.event_reference, ok)
        // ok  → status = QUEUE_SUCCESS, worker_reference = 0   (released; re-assigned for execute)
        // !ok → status = QUEUE_FAIL, failed = true, and:
        //       for target in e.targets: home_shard(target).withdraw_pending(e.event_reference, e.event_tic, target)

data_shard.declare_pending(event_reference, tic, target_reference) -> bool:
    row = state_log.find_or_create(target_reference, tic)     // payload seeded from the resolved value at <tic
    if event_reference not in row.events:
        row.events.insert_ascending(event_reference)          // ASCENDING == the composition order
        row.settled = false
    refresh_ready(row)                                        // idempotent — safe to re-drive
    return true
```

## T=2 — execute

```
for e in worker.event_log where status == RUNNING:
    if e.event_tic <= master_tic:                     // the tic is sealed/promoted — too late to write it
        worker → event_shard.fail(e.event_reference) ; continue

    // 1. ask for the holds this event needs (idempotent; safe on re-drive)
    for target in e.targets: worker → home_shard(target).acquire(WRITE, e.event_reference, e.event_tic,     target, self)
    for src    in e.reads:   worker → home_shard(src)   .acquire(READ,  e.event_reference, e.event_tic - 1, src,    self)

    // 2. `blocked` is NOT worker state — it is the absence of a ready hold. Never spin, never
    //    block: leave the row and revisit next pass (the design's "defer, don't block").
    if any hold of e is !ready: continue

    // 3. compute — all targets into scratch, FROM the holds' bases. No table reads.
    scratch = {}
    for target in e.targets:
        scratch[target] = vm.run(e.actions, base: hold(e, target).base, reads: {s: hold(e, s).base})

    // 4. commit — ONE call per data shard (atomic there), never one per target.
    for shard, group in scratch.group_by(home_shard):
        ok = worker → shard.apply(e.event_reference, e.event_tic, group as Vec<TargetState>)
        if !ok: break                                  // convergent: re-drive replays; apply is idempotent
    if all ok: worker → event_shard.complete(e.event_reference)

data_shard.acquire(kind, event_reference, tic, target_reference, worker_reference):
    hold = state_hold.find_or_create(target_reference, tic, event_reference, kind)
    hold.worker_reference = worker_reference ; hold.lease_tic = master_tic + LEASE
    refresh_ready(state_log(target_reference, tic))

// THE scheduler. One place decides ordering, and it is the store, not the worker.
data_shard.refresh_ready(row):
    head = row.events.first()                          // lowest event_reference = next to apply
    for h in state_hold where target_reference == row.target_reference and tic == row.tic:
        if h.kind == WRITE:
            h.ready = (h.event_reference == head)      // strict order → deterministic composition
            h.base  = row.payload                      // what the head composes FROM
    for h in state_hold where kind == READ and target_reference == row.target_reference:
        // THE READ RULE: settled through T iff no pending row AT OR BELOW T.
        h.ready = no state_log row (row.target_reference, t) with t <= h.tic and !settled
        h.base  = resolved payload of target_reference at <= h.tic

data_shard.apply(event_reference, tic, results: Vec<TargetState>) -> bool:
    for r in results:
        row = state_log(r.target_reference, tic)
        require(row.events.first() == event_reference)  // ORDER FENCE: only the head may write
        require(hold(r.target_reference, tic, event_reference, WRITE).worker_reference == caller)
        row.payload = r.payload
        row.events.remove(event_reference)
        row.settled = row.events.is_empty()
    release_holds(event_reference)
    for r in results: refresh_ready(state_log(r.target_reference, tic))   // ready the next event
    return true

data_shard.reap():                                     // called by the master with promote()
    for h in state_hold where h.lease_tic < master_tic: delete h   // dead worker's holds
    for row in state_log touched by those: refresh_ready(row)
```

## T=3 — promote (the master, right after `bump`)

```
// The master is the metronome — it is GUARANTEED to run. So liveness lives here, not in a
// worker that may never ask.
master → data_shard.promote(t):
    reap()
    for row in state_log where tic <= t and settled and !promoted:
        state.upsert(row.target_reference, tic: row.tic, row.payload)   // value goes live ON its tic
        row.promoted = true
    gc: delete state_log rows promoted, not-latest, holder-free, older than horizon

master → event_shard.settle(t):
    for e in event_log where status in (COMPLETE, QUEUE_FAILED) and event_tic <= t:
        event.insert(e)          // → THE LOG (history / audit / replay)
        event_log.delete(e)      // → THE QUEUE stays small. This is what makes the
                                 //   worker's subscription cheap, permanently.
```

---

## Decisions (what changed from the sketch, and why)

1. **`status` and `worker_reference` are separate fields.** The sketch assigned `"queueing"` *to*
   `worker_reference`, so `WHERE worker_reference = self` could never match and the worker would
   never receive the work it just requested. Assignment now sets **both**.
2. **`state_hold` carries `ready` + `base`; `dirty_count` is gone.** The worker cannot see
   `state_log`, so it needs (a) a go-signal and (b) a value to compute from. A count gives neither.
   Worse, the read rule is a question about **tics** ("pending at or below T") and a count can't
   answer it. `ready` is computed by the store, which *can* see everything.
3. **`blocked` is not worker state** — it's `!hold.ready`. The sketch read `worker.blocked[...]`
   that nothing ever assigned; that was the read rule and composition order, i.e. all of the
   difficulty, elided. One scheduler (`refresh_ready`), in the store.
4. **Composition = serialize per `(target, tic)` by ascending `event_reference`**, enforced by an
   order fence in `apply`. Each event composes on the previous one's result via `base`.
   **Deadlock-free** for multi-target events *because* `event_reference` is a global total order:
   every queue readies in the same order, which is the classic acquire-in-a-global-order result.
   (This is the one property that must not be broken. If ordering ever becomes per-shard or
   per-target, multi-target events can deadlock.)
5. **A `reads` set on the row**, issuer-designated like `targets`. The store must create READ holds
   *before* the worker computes, and it must not interpret the program to discover them — same
   argument that makes `targets` a column: no game semantics in the spine.
6. **Reclaim + drop live in the module, not the worker.** Filtered subscriptions make an orphaned
   row invisible to every other worker, so `claim`-on-lease-expiry (which works today *only*
   because everyone sees everything — commit `8571018`) is impossible. `request_work` reclaims,
   and the master's `promote` reaps holds. A row nobody claims still fails.
7. **Commit batches per data shard** (`Vec<TargetState>`), not per target. Per-target calls let a
   row half-land; the design says a row is atomic, and [issue 001] exists for exactly this. Across
   shards it stays convergent (idempotent re-drive), which is what `apply` being idempotent buys.
8. **Execute has a late guard** (`event_tic <= master_tic → fail`), mirroring enqueue's. Without it
   a late worker writes into a tic that was already promoted.
9. **`event_log` = queue, `event` = log.** The sketch's T=3 "promote" implied this; it's the point
   worth naming. It's what keeps the worker's subscription small forever, instead of growing with
   history.
10. **Execute at `event_tic - 1`, promote at `event_tic`.** A tic is sealed once no new event can
    land on it: appends go to `master + 3`, so tic N is sealed at `master = N-2`. Executing at N-1
    is therefore safe and makes the value go live exactly on its tic. (Today's worker waits for
    `master >= event_tic`, which over-waits by design-conservatism.)

## Open

- **Partition policy.** `request_work` hands out an ascending batch, so two workers get disjoint
  rows — but nothing makes a worker's rows *local* to the data shards it serves. Cheapest fix:
  assign by `home_shard(targets[0])`. Needs a decision.
- **Read set derivation.** Who computes `reads`? The edge (composing the program) is the natural
  owner, but it must agree with the VM about which operands a verb reads — `action_reads_actor`
  is that knowledge today, and it lives in `shared`.
- **`base` copy cost.** Every hold carries a payload copy. That's the price of the worker not
  subscribing to `state_log`; it's bounded by holds-in-flight, not by object count. Measure.
- **Cold.** The worker can't see `cold` any more, so `find-or-mint` (which resolves a
  `cold_reference` to a hot entity at enqueue) needs a home: probably `declare_pending` mints
  server-side, since the store *can* see cold. This is unsolved and it touches
  [hot-cold.md](../../components/server/spacetime/modules/shard/intent/hot-cold.md).
