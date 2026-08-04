//! `needs_eval` — THE evaluation of needs → conditions → mood (needs-moodlets F3).
//!
//! One implementation, two consumers: the npc Brain imports this crate natively, the client
//! reaches it through `shared/wasm`. A band comparison re-derived in TS would be the
//! two-copies drift class this repo has paid for twice (subframe-ingest I8's halo,
//! lod-aftermath I3's dialect split) — if the panel and the Brain ever disagree on a
//! crossing tic, it must be impossible by construction.
//!
//! The model (stream F1/F2/F4/F5):
//! - a need row is `(satisfaction q8, set_tic)` and is NEVER ticked — the current value is
//!   `sat0 − elapsed / deplete`, clamped at 0, where `deplete` is the corpus TICS full→empty;
//! - a DERIVED condition is a band `lo <= sat < hi` on its need — computed, never granted;
//! - a TIMED condition is a stored grant `(condition_id, grant_tic)` alive while
//!   `elapsed < duration` (expiry DERIVED from the corpus, never stored);
//! - mood = `clamp(0.5 + Σ active offsets, 0..1)`.
//!
//! Tics are u16 and WRAP (~3 h at 6 Hz); every elapsed is `now.wrapping_sub(then)`. A need
//! left unwritten past the wrap window reads as freshly set — acceptable while every writer
//! (mint, drink, drills) touches rows far more often than the window.

use crate::loader::{Bundle, ConditionParams, NeedParams};

/// The base mood an unburdened pawn sits at (F5).
pub const MOOD_BASE: f64 = 0.5;

/// One ACTIVE condition: the corpus id, its mood offset, the tics it has left (`0` = DERIVED,
/// alive exactly while its band holds), and its authored display priority.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveCondition {
    pub condition_id: u16,
    pub mood: f64,
    pub remaining: u16,
    /// The corpus `priority`, carried so consumers can render but never re-decide the order —
    /// [`active_conditions`] has already sorted (conditions F3).
    pub priority: i32,
}

/// The current satisfaction of a need row at `now` — `sat0 − elapsed/deplete`, clamped at 0.
/// A `deplete` of 0 (unauthored) never drains.
pub fn satisfaction_at(satisfaction: u8, set_tic: u16, deplete: f64, now: u16) -> f64 {
    let s0 = f64::from(satisfaction) / 255.0;
    if deplete <= 0.0 {
        return s0;
    }
    let elapsed = f64::from(now.wrapping_sub(set_tic));
    (s0 - elapsed / deplete).max(0.0)
}

/// Every condition active at `now`: band-DERIVED conditions from the need rows +
/// unexpired timed grants. Rows naming an unknown need/condition are skipped (a corpus/state
/// version skew reads as "no condition", never a panic).
///
/// `needs` = decoded `NEED` entries `(need_id, satisfaction, set_tic)`; `grants` = decoded
/// `CONDITION` entries `(condition_id, grant_tic)` (`resonantdust_codec::payload`).
///
/// **The result is SORTED** — `priority` desc, then `|mood|` desc, then `condition_id` asc
/// (conditions F2/F3). The order is a corpus rule, so it is decided here, once, for every
/// observer: the details panel maximizes the first four and the npc reads the same ranking
/// for its decisions. No consumer re-sorts; a second implementation in TS is precisely the
/// drift class this crate exists to prevent. The tie-breaks make it a TOTAL order, so the
/// cards cannot swap places between two evaluations of an unchanged pawn.
pub fn active_conditions(
    bundle: &Bundle,
    needs: &[(u8, u8, u16)],
    grants: &[(u8, u16)],
    now: u16,
) -> Vec<ActiveCondition> {
    let mut out = Vec::new();
    for &(nid, q, t) in needs {
        let Some(np) = need_of(bundle, nid) else { continue };
        let sat = satisfaction_at(q, t, np.deplete, now);
        for band in &np.bands {
            if sat >= band.lo && sat < band.hi {
                if let Some(cid) = bundle.condition_id(&band.condition) {
                    if let Some(cp) = condition_of(bundle, cid as u8) {
                        out.push(ActiveCondition {
                            condition_id: cid,
                            mood: cp.mood,
                            remaining: 0,
                            priority: cp.priority,
                        });
                    }
                }
            }
        }
    }
    for &(cid, gt) in grants {
        let Some(cp) = condition_of(bundle, cid) else { continue };
        if cp.duration <= 0.0 {
            continue; // a stored grant of a DERIVED condition is inert (F2)
        }
        let elapsed = f64::from(now.wrapping_sub(gt));
        if elapsed < cp.duration {
            out.push(ActiveCondition {
                condition_id: u16::from(cid),
                mood: cp.mood,
                remaining: (cp.duration - elapsed) as u16,
                priority: cp.priority,
            });
        }
    }
    // `sort_by` (stable) with a total comparator — see the doc comment. `|mood|` compares with
    // `total_cmp` so a NaN authored into the corpus orders instead of panicking.
    out.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| b.mood.abs().total_cmp(&a.mood.abs()))
            .then_with(|| a.condition_id.cmp(&b.condition_id))
    });
    out
}

