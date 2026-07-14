# Server simulation pipeline — event/state split, per-object tic frontier, deterministic resolution

**Status:** Design in progress (2026-07-09), branch `0.2`. A sizable server-side
deviation from `0.1`. Nothing built yet. **One risk to spike before committing the
build** (cross-shard subscription feasibility) and **two scope decisions pending**
(backpressure policy, v1 shard mobility) — see [Open](#open--pending-decisions).

This doc covers the **server-side simulation architecture**: how the server computes
world state in ticks across multiple SpacetimeDB shards. It is the companion to and
partial supersession of [`sync.md`](../intent/sync.md): `sync.md`'s **client** model (synced
clock, render delay `D`, interpolate-by-`valid_at`, gray-out laggards) is **retained**;
what changes is the **server** — from a single authoritative event-log-and-project
loop into a three-process pipeline that pipelines simulation one tic ahead of what
clients see. Also a companion to [`object-shard.md`](object-shard.md).

## Why this rework

The `sync-experiment` probe (branch `sync-experiment`) verified the core lesson:
**authoritative tick-based state broadcast keeps every client in sync** — objects
whose positions were updated at a tic stayed identical across all displays, with
none of the old FPS-style per-client reconciliation. That validated moving the
whole game onto a tick-broadcast substrate, and exposed that the current single-
server model conflates three jobs that want to be separate processes:

1. **Advancing time** (a metronome).
2. **Ingesting client intent and serving state to clients** (I/O).
3. **Computing the next state** (compute, potentially heavy, potentially sharded).

Splitting them lets simulation run **one logical tic ahead** of what clients see,
hiding compute latency behind the render delay `D` clients already tolerate, and
lets compute scale horizontally across shards while I/O and the clock stay simple.

## The three processes

- **`server_master`** — the metronome. On a fixed interval it advances `master_tic`
  on every shard (one reducer call per shard). It owns no state logic; it only
  drives the clock forward. (Rename note: the current `server` becomes
  **`server_edge`**; `server_master` and `server_simulation` are new.)
