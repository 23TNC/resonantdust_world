//! The wall↔tic estimate — the client's ONLY notion of the simulation clock (`ACTIONS.md`
//! §Movement; first-pawns P3, rate-tracking pawn-movement F6). Never synced: every
//! `state`/`event` arrival carries a wire tic, and "a row for tic `V` arrived at wall `W`"
//! anchors a mapping the stream itself refines.
//!
//! Two facts shape the rules (pawn-movement I4/I5):
//!
//! - **The true rate is NOT exactly `TIC_HZ`** — the durable tic was measured at 5.41 Hz
//!   against an authored 6. So the estimator LEARNS the rate from its own anchor history
//!   (two-point over the retirement window, clamped to a sane band) instead of assuming the
//!   constant; extrapolating at a wrong rate with a max-only anchor rule ratchets the estimate
//!   ahead without bound.
//! - **Delay only makes an arrival LAG the true clock** — so within an anchor's lifetime the
//!   best candidate is still whichever arrival implies the FURTHEST current tic (serial
//!   comparison; the ring wraps). But beyond [`REANCHOR_MS`] freshness wins: the old anchor
//!   retires (becoming the rate baseline) and the newest arrival re-anchors, bounding the
//!   accumulated rate error to one window.
//!
//! Stale-replay poison: the `event` table replays history on subscribe, and an old-epoch tic
//! can wrap serially AHEAD of now (observed: 39682 against a true ~9040). A candidate implying
//! an implausible forward jump is rejected ([`POISON_BAND`]); if EVERY arrival sustains
//! "implausibly behind" (the poisoned-cold-anchor case — the garbage got in first), a streak
//! counter hard-resets to the live stream ([`POISON_STREAK`]).
//!
//! The engines emit re-anchors to the host ([`crate::api::Event::TicAnchor`]) as a DIAGNOSTIC.
//! A host does NOT rebuild the estimate from them — it asks `Client::now_tic()`, which reads this
//! estimator through [`TicEstimate::delta_since`]. Handing hosts the formula is what produced two
//! clocks (npc kept its own anchor, webgl hand-rolled the conversion eight times); the rate stays
//! readable because a renderer legitimately needs it to pace a smoothing chase.

use resonantdust_codec::tic::TIC_HZ;

/// Anchor lifetime: past this, the next arrival re-anchors unconditionally (and the retired
/// anchor becomes the rate baseline). Delivery delay bounds the error a re-anchor can
/// introduce (~a few tics); rate drift over one window stays under a tic at the observed
/// deficit — both far below the unbounded ratchet this replaces.
const REANCHOR_MS: f64 = 10_000.0;

/// Rate learning needs at least this span between the retired anchor and the live one —
/// below it, delivery jitter dominates the slope.
const MIN_RATE_SPAN_MS: f64 = 5_000.0;

/// Learned-rate clamp band around the authored rate (fraction of `TIC_HZ`). Wide enough for
/// real drift (0.9× observed), tight enough that garbage can't run the clock backwards.
const RATE_BAND: (f64, f64) = (0.5, 1.5);

/// A candidate further than this (tics) from the estimate — ahead, or behind — is implausible
/// and rejected; either direction feeds its own [`POISON_STREAK`] counter. 900 tics ≈ 2.5 min
/// at 6 Hz, far beyond any real delivery lead.
const POISON_BAND: f64 = 900.0;

/// This many consecutive implausible arrivals IN ONE DIRECTION mean the ANCHOR is the odd one
/// out — hard-reset to the live stream. Behind-streaks catch a garbage first anchor (an
/// old-epoch replay observed first); ahead-streaks catch anchoring on a STALE RESTING ROW at
/// page load (a resting pawn's `state` keeps its last-write tic forever, so a fresh subscribe
/// can legitimately observe a very old tic first — measured: d stuck at −4600 while every
/// live arrival was rejected as "implausibly ahead").
const POISON_STREAK: u32 = 8;

/// The best-known wall↔tic anchor + learned rate. See the module docs for the rules.
#[derive(Debug)]
pub struct TicEstimate {
    /// The live anchor `(tic, wall_ms)`.
    anchor: Option<(u16, f64)>,
    /// The last RETIRED anchor — the rate estimate's other point.
    prev: Option<(u16, f64)>,
    /// Learned rate, tics per millisecond.
    rate: f64,
    /// Consecutive implausibly-behind arrivals (poisoned-anchor detector).
    behind_streak: u32,
    /// Consecutive implausibly-ahead arrivals (stale-resting-row-anchor detector).
    ahead_streak: u32,
}

impl Default for TicEstimate {
    fn default() -> Self {
        Self { anchor: None, prev: None, rate: TIC_HZ as f64 / 1000.0, behind_streak: 0, ahead_streak: 0 }
    }
}

