//! event_shard — the event queue (`event_log`) and the settled, client-visible log (`event`).
//! Shapes: `docs/TABLES.md`. Flow: `docs/intent/spacetime-again/`. This module owns the queue and
//! groups its own events locally; the orchestrator merges across shards and assigns workers, and
//! the worker/master drive the lifecycle. Nothing here calls another component — the orchestrator,
//! worker, and master call *in*.

use spacetimedb::{reducer, table, ReducerContext, Table};

use resonantdust_codec::action;
use resonantdust_codec::object::TYPE_EVENT;
use resonantdust_codec::refs::{pack_entity_reference, pack_server_reference, SERVER_REF_NONE};
use resonantdust_codec::status::{
    pack_status, status_has_flag, status_set_phase, status_with_flag, EVENT_ASSIGNED,
    EVENT_COMPLETE, EVENT_FLAG_FAILED, EVENT_FLAG_PROMOTE, EVENT_QUEUED, EVENT_RUNNING,
};
use resonantdust_codec::tic;
use resonantdust_codec::uid::pack_event_uid;

/// This shard's `server_reference` (`type_id = TYPE_EVENT`, `server_id = 0`) — the high byte of
/// every `event_reference` it mints. One event shard today; a second takes `server_id = 1`.
const SERVER_REFERENCE: u8 = pack_server_reference(TYPE_EVENT, 0);

/// The right-shift: an event queued now becomes valid `TIC_GAP` tics out.
const TIC_GAP: u16 = 3;

// ── the tic clock ────────────────────────────────────────────────────────────────
// Single row this shard *reads*; the master bumps it in lockstep with every other shard. Owned
// here (not shared) so the module is self-contained — the master just writes it. Seeded at init.

#[table(accessor = clock, public)]
pub struct Clock {
    #[primary_key]
    pub id: u8,
    pub master_tic: u16,
}

fn master_tic(ctx: &ReducerContext) -> u16 {
    ctx.db.clock().id().find(0).map(|c| c.master_tic).unwrap_or(0)
}

/// Advance the tic. Called by the master, in lockstep across shards. (Kept minimal until
/// `server/master` exists — it will own the cadence.)
#[reducer]
pub fn bump(ctx: &ReducerContext, master_tic: u16) -> Result<(), String> {
    ctx.db.clock().id().delete(0);
    ctx.db.clock().insert(Clock { id: 0, master_tic });
    Ok(())
}

// ── orchestrator assignment ─────────────────────────────────────────────────────────
// Which orchestrator owns this shard's tics. The MASTER stamps it at standup (`set_orchestrator`);
// `queue` reads it and writes it onto every event row, so the owning orchestrator receives the work
// over `WHERE orchestrator_reference = self`. An event shard with no orchestrator assigned is
// non-functional — `queue` rejects — which is why the edge routes only to shards that have one.

#[table(accessor = orchestrator, public)]
pub struct Orchestrator {
    #[primary_key]
    pub id: u8,
    /// `orchestrator_reference` — `SERVER_REF_NONE` until the master assigns one.
    pub orchestrator_reference: u8,
}

fn orchestrator_reference(ctx: &ReducerContext) -> u8 {
    ctx.db
        .orchestrator()
        .id()
        .find(0)
        .map(|o| o.orchestrator_reference)
        .unwrap_or(SERVER_REF_NONE)
}

/// The master assigns this shard's orchestrator at standup (and reassigns on takeover). Absolute
/// write, master is the sole caller — idempotent.
#[reducer]
pub fn set_orchestrator(ctx: &ReducerContext, orchestrator_reference: u8) -> Result<(), String> {
    ctx.db.orchestrator().id().delete(0);
    ctx.db.orchestrator().insert(Orchestrator { id: 0, orchestrator_reference });
    Ok(())
}

// ── the mint counter ──────────────────────────────────────────────────────────────
// event_reference = server_reference:8 | ++counter:24. NOT SpacetimeDB auto_inc on the column —
// that would carry into the server byte and break the global order. The shard composes it.

#[table(accessor = event_counter)]
pub struct EventCounter {
    #[primary_key]
    pub id: u8,
    pub next: u32,
}

