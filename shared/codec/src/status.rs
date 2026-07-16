//! The status bytes of the rebuild's logs — `event_status` and `state_status`.
//! Authoritative layout: `docs/VARIABLES.md`.
//!
//! Both share one shape: `u8 = flags:4 | status:4`. **`status` is *where it is*** (the phase),
//! **`flags` is *what was asked of it or happened to it*** (a request or an outcome). One nibble of
//! phase (16 values, few used) and one of flags means, e.g., a `FAILED` that keeps the phase it
//! died in — `QUEUEING|FAILED` instead of a distinct terminal state.

const FLAGS_SHIFT: u8 = 4;
const NIBBLE: u8 = 0xF;

/// Compose a status byte from its `flags` (high nibble) and `status` (low nibble). Each is masked
/// to its nibble, so an over-range argument can't bleed into the other.
pub fn pack_status(flags: u8, status: u8) -> u8 {
    ((flags & NIBBLE) << FLAGS_SHIFT) | (status & NIBBLE)
}

/// The `flags` nibble (bits 4–7).
pub fn status_flags(s: u8) -> u8 {
    (s >> FLAGS_SHIFT) & NIBBLE
}

/// The `status` nibble (bits 0–3) — the phase.
pub fn status_phase(s: u8) -> u8 {
    s & NIBBLE
}

/// Set `flag` bits on a status byte (they live in the high nibble).
pub fn status_with_flag(s: u8, flag: u8) -> u8 {
    s | ((flag & NIBBLE) << FLAGS_SHIFT)
}

/// True if every bit of `flag` is set in the status byte's flags nibble.
pub fn status_has_flag(s: u8, flag: u8) -> bool {
    status_flags(s) & flag == flag
}

/// Replace the phase (low nibble), leaving flags intact.
pub fn status_set_phase(s: u8, phase: u8) -> u8 {
    (s & (NIBBLE << FLAGS_SHIFT)) | (phase & NIBBLE)
}

// ── event_status ────────────────────────────────────────────────────────────────
// status = the phase; reclaim rewinds to a phase's start, so each exists.

/// Created; awaiting the shard's local grouping.
pub const EVENT_QUEUED: u8 = 0;
/// The shard put it in an `event_group` (a shared-target component within the shard).
pub const EVENT_GROUPED: u8 = 1;
/// The orchestrator gave its work-group a worker.
pub const EVENT_ASSIGNED: u8 = 2;
/// The worker is composing it.
pub const EVENT_RUNNING: u8 = 3;
/// Done.
pub const EVENT_COMPLETE: u8 = 4;

/// flags bit — it failed (keeps the phase it died in: `GROUPED|FAILED`, `RUNNING|FAILED`, …).
pub const EVENT_FLAG_FAILED: u8 = 1 << 0;
/// flags bit — the program asked to promote this event to `event` on settle.
pub const EVENT_FLAG_PROMOTE: u8 = 1 << 1;

// ── state_status ──────────────────────────────────────────────────────────────
// Composition-settled is `dirty == false` (a separate boolean column), NOT a status here.
// state_status only tracks the promotion projection.

/// Not yet projected to `state`.
pub const STATE_OPEN: u8 = 0;
/// The value has been upserted into `state`.
pub const STATE_PROMOTED: u8 = 1;

/// flags bit — the program asked to promote this slot to `state` once settled.
pub const STATE_FLAG_PROMOTE: u8 = 1 << 0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nibbles_roundtrip_and_are_disjoint() {
        for f in 0u8..16 {
            for p in 0u8..16 {
                let s = pack_status(f, p);
                assert_eq!(status_flags(s), f);
                assert_eq!(status_phase(s), p);
            }
        }
        // both nibbles saturated fill the byte
        assert_eq!(pack_status(0xF, 0xF), 0xFF);
        // over-range args mask to their own nibble, never bleed
        assert_eq!(pack_status(0xFF, 0), 0xF0);
        assert_eq!(pack_status(0, 0xFF), 0x0F);
    }

    #[test]
    fn flag_and_phase_are_independent() {
        // A FAILED at each phase keeps the phase — the whole point of the split.
        let s = pack_status(EVENT_FLAG_FAILED, EVENT_RUNNING);
        assert_eq!(status_phase(s), EVENT_RUNNING);
        assert!(status_has_flag(s, EVENT_FLAG_FAILED));
        assert!(!status_has_flag(s, EVENT_FLAG_PROMOTE));

        // adding a flag leaves the phase; changing the phase leaves the flags
        let s = status_with_flag(s, EVENT_FLAG_PROMOTE);
        assert_eq!(status_phase(s), EVENT_RUNNING);
        assert!(status_has_flag(s, EVENT_FLAG_PROMOTE) && status_has_flag(s, EVENT_FLAG_FAILED));
        let s = status_set_phase(s, EVENT_COMPLETE);
        assert_eq!(status_phase(s), EVENT_COMPLETE);
        assert!(status_has_flag(s, EVENT_FLAG_PROMOTE) && status_has_flag(s, EVENT_FLAG_FAILED));
    }

    #[test]
    fn phases_and_flags_fit_their_nibbles() {
        for p in [EVENT_QUEUED, EVENT_GROUPED, EVENT_ASSIGNED, EVENT_RUNNING, EVENT_COMPLETE, STATE_OPEN, STATE_PROMOTED] {
            assert!(p <= NIBBLE, "phase {p} fits the low nibble");
        }
        for f in [EVENT_FLAG_FAILED, EVENT_FLAG_PROMOTE, STATE_FLAG_PROMOTE] {
            assert!(f <= NIBBLE, "flag {f} fits the high nibble");
        }
    }
}