impl TicEstimate {
    /// Feed one arrival (`tic` at wall `wall_ms`). Returns `true` if the anchor moved — the
    /// host should be told (re-anchors are sparse: max-rule improvements plus one per
    /// [`REANCHOR_MS`] window).
    pub fn observe(&mut self, tic: u16, wall_ms: f64) -> bool {
        let Some((atic, awall)) = self.anchor else {
            self.anchor = Some((tic, wall_ms));
            return true;
        };
        let est = self.estimate_at(wall_ms).unwrap();
        let ahead = (tic.wrapping_sub(est.floor() as u16) as i16) as f64 - est.fract();

        if ahead > POISON_BAND {
            // Implausibly ahead. Either an old-epoch replay wrapping serially ahead
            // (transient — never anchor it) or OUR anchor is an ancient resting row and
            // this is the live stream — a sustained streak means the latter.
            self.ahead_streak += 1;
            if self.ahead_streak >= POISON_STREAK {
                *self = Self::default();
                self.anchor = Some((tic, wall_ms));
                return true;
            }
            return false;
        }
        if ahead < -POISON_BAND {
            // Implausibly behind — the mirror case: a stale replay (harmless), or the
            // anchor is the garbage and this is the live stream.
            self.behind_streak += 1;
            if self.behind_streak >= POISON_STREAK {
                *self = Self::default();
                self.anchor = Some((tic, wall_ms));
                return true;
            }
            return false;
        }
        // intent-queue-ui: only an in-band arrival AT-OR-AHEAD of the estimate counts as
        // health. A poisoned-low anchor sees all live traffic implausibly ahead — and the
        // ONLY in-band arrivals are replays of the very stale row that seeded it, always
        // BEHIND. Letting those reset the streak made the poison defend itself forever
        // (seen live: a page anchored on a 3-hour-old resting row never healed while the
        // row kept re-replaying on zone churn). Healthy streams re-anchor on in-band-ahead
        // arrivals constantly, so the streaks still reset in normal operation.
        if ahead >= 0.0 {
            self.behind_streak = 0;
            self.ahead_streak = 0;
        }

        if wall_ms - awall > REANCHOR_MS {
            // The anchor aged out: retire it as the rate baseline and take the fresh arrival
            // (freshness beats the max rule beyond the delivery-delay bound), then re-learn
            // the rate over the retired→live span.
            self.prev = Some((atic, awall));
            self.anchor = Some((tic, wall_ms));
            if let Some((ptic, pwall)) = self.prev {
                let span = wall_ms - pwall;
                if span >= MIN_RATE_SPAN_MS {
                    let dt = (tic.wrapping_sub(ptic) as i16) as f64;
                    let (lo, hi) = RATE_BAND;
                    let base = TIC_HZ as f64 / 1000.0;
                    self.rate = (dt / span).clamp(lo * base, hi * base);
                }
            }
            return true;
        }

        if ahead > 0.0 {
            self.anchor = Some((tic, wall_ms));
            return true;
        }
        false
    }

    /// The current anchor, if any arrival has been observed.
    pub fn anchor(&self) -> Option<(u16, f64)> {
        self.anchor
    }

    /// Seed the RATE from a persisted hint (movement-hardening F5) — ignored once the stream
    /// has anchored (the hint only skips the cold-page warmup; the stream stays
    /// authoritative). Clamped TIGHTER than the learner's band (±20% of authored): a hint is
    /// worth little and a polluted one (a learning window spanning a server stall measurably
    /// stored 8.48 against a true 6.0) must not start the page 40% fast.
    pub fn seed_rate(&mut self, tics_per_sec: f64) {
        if self.anchor.is_some() || !tics_per_sec.is_finite() {
            return;
        }
        let base = TIC_HZ as f64 / 1000.0;
        self.rate = (tics_per_sec / 1000.0).clamp(0.8 * base, 1.2 * base);
    }

    /// The learned rate in tics per SECOND (starts at `TIC_HZ`; refined from the stream).
    pub fn tics_per_sec(&self) -> f64 {
        self.rate * 1000.0
    }

    /// The estimated (fractional, WRAPPING — integer part is a `u16` ring value) tic at
    /// `now_ms`. Use [`Self::delta_since`] for arithmetic; never compare these with `<`.
    pub fn estimate_at(&self, now_ms: f64) -> Option<f64> {
        let (tic, wall) = self.anchor?;
        Some(tic as f64 + (now_ms - wall) * self.rate)
    }

