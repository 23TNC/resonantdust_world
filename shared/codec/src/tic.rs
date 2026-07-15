//! `tic : u16` — the simulation clock. Wraps, and is compared by serial arithmetic (RFC 1982):
//! a small number can be *after* a large one, because the space is a ring.
//!
//! **Never compare tics with `<` / `<=`.** Across the wrap they invert: `0` is after `65535`, but
//! `0 < 65535`. Use the helpers here.
//!
//! Meaningful only within [`TIC_WINDOW`] (32767) of each other — half the ring. Beyond that the
//! answer flips, which is the price of the ring and the reason `tic` is a u16 rather than a u8:
//! at 2 Hz that window is ~4.5 hours, against a scheduling need of a few tics.

use core::cmp::Ordering;

/// Half the ring, minus the ambiguous antipode. Two tics further apart than this cannot be
/// ordered — `tic_cmp` will answer, and be wrong.
pub const TIC_WINDOW: u16 = i16::MAX as u16; // 32767

/// Order two tics on the ring. `a` is *after* `b` when the forward distance `a - b` lands in the
/// near half; *before* when it lands in the far half.
///
/// Exactly antipodal (`a - b == 32768`) is undefined by RFC 1982; here it reports `Less`, since
/// the signed cast makes it `i16::MIN`. Don't rely on it — stay inside [`TIC_WINDOW`].
pub fn tic_cmp(a: u16, b: u16) -> Ordering {
    (a.wrapping_sub(b) as i16).cmp(&0)
}

/// `a` is strictly after `b`.
pub fn tic_after(a: u16, b: u16) -> bool {
    tic_cmp(a, b) == Ordering::Greater
}

/// `a` is strictly before `b`.
pub fn tic_before(a: u16, b: u16) -> bool {
    tic_cmp(a, b) == Ordering::Less
}

/// `a` is at or after `b` — the ring form of `a >= b`.
pub fn tic_at_or_after(a: u16, b: u16) -> bool {
    !tic_before(a, b)
}

/// `a` is at or before `b` — the ring form of `a <= b`. This is the causality guard's test
/// (`event_tic <= now → too late`) and the read rule's (`t <= h.tic`).
pub fn tic_at_or_before(a: u16, b: u16) -> bool {
    !tic_after(a, b)
}

/// `t` advanced by `n` tics, wrapping. The ring form of `t + n`.
pub fn tic_add(t: u16, n: u16) -> u16 {
    t.wrapping_add(n)
}

/// `t` rewound by `n` tics, wrapping. The ring form of `t - n`.
pub fn tic_sub(t: u16, n: u16) -> u16 {
    t.wrapping_sub(n)
}

/// Signed distance from `b` to `a`: positive if `a` is after `b`, negative if before. Saturates
/// nowhere — beyond [`TIC_WINDOW`] it wraps, like every other operation here.
pub fn tic_distance(a: u16, b: u16) -> i16 {
    a.wrapping_sub(b) as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordering_holds_away_from_the_wrap() {
        assert!(tic_after(5, 3));
        assert!(tic_before(3, 5));
        assert!(tic_at_or_before(3, 3));
        assert!(tic_at_or_after(3, 3));
        assert_eq!(tic_cmp(3, 3), Ordering::Equal);
    }

    #[test]
    fn ordering_holds_across_the_wrap() {
        // The whole point: 2 is AFTER 65534, though 2 < 65534 numerically.
        assert!(tic_after(2, 65534));
        assert!(tic_before(65534, 2));
        assert!(tic_at_or_before(65534, 2));
        // ...and a naive comparison would get both backwards.
        assert!(2u16 < 65534);
    }

    #[test]
    fn the_causality_guard_still_fires_across_the_wrap() {
        // `event_tic <= now → QUEUE_FAILED`. An event scheduled just before the wrap, now that
        // the clock has passed it, must read as late — this is the guard u8 would have broken.
        let event_tic = 65_533u16;
        let now = tic_add(event_tic, 3); // 0 — wrapped
        assert_eq!(now, 0);
        assert!(tic_at_or_before(event_tic, now), "stale event must read as late");
    }

    #[test]
    fn scheduling_forward_across_the_wrap_stays_in_the_future() {
        // TIC_GAP: an append targets master + 3. Straddling the wrap it must still be ahead.
        for master in [65_533u16, 65_534, 65_535, 0, 1] {
            let target = tic_add(master, 3);
            assert!(tic_after(target, master), "master {master} → target {target}");
            assert!(!tic_at_or_before(target, master));
        }
    }

    #[test]
    fn add_and_sub_round_trip_through_the_wrap() {
        for t in [0u16, 1, 65_535, 32_768] {
            for n in [0u16, 1, 3, 4, 1000] {
                assert_eq!(tic_sub(tic_add(t, n), n), t);
            }
        }
    }

    #[test]
    fn distance_is_signed_and_symmetric() {
        assert_eq!(tic_distance(5, 3), 2);
        assert_eq!(tic_distance(3, 5), -2);
        // across the wrap
        assert_eq!(tic_distance(2, 65_534), 4);
        assert_eq!(tic_distance(65_534, 2), -4);
    }

    #[test]
    fn the_window_is_the_limit_and_beyond_it_the_answer_inverts() {
        // Ordering is trustworthy out to TIC_WINDOW...
        assert!(tic_after(TIC_WINDOW, 0));
        assert_eq!(tic_distance(TIC_WINDOW, 0), i16::MAX);
        // ...and one tic further, it lies. This is the documented failure mode, pinned so it
        // can't drift silently: anything that must span more than 32767 tics needs an epoch.
        assert!(tic_before(TIC_WINDOW + 1, 0), "antipode+ reads as before — the ring's price");
    }

    #[test]
    fn ordering_is_total_within_a_window() {
        // Every tic in a contiguous run orders consistently, including over the wrap.
        let start = 65_500u16;
        for i in 0u16..64 {
            for j in 0u16..64 {
                let (a, b) = (tic_add(start, i), tic_add(start, j));
                let expect = i.cmp(&j);
                assert_eq!(tic_cmp(a, b), expect, "i={i} j={j} a={a} b={b}");
            }
        }
    }
}
