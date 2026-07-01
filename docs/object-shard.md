# Object shard

**Status:** design spec (not yet built). This is the plan for the second class of
data shard and the cross-shard transfer protocol that converts things between the
two.

## Why object shards exist

Two motivating reasons, both about escaping the region partition:

1. **Freedom from regions — the biggest reason.** Zone shards are partitioned *by
   region*, so a thing that lives in one can't move across a region boundary
   without a shard-to-shard transfer. Object shards are **not** region-partitioned:
   a free thing or pawn carries its own position, so crossing a region (or zone)
   boundary is just a **position update on its row** — no transfer, no saga. That
   makes boundary crossing, the common case for anything mobile, trivial.
2. **Attaching things to pawns.** A pawn picking up a sword or dragging a chair —
   the carried thing moves *with* the pawn, across regions, indefinitely. That's
   impossible for something affixed into a region shard; it needs the mobile,
   region-independent home an object shard provides. (Pawns are deferred, so this
   is the forward-looking reason; it drives the [instance-identity
   question](#positioning--identity).)

**So keep the two operations distinct:** *movement* (including across boundaries
and pick-up/drop) is a plain position/parent update **within one object shard** —
cheap, frequent, no coordination. The [transfer protocol](#the-transfer-protocol)
is *only* the affixed↔free **conversion** (release/settle) — rarer, and the one
thing that actually crosses shards. Most of what free things do never touches the
transfer machinery at all.

## Two kinds of thing, two kinds of shard

The world holds two representations of "stuff," and each gets its own shard class:

- **Zone (region) shards** — `region_shard` module, exists today. *Location-bound*
  state: the 16×16 terrain plus the things that have **settled into** the world —
  primary things on the floor, things affixed to a wall. Stored the cheap way: a
  packed blob per zone (`cold_zones.tiles: Vec<u16>`, `cold_zones.things:
  Vec<u32>`) with a hot overlay and a GC fold. Great for terrain; no per-object
  identity.
- **Object shards** — `object_shard` module, this spec. *Not* location-bound in
  the affixed sense, but still **positioned and rendered**: freed things now, and
  pawns later — things with a mutable, sub-tile position. One row per object, not a
  packed blob. (Pawns, and the richer instance identity they need, are **deferred**;
  the object shard holds free things first.)

We follow the **Prison Architect** model. You never edit an affixed thing in
place. To move or re-attach something that's part of the world, you first
**release** it out of its zone shard into an object shard, manipulate it there,
and later **settle** it back into a zone shard. The two representations are
converted into each other; nothing is mutated across the boundary.

The `region_shard` module header already anticipates this:

> Mobile things and pawns live in a separate `object_shard` (planned) — something
> dropped into the world starts in an object shard, and becomes part of a region
> shard once it settles.

## Terminology: the "gate" is the world server

In this doc "**gate**" means the **world server** (`server/` crate). There is no
separate gate process — the world server already accepts client logins, resolves
zone→shard routing from the `index`, holds lazy per-endpoint shard connections,
and relays rows to clients. The transfer protocol below is more of the same work,
so it lands in `server/src/ws.rs` alongside the existing relay, not in a new
component. The `gateway/` crate (the pre-login HTTP directory) is unrelated.

## Object shard tables

A free ("loose") thing is a **`hot_things` row plus a `u8` sub-tile offset**. Hot
things and loose things are largely the same shape — the difference is *destiny*: a
**hot thing is on its way to becoming cold** (the GC fold bakes it into the zone's
cold blob and it settles into terrain), whereas a **loose thing has far more
mobility** and never folds — it moves freely, crosses boundaries, attaches to
pawns, and only ever leaves the object shard by an explicit [settle
transfer](#the-transfer-protocol).

So the object shard **reuses hot's row shape and bitemporal primitives**
(`valid_at`, `time`, `sequence`) but has **no cold layer and no hot→cold fold**.
Its "GC" is therefore *not* a fold — it's prior-version reaping, the
[presence reconcile](#self-heal-recount-on-init--periodic-reconcile), and the
[stale-transfer sweep](#witnessing-region-scoped). Because loose things churn
position heavily, reaping keeps the latest and discards history aggressively. The
shard is otherwise **populated and registered like a zone shard** (registers in the
`index` `shards` table via `set_shard`; seeded through a `seed`-style reducer).
**Pawns are deferred** — the object shard holds free things only for now; the
richer instance identity pawns (and attachment) need comes later.

| Table | Purpose | Key fields |
| ----- | ------- | ---------- |
| `free_things` | one row per free (movable, sub-tile) thing | `valid_at (u64 PK)`, `zone_id (u32, indexed)`, `location (u8)`, `rotation (u8)`, `id (u16)`, `offset (u8 = x_off:4 \| y_off:4)` |
| `presence` | per-region object count | `region_id (u32 PK)`, `count (u32)` |
| `transfer` | in-flight transfers, both directions | `transfer_id (u64 PK)`, `direction (u8)`, `state (u8)`, `region_id (u32, indexed)`, `payload (location, rotation, id, offset)`, `claimed_by (u16)`, `claimed_at (u64)`, `created_at (u64)` |

`free_things` is the `hot_things` layout (`region_shard/src/hot.rs`: `zone_id`,
`location`, `rotation`, `id`) **plus `offset`**. The offset gives sub-tile position
at 1/16-tile granularity — 4px on a 64px tile — which is the whole point: affixed
things are tile-snapped, a free thing needs to sit anywhere in the tile to move
smoothly. It's a deliberate *separate* `u8`, **not** carved from the packed thing's
`u10` reserved bits — those stay as headroom for later.

The zone shard (`region_shard`) gains the **same `transfer` table** so it can be a
transfer endpoint too. It does **not** get a `presence` table — zone routing is
static (see [Routing](#routing-static-zones-vs-dynamic-objects)).

## The transfer protocol

A transfer moves one object from a **source** shard to a **destination** shard.
Release = zone→object; settle = object→zone. The two are mirror images, so the
protocol is defined once over abstract source/destination roles; each shard
implements both roles.

The gate drives it, but **the transfer tables are the source of truth, not the
gate.** A gate crash never strands a transfer: the rows are durable, every gate
covering that region witnesses them (§ [Witnessing](#witnessing-region-scoped)),
and every op is idempotent, so any such gate can finish what another started — with
a shard-side sweep as the backstop when no gate currently covers the region.

### State machine (release shown; settle is the mirror)

```
 SOURCE shard (zone)                 GATE (world server)            DEST shard (object)
 ───────────────────                 ───────────────────            ───────────────────
 begin_release ──────▶ out(Pending) ── observes Pending ──▶ receive ─────▶ in(Received)
   · object suppressed                                        · object row inserted LIVE
     from live view                                             · presence[region]++
     (pending-out lock)                                         · object visible now
                                     ◀── observes Received ──
 ack_release ◀───────                                        (gate calls ack on source)
   · delete object from                out(Done)
     packed store
   · out → Done
                                       ── sweep reaps Done / Confirmed rows after grace ──
```

Three hops — `begin_release`, `receive`, `ack_release` — with **roll-forward-only**
semantics:

- **The source suppresses the moment it's Pending.** A pending outbound transfer
  is the *lock*: every gate's live-view overlay hides the affixed thing while a
  pending-out row exists for it. No extra flag needed on the packed thing.
- **The destination inserts the object live on `receive`.** For the brief overlap
  the object physically exists in both shards but renders only on the
  destination (source already suppressed it). Never lost, never double-rendered.
- **The source deletes only on `ack_release`.** `receive` is already durable, so
  the transfer only ever rolls *forward* — no abort/rollback path needed once the
  destination has the object.
- **All three ops are idempotent on `transfer_id`.** Re-running any of them is a
  no-op, so replay by a witness gate is safe.

### transfer_id and idempotency

`transfer_id` is minted by the **source shard** in `begin_release`, from its own
`sequence` allocator plus its `shard_id`, so it's globally unique without any
cross-shard coordination:

```
transfer_id = (source_shard_id as u64) << 48 | source_sequence_stamp
```

It threads through all three ops as the idempotency key. `receive` keyed by an
existing `transfer_id` returns the existing object row; `ack_release` on an
already-Done transfer is a no-op.

### Leases (avoid the thundering herd)

Idempotency makes witness recovery *correct*; a lease makes it *efficient*.
Without one, all ten witnessing gates race to drive every transfer — all correct,
all wasteful. The `transfer` row carries `claimed_by` + `claimed_at`; the
originating gate holds the lease and drives the transfer, and a witness only steps
in once the lease goes stale (originating gate died). Lease = optimization,
idempotency = the guarantee underneath.

### Representative reducers

Each shard implements both roles. Zone-shard side:

```
// source role (release out)  — zone shard sending an affixed thing to an object shard
begin_release(now_ms, zone_id, location, id) -> transfer_id   // creates out(Pending), suppresses the affixed thing
ack_release(transfer_id)                                      // deletes the affixed thing, out→Done  (idempotent)
// destination role (settle in) — zone shard receiving a free thing back
receive_settle(transfer_id, zone_id, location, rotation, id)  // snaps to tile (drops offset), inserts thing (idempotent)
confirm_settle(transfer_id)                                   // in→Confirmed (idempotent)
```

Object-shard side is symmetric: `receive` / `confirm_receive` (destination —
inserts a `free_things` row *with* `offset`, `presence[region]++`) and
`begin_release_object` / `ack_release_object` (source — deletes the row,
`presence[region]--`). Release into a free thing starts `offset` at tile-center;
settle back snaps to the tile and drops it.

## Presence and routing

### Routing: static zones vs dynamic objects

These are **two different routing regimes** — the key thing to keep straight:

- **Zone shards route statically.** `zone_id → region_id → shard_id` via the
  `index` module's `region_shards` table. A gate resolves a zone the instant a
  player anchors near it. No counting involved.
- **Object shards route dynamically.** An object's home shard is *chosen at
  release time* (by placement policy, below) and objects for one region can
  deliberately span several object shards. There is no static `region → object
  shard` row to look up. Instead, **presence is the routing table**: a gate
  discovers where a region's objects live by reading every object shard's
  `presence`, and subscribes an object shard for a region only when that region's
  count is non-zero.

### Presence maintenance

`presence.count[region_id]` is maintained **inside the object shard's reducers**,
transactionally with the object row: `receive` inserts the object row *and*
increments the count in one reducer; release decrements. SpacetimeDB reducers are
atomic, so an ordinary crash rolls back both together — row and count cannot
desync from a crash. Gates only ever *read* presence; they never compute it.

### Self-heal: recount on init + periodic reconcile

Atomicity covers crashes, but **not** snapshot/backup restore, schema migration,
or logic bugs. So presence is treated as reconcilable-derived:

- On module `init`, treat all counts as dirty and **recount by scanning
  `objects`** — cheap insurance on every cold start.
- A scheduled **reconcile sweep** (reuse the `gc_schedule` pattern from
  `region_shard`) periodically recomputes counts from ground truth and corrects
  any drift.

The **real integrity frontier is the cross-shard transfer**, not the intra-shard
count — that's where atomicity doesn't reach and where idempotency + witness
recovery earn their keep.

## Witnessing (region-scoped)

Transfers are recovered by *witness* gates: gates subscribe shards' `transfer`
tables, so if the gate driving a transfer dies, another finishes it (ops are
idempotent, § transfer_id). **Witnessing is region-scoped from the start** — a
gate subscribes `transfer WHERE region_id IN (its active regions)`: the same
regions it already serves players for, and only those. It never witnesses
transfers in a region no player of its cares about. This is why every transfer row
carries `region_id`, and it's a SpacetimeDB query filter, so it costs nothing
extra to express.

Cost is therefore bounded by **coverage, not the global product**. A gate's
transfer subs ≈ (its active regions) × (shards serving them); a transfer fans out
only to the gates covering that region, not all gates; presence subs are per
object shard the gate actually needs. For intuition, the *unscoped* bus (every
gate witnesses everything) at 10 gates / 20 shards would be `gates × shards` ≈ 300
subs with 10-way fan-out per transfer — that's the ceiling scoping stays under, by
keeping each gate to its neighborhood.

Region-scoping makes the **stale-transfer sweep load-bearing, not optional.** A
transfer whose driving gate dies in a region *no* gate currently covers has no
live witness. So each shard runs a scheduled sweep (reuse the `gc_schedule`
pattern) that flags transfers stuck past a timeout; they get re-driven when a gate
next subscribes that region, or aborted if unrecoverable. Nobody's blocked
meanwhile — nobody's looking at that region.

## Placement policy (where to release an object)

When a gate releases a thing, it picks the destination object shard. Bias toward
**the object shard that already serves that region** (non-zero `presence` for the
region) — releasing there adds nothing to the gate's subscription fan-out, since
it's already subscribed to that shard for that region. Isolate this as a pluggable
seam (mirroring `gateway/src/resolve.rs::pick_server`): today = minimize fan-out;
later it takes load and other factors. Note the standing tension — packing a
region onto one shard minimizes fan-out but concentrates load; that's a conscious
trade the seam exists to rebalance.

## Positioning & identity

A free thing carries the same `id` (u16) as its affixed form plus the `u8` offset —
so release/settle is largely a **shape change** (add `offset` on release, snap it
away on settle), not an identity remap. No global u64 id and no companion registry
for now; those belong with the richer instance identity **pawns** need, which is
deferred.

One consequence to nail down: with a sub-tile offset, **several free things can
share a tile**, so `location` alone no longer identifies an object within a zone
the way it does for a tile-snapped affixed thing (one per cell). The object shard
needs a per-instance handle for updates and transfers. The bitemporal `valid_at`
PK already gives each row identity; whether free things also need a *stable*
cross-update instance id — one that survives a move and is distinct from `id`
(= what-kind) — is the one thing left open here.

**Attachment sharpens this.** Reason #2 (a pawn carrying a sword/chair) needs a
loose thing to reference its **parent** — the carrier — so its position derives
from the pawn instead of a zone `location`. That reference is a stable instance id
by another name: you can't say "sword is attached to pawn 7" without pawn 7 and the
sword both having durable handles. So the stable-instance-id decision and the
attachment model are the same decision, and both land with the deferred pawn work.
The free-thing row should leave room for a future `parent`/attachment field.

## Client-side rendering

A rendered zone becomes a **three-way merge**: cold + hot (zone shard, packed,
tile-snapped) plus free things (object shard, `hot_things` layout + `offset`),
overlaid in that order. Free things render at their sub-tile `offset` (1/16 tile),
so they can sit and move between grid cells where affixed things snap. The client's
zone view (`client/src/zones.rs`, and the pixijs viewport) gains a second data
source keyed by the same `zone_id` it already demuxes on. (Pawns will render
through this same free-thing path once added.)

## Implementation plan

Phased so each step is independently verifiable, mirroring how `region_shard` was
brought up.

1. **`object_shard` module skeleton.** New `spacetime/server/modules/object_shard`
   reusing `region_shard`'s hot machinery: `free_things` (the `hot_things` layout +
   `offset`) + `presence` tables, `init` (recount), a reconcile `gc_schedule`, a
   `seed`-style populate reducer, and CRUD reducers that maintain presence. Registers
   in the `index` `shards` table like a zone shard. DB name has **no underscores**
   (`object-0`, per the `shard-0` naming constraint). Verifiable in isolation: insert
   free things, watch presence stay correct across restart.
2. **Codec + shared types.** The `offset` pack/unpack (`u4 x | u4 y`) and the
   free-thing row layout in `shared/codec`, so shard and client agree (same posture
   as the existing packed helpers).
3. **Gate reads presence.** World server subscribes object-shard `presence`,
   resolves object shards for a region dynamically, and relays free-thing rows to the
   client (new `RowData::FreeThing` variant in `server/src/protocol.rs`).
4. **Client three-way merge.** Render free things (at their sub-tile `offset`) atop
   cold+hot in the pixijs viewport; extend `zones.rs` demux to the object source.
5. **Transfer protocol.** Add the `transfer` table to both modules, the role
   reducers, `transfer_id` minting, and the pending-out suppression lock. Drive the
   three-hop handshake from the world server for a single gate first (no witnesses).
6. **Region-scoped witnessing + leases.** Gates witness `transfer WHERE region_id
   IN (active regions)`; add the lease fields and the **load-bearing**
   stale-transfer sweep; test recovery by killing the driving gate mid-transfer,
   including for an unwitnessed region.
7. **Placement policy seam.** Fan-out-minimizing release placement, isolated for
   later load-awareness.

## Open questions

- **Stable free-thing instance handle** (§ Positioning & identity) — with multiple
  free things per tile, `location` isn't a unique key, and attaching a thing to a
  pawn needs a durable parent reference; both point at the same decision: whether a
  free thing needs a stable instance id (distinct from `id` = what-kind) beyond the
  `valid_at` PK. Lands with the deferred pawn/attachment work; the free-thing row
  should reserve room for a future `parent`/attachment field. This is the only
  substantive one left.