    /// Fractional tics elapsed at `now_ms` since wire tic `t` (serial — correct across the
    /// wrap; negative = `t` is still in the estimated future).
    pub fn delta_since(&self, t: u16, now_ms: f64) -> Option<f64> {
        let (tic, wall) = self.anchor?;
        let serial = (tic.wrapping_sub(t) as i16) as f64;
        Some(serial + (now_ms - wall) * self.rate)
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
        // A stale replay a little behind is ignored (and is NOT poison — within the band).
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

    #[test]
    fn a_slow_server_rate_is_learned_and_the_lead_stays_bounded() {
        // The I5 scenario: the true clock runs 5.4 Hz against an authored 6. Feed an arrival
        // every 2 s for 2 min; without rate learning the estimate would lead by ~70 tics by
        // the end — with it, the delta of a JUST-ARRIVED tic must stay near zero.
        let mut e = TicEstimate::default();
        let true_rate = 5.4 / 1000.0; // tics per ms
        let mut worst: f64 = 0.0;
        let mut t_ms = 0.0;
        while t_ms <= 120_000.0 {
            let tic = (t_ms * true_rate) as u16;
            e.observe(tic, t_ms);
            worst = worst.max(e.delta_since(tic, t_ms).unwrap());
            t_ms += 2_000.0;
        }
        // Ratcheting would grow this past 60; learning + windowed re-anchor keeps it small.
        assert!(worst < 10.0, "worst lead {worst}");
        let learned = e.tics_per_sec();
        assert!((learned - 5.4).abs() < 0.4, "learned {learned}");
    }

    #[test]
    fn a_serially_ahead_stale_replay_is_rejected() {
        let mut e = TicEstimate::default();
        assert!(e.observe(9_040, 0.0));
        // The observed poison: an old-epoch event tic that wraps serially AHEAD by ~30k.
        assert!(!e.observe(39_682, 100.0));
        // The estimate is untouched — a fresh arrival still reads as "now".
        let d = e.delta_since(9_041, 200.0).unwrap();
        assert!(d.abs() < 2.0, "d {d}");
    }

    #[test]
    fn an_ancient_resting_row_anchor_self_heals_off_the_live_stream() {
        // The measured failure: a fresh subscribe replays a resting pawn's state row whose
        // tic is ~4600 old; it anchors FIRST, and every live arrival then reads as
        // implausibly AHEAD. The streak must hand the anchor to the live stream.
        let mut e = TicEstimate::default();
        assert!(e.observe(10_700, 0.0));
        let mut reanchored = false;
        for i in 0..POISON_STREAK as u16 {
            reanchored = e.observe(15_400 + i, 100.0 + f64::from(i) * 300.0);
        }
        assert!(reanchored, "ahead streak should hard-reset the anchor");
        let d = e.delta_since(15_400 + POISON_STREAK as u16 - 1, 2_300.0).unwrap();
        assert!(d.abs() < 2.0, "d {d}");
    }

    #[test]
    fn a_poisoned_cold_anchor_self_heals_off_the_live_stream() {
        let mut e = TicEstimate::default();
        // The garbage got in FIRST (cold page, replay delivered before any state row).
        assert!(e.observe(39_682, 0.0));
        // The live stream (~9040s) reads implausibly behind — a sustained streak resets.
        let mut reanchored = false;
        for i in 0..POISON_STREAK as u16 {
            reanchored = e.observe(9_040 + i, 100.0 + i as f64 * 300.0);
        }
        assert!(reanchored, "streak should hard-reset the anchor");
        let d = e.delta_since(9_040 + POISON_STREAK as u16 - 1, 2_300.0).unwrap();
        assert!(d.abs() < 2.0, "d {d}");
    }

    #[test]
    fn a_replayed_stale_row_cannot_defend_a_poisoned_anchor() {
        // Seen live (intent-queue-ui): the anchor seeded off a ~10k-stale resting row, and
        // ZONE CHURN kept replaying THAT row — always in-band-BEHIND the estimate it
        // created — zeroing the ahead streak before live traffic (always far ahead) could
        // reach POISON_STREAK. The heal must survive interleaved stale replays.
        let mut e = TicEstimate::default();
        assert!(e.observe(9_267, 0.0)); // the stale resting row anchors first
        let mut reanchored = false;
        for i in 0..POISON_STREAK as u16 {
            let wall = 100.0 + f64::from(i) * 400.0;
            // live traffic, far ahead…
            reanchored = e.observe(19_800 + i * 2, wall);
            // …interleaved with the SAME stale row replaying (in-band, behind).
            e.observe(9_267, wall + 100.0);
        }
        assert!(reanchored, "the interleaved stale replay must not reset the streak");
        let d = e.delta_since(19_800 + (POISON_STREAK as u16 - 1) * 2, 3_000.0).unwrap();
        assert!(d.abs() < 12.0, "d {d}");
    }
}
