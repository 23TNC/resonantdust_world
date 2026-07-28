//! The wall↔tic estimate — the client's ONLY notion of the simulation clock (`ACTIONS.md`
//! §Movement; first-pawns P3). Never synced: every `state`/`event` arrival carries a wire tic,
//! and "a row for tic `V` arrived at wall `W`" anchors a mapping the stream itself refines —
//! extrapolation is `TIC_HZ` (the codec authority). Arrival DELAY only makes an anchor LAG the
//! true clock, never lead it, so the best anchor is whichever arrival implies the FURTHEST
//! current tic — `observe` keeps exactly that one (serial comparison; the ring wraps).
//!
//! The engines emit a re-anchor to the host ([`crate::api::Event::TicAnchor`]); a host computes
//! fractional deltas locally (`serial(anchor.tic − t) + (now − anchor.wall) · TIC_HZ / 1000`),
//! which is what speculation walks on.

use resonantdust_codec::tic::TIC_HZ;

/// The best-known wall↔tic anchor. See the module docs for the max-estimate rule.
#[derive(Default)]
pub struct TicEstimate {
    anchor: Option<(u16, f64)>,
}

impl TicEstimate {
    /// Feed one arrival (`tic` at wall `wall_ms`). Returns `true` if this became the new
    /// anchor — the host should be told (re-anchors are sparse; a lagging arrival is ignored).
    pub fn observe(&mut self, tic: u16, wall_ms: f64) -> bool {
        let better = match self.anchor {
            None => true,
            // The candidate's estimate NOW vs the current one's: both extrapolate to `wall_ms`,
            // so comparing at the candidate's own wall time needs only the current estimate.
            Some(_) => {
                let est = self.estimate_at(wall_ms).unwrap();
                let ahead = (tic.wrapping_sub(est.floor() as u16) as i16) as f64 - est.fract();
                ahead > 0.0
            }
        };
        if better {
            self.anchor = Some((tic, wall_ms));
        }
        better
    }

    /// The current anchor, if any arrival has been observed.
    pub fn anchor(&self) -> Option<(u16, f64)> {
        self.anchor
    }

    /// The estimated (fractional, WRAPPING — integer part is a `u16` ring value) tic at
    /// `now_ms`. Use [`Self::delta_since`] for arithmetic; never compare these with `<`.
    pub fn estimate_at(&self, now_ms: f64) -> Option<f64> {
        let (tic, wall) = self.anchor?;
        Some(tic as f64 + (now_ms - wall) / 1000.0 * TIC_HZ as f64)
    }

    /// Fractional tics elapsed at `now_ms` since wire tic `t` (serial — correct across the
    /// wrap; negative = `t` is still in the estimated future).
    pub fn delta_since(&self, t: u16, now_ms: f64) -> Option<f64> {
        let (tic, wall) = self.anchor?;
        let serial = (tic.wrapping_sub(t) as i16) as f64;
        Some(serial + (now_ms - wall) / 1000.0 * TIC_HZ as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_advance_and_lagging_arrivals_are_ignored() {
        let mut e = TicEstimate::default();
        assert!(e.observe(100, 0.0));
        // 1 s later a row for tic 100 + TIC_HZ arrives — dead on the estimate, not ahead.
        assert!(!e.observe(100 + TIC_HZ, 1000.0));
        // A fresher promote (2 tics ahead of the estimate) re-anchors.
        assert!(e.observe(100 + TIC_HZ + 2, 1000.0));
        // A stale replay far behind is ignored.
        assert!(!e.observe(90, 1100.0));
    }

    #[test]
    fn delta_is_serial_and_fractional() {
        let mut e = TicEstimate::default();
        e.observe(65530, 0.0); // near the wrap
        // Half a second later, 8 tics after 65526: 4 serial + 3 elapsed at 6 Hz.
        let d = e.delta_since(65526, 500.0).unwrap();
        assert!((d - (4.0 + 0.5 * TIC_HZ as f64)).abs() < 1e-9);
        // A future tic (across the wrap) is negative.
        assert!(e.delta_since(4, 0.0).unwrap() < 0.0);
    }
}
