//! The transfer protocol — zone-shard side (the *source* of a release).
//!
//! A release moves an affixed thing out of this zone shard into an object shard,
//! where it becomes a loose thing. The three-hop, roll-forward handshake (see
//! docs/object-shard.md), driven by the gate:
//!
//! 1. **`begin_release`** (here) — read the thing currently affixed at
//!    `(zone_id, location)`, mint a `transfer_id`, and record a **pending
//!    outbound** transfer. The thing is *not* deleted yet: the pending row is the
//!    suppression lock (a gate hides it from the live view while pending), and
//!    keeping it means an abandoned transfer can be re-driven or aborted without
//!    losing the thing.
//! 2. `object_shard::receive` — the gate relays the payload; the object shard
//!    inserts the loose thing and writes a receipt.
//! 3. **`ack_release`** (here) — the gate, having seen the receipt, tells this
//!    shard the thing has safely landed; only now is the affixed thing deleted and
//!    the transfer marked done.
//!
//! Both reducers are **idempotent** on `transfer_id`, so any gate can drive or
//! replay the handshake. `transfer_id` is minted here from `pack_valid_at(now_ms,
//! seq)` — unique within this shard; a shard-id high field is a later concern when
//! more than one zone shard can originate transfers.

use spacetimedb::{reducer, table, ReducerContext, Table};

use resonantdust_codec::packed::{
    pack_valid_at, region_of, thing_location, thing_object_id, thing_rotation,
};

use crate::hot::{things_hist, HotThing};
use crate::sequence;
use crate::time::now_ms;
use crate::zones;

/// Outbound transfer (a release leaving here). Settle (inbound, object → zone) is
/// the mirror, added with its gate driver later.
pub const DIR_OUT: u8 = 0;
/// The transfer is recorded and the thing suppressed, awaiting the object shard's
/// receipt.
pub const ST_PENDING: u8 = 0;
/// The thing has landed at the destination and been deleted here — closed.
pub const ST_DONE: u8 = 1;

/// An in-flight (or settled) transfer originating at this shard. Keyed by the
/// `transfer_id` this shard mints.
#[table(accessor = transfers, public)]
pub struct Transfer {
    #[primary_key]
    pub transfer_id: u64,
    pub direction: u8,
    pub state: u8,
    /// The region the thing is leaving — carried for region-scoped witnessing.
    #[index(btree)]
    pub region_id: u32,
    pub zone_id: u32,
    pub location: u8,
    pub rotation: u8,
    /// What-kind def id of the affixed thing being released.
    pub id: u16,
    pub created_at: u64,
}

/// The thing currently affixed at `(zone_id, location)` — the hot overlay if a
/// row exists there (a `0` id means the cell was cleared → nothing), else the cold
/// blob's entry for that cell. `None` when the cell holds no thing.
fn current_thing(ctx: &ReducerContext, zone_id: u32, location: u8) -> Option<(u8, u16)> {
    if let Some(h) = things_hist::prior_at(ctx, zone_id, location, now_ms(ctx)) {
        return if h.id != 0 { Some((h.rotation, h.id)) } else { None };
    }
    let cold = zones::latest(ctx, zone_id)?;
    cold.things.iter().find_map(|&p| {
        (thing_location(p) == location).then(|| (thing_rotation(p), thing_object_id(p)))
    })
}

/// Begin releasing the thing affixed at `(zone_id, location)`. Records a pending
/// outbound transfer (the suppression lock); the thing stays put until
/// [`ack_release`]. Errors if the cell holds no thing.
#[reducer]
pub fn begin_release(
    ctx: &ReducerContext,
    now_ms: u64,
    zone_id: u32,
    location: u8,
) -> Result<(), String> {
    let (rotation, id) = current_thing(ctx, zone_id, location).ok_or_else(|| {
        format!("begin_release: no thing at zone {zone_id:#010x} cell {location}")
    })?;
    let transfer_id = pack_valid_at(now_ms, sequence::next_sequence(ctx));
    ctx.db.transfers().insert(Transfer {
        transfer_id,
        direction: DIR_OUT,
        state: ST_PENDING,
        region_id: region_of(zone_id),
        zone_id,
        location,
        rotation,
        id,
        created_at: now_ms,
    });
    Ok(())
}

/// Finish a release once the object shard has the thing (its receipt observed by
/// the gate): delete the affixed thing and mark the transfer done. Idempotent —
/// an unknown or already-done `transfer_id` is a no-op.
#[reducer]
pub fn ack_release(ctx: &ReducerContext, now_ms: u64, transfer_id: u64) -> Result<(), String> {
    let Some(t) = ctx.db.transfers().transfer_id().find(transfer_id) else {
        return Ok(());
    };
    if t.direction != DIR_OUT {
        return Err(format!("ack_release: {transfer_id} is not an outbound transfer"));
    }
    if t.state == ST_DONE {
        return Ok(());
    }
    // Delete the affixed thing: a hot 'clear' (id 0) at the cell, which the GC
    // fold bakes out of cold (`upsert_thing` drops a 0-id cell).
    things_hist::write_at(
        ctx,
        HotThing {
            valid_at: 0,
            zone_id: t.zone_id,
            location: t.location,
            rotation: 0,
            id: 0,
        },
        now_ms,
    );
    ctx.db.transfers().transfer_id().delete(transfer_id);
    ctx.db.transfers().insert(Transfer { state: ST_DONE, ..t });
    Ok(())
}