const OBJECT_REF_MASK: u32 = 0x00FF_FFFF;

fn next_event_reference(ctx: &ReducerContext) -> u32 {
    let n = ctx.db.event_counter().id().find(0).map(|c| c.next).unwrap_or(1);
    ctx.db.event_counter().id().delete(0);
    ctx.db.event_counter().insert(EventCounter { id: 0, next: n.wrapping_add(1) & OBJECT_REF_MASK });
    pack_entity_reference(SERVER_REFERENCE, n & OBJECT_REF_MASK)
}

// ── event_log — the queue (in flight only) ─────────────────────────────────────────

#[table(accessor = event_log, public)]
pub struct EventLog {
    /// `entity_reference`, `type_id = TYPE_EVENT`. Ascending = the global composition order.
    #[primary_key]
    pub event_reference: u32,
    #[index(btree)]
    pub event_tic: u16,
    #[index(btree)]
    pub status: u8,
    /// The RPN program — `docs/ACTIONS.md`.
    pub actions: Vec<u32>,
    /// Shard-local group: events sharing a target. Singleton (`= event_reference`) for now — the
    /// orchestrator does the real cross-shard union by scanning `actions`.
    #[index(btree)]
    pub event_group: u32,
    /// The orchestrator that owns this tic (subscription key). Assigned by the master; unset here.
    #[index(btree)]
    pub orchestrator_reference: u8,
    /// The worker the orchestrator assigned (subscription key). `SERVER_REF_NONE` = unassigned.
    #[index(btree)]
    pub worker_reference: u8,
    /// The `macro_position_reference`s this event's targets occupy — provided by the worker at
    /// `complete` (it holds the targets' positions), consumed at `settle` to fan `event` out per
    /// zone. Empty until then.
    pub zones: Vec<u16>,
}

// ── event — the settled, client-visible log ─────────────────────────────────────────
// One row per zone the event's targets occupy, so a client subscribed to any one sees a cross-zone
// event reaching into it. `event_reference` is therefore not the key; `uid` is.

#[table(accessor = event, public)]
pub struct Event {
    #[primary_key]
    pub uid: u64,
    #[index(btree)]
    pub macro_position_reference: u16,
    pub event_tic: u16,
    #[index(btree)]
    pub event_reference: u32,
    pub status: u8,
    pub actions: Vec<u32>,
}

// ── init ────────────────────────────────────────────────────────────────────────

#[reducer(init)]
pub fn init(ctx: &ReducerContext) {
    if ctx.db.clock().id().find(0).is_none() {
        ctx.db.clock().insert(Clock { id: 0, master_tic: 0 });
    }
    if ctx.db.event_counter().id().find(0).is_none() {
        ctx.db.event_counter().insert(EventCounter { id: 0, next: 1 });
    }
    if ctx.db.orchestrator().id().find(0).is_none() {
        ctx.db.orchestrator().insert(Orchestrator { id: 0, orchestrator_reference: SERVER_REF_NONE });
    }
}

// ── queue — the only door in (the edge) ─────────────────────────────────────────────

/// Append a program to the queue. Validates it parses ([`action::program`]) — a wrong arity
/// mis-frames the rest with no re-sync point, so reject here rather than hand a worker garbage.
/// Mints the `event_reference`, stamps `event_tic = master + 3`, latches `PROMOTE` from the program.
#[reducer]
pub fn queue(ctx: &ReducerContext, actions: Vec<u32>) -> Result<(), String> {
    for inst in action::program(&actions) {
        inst.map_err(|e| format!("bad program: {e:?}"))?;
    }

    // No orchestrator ⇒ nothing would ever group or assign this event. Reject at the door rather
    // than accept dead work; the edge routes only to shards that have one, so this fires only in a
    // standup race.
    let orchestrator = orchestrator_reference(ctx);
    if orchestrator == SERVER_REF_NONE {
        return Err("event shard has no orchestrator assigned".into());
    }

    let event_reference = next_event_reference(ctx);
    let event_tic = tic::tic_add(master_tic(ctx), TIC_GAP);
    let flags = if action::asks_promote_event(&actions) { EVENT_FLAG_PROMOTE } else { 0 };

    ctx.db.event_log().insert(EventLog {
        event_reference,
        event_tic,
        status: pack_status(flags, EVENT_QUEUED),
        actions,
        event_group: event_reference, // singleton until real local grouping
        orchestrator_reference: orchestrator,
        worker_reference: SERVER_REF_NONE,
        zones: Vec::new(),
    });
    Ok(())
}

