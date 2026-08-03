//! `needs_eval` — THE evaluation of needs → moodlets → mood (needs-moodlets F3).
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
//! - a CONDITIONAL moodlet is a band `lo <= sat < hi` on its need — derived, never granted;
//! - a TIMED moodlet is a stored grant `(moodlet_id, grant_tic)` alive while
//!   `elapsed < duration` (expiry DERIVED from the corpus, never stored);
//! - mood = `clamp(0.5 + Σ active offsets, 0..1)`.
//!
//! Tics are u16 and WRAP (~3 h at 6 Hz); every elapsed is `now.wrapping_sub(then)`. A need
//! left unwritten past the wrap window reads as freshly set — acceptable while every writer
//! (mint, drink, drills) touches rows far more often than the window.

use crate::loader::{Bundle, MoodletParams, NeedParams};

/// The base mood an unburdened pawn sits at (F5).
pub const MOOD_BASE: f64 = 0.5;

/// One ACTIVE moodlet: the corpus id, its mood offset, and — for a timed grant — the tics
/// it has left (`0` = conditional, alive exactly while its band holds).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveMoodlet {
    pub moodlet_id: u16,
    pub mood: f64,
    pub remaining: u16,
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

/// Every moodlet active at `now`: band (conditional) moodlets from the need rows +
/// unexpired timed grants. Rows naming an unknown need/moodlet are skipped (a corpus/state
/// version skew reads as "no moodlet", never a panic).
///
/// `needs` = decoded `NEED` entries `(need_id, satisfaction, set_tic)`; `grants` = decoded
/// `MOODLET` entries `(moodlet_id, grant_tic)` (`resonantdust_codec::payload`).
pub fn active_moodlets(
    bundle: &Bundle,
    needs: &[(u8, u8, u16)],
    grants: &[(u8, u16)],
    now: u16,
) -> Vec<ActiveMoodlet> {
    let mut out = Vec::new();
    for &(nid, q, t) in needs {
        let Some(np) = need_of(bundle, nid) else { continue };
        let sat = satisfaction_at(q, t, np.deplete, now);
        for band in &np.bands {
            if sat >= band.lo && sat < band.hi {
                if let Some(mid) = bundle.moodlet_id(&band.moodlet) {
                    if let Some(mp) = moodlet_of(bundle, mid as u8) {
                        out.push(ActiveMoodlet { moodlet_id: mid, mood: mp.mood, remaining: 0 });
                    }
                }
            }
        }
    }
    for &(mid, gt) in grants {
        let Some(mp) = moodlet_of(bundle, mid) else { continue };
        if mp.duration <= 0.0 {
            continue; // a stored grant of a CONDITIONAL moodlet is inert (F2)
        }
        let elapsed = f64::from(now.wrapping_sub(gt));
        if elapsed < mp.duration {
            out.push(ActiveMoodlet {
                moodlet_id: u16::from(mid),
                mood: mp.mood,
                remaining: (mp.duration - elapsed) as u16,
            });
        }
    }
    out
}

/// The mood the active set sums to: `clamp(0.5 + Σ offsets, 0..1)` (F5).
pub fn mood(active: &[ActiveMoodlet]) -> f64 {
    (MOOD_BASE + active.iter().map(|m| m.mood).sum::<f64>()).clamp(0.0, 1.0)
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
                if s0 > threshold {
                    // First elapsed with `sat < threshold`: sat == threshold is NOT below, so +1.
                    let cross = ((s0 - threshold) * np.deplete).floor() as u16 + 1;
                    consider(t.wrapping_add(cross));
                }
            }
        }
    }
    for &(mid, gt) in grants {
        let Some(mp) = moodlet_of(bundle, mid) else { continue };
        if mp.duration > 0.0 {
            consider(gt.wrapping_add(mp.duration.ceil() as u16));
        }
    }
    best
}

fn need_of(bundle: &Bundle, id: u8) -> Option<NeedParams> {
    let name = bundle.need_name(u16::from(id))?.to_string();
    bundle.need_params(&name)
}

