//! Global monotonic u16 sequence allocator for `valid_at` PK disambiguation.
//!
//! Copied from the `zone_shard` module (see its `sequence.rs`). The low 16 bits
//! of a packed `valid_at` are a per-module sequence that disambiguates
//! same-millisecond writes; each shard's module has its own independent counter,
//! and cross-shard collisions are impossible because one shard's PKs are never
//! compared with another's.
//!
//! `next_sequence` returns a fresh u16 on every call — read-modify-write against
//! a single-row counter table, wrapping at 65536.

use spacetimedb::{table, ReducerContext, Table};

/// Single-row counter table holding the next sequence value to hand out. PK is
/// always `0` — the one-row pattern.
#[table(accessor = sequence_counter)]
pub struct SequenceCounter {
    #[primary_key]
    pub id: u8,
    pub next: u16,
}

/// Allocate the next u16 sequence number. Wraps at 65536. Lazy-seeds on first
/// call after a fresh deployment — starts at 0.
pub fn next_sequence(ctx: &ReducerContext) -> u16 {
    if let Some(counter) = ctx.db.sequence_counter().id().find(0) {
        let allocated = counter.next;
        ctx.db.sequence_counter().id().delete(0);
        ctx.db.sequence_counter().insert(SequenceCounter {
            id: 0,
            next: allocated.wrapping_add(1),
        });
        allocated
    } else {
        ctx.db.sequence_counter().insert(SequenceCounter { id: 0, next: 1 });
        0
    }
}
