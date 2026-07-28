//! Client clock discipline — the offset that lets every client render a *shared*
//! world instant.
//!
//! The server is authoritative for time. A client that wants to render "the world as
//! of time T" must first know what the server thinks the time is right now — i.e. the
//! offset between its own wall clock and the server's. This module estimates that
//! offset from ping/pong round-trips,
//! NTP-style: each pong carries the server clock at reply time; combined with the
//! measured round-trip, that pins `offset = server_clock − client_clock`.
//!
//! **Best-RTT wins, over a sliding window.** Network jitter is one-sided (a packet
//! can only be delayed, never arrive early) — and on the *server* side a reply can
//! sit behind a slow handler (head-of-line blocking) before its clock is stamped.
//! Both distortions inflate the round-trip *and* bias the offset in the same
//! direction, so the *fastest* round-trip in recent history is the least-distorted
//! sample, and its offset is the one we adopt. Keeping a **window** (rather than an
//! all-time best) is what makes it self-heal: a stale bad lock ages out, and slow
//! clock drift gets re-measured as fresh samples replace old ones.
//!
//! This produces the *raw* offset. Smoothing it into a monotonic, gently-slewed
//! render clock (so two clients converge without visible jumps) happens one layer
//! up, in the JS host (`WasmClient`) where a per-frame clock already lives; this
//! module just hands out the best raw estimate plus diagnostics.
//!
//! Host-agnostic and transport-free — both the native (`engine`) and web (`web`)
//! engines own a [`Clock`], feed it [`on_pong`](Clock::on_pong), and forward its
//! [`snapshot`](Clock::snapshot) to the host as `Event::ClockSync`.

use std::collections::VecDeque;

/// How many recent round-trip samples the estimator keeps. At the ~2 s ping
/// cadence this spans ~30 s — long enough to always hold a clean low-RTT sample,
/// short enough that a stale one ages out and drift is re-measured.
const WINDOW: usize = 16;

/// One round-trip observation: the measured round-trip and the offset it implies.
/// The login seed is stored as a sample too, with [`SEED_RTT`] so a real pong
/// (any finite RTT) always out-ranks it under best-RTT selection.
#[derive(Debug, Clone, Copy)]
struct Sample {
    rtt_ms: u64,
    offset_ms: i64,
}

/// Sentinel RTT for the login seed — larger than any real round-trip, so the seed
/// is adopted only while it's the sole sample and is immediately displaced by the
/// first genuine pong.
const SEED_RTT: u64 = u64::MAX;

/// The clock's current estimate plus diagnostics, emitted to the host on every
/// pong (and the initial login seed) as `Event::ClockSync`. Field names line up
/// with the webgl debug HUD's `ClockStats` so the sync tab renders them without
/// translation.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClockSnapshot {
    /// The server wall clock (ms) estimated for *now* — `client_now + offset`.
    /// The host derives its own offset as `server_now_ms − Date.now()` at receive.
    pub server_now_ms: u64,
    /// Whether any estimate exists yet (a login seed or at least one pong).
    pub synced: bool,
    /// The adopted `server_clock − client_clock` estimate, in ms (may be negative)
    /// — the offset of the lowest-RTT sample in the window.
    pub offset_ms: i64,
    /// The most recent *real* round-trip time, if a pong has landed.
    pub rtt_ms: Option<u64>,
    /// The lowest round-trip in the window — the sample whose offset we adopted.
    pub best_rtt_ms: Option<u64>,
    /// The offset from the lowest-RTT sample in the window (== `offset_ms` once a
    /// real pong exists; the seed keeps `offset_ms` set while this is `None`).
    pub best_offset_ms: Option<i64>,
    /// The offset from the highest-RTT sample in the window — the far end of the
    /// jitter spread, for the HUD's spread readout.
    pub worst_offset_ms: Option<i64>,
    /// How many real pongs have been folded in over the session (≈ captures).
    pub samples: u32,
}

/// Rolling clock-offset estimator. One per live session; cleared on disconnect by
/// dropping it (a reconnect starts a fresh estimate against the new server clock).
#[derive(Debug, Default)]
pub struct Clock {
    synced: bool,
    /// Adopted offset (server − client), from the lowest-RTT sample in the window.
    offset_ms: i64,
    /// Recent samples (login seed + real pongs), bounded to [`WINDOW`].
    window: VecDeque<Sample>,
    /// The most recent real round-trip (excludes the seed sentinel).
    last_rtt_ms: Option<u64>,
    /// Total real pongs folded over the session (monotonic; not window-bounded).
    samples: u32,
}

impl Clock {
    pub fn new() -> Self {
        Self::default()
    }

    /// Seed a coarse offset from a login reply, before any round-trip sample
    /// exists. `server_ms` is the server wall clock stamped on `login_ok`,
    /// `client_recv_ms` the client clock when it arrived. There's no RTT
    /// correction (the login send time isn't threaded here), so it's biased by
    /// roughly the login reply latency — a real ping refines it within seconds.
    /// Only seeds while unsynced, so it never clobbers a measured estimate.
    pub fn seed_login(&mut self, server_ms: u64, client_recv_ms: u64) {
        if self.synced {
            return;
        }
        let offset = server_ms as i64 - client_recv_ms as i64;
        self.window.push_back(Sample { rtt_ms: SEED_RTT, offset_ms: offset });
        self.synced = true;
        self.recompute();
    }

