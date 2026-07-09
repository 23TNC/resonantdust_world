//! The read rule: **readiness is the absence of pending work at or below the read
//! tic**, never the presence of a resolved value.
//!
//! To read entity `Q` at tic `T`, the worker computes `Q`'s lowest pending tic — the
//! minimum `tic` over `Q`'s `dirty>0` `state_log` rows (its subscription gives it
//! this cheaply, because in-order resolution keeps every pending row above every
//! resolved one). `Q` is resolved through `T` iff it has *no* pending row at or below
//! `T`. An unchanged entity (no pending rows at all) is resolved at every tic — which
//! is why the rule keys off absence, not a stale resolved row (see `docs/simulation.md`).

/// Is `entity` resolved through `read_tic`?
///
/// `min_pending_tic` is the entity's lowest `dirty>0` `state_log` tic, or `None` if it
/// has no pending rows. Resolved iff there is no pending work at or below `read_tic`
/// (`None`, or a lowest-pending strictly above it). When this returns `false`, the
/// worker parks on that pending row and is woken when it flips to `dirty==0`.
pub fn resolved_through(min_pending_tic: Option<u32>, read_tic: u32) -> bool {
    match min_pending_tic {
        None => true,
        Some(m) => m > read_tic,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_pending_is_always_resolved() {
        // An idle entity (no dirty rows) is resolved at any tic — the attacker-idle
        // case that a "presence of a resolved row" rule would deadlock.
        assert!(resolved_through(None, 0));
        assert!(resolved_through(None, 1_000_000));
    }

    #[test]
    fn pending_below_or_at_read_tic_blocks() {
        assert!(!resolved_through(Some(5), 5), "pending exactly at T blocks");
        assert!(!resolved_through(Some(3), 5), "pending below T blocks (resolve in order)");
    }

    #[test]
    fn pending_above_read_tic_is_ready() {
        // Pending work strictly above the read tic is irrelevant to it.
        assert!(resolved_through(Some(6), 5));
    }
}
