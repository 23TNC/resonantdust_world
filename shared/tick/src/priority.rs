//! The priority total order and the read-tic it selects.
//!
//! `priority` is a deterministic hash of `(tic, entity_key)` — a total order over the
//! unified entity key that **rotates every tic** (so mutual-combat "tempo" isn't
//! permanently biased toward any entity). Resolving `O`'s event whose actor is `Q`
//! honors the *same-tic* read of `Q` iff `Q` ranks below `O`; otherwise it reads `Q`
//! at `T-1`. Because the rank is a strict total order, in any dependency cycle exactly
//! one edge is a back-edge (reads `T-1`), so the dependency graph is a DAG by
//! construction and never needs cycle detection (see `docs/simulation.md`).
//!
//! The hash must be identical on every machine and stable forever — so it is written
//! out explicitly (a SplitMix64 mix) rather than using `std`'s unspecified hasher.

/// One SplitMix64 finalizing mix. Deterministic and well-distributed.
fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Priority of `entity_key` at `tic` — the value the DAG orders on. Varies with
/// `tic` so first-strike tempo rotates; deterministic and stable across machines.
pub fn priority(tic: u32, entity_key: u64) -> u64 {
    // Mix the tic first so adjacent tics decorrelate, fold in the key, mix again.
    mix(entity_key ^ mix(tic as u64))
}

/// Strict total-order rank of an entity at a tic: `(priority, entity_key)`. The key
/// tie-breaks a (rare) priority collision, so the order is strict over distinct keys
/// — which is what guarantees exactly one back-edge per cycle.
fn rank(tic: u32, entity_key: u64) -> (u64, u64) {
    (priority(tic, entity_key), entity_key)
}

/// The tic at which to read `actor_key` when resolving `target_key`'s event at `tic`.
///
/// - `actor` ranks below `target` → read the actor's **same-tic** (`tic`) state
///   (a causal, priority-ascending edge; the actor resolves first).
/// - otherwise → read the actor at **`tic - 1`** (a back-edge; 1-tic-delayed), which
///   is what keeps cycles deadlock-free. Saturates at `0`.
pub fn actor_read_tic(actor_key: u64, target_key: u64, tic: u32) -> u32 {
    if rank(tic, actor_key) < rank(tic, target_key) {
        tic
    } else {
        tic.saturating_sub(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use resonantdust_codec::refs::{pack_cold_entity, pack_hot_entity, SERVER_REF_NONE};

    /// A minted (hot) object `entity_reference` with `id` — the test key.
    fn obj(id: u32) -> u64 {
        pack_hot_entity(SERVER_REF_NONE, id)
    }

    #[test]
    fn priority_is_deterministic_and_tic_varying() {
        let k = obj(7);
        assert_eq!(priority(5, k), priority(5, k), "same inputs → same output");
        // Overwhelmingly likely to differ across tics (decorrelated mix).
        assert_ne!(priority(5, k), priority(6, k));
    }

    #[test]
    fn actor_read_tic_is_asymmetric_and_total() {
        // For any distinct pair at a tic, exactly one reads T and the other T-1 —
        // that asymmetry is the DAG edge direction.
        let a = obj(1);
        let b = obj(2);
        let tic = 10;
        let ab = actor_read_tic(a, b, tic); // resolving b, reading a
        let ba = actor_read_tic(b, a, tic); // resolving a, reading b
        assert!(
            (ab == tic && ba == tic - 1) || (ab == tic - 1 && ba == tic),
            "exactly one of the mutual edges is same-tic, the other T-1 (got {ab}, {ba})"
        );
    }

    #[test]
    fn read_tic_spans_object_and_zone_classes() {
        // A cross-class pair still yields a strict order (no panic, one back-edge).
        let obj = obj(42);
        let zone = pack_cold_entity(0, resonantdust_codec::object::pack_cold_reference(0x11, 0x23, 17, 2));
        let tic = 4;
        let a = actor_read_tic(obj, zone, tic);
        let b = actor_read_tic(zone, obj, tic);
        assert!((a == tic && b == tic - 1) || (a == tic - 1 && b == tic));
    }

    #[test]
    fn self_edge_reads_prior_tic() {
        // An entity that reads itself (self-target) can't rank below itself → T-1,
        // so it never self-deadlocks.
        let k = obj(9);
        assert_eq!(actor_read_tic(k, k, 8), 7);
    }
}