    /// Fold in one ping/pong round-trip. `client_send_ms` is when the ping left,
    /// `server_ms` the server clock stamped on the pong, `client_recv_ms` when the
    /// pong arrived. Pushes the sample into the window (evicting the oldest) and
    /// re-adopts the offset of the lowest-RTT sample now in the window.
    pub fn on_pong(&mut self, client_send_ms: u64, server_ms: u64, client_recv_ms: u64) {
        let rtt = client_recv_ms.saturating_sub(client_send_ms);
        // The server stamped `server_ms` roughly at the round-trip midpoint, so
        // the client clock then was `client_send_ms + rtt/2`; the offset is the
        // gap between the two.
        let offset = server_ms as i64 - client_send_ms as i64 - (rtt / 2) as i64;

        self.samples += 1;
        self.last_rtt_ms = Some(rtt);
        self.window.push_back(Sample { rtt_ms: rtt, offset_ms: offset });
        while self.window.len() > WINDOW {
            self.window.pop_front();
        }
        self.synced = true;
        self.recompute();
    }

    /// Re-derive the adopted offset from the current window: the offset of the
    /// lowest-RTT sample (best-RTT wins). A no-op guard on an empty window keeps
    /// the last estimate.
    fn recompute(&mut self) {
        if let Some(best) = self
            .window
            .iter()
            .min_by_key(|s| s.rtt_ms)
        {
            self.offset_ms = best.offset_ms;
        }
    }

    /// The server wall-clock estimate (ms) for a client instant `now_ms`. Floors
    /// at 0 so a wild early offset can't underflow the unsigned result.
    pub fn synced_now_ms(&self, now_ms: u64) -> u64 {
        (now_ms as i64 + self.offset_ms).max(0) as u64
    }

    /// Whether an estimate exists yet (login seed or at least one pong).
    pub fn synced(&self) -> bool {
        self.synced
    }

    /// A snapshot for the host, computed against the client instant `now_ms`.
    pub fn snapshot(&self, now_ms: u64) -> ClockSnapshot {
        // Best/worst are over *real* samples only (the seed sentinel is excluded),
        // so the HUD's RTT/offset spread reflects genuine round-trips.
        let real = || self.window.iter().filter(|s| s.rtt_ms != SEED_RTT);
        let best = real().min_by_key(|s| s.rtt_ms);
        let worst = real().max_by_key(|s| s.rtt_ms);
        ClockSnapshot {
            server_now_ms: self.synced_now_ms(now_ms),
            synced: self.synced,
            offset_ms: self.offset_ms,
            rtt_ms: self.last_rtt_ms,
            best_rtt_ms: best.map(|s| s.rtt_ms),
            best_offset_ms: best.map(|s| s.offset_ms),
            worst_offset_ms: worst.map(|s| s.offset_ms),
            samples: self.samples,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_is_symmetric_midpoint() {
        let mut c = Clock::new();
        // Ping left at t=1000, pong stamped server=5000, arrived at t=1200 → rtt
        // 200, one-way 100, so client clock at the midpoint was 1100; offset 3900.
        c.on_pong(1000, 5000, 1200);
        assert_eq!(c.offset_ms, 3900);
        assert_eq!(c.synced_now_ms(2000), 5900);
        assert!(c.synced());
    }

    #[test]
    fn lowest_rtt_in_window_wins() {
        let mut c = Clock::new();
        c.on_pong(1000, 5000, 1400); // rtt 400
        let coarse = c.offset_ms;
        c.on_pong(2000, 6000, 2100); // rtt 100 — faster, adopted
        assert_eq!(c.snapshot(0).best_rtt_ms, Some(100));
        assert_ne!(c.offset_ms, coarse);
        assert_eq!(c.offset_ms, 6000 - 2000 - 50);
        // A later slow sample must NOT displace the faster one's offset while the
        // fast sample is still in the window.
        c.on_pong(3000, 7000, 3600); // rtt 600
        assert_eq!(c.snapshot(0).best_rtt_ms, Some(100));
        assert_eq!(c.offset_ms, 6000 - 2000 - 50);
        assert_eq!(c.samples, 3);
    }

    #[test]
    fn window_ages_out_a_stale_best() {
        let mut c = Clock::new();
        // One anomalously-fast sample, then WINDOW slower ones. Once the fast
        // sample falls out of the window, its offset is no longer adopted — the
        // estimator self-heals instead of locking forever.
        c.on_pong(0, 10_000, 20); // rtt 20, offset ~9990 (stale/bad)
        assert_eq!(c.offset_ms, 10_000 - 0 - 10);
        for i in 1..=WINDOW as u64 {
            // rtt 200 each, consistent offset ~5000.
            let send = i * 1000;
            c.on_pong(send, send + 5100, send + 200);
        }
        // The fast sample has been evicted; the adopted offset is now the slow
        // cohort's (rtt-200) value, not the stale 9990.
        assert_eq!(c.offset_ms, 5100 - 100);
        assert_eq!(c.snapshot(0).best_rtt_ms, Some(200));
    }

    #[test]
    fn login_seed_is_coarse_and_yields_to_pong() {
        let mut c = Clock::new();
        c.seed_login(5000, 1000); // offset 4000, sentinel-RTT sample
        assert_eq!(c.offset_ms, 4000);
        assert!(c.synced());
        // The seed carries no real RTT, so best_rtt is None until a pong lands.
        assert_eq!(c.snapshot(0).best_rtt_ms, None);
        c.on_pong(2000, 6000, 2100); // rtt 100 → offset 3950, adopted over the seed
        assert_eq!(c.offset_ms, 3950);
        assert_eq!(c.snapshot(0).best_rtt_ms, Some(100));
    }
}