fn moodlet_of(bundle: &Bundle, id: u8) -> Option<MoodletParams> {
    let name = bundle.moodlet_name(u16::from(id))?.to_string();
    bundle.moodlet_params(&name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::load;

    /// thirst: deplete 1000 tics, Thirsty [0.10, 0.35) −0.15, Dehydrated [0, 0.10) −0.40;
    /// quenched: +0.20 timed 100 tics.
    fn fixture() -> Bundle {
        let src = "\
<need>
  ::thirst>
    @define>
      1000 &need.deplete set
      \"thirsty &need.band.0.moodlet set
      0.10 &need.band.0.lo set
      0.35 &need.band.0.hi set
      \"dehydrated &need.band.1.moodlet set
      0 &need.band.1.lo set
      0.10 &need.band.1.hi set
      0 return
<moodlet>
  ::thirsty>
    @define>
      -0.15 &moodlet.mood set
      0 return
  ::dehydrated>
    @define>
      -0.40 &moodlet.mood set
      0 return
  ::quenched>
    @define>
      0.20 &moodlet.mood set
      100 &moodlet.duration set
      0 return
";
        load(&[("needs.rd".into(), src.into())]).expect("fixture loads")
    }

    #[test]
    fn the_crossing_tic_is_exact() {
        // Full (q 255 = 1.0) at tic 0, deplete 1000: sat < 0.35 first at elapsed
        // floor(0.65·1000)+1 = 651 — AT 650 sat == 0.35 exactly and the band is NOT active.
        let b = fixture();
        let needs = [(1u8, 255u8, 0u16)];
        assert!(active_moodlets(&b, &needs, &[], 650).is_empty(), "sat == hi is not < hi");
        let at651 = active_moodlets(&b, &needs, &[], 651);
        assert_eq!(at651.len(), 1);
        assert_eq!(at651[0].moodlet_id, 1, "Thirsty");
        assert_eq!(next_crossing_tic(&b, &needs, &[], 0), Some(651), "the computed crossing");
        // From inside Thirsty the next crossing is the 0.10 edge: floor(0.90·1000)+1 = 901.
        assert_eq!(next_crossing_tic(&b, &needs, &[], 651), Some(901));
        let at901 = active_moodlets(&b, &needs, &[], 901);
        assert_eq!(at901[0].moodlet_id, 2, "Dehydrated — bands are exclusive");
        assert_eq!(at901.len(), 1);
    }

    #[test]
    fn empty_clamps_and_stays_dehydrated() {
        // Past full depletion sat clamps at 0, which the [0, 0.10) band CONTAINS — a
        // starved wolf stays Dehydrated, it does not wrap back to content.
        let b = fixture();
        let needs = [(1u8, 255u8, 0u16)];
        let m = active_moodlets(&b, &needs, &[], 5000);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].moodlet_id, 2);
        assert_eq!(satisfaction_at(255, 0, 1000.0, 5000), 0.0);
        assert!((mood(&m) - 0.10).abs() < 1e-9, "0.5 − 0.40");
        // …and nothing ahead can change without a write.
        assert_eq!(next_crossing_tic(&b, &needs, &[], 5000), None);
    }

    #[test]
    fn absent_rows_and_unknown_ids_are_calm() {
        let b = fixture();
        assert!(active_moodlets(&b, &[], &[], 100).is_empty());
        assert_eq!(mood(&[]), MOOD_BASE);
        assert_eq!(next_crossing_tic(&b, &[], &[], 100), None);
        // unknown need id 9 / moodlet id 9 skip, never panic
        assert!(active_moodlets(&b, &[(9, 200, 0)], &[(9, 0)], 100).is_empty());
    }

    #[test]
    fn timed_grants_expire_and_stack_with_bands() {
        let b = fixture();
        let grants = [(3u8, 1000u16)]; // quenched, 100 tics
        let live = active_moodlets(&b, &[], &grants, 1099);
        assert_eq!(live.len(), 1);
        assert_eq!((live[0].moodlet_id, live[0].remaining), (3, 1));
        assert!(active_moodlets(&b, &[], &grants, 1100).is_empty(), "expiry at exactly duration");
        assert_eq!(next_crossing_tic(&b, &[], &grants, 1050), Some(1100));
        // Thirsty (−0.15) + Quenched (+0.20) sum: 0.5 − 0.15 + 0.20 = 0.55.
        let both = active_moodlets(&b, &[(1, 64, 1000)], &grants, 1050); // q64 ≈ 0.251 → Thirsty
        assert_eq!(both.len(), 2);
        assert!((mood(&both) - 0.55).abs() < 1e-9);
    }

    #[test]
    fn tic_wrap_is_handled() {
        // set near the top of the u16 window; `now` wrapped past 0.
        let b = fixture();
        let needs = [(1u8, 255u8, 65000u16)];
        // elapsed 1187 → sat ≈ 1 − 1.187 → clamped… no: 1 − 1187/1000 < 0 → 0 → Dehydrated.
        let m = active_moodlets(&b, &needs, &[], 65000u16.wrapping_add(1187));
        assert_eq!(m[0].moodlet_id, 2);
        // elapsed 651 crosses into Thirsty exactly as unwrapped.
        let m = active_moodlets(&b, &needs, &[], 65000u16.wrapping_add(651));
        assert_eq!(m[0].moodlet_id, 1);
    }
}
