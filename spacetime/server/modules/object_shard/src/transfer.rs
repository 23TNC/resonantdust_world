//! The transfer protocol — object-shard side (the *destination* of a release).
//!
//! A release moves an affixed thing out of a zone shard and into this object
//! shard as a loose thing. The three-hop, roll-forward handshake (see
//! docs/object-shard.md) is driven by the gate:
//!
//! 1. `zone_shard::begin_release` — the source mints a `transfer_id` and records a
//!    pending outbound transfer.
//! 2. **`receive`** (here) — the gate relays the transfer's payload; this shard
//!    inserts the loose thing (via [`crate::free_things::place`], maintaining
//!    `presence`) and writes a **receipt** row keyed by the same `transfer_id`,
//!    carrying the allocated `object_id`.
//! 3. `zone_shard::ack_release` — the gate, having seen the receipt, tells the
//!    source to delete the now-departed affixed thing.
//!
//! `receive` is **idempotent** on `transfer_id`: a replay (another gate, a retry)
//! finds the existing receipt and does nothing, so the object is never duplicated.
//! The receipt is the row the gate watches to know the middle hop committed.

use spacetimedb::{reducer, table, ReducerContext, Table};

use resonantdust_codec::packed::{pack_offset, region_of, OFFSET_STEPS};

use crate::free_things;

/// Inbound transfer (a release landing here). Only one direction is modelled on
/// the object side for now — settle (object → zone) is the mirror, added with its
/// gate driver later.
pub const DIR_IN: u8 = 1;
/// The object exists here and the receipt is recorded; the gate can now ack the
/// source. (A later `confirmed` state would close a 4-hop settle; unused today.)
pub const ST_RECEIVED: u8 = 1;

/// A receipt for a transfer that has landed here. Keyed by the source-minted
/// `transfer_id`; carries the `object_id` the gate needs to correlate the loose
/// thing with the transfer, and the payload for auditing / a future re-drive.
#[table(accessor = transfers, public)]
pub struct Transfer {
    #[primary_key]
    pub transfer_id: u64,
    pub direction: u8,
    pub state: u8,
    /// The region the loose thing landed in — carried so witness gates can filter
    /// `transfer WHERE region_id IN (...)` once region-scoped witnessing lands.
    #[index(btree)]
    pub region_id: u32,
    pub zone_id: u32,
    pub location: u8,
    pub rotation: u8,
    /// What-kind def id of the thing.
    pub id: u16,
    /// The loose thing's stable instance handle, allocated by [`receive`].
    pub object_id: u64,
    pub offset: u8,
    pub created_at: u64,
}

/// Land a released thing: insert the loose thing and record the receipt.
/// Idempotent on `transfer_id`. The thing enters at tile-centre (`offset` =
/// mid-tile) since an affixed thing carries no sub-tile position.
#[reducer]
pub fn receive(
    ctx: &ReducerContext,
    now_ms: u64,
    transfer_id: u64,
    zone_id: u32,
    location: u8,
    rotation: u8,
    id: u16,
) -> Result<(), String> {
    // Idempotent: a receipt already exists → this transfer already landed.
    if ctx.db.transfers().transfer_id().find(transfer_id).is_some() {
        return Ok(());
    }

    let offset = pack_offset(OFFSET_STEPS / 2, OFFSET_STEPS / 2);
    let object_id = free_things::next_object_id(ctx);
    free_things::place(ctx, object_id, zone_id, location, rotation, id, offset, now_ms)?;

    ctx.db.transfers().insert(Transfer {
        transfer_id,
        direction: DIR_IN,
        state: ST_RECEIVED,
        region_id: region_of(zone_id),
        zone_id,
        location,
        rotation,
        id,
        object_id,
        offset,
        created_at: now_ms,
    });
    Ok(())
}