// ── lifecycle (the orchestrator + worker drive these) ───────────────────────────────

fn set_phase(ctx: &ReducerContext, event_reference: u32, phase: u8) -> Result<EventLog, String> {
    let mut e = ctx
        .db
        .event_log()
        .event_reference()
        .find(event_reference)
        .ok_or_else(|| format!("no event {event_reference:#010x}"))?;
    e.status = status_set_phase(e.status, phase);
    Ok(ctx.db.event_log().event_reference().update(e))
}

/// The orchestrator assigns a worker to a work-group's events. Stamps `worker_reference` (the
/// subscription key) and advances to `ASSIGNED`.
#[reducer]
pub fn assign(ctx: &ReducerContext, events: Vec<u32>, worker: u8) -> Result<(), String> {
    for r in events {
        let mut e = ctx
            .db
            .event_log()
            .event_reference()
            .find(r)
            .ok_or_else(|| format!("no event {r:#010x}"))?;
        e.worker_reference = worker;
        e.status = status_set_phase(e.status, EVENT_ASSIGNED);
        ctx.db.event_log().event_reference().update(e);
    }
    Ok(())
}

/// The worker took the event into execution.
#[reducer]
pub fn running(ctx: &ReducerContext, event_reference: u32) -> Result<(), String> {
    set_phase(ctx, event_reference, EVENT_RUNNING).map(|_| ())
}

/// The worker finished. `zones` are the `macro_position_reference`s its targets occupy — kept for
/// `settle` to fan `event` out per zone.
#[reducer]
pub fn complete(ctx: &ReducerContext, event_reference: u32, zones: Vec<u16>) -> Result<(), String> {
    let mut e = set_phase(ctx, event_reference, EVENT_COMPLETE)?;
    e.zones = zones;
    ctx.db.event_log().event_reference().update(e);
    Ok(())
}

/// Mark an event failed — keeps the phase it died in (`GROUPED|FAILED`, `RUNNING|FAILED`, …). Load-
/// shed on a straggler the orchestrator couldn't merge in time, or a worker that missed its tic.
#[reducer]
pub fn fail(ctx: &ReducerContext, event_reference: u32) -> Result<(), String> {
    let mut e = ctx
        .db
        .event_log()
        .event_reference()
        .find(event_reference)
        .ok_or_else(|| format!("no event {event_reference:#010x}"))?;
    e.status = status_with_flag(e.status, EVENT_FLAG_FAILED);
    ctx.db.event_log().event_reference().update(e);
    Ok(())
}

/// The master, each tic: move terminal rows (`COMPLETE`, or `FAILED` at any phase) out of the queue.
/// A `PROMOTE`d event lands in `event` — **one row per zone** its targets occupy — then the queue
/// row is deleted, keeping the queue (and every subscription to it) small forever.
#[reducer]
pub fn settle(ctx: &ReducerContext, through_tic: u16) -> Result<(), String> {
    let terminal: Vec<EventLog> = ctx
        .db
        .event_log()
        .iter()
        .filter(|e| {
            let complete = resonantdust_codec::status::status_phase(e.status) == EVENT_COMPLETE;
            let failed = status_has_flag(e.status, EVENT_FLAG_FAILED);
            (complete || failed) && tic::tic_at_or_before(e.event_tic, through_tic)
        })
        .collect();

    for e in terminal {
        if status_has_flag(e.status, EVENT_FLAG_PROMOTE) {
            for &zone in &e.zones {
                ctx.db.event().insert(Event {
                    uid: pack_event_uid(zone, e.event_tic, e.event_reference),
                    macro_position_reference: zone,
                    event_tic: e.event_tic,
                    event_reference: e.event_reference,
                    status: e.status,
                    actions: e.actions.clone(),
                });
            }
        }
        ctx.db.event_log().event_reference().delete(e.event_reference);
    }
    Ok(())
}