- **`server_edge`** — client I/O. Clients subscribe to `state` *through* edge (they
  see exactly one state per object, at that object's promoted tic). Edge does
  **thin intent validation only** — "does this player control this actor, is the
  action well-formed, is it rate-limited" — against the visible `state`. It does
  **not** simulate or resolve. It appends validated intent to `event_log` at
  `event_tic`. **No distributed locks.**
- **`server_simulation`** — a **pool of stateless compute workers**. Each worker
  claims units of work (a `(target, tic)` resolution), reads the inputs, computes
  the resolved object state, and commits it via a fenced reducer. Workers are
  interchangeable and evictable.

The only authority is the simulation: **apply all reasonable intent, resolve it
authoritatively.** Edge lets borderline intent through; simulation is the sole
place an action is actually adjudicated (and, where illegal at resolution time,
voided).

## Shard classes — object and zone

Two shard classes run the **same** pipeline, differing only in **identity**:

- **Object shards** — free/mobile entities (pawns, in-transit items, projectiles),
  each with its own `object_id`. Dynamic.
- **Zone shards** — the map grid: terrain and things *written into* the world at a
  location. These have **no id of their own**; identity is **`(zone_id, location)`**,
  because position *is* the identity (no id allocation, no lookup). Mostly static —
  and "mostly static" costs nothing, because idle cells write no rows (write-on-change,
  not on-tick). A zone cell enters the pipeline only when an event targets it (a wall
  is damaged, a plant grows, terrain is dug).

Throughout this doc, "object" means **entity** — either form. Two things force a
**unified, tagged `u64` entity key** rather than two separate id spaces:

1. **Event addressing.** An event must target either form, so `actor_id`/`target_id`
   are **`u64` entity keys**: `[tag=OBJ | object_id]` or
   `[tag=ZONE | zone_id<<8 | location]`. The tag also gives global uniqueness across
   classes.
2. **Priority needs a global total order across *both* classes.** Object↔zone cycles
   are real (a pawn trips a trap tile; the trap modifies the pawn), so
   `priority = hash(tic, entity_key)` must range over one comparable key space — else
   a cross-class cycle can't be broken. Not optional.

Table identity stays natural per class (object shard PK `object_id`; zone shard PK
`(zone_id, location)`); the `u64` key is the canonical form for addressing and
priority. **Attach/detach is object↔zone migration:** a free object written to the
world becomes a `(zone_id, location)` cell (and back), via the transfer/receive/ack
saga — see [Cross-shard transfers](#cross-shard-transfers). The two-shard-class model
*is* [`object-shard.md`](object-shard.md) realized; "at most one transfer per tic,
intentional" is build/deconstruct.

## The tic model

Every object carries its own `state_tic` (a **per-object frontier**), so time
advances per object, not globally. The metronome runs at a **configurable rate
(`TIC_HZ`, default 2 Hz)** — one constant, threaded through master's cadence and any
client timing; no rate is baked into logic. Three logical positions on one tic axis:

- **`master_tic`** — the metronome value. The visible "present."
- **`event_tic = master_tic + 2`** — where new intent is appended. Two tics ahead
  of the present.
- **object `state_tic`** — how far each object has been resolved. Healthy objects
  sit at `master_tic`; lagging objects trail behind (and render grayed-out).

The **+2 offset is load-bearing**: it gives the pipeline "queue at tic+2, execute
at tic+1, promote at tic":

- Intent for logical tic `T` is appended while `event_tic == T`, i.e. while
  `master_tic == T − 2`.
- Once master advances past `T − 2`, **no new events for `T` can arrive** — the
  event set for `T` is **sealed**. Since a worker only ever resolves an object's
  `tic+1` when master has advanced there, the events it resolves against are
  always already sealed. This is why edge can skip validation-against-in-flight-
  events entirely: by resolution time the input set is fixed.

Per-object frontiers mean a slow region freezes **locally**: only the objects (and
their downstream dependents) that can't keep up trail behind; the rest of the world
keeps advancing. A frozen object is a correct, bounded state, surfaced to the player
as gray, not an unknown.

## Tables

Three tables per shard. `event_log` and `state_log` are the simulation's working
set; `state` is the client-visible projection.

### `event_log` — intent, sealed per tic
```
u64      event_reference   // autoinc, PER SHARD (sufficient for per-target order)
u16      from_server_id
u32      event_tic
u16      actor_shard_id    // where the actor lives; lets a worker subscribe to it
u64      actor_key         // tagged entity key: [OBJ|object_id] or [ZONE|zone_id<<8|location]
u64      target_key        // tagged entity key (same encoding)
u16      action
[u64; 2] data
u8       status            // for soft-delete + GC + debug
```
Events are appended to the **target's** shard (so all events for one target are
co-located and totally ordered by `event_reference`). `actor_shard_id` tells the
resolving worker where to read the actor's resolved state; the target's shard is
implicit (the event lives on it). `actor_key`/`target_key` are the unified `u64`
entity keys (see [Shard classes](#shard-classes--object-and-zone)) so an event can
address an object or a zone cell uniformly. AoE = one row per target.

### `state_log` — sparse change log + work items + frontier, all in one
```
u64      state_id          // autoinc
u32      state_tic
u16      dirty             // 0 = resolved; >0 = pending (value is event count, informational)
u32      object_id
u16      kind
u32      zone_id
u8       location
u8       rotation
u8       offset
[u64; 2] data
u16      server_id         // fencing: worker currently assigned this (target,tic)
u32      created_at        // eviction: when the pending row was created
u32      assigned_at       // eviction: when a worker took it
u8       status            // soft-delete + GC + debug
```
A single row type does triple duty: **`dirty>0` = work item + frontier-exception
marker; `dirty==0` = resolved value.** Idle objects write **nothing** (see the
sparse-frontier invariant), so this table costs `O(events)`, not `O(objects)`.
**Identity is per shard class:** on object shards the row is keyed by `object_id`
(`+ tic`) and `zone_id`/`location` hold current position; on zone shards it's keyed
by `(zone_id, location)` (`+ tic`) and `object_id` is unused. The canonical `u64`
entity key (for event matching + priority) derives from whichever.

### `state` — client-visible latest
```
u32      object_id
u32      state_tic         // per-object; how stale this object is (drives gray-out)
u16      kind
u32      zone_id
u8       location
u8       rotation
u8       offset
[u64; 2] data
```
`state` is the promotion target at the object's promoted tic. It is **separate from
`state_log` on purpose**: clients must never see the `tic+1` lookahead that lives in
`state_log`; they only ever see promoted `state`.

## The resolution algorithm

Driven per object, one worker per work unit:

1. **Work generation (at a tic bump).** For the advancing tic, iterate `event_log`
   (not objects) and collect the distinct targets with events. Write a `state_log`
   pending row `(target, tic)` with `dirty` = event count. Only targets-with-events
   get rows; everyone else advances for free.
2. **Claim.** A worker claims a pending row (sets `server_id`, `assigned_at`). The
   rule: **all events for one `(target, tic)` are resolved by one worker** — this is
   what makes composition of multiple same-target events deterministic (no cross-
   worker add races).
3. **Read inputs.** Gather the target's events for this tic (ordered by
   `event_reference`) and, for each event's actor, read the actor's resolved state
   at the required tic — subscribing cross-shard via `actor_shard_id` when the actor
   lives elsewhere. Apply the **read rule** and **priority rule** below to decide
   readiness and which tic to read.
4. **Block or resolve.** If any actor isn't ready, park the work item (stay
   subscribed; the actor's row updating wakes you). Otherwise compose all events
   against the inputs into the new object state.
5. **Commit (fenced).** Call the `resolve` reducer with `(target, tic, server_id,
   computed_state)`. Inside its single-shard transaction it: checks
   `row.server_id == caller` (fence — reject evicted/stale workers), writes the
   resolved `state_log` row (`dirty = 0`, filled data), marks the consumed events
   `status = complete`, and **promotes to `state`** per the promotion rule.

The commit is **single-shard atomic**. Cross-shard interaction is **reads only**
(via subscription). The resolve reducer is a **trusted authenticated write of a
precomputed result**, not a re-computation — it has no cross-shard reads inside the
transaction. Correctness therefore rests on worker determinism + the fence.

### The read rule — readiness is *absence of pending work*, not presence of a value

To read actor `Q` for an event at tic `T`:

> **`Q` is resolved at `T` iff `Q` has no `dirty>0` row with `tic ≤ T`.** If resolved,
> its value at `T` is `Q`'s most-recent `dirty==0` row with `tic ≤ T` (idle-carried
> forward). Otherwise **block** on the lowest such pending row.

This is the corrected rule and the subtle one. The naive "is `Q`'s newest resolved
row at tic ≥ T" **deadlocks the most common action in the game**: A attacks B while
nothing attacks A — A writes no new row, its newest resolved row is old, so B blocks
forever on an A-row that never comes. An unchanged object is implicitly resolved at
*every* tic; only a **pending row at or below `T`** means "not yet resolved." Pending
work *above* `T` is irrelevant to `T`.

This is cheap because of the in-order-resolution invariant: since objects resolve
tics in order, **all of an object's pending rows sit above all its resolved rows**,
so "is there pending work ≤ T" is a single `min(tic) where object_id=Q AND dirty>0`
lookup.

### The priority rule — deterministic cycle-breaking

Same-tic dependencies form cycles (mutual combat: A→B and B→A both wait on the
other reaching `dirty==0`; also self-target A→A). Intra-tic causal chaining
*requires* same-tic reads, which *permit* cycles — there is no deadlock-free fully-
causal version. You will read `T−1` on some edge of every cycle regardless; decide
which edge **statically** (cheap) rather than via runtime distributed cycle
detection (expensive).

> When resolving `O`'s event whose actor is `Q` at tic `T`: honor the same-tic read
> (`Q`'s state at `T`) **iff `priority(Q) < priority(O)`; otherwise read `Q` at
> `T−1`** (its previous completed tic).

Edges then only point low→high priority → the dependency graph is a **DAG by
construction** → no cycles → deadlock-free → bounded lag → GC horizon always
advances. `priority` must be a global, deterministic total order **over the unified
entity key** (so object↔zone cycles are comparable); proposed
**`hash(tic, entity_key)`** so mutual-combat "tempo" isn't permanently biased toward
low ids and evens out over tics. Result: **causal/preemptive semantics on priority-
ascending edges** (A kills B ⇒ dead B can't hit C, same tic) and **simultaneous
(1-tic-delayed) on back-edges** — the unavoidable price of deadlock-freedom, paid
at zero runtime cost.

**Priority is per-object, not per-event — keep the two orderings separate.** There are
two distinct orderings and conflating them (putting priority on events) forces
partial/incrementally-versioned object state and breaks atomic resolution:

- *Multiple events on one target* → ordered by **`event_reference`**, composed by that
  target's single worker in one atomic write (invariant #5). `AB AB` is just two
  entries in the compose list.
- *Cross-object reads* (read the actor at `T` or `T−1`) → decided by **`priority(Q)`
  vs `priority(O)`**, per object.

Priority never orders a target's own events; `event_reference` never decides a
cross-object read. `priority = hash(tic, entity_key)`, tie-break `entity_key`. Under
this split, N events between the same pair and full 3-cycles both resolve with one
atomic write per object and no per-event machinery.

**Cycles are never detected — the comparison is purely local.** A shard resolving an
edge compares only `priority(actor)` vs `priority(target)`; it needs zero knowledge
of the rest of the chain. This matters because no shard *can* see a whole cycle: in
A→B→C→A, shard A sees only C→A, shard B only A→B, shard C only B→C. But under a
global total order the three edges cannot all ascend (that needs A<B<C<A), so
**exactly one is the descending back-edge** and reads `T−1` — cutting the wait-cycle
without any shard ever seeing more than its own edge. The `T−1` read uses the same
absence-of-pending-work gate, one tic down.

**Observable consequence — chosen, not incidental.** Resolution *order* is compute
order; all effects land at tic `T`, so for non-lethal resolution the order is invisible
(damage reads attack power, which taking damage doesn't lower — everyone takes their
hit regardless of who resolved first). It becomes visible in exactly one case: when an
object **dies/is-disabled this tic**, since a dead attacker's outgoing effect is
negated. That is the preemption we chose ("dead B can't hit C"). Its flip side:
**mutual lethal exchanges yield no double-KO — the higher-`hash` object survives** (it
reads its attacker at `T`, already dead; the lower one read its attacker at `T−1`,
still alive). This is inherent — preemptive kills and symmetric mutual kills are
contradictory (apply-death-first vs compute-blow-from-pre-death-state). `hash(tic,
object_id)` rotates *which* object preempts each tic, so it's a fair, deterministic
per-tic first-strike, not a standing bias.

### Conserved quantities — a transfer is two events, never a read

The actor-as-read-only-input model is sound **only for things the action doesn't
consume** (position, a damage stat, current HP). It is **wrong for anything the
action takes *from* the actor**, because reading an actor doesn't deplete it — so
two events on different shards can both read A's one dollar and both "give it away,"
minting money:

> A has \$1 and (as credit-only mutations of the recipients) "gives" \$1 to B and \$1
> to C. `credit-B` reads A (\$1) ✓, `credit-C` reads A (\$1) ✓ → B +\$1, C +\$1, A
> still \$1. **One dollar became two.**

The rule: **anything an action removes from its actor is a mutation *of* the actor —
an event targeting the actor (a debit)** — not a read. A transfer is therefore **two
events**: `debit(target = source)` + `credit(target = recipient)`, with the credit
conditioned on the debit's resolved outcome. The double-spend is then caught for free
by **one-worker-per-`(target,tic)`** (invariant #5):

> Two debits now target A. A's single worker applies them in `event_reference` order:
> `debit→B` (\$1→\$0, ok), then `debit→C` (\$0, insufficient → **void**). `credit-B`
> reads `debit→B` = ok → B +\$1; `credit-C` reads `debit→C` = void → C +\$0. A \$0, B
> +\$1, C \$0 — conserved.

Serialization on the source side *is* the overdraft guard; it only requires the
withdrawal be a target-`A` event (which the schema already supports: `target_id = A,
action = debit`). Mutual transfers don't cycle — a debit depends only on the source's
own balance, and the credit depends on the debit, a natural debit→credit DAG.
(Cyclic *conditional* transfers — A pays B iff C pays A, round-robin — can form a
real cycle; there the priority back-edge reads a stale balance and that leg may void.
Deterministic, and acceptable.) **Cross-shard** transfers (debit and credit on
different shards) need the transfer/receive/ack saga — see
[Cross-shard transfers](#cross-shard-transfers).

### Promotion to `state`

Two cases cover every object:

- **At the tic bump:** for each `state_log` row where `object.tic == master_tic` and
  `dirty == 0`, copy to `state`. (The normal pipeline case — objects resolve `tic+1`
  ahead, then promote when master reaches that tic.)
- **On resolve, if `object.tic ≤ master_tic`:** copy immediately. (The catch-up
  case — a lagging object resolving a tic already at/under the visible present;
  promote so the client sees it fast-forward through its backlog.)

Objects still `dirty>0` at the promoted tic are simply not promoted → they hold their
old `state` value → they render stale/gray. That is the localized freeze surfacing.

## Invariants (the load-bearing set)

1. **Sealed-by-+2.** Events for tic `T` are immutable once `master_tic > T − 2`;
   workers only resolve sealed tics.
2. **Immutable once resolved.** A `state_log` row mutates exactly once (`dirty:N→0`,
   data filled); consumers gate on `dirty==0`, so resolved values never change under
   a cross-shard reader.
3. **Absence gates readiness.** Ready ⟺ no `dirty>0` row at or below the read tic.
4. **In-order resolution.** An object resolves tics ascending; pending rows always
   sit above resolved rows (makes #3 a single `min` lookup).
5. **One worker per `(target, tic)`.** Deterministic multi-event composition, ordered
   by per-shard `event_reference`.
6. **Fence in reducer.** The resolve reducer checks `row.server_id == caller` in the
   same transaction as the write; eviction just flips `server_id`.
7. **Priority-DAG.** Same-tic actor reads honored only up the `priority` order; back-
   edges read `T−1`. No cycles, ever — and never detected (purely local comparison).
8. **Conservation via debit-events.** Anything an action removes from its actor is an
   event *targeting* the actor, so all withdrawals funnel through that actor's single
   worker (#5) and overdrafts void. Reading an actor is only for non-consumed data.
9. **Single-shard writes, cross-shard reads.** Every mutation is one atomic per-shard
   reducer; the only cross-shard mechanism is subscription reads.
10. **Determinism (server-side only).** Resolution is a pure function of sealed events
    + immutable input rows: integer/fixed-point math, **no wall-clock, no unseeded
    RNG** (seed from `tic` + actor/target ids). `created_at`/`assigned_at` are
    scheduling metadata only and must never feed resolved state. This applies to
    **server_simulation only** — the client never resolves (see [Client](#client)).

## Cross-shard transfers

When a conserved quantity (or a whole object) crosses a shard boundary, the debit and
credit land on different shards, so single-shard atomicity isn't enough — a debit that
commits without its credit loses the quantity. The **transfer/receive/ack saga**
(detailed in [`object-shard.md`](object-shard.md)) keeps it conservation-safe. Modeled
as ordinary events (`actor`/`target`/`action` with the counterpart shard in `data`),
with a fixed intra-tic ordering:

- **receive first** — so a transferred-in object is present for anything acting on it
  this tic;
- **transfer before ack**;
- **ack last** — the source holds the object/quantity in a *transfer-state* at tic N
  until the ack lands, so nothing is lost mid-flight (if the receive never happens,
  the source stays in transfer-state and retries rather than dropping it).

The dependency chain forces the sequence: the receiving side blocks on the source's
`X` (actor on the source shard) until it's non-dirty, so the source finishes all of
`X`'s events *before* the transfer; the source then blocks on the receiver's `X` for
the ack. **Whole-object migration is naturally at most one per tic** (an object can't
migrate two places at once — and attach/detach from the map is a rare, intentional
op). **Fungible-resource** transfers are *not* so limited: a source can debit to many
recipients in one tic (serialized by its worker), each cross-shard leg its own saga.

## Client

The client **never resolves or projects** — a deliberate divergence from `sync.md`,
where the client ran deterministic projection. Here **only the Rust workers resolve**,
which *removes the client-side determinism tax entirely*: no cross-machine float-drift
or RNG-replay worries, because no client simulates. The client:

- gets `state` per object from `server_edge` (subscriptions only; edge computes no tic
  logic — clean separation of concern);
- retains the `sync.md` **rendering** model — synced clock, render delay `D` —
  **interpolating between successive server states for visual smoothness only** (a
  position lerp, not a sim projection);
- shares just one piece of logic with the server via wasm: the **intent-legality
  predicate** ("can I do this given state?"), used for client-side action previews and
  by edge for thin validation. The predicate *proposes*; the server *disposes*.

**Deferred: stale-object indication.** Per-object frontiers mean a rendered snapshot is
temporally skewed (an object at tic 100 beside one lagging at 97), and lagging objects
render stale. Surfacing that to the player (e.g. gray-out) would require edge to
collect tic information, which crosses the separation we want to keep. Deferred — for
now laggards just render as-is; a staleness channel can be added later if it matters.

## Garbage collection

- Soft-delete via `status` (keeps rows for debug); hard-delete later.
- **GC horizon = global minimum object frontier.** `event_log`/`state_log` rows for
  tic `T` are droppable once every object has resolved past `T`. The priority-DAG
  keeps the min frontier tracking `master_tic`, so GC keeps up; a permanently-stuck
  object would pin the horizon (another reason cycles must be impossible, not merely
  tolerated).
- Retain the latest `dirty==0` row per object as the carry-forward floor.

## Failure & eviction

- **Workers** are stateless and interchangeable. A slow/dead worker's `(target,tic)`
  is reaped on an `assigned_at` timeout and reassigned; the fence (#6) makes the late
  original's write a no-op. Recompute is safe because resolution is deterministic.
- **`server_master`** is a singleton clock; if it stops, the world stops (acceptable
  for a colony sim — confirm). **`server_edge`** down = no ingress/egress, sim
  unaffected. All durable state lives in SpacetimeDB, so processes are crash-
  recoverable by construction; only in-flight worker compute is lost and redone.

## Why intra-tic causal chaining (and not simultaneous ticks)

The alternative — every action reads a frozen `state_tic` snapshot, deltas merge,
chains unfold one hop per tic — is simpler (embarrassingly parallel, no fixpoint, no
cross-shard critical path) but was **rejected**: it can't express "A's kill this tic
preempts B's action this tic" without a hop delay, and we want that causality. The
cost of keeping it is the same-tic dependency machinery (dirty gate, cross-shard
blocking, priority-DAG). The priority-DAG is what makes that cost bounded and
deadlock-free rather than open-ended.

Trade-off accepted knowingly: the resolution of a deep dependency chain is
**sequential and cross-shard within one tic**, so deep persistent chains (a long
melee) are the worst case and also the most gameplay-critical — mitigated by per-
object lag (they trail and catch up rather than stalling the world), not eliminated.

## Build path (to be planned)

Greenfield on `0.2` off stable `0.1`. Keep `chat` / `players` / `regindex` as-is.
Rename `server → server_edge`; add `server_master` + `server_simulation`; the dynamic
layer becomes `event_log` / `state_log` / `state` on **object** and **zone** shard
classes (fresh tables — not grafted onto the bitemporal `free_things`).

- **Phase 1 — within-class pipelines** (no cross-shard subscription). Prove the whole
  machine one class at a time: a pawn moving on an object shard, a tile changing on a
  zone shard. Exercises tic model, work-gen, read rule, priority-DAG, fencing,
  promotion, GC. Minimal action (`move`) + minimal state (position). Acceptance:
  headless bots converge to identical state; scripted mutual-combat + chain resolve
  deterministically (higher-`hash` survives; preemption holds).
- **In parallel — the subscription spike** (gates Phase 2, not Phase 1).
- **Phase 2 — cross-class interaction**: pawn modifies tile; attach/detach via the
  transfer/receive/ack saga. Gated on the spike.

**Drop legacy as replacements land** (not preemptively — keep it building): the
`debug_mover` 150 ms streamer, the position-streaming `free_things` writer, the client
`WorldBridge` projector + `TILE_TRAVEL_MS` dup, and the `sync-experiment` scaffolding
once the real pipeline supersedes it.

## Open / pending decisions

- **[SPIKE — parallel with Phase 1, gates Phase 2] Cross-shard subscription
  feasibility.** The cross-class dependency mechanism assumes a worker can reactively
  subscribe to another shard's `state_log` filtered by entity and gated on
  `min(tic) where dirty>0`, woken on the `dirty→0` transition, at busy churn rates. If
  SpacetimeDB can't express that query cheaply or handle the volume, the blocking
  design needs rework. Phase 1 (within-class) doesn't touch it. Minimal spike: one
  worker blocks on another shard's row and wakes on resolution.
- **[DECIDED 2026-07-09] Backpressure policy.** Dropped the old adaptive controller.
  `event_tic = master + 2` (fixed); rely on per-object lag; keep only a coarse runaway
  guard if `master_tic` outruns the global min frontier by too much.
- **[DECIDED 2026-07-09] Tic rate.** 2 Hz default, `TIC_HZ` configurable.
- **[DECIDED 2026-07-09] Priority function.** `hash(tic, entity_key)`, tie-break
  `entity_key`, over the unified object|zone key space.
- **[DECIDED 2026-07-09] Zones retained as a shard class.** Object shards
  (`object_id`) + zone shards (`(zone_id, location)`) on the same pipeline; static data
  lives on zone shards and costs nothing when idle.
- **[DECISION — leaning defer] v1 object↔zone mobility.** Attach/detach (object↔zone
  migration) is the [`object-shard.md`](object-shard.md) saga — Phase 2. v1 assumes a
  stable entity→shard mapping within a class.
- **[SETTLED] Resolution-logic home.** Resolution runs in the worker (Rust); the
  client never resolves. Shared wasm = the intent-legality predicate only. Determinism
  rules (invariant #10) apply server-side only — commit to no-wall-clock / seeded-RNG /
  integer-math up front, because one nondeterministic action silently forks state.
- **Worker claim/assign/evict protocol** — the fence (#6) is specified; the *claim*
  path (how workers discover unclaimed rows without contention) and the *reaper*
  (who runs it, timeout) are a subsystem to design early in the plan.
- **Client interpolation** — visual lerp between successive server states; small
  rework of the `sync.md` interpolator (no longer a sim projector), scope TBD.
- **[DEFERRED] Stale-object indication** — gray-out etc.; would make edge compute tic
  info, which we're avoiding for now. Add later if needed.
- **Numbers to set** — target tic rate (2 Hz candidate), GC retention window,
  eviction timeout.
- **Is `[u64; 2] data` enough** for all action params / all object state? Confirm
  extensibility before locking the schema.
