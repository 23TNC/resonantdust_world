//! Client clock discipline — the offset that lets every client render a *shared*
//! world instant.
//!
//! The server stamps authoritative time (`valid_at`, ms) on every row. A client
//! that wants to render "the world as of time T" must first know what the server
//! thinks the time is right now — i.e. the offset between its own wall clock and
//! the server's. This module estimates that offset from ping/pong round-trips,
//! NTP-style: each pong carries the server clock at reply time; combined with the
//! measured round-trip, that pins `offset = server_clock − client_clock`.
//!
//! The estimate is **best-RTT wins**: network jitter is one-sided (a packet can
//! only be delayed, never arrive early), so the *fastest* round-trip seen is the
//! least distorted, and its offset is the one we trust. A running/last offset and
//! the RTT extremes are kept purely for the debug HUD's sync tab.
//!
//! Host-agnostic and transport-free — both the native (`engine`) and web (`web`)
//! engines own a [`Clock`], feed it [`on_pong`](Clock::on_pong), and forward its
//! [`snapshot`](Clock::snapshot) to the host as `Event::ClockSync`.

/// The clock's current estimate plus diagnostics, emitted to the host on every
/// pong (and the initial login seed) as `Event::ClockSync`. Field names line up
/// with the pixijs debug HUD's `ClockStats` so the sync tab renders them without
/// translation.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClockSnapshot {
    /// The server wall clock (ms) estimated for *now* — `client_now + offset`.
    /// The host derives its own offset as `server_now_ms − Date.now()` at receive.
    pub server_now_ms: u64,
    /// Whether any estimate exists yet (a login seed or at least one pong).
    pub synced: bool,
    /// Best estimate of `server_clock − client_clock`, in ms (may be negative).
    pub offset_ms: i64,
    /// The most recent round-trip time, if a pong has landed.
    pub rtt_ms: Option<u64>,
    /// The lowest round-trip seen — the sample whose offset we adopted.
    pub best_rtt_ms: Option<u64>,
    /// The offset from the lowest-RTT sample (the authoritative one).
    pub best_offset_ms: Option<i64>,
    /// The offset from the highest-RTT sample — the far end of the jitter spread.
    pub worst_offset_ms: Option<i64>,
    /// How many pongs have been folded in (≈ captures / rtt samples).
    pub samples: u32,
}

/// Rolling clock-offset estimator. One per live session; cleared on disconnect by
/// dropping it (a reconnect starts a fresh estimate against the new server clock).
#[derive(Debug, Default)]
pub struct Clock {
    synced: bool,
    /// Adopted offset (server − client), from the lowest-RTT sample so far.
    offset_ms: i64,
    last_rtt_ms: Option<u64>,
    last_offset_ms: Option<i64>,
    best_rtt_ms: Option<u64>,
    best_offset_ms: Option<i64>,
    worst_rtt_ms: u64,
    worst_offset_ms: Option<i64>,
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
    /// roughly half the login round-trip — a real ping refines it within seconds.
    /// Only seeds while unsynced, so it never clobbers a measured estimate.
    pub fn seed_login(&mut self, server_ms: u64, client_recv_ms: u64) {
        if self.synced {
            return;
        }
        self.offset_ms = server_ms as i64 - client_recv_ms as i64;
        self.synced = true;
    }

    /// Fold in one ping/pong round-trip. `client_send_ms` is when the ping left,
    /// `server_ms` the server clock stamped on the pong, `client_recv_ms` when the
    /// pong arrived. Updates the running diagnostics always, and adopts the offset
    /// only when this is the lowest-RTT sample yet (best-RTT wins).
    pub fn on_pong(&mut self, client_send_ms: u64, server_ms: u64, client_recv_ms: u64) {
        let rtt = client_recv_ms.saturating_sub(client_send_ms);
        // The server stamped `server_ms` roughly at the round-trip midpoint, so
        // the client clock then was `client_send_ms + rtt/2`; the offset is the
        // gap between the two.
        let offset = server_ms as i64 - client_send_ms as i64 - (rtt / 2) as i64;

        self.samples += 1;
        self.last_rtt_ms = Some(rtt);
        self.last_offset_ms = Some(offset);

        if self.best_rtt_ms.map_or(true, |b| rtt < b) {
            self.best_rtt_ms = Some(rtt);
            self.best_offset_ms = Some(offset);
            self.offset_ms = offset; // adopt the least-distorted estimate
            self.synced = true;
        }
        if self.worst_offset_ms.is_none() || rtt >= self.worst_rtt_ms {
            self.worst_rtt_ms = rtt;
            self.worst_offset_ms = Some(offset);
        }
    }

    /// The server wall-clock estimate (ms) for a client instant `now_ms`. Floors
    /// at 0 so a wild early offset can't underflow the unsigned result.
    pub fn synced_now_ms(&self, now_ms: u64) -> u64 {
        (now_ms as i64 + self.offset_ms).max(0) as u64
    }

    /// A snapshot for the host, computed against the client instant `now_ms`.
    pub fn snapshot(&self, now_ms: u64) -> ClockSnapshot {
        ClockSnapshot {
            server_now_ms: self.synced_now_ms(now_ms),
            synced: self.synced,
            offset_ms: self.offset_ms,
            rtt_ms: self.last_rtt_ms,
            best_rtt_ms: self.best_rtt_ms,
            best_offset_ms: self.best_offset_ms,
            worst_offset_ms: self.worst_offset_ms,
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
    fn best_rtt_wins() {
        let mut c = Clock::new();
        c.on_pong(1000, 5000, 1400); // rtt 400
        let coarse = c.offset_ms;
        c.on_pong(2000, 6000, 2100); // rtt 100 — faster, should be adopted
        assert_eq!(c.best_rtt_ms, Some(100));
        assert_ne!(c.offset_ms, coarse);
        assert_eq!(c.offset_ms, 6000 - 2000 - 50);
        // A later slow sample must NOT displace the fast one's offset.
        c.on_pong(3000, 7000, 3600); // rtt 600
        assert_eq!(c.best_rtt_ms, Some(100));
        assert_eq!(c.offset_ms, 6000 - 2000 - 50);
        assert_eq!(c.samples, 3);
    }

    #[test]
    fn login_seed_is_coarse_and_yields_to_pong() {
        let mut c = Clock::new();
        c.seed_login(5000, 1000); // offset 4000, no rtt correction
        assert_eq!(c.offset_ms, 4000);
        assert!(c.synced());
        c.on_pong(2000, 6000, 2100); // rtt 100 → offset 3950, adopted over the seed
        assert_eq!(c.offset_ms, 3950);
    }
}
