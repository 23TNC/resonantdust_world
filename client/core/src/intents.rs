//! The pawn **intent queue** — what a pawn has committed to doing, and when it finishes
//! (shared-simulation P3b).
//!
//! `QueueState` was documented "Display truth only" and both hosts believed it: `client/webgl`
//! mirrored it for a strip, and `client/npc` did not read it at all — it GUESSED, holding an
//! `Instant` deadline of `duration / 6.0 + 3.0` seconds and calling itself busy until wall time
//! passed. That is a frame stating **when a pawn's committed act finishes**, which is the one
//! thing a brain must not get wrong, filed as decoration because its first reader was a panel.
//!
//! It is a MODEL. Non-durable — the worker's ephemeral map stays the authority (lumberjack F1) —
//! and self-correcting, but a lost fan is a stale ANSWER a brain acts on, not merely a stale
//! picture.

use std::collections::HashMap;

/// One committed entry in a pawn's queue.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QueueEntry {
    /// The worker-minted display identity — what a cancel names.
    pub entry_id: u32,
    pub interaction_ref: u32,
    /// `1` = walking to it, `2` = running it. A phase-1 entry has NO end tic.
    pub phase: u32,
    pub started_tic: u16,
    /// The tic the act completes at. Meaningful for phase 2 only; a walking entry fans `0`.
    pub fire_tic: u16,
}

/// A locally-armed intent: the host queued an `EXECUTE_INTERACTION` and the fan has not come back
/// yet. Without it there is a gap of the fire→fan round trip in which the queue says idle and the
/// pawn is not — which is precisely the window the brains' `+ 3.0` second margin was covering.
#[derive(Debug, Clone, Copy)]
struct Pending {
    /// Past this tic the arm expires on its own, so a fan that never arrives cannot wedge a pawn
    /// busy forever. A TIC deadline, not a wall clock: the thing being waited on is tic-paced.
    deadline: u16,
}

/// Every pawn's committed work.
#[derive(Debug, Default)]
pub struct IntentQueues {
    queues: HashMap<u32, (u16, Vec<QueueEntry>)>,
    pending: HashMap<u32, Pending>,
}

impl IntentQueues {
    pub fn new() -> Self {
        Self::default()
    }

    /// One authoritative snapshot, replaced WHOLE. Returns `false` when rejected as a replay.
    ///
    /// The fan is the entire queue, so a partial merge would invent state the worker never had.
    /// Equal tics accept: two mutations can fan in one composing tic and the last is the truth.
    pub fn observe(&mut self, entity: u32, event_tic: u16, entries: &[u32]) -> bool {
        if let Some((prev, _)) = self.queues.get(&entity) {
            if resonantdust_codec::tic::tic_after(*prev, event_tic) {
                return false;
            }
        }
        let decoded: Vec<QueueEntry> = entries
            .chunks_exact(4)
            .map(|c| QueueEntry {
                entry_id: c[0],
                interaction_ref: c[1],
                phase: c[2],
                started_tic: ((c[3] >> 16) & 0xffff) as u16,
                fire_tic: (c[3] & 0xffff) as u16,
            })
            .collect();
        // Any authoritative fan supersedes the local arm — the server has answered.
        self.pending.remove(&entity);
        if decoded.is_empty() {
            self.queues.remove(&entity);
        } else {
            self.queues.insert(entity, (event_tic, decoded));
        }
        true
    }

    /// The host just queued an interaction for this pawn. Hold it busy until the fan confirms —
    /// or until `duration` tics have passed, whichever comes first.
    ///
    /// **Superseded by the next fan of any kind**, including an EMPTY one: an order that was
    /// refused or raced away fans nothing, and the arm must not outlive that answer.
    pub fn arm_pending(&mut self, entity: u32, at_tic: u16, duration: u16) {
        self.pending.insert(entity, Pending { deadline: at_tic.wrapping_add(duration.max(1)) });
    }

    pub fn entries(&self, entity: u32) -> &[QueueEntry] {
        self.queues.get(&entity).map(|(_, e)| e.as_slice()).unwrap_or(&[])
    }