/// The mood the active set sums to: `clamp(0.5 + Σ offsets, 0..1)` (F5).
pub fn mood(active: &[ActiveCondition]) -> f64 {
    (MOOD_BASE + active.iter().map(|c| c.mood).sum::<f64>()).clamp(0.0, 1.0)
}

/// The next FUTURE tic at which the active set can change without any new write: the
/// earliest band-threshold crossing of any need + the earliest timed-grant expiry. `None`
/// when nothing ahead can change (every need already at rest, no live grants). This is what
/// lets observers sleep between crossings instead of sampling (F4).
pub fn next_crossing_tic(
    bundle: &Bundle,
    needs: &[(u8, u8, u16)],
    grants: &[(u8, u16)],
    now: u16,
) -> Option<u16> {
    let mut best: Option<u16> = None;
    let mut consider = |tic: u16| {
        let ahead = tic.wrapping_sub(now);
        if ahead == 0 || ahead > u16::MAX / 2 {
            return; // not strictly in the future (or wrapped past — stale)
        }
        best = Some(match best {
            Some(b) if b.wrapping_sub(now) <= ahead => b,
            _ => tic,
        });
    };
    for &(nid, q, t) in needs {
        let Some(np) = need_of(bundle, nid) else { continue };
        if np.deplete <= 0.0 {
            continue;
        }
        let s0 = f64::from(q) / 255.0;
        for band in &np.bands {
            for threshold in [band.lo, band.hi] {
                // A threshold at 0 is never crossed: satisfaction CLAMPS there, so a
                // bottom band (`lo == 0`) holds forever — waking at the clamp tic would
                // be a spurious no-change wake (seen live: a starved wolf's "next").
                if threshold <= 0.0 {
                    continue;
                }
                if s0 > threshold {
                    // First elapsed with `sat < threshold`: sat == threshold is NOT below, so +1.
                    let cross = ((s0 - threshold) * np.deplete).floor() as u16 + 1;
                    consider(t.wrapping_add(cross));
                }
            }
        }
    }
    for &(cid, gt) in grants {
        let Some(cp) = condition_of(bundle, cid) else { continue };
        if cp.duration > 0.0 {
            consider(gt.wrapping_add(cp.duration.ceil() as u16));
        }
    }
    best
}

fn need_of(bundle: &Bundle, id: u8) -> Option<NeedParams> {
    let name = bundle.need_name(u16::from(id))?.to_string();
    bundle.need_params(&name)
}