    /// **Is this pawn committed to something right now.**
    ///
    /// True for a running entry until its `fire_tic`, and for a WALKING one regardless — a
    /// phase-1 entry fans `fire_tic 0`, so reading "0 is in the past" would call a walking pawn
    /// idle and let its brain issue a second order over the first.
    pub fn busy(&self, entity: u32, now: u16) -> bool {
        if let Some(p) = self.pending.get(&entity) {
            if !resonantdust_codec::tic::tic_after(now, p.deadline) {
                return true;
            }
        }
        self.entries(entity).iter().any(|e| match e.phase {
            1 => true,
            _ => !resonantdust_codec::tic::tic_after(now, e.fire_tic),
        })
    }

    /// When this pawn's running act completes, if one is running.
    pub fn fires_at(&self, entity: u32) -> Option<u16> {
        self.entries(entity).iter().find(|e| e.phase == 2).map(|e| e.fire_tic)
    }

    pub fn forget(&mut self, entity: u32) {
        self.queues.remove(&entity);
        self.pending.remove(&entity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: u32, phase: u32, started: u16, fire: u16) -> [u32; 4] {
        [id, 7, phase, (u32::from(started) << 16) | u32::from(fire)]
    }

    /// The criterion: a replayed OLDER fan must not roll the snapshot back. A zone re-subscribe
    /// replays history, and the strip — or worse, a brain — would act on a queue the pawn left.
    #[test]
    fn a_replayed_older_fan_does_not_roll_the_snapshot_back() {
        let mut q = IntentQueues::new();
        assert!(q.observe(1, 200, &entry(9, 2, 190, 260)));
        assert!(!q.observe(1, 100, &entry(3, 2, 90, 160)), "an older fan must be refused");
        assert_eq!(q.entries(1)[0].entry_id, 9);
        // Equal tics accept — two mutations can fan in one composing tic.
        assert!(q.observe(1, 200, &entry(11, 2, 195, 270)));
        assert_eq!(q.entries(1)[0].entry_id, 11);
    }

    /// A WALKING entry fans `fire_tic 0`. Reading that as "already fired" calls a walking pawn
    /// idle, and its brain issues a second order over the first — the bug this guards.
    #[test]
    fn a_walking_entry_reads_busy_despite_a_zero_fire_tic() {
        let mut q = IntentQueues::new();
        q.observe(1, 100, &entry(9, 1, 100, 0));
        assert!(q.busy(1, 500), "phase 1 is busy regardless of fire_tic");
        assert_eq!(q.fires_at(1), None, "a walk has no completion tic to report");
    }

    /// A running entry is busy until its fire tic, and idle after.
    #[test]
    fn a_running_entry_is_busy_until_it_fires() {
        let mut q = IntentQueues::new();
        q.observe(1, 100, &entry(9, 2, 100, 160));
        assert!(q.busy(1, 159));
        assert!(q.busy(1, 160), "the fire tic itself still counts as committed");
        assert!(!q.busy(1, 161));
        assert_eq!(q.fires_at(1), Some(160));
    }

    /// The fire→fan gap: the host has queued something and the server has not answered yet. The
    /// pawn must read BUSY across that window, or its brain fires a second order into it — the
    /// gap the deleted `+ 3.0` second margin was really covering.
    #[test]
    fn a_local_arm_holds_until_the_fan_or_its_deadline() {
        let mut q = IntentQueues::new();
        q.arm_pending(1, 100, 20);
        assert!(q.busy(1, 101), "armed and unanswered — busy");
        assert!(q.busy(1, 120), "the deadline tic itself still holds");
        assert!(!q.busy(1, 121), "past the deadline it expires rather than wedging the pawn");
    }

    /// ANY fan supersedes the arm — including an empty one, which is the refusal.
    #[test]
    fn a_fan_supersedes_the_local_arm() {
        let mut q = IntentQueues::new();
        q.arm_pending(1, 100, 200);
        assert!(q.busy(1, 150));
        q.observe(1, 101, &[]); // refused: nothing queued
        assert!(!q.busy(1, 150), "the refusal released the pawn, not the deadline");
    }

    /// An EMPTY fan clears the pawn — the refusal signal a brain currently cannot see: an order
    /// that was rejected or raced away leaves no entry, and the brain must be released.
    #[test]
    fn an_empty_fan_clears_the_queue() {
        let mut q = IntentQueues::new();
        q.observe(1, 100, &entry(9, 2, 100, 160));
        assert!(q.busy(1, 120));
        q.observe(1, 101, &[]);
        assert!(!q.busy(1, 120), "an empty fan releases the pawn");
        assert!(q.entries(1).is_empty());
    }
}