fn condition_of(bundle: &Bundle, id: u8) -> Option<ConditionParams> {
    let name = bundle.condition_name(u16::from(id))?.to_string();
    bundle.condition_params(&name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::load;

    /// thirst: deplete 1000 tics, Thirsty [0.10, 0.35) −0.15, Dehydrated [0, 0.10) −0.40;
    /// quenched: +0.20 timed 100 tics.
    fn fixture() -> Bundle {
        let src = r#"
[[need]]
id = 1
name = "thirst"
deplete = 1000
band = [
  { condition = "thirsty", lo = 0.10, hi = 0.35 },
  { condition = "dehydrated", lo = 0.0, hi = 0.10 },
]

[[condition]]
id = 1
name = "thirsty"
mood = -0.15

[[condition]]
id = 2
name = "dehydrated"
mood = -0.40

[[condition]]
id = 3
name = "quenched"
mood = 0.20
duration = 100
"#;
        load(&[("needs.toml".into(), src.into())]).expect("fixture loads")
    }

    /// A fixture built to make EVERY sort key decide something, and to make the
    /// insertion order deliberately wrong on all three:
    /// - `low` (id 1) is the LAST band on the need but authors the highest priority;
    /// - `mid_a` / `mid_b` tie on priority, so `|mood|` splits them (`mid_b` is stronger);
    /// - `tie_a` / `tie_b` tie on priority AND `|mood|`, so `condition_id` splits them —
    ///   and `tie_b` (id 4) precedes `tie_a` (id 5) in the corpus.
    fn sort_fixture() -> Bundle {
        let src = r#"
[[need]]
id = 1
name = "n"
deplete = 0
band = [
  { condition = "mid_a", lo = 0.0, hi = 1.01 },
  { condition = "mid_b", lo = 0.0, hi = 1.01 },
  { condition = "tie_b", lo = 0.0, hi = 1.01 },
  { condition = "low",   lo = 0.0, hi = 1.01 },
]

[[condition]]
id = 1
name = "low"
mood = -0.01
priority = 99

[[condition]]
id = 2
name = "mid_a"
mood = -0.10
priority = 10

[[condition]]
id = 3
name = "mid_b"
mood = 0.50
priority = 10

[[condition]]
id = 4
name = "tie_b"
mood = -0.20
priority = 5

[[condition]]
id = 5
name = "tie_a"
mood = 0.20
priority = 5
duration = 100
"#;
        load(&[("needs.toml".into(), src.into())]).expect("sort fixture loads")
    }

    #[test]
    fn the_order_is_priority_then_magnitude_then_id() {
        let b = sort_fixture();
        // Bands are non-exclusive here ON PURPOSE — this test is about ORDER, not membership,
        // and overlapping bands are the cheapest way to get four derived conditions at once.
        let order: Vec<u16> = active_conditions(&b, &[(1, 255, 0)], &[(5, 0)], 50)
            .iter()
            .map(|c| c.condition_id)
            .collect();
        assert_eq!(
            order,
            vec![1, 3, 2, 4, 5],
            "priority desc (low first), then |mood| desc (mid_b 0.50 > mid_a 0.10), \
             then id asc (tie_b 4 before tie_a 5 — equal priority AND equal |mood| 0.20)"
        );
        // Priority rides out with each row so a consumer can render it and never re-sort.
        let by_id = |id| active_conditions(&b, &[(1, 255, 0)], &[(5, 0)], 50)
            .into_iter()
            .find(|c| c.condition_id == id)
            .unwrap();
        assert_eq!(by_id(1).priority, 99);
        assert_eq!(by_id(5).priority, 5, "a TIMED grant carries priority too");
    }

    #[test]
    fn the_crossing_tic_is_exact() {
        // Full (q 255 = 1.0) at tic 0, deplete 1000: sat < 0.35 first at elapsed
        // floor(0.65·1000)+1 = 651 — AT 650 sat == 0.35 exactly and the band is NOT active.
        let b = fixture();
        let needs = [(1u8, 255u8, 0u16)];
        assert!(active_conditions(&b, &needs, &[], 650).is_empty(), "sat == hi is not < hi");
        let at651 = active_conditions(&b, &needs, &[], 651);
        assert_eq!(at651.len(), 1);
        assert_eq!(at651[0].condition_id, 1, "Thirsty");
        assert_eq!(next_crossing_tic(&b, &needs, &[], 0), Some(651), "the computed crossing");
        // From inside Thirsty the next crossing is the 0.10 edge: floor(0.90·1000)+1 = 901.
        assert_eq!(next_crossing_tic(&b, &needs, &[], 651), Some(901));
        let at901 = active_conditions(&b, &needs, &[], 901);
        assert_eq!(at901[0].condition_id, 2, "Dehydrated — bands are exclusive");
        assert_eq!(at901.len(), 1);
    }

    #[test]
    fn empty_clamps_and_stays_dehydrated() {
        // Past full depletion sat clamps at 0, which the [0, 0.10) band CONTAINS — a
        // starved wolf stays Dehydrated, it does not wrap back to content.
        let b = fixture();
        let needs = [(1u8, 255u8, 0u16)];
        let c = active_conditions(&b, &needs, &[], 5000);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].condition_id, 2);
        assert_eq!(satisfaction_at(255, 0, 1000.0, 5000), 0.0);
        assert!((mood(&c) - 0.10).abs() < 1e-9, "0.5 − 0.40");
        // …and nothing ahead can change without a write.
        assert_eq!(next_crossing_tic(&b, &needs, &[], 5000), None);
        // Inside Dehydrated but not yet at 0 (sat 0.05 at elapsed 950): the only threshold
        // below is the CLAMP (band.lo == 0), which is never crossed — no spurious wake.
        assert_eq!(next_crossing_tic(&b, &needs, &[], 950), None);
    }

    #[test]
    fn absent_rows_and_unknown_ids_are_calm() {
        let b = fixture();
        assert!(active_conditions(&b, &[], &[], 100).is_empty());
        assert_eq!(mood(&[]), MOOD_BASE);
        assert_eq!(next_crossing_tic(&b, &[], &[], 100), None);
        // unknown need id 9 / condition id 9 skip, never panic
        assert!(active_conditions(&b, &[(9, 200, 0)], &[(9, 0)], 100).is_empty());
    }

    #[test]
    fn timed_grants_expire_and_stack_with_bands() {
        let b = fixture();
        let grants = [(3u8, 1000u16)]; // quenched, 100 tics
        let live = active_conditions(&b, &[], &grants, 1099);
        assert_eq!(live.len(), 1);
        assert_eq!((live[0].condition_id, live[0].remaining), (3, 1));
        assert!(active_conditions(&b, &[], &grants, 1100).is_empty(), "expiry at exactly duration");
        assert_eq!(next_crossing_tic(&b, &[], &grants, 1050), Some(1100));
        // Thirsty (−0.15) + Quenched (+0.20) sum: 0.5 − 0.15 + 0.20 = 0.55.
        let both = active_conditions(&b, &[(1, 64, 1000)], &grants, 1050); // q64 ≈ 0.251 → Thirsty
        assert_eq!(both.len(), 2);
        assert!((mood(&both) - 0.55).abs() < 1e-9);
    }

    #[test]
    fn tic_wrap_is_handled() {
        // set near the top of the u16 window; `now` wrapped past 0.
        let b = fixture();
        let needs = [(1u8, 255u8, 65000u16)];
        // elapsed 1187 → sat ≈ 1 − 1.187 → clamped… no: 1 − 1187/1000 < 0 → 0 → Dehydrated.
        let c = active_conditions(&b, &needs, &[], 65000u16.wrapping_add(1187));
        assert_eq!(c[0].condition_id, 2);
        // elapsed 651 crosses into Thirsty exactly as unwrapped.
        let c = active_conditions(&b, &needs, &[], 65000u16.wrapping_add(651));
        assert_eq!(c[0].condition_id, 1);
    }
}
