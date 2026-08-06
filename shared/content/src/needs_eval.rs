//! `needs_eval` — THE evaluation of needs → conditions → mood (needs-moodlets F3).
//!
//! One implementation, three consumers: the worker, the npc Brain, and the client through
//! `shared/wasm`. A band comparison re-derived in TS would be the two-copies drift class
//! this repo has paid for twice — if the panel and the Brain ever disagree on a crossing
//! tic, it must be impossible by construction.
//!
//! The model (needs-moodlets F1/F2/F4/F5, stat-model F1..F7):
//! - a need row is the PACKED `(value:16 | kind:12 | variant:4, set_tic)` from the `needs`
//!   sub-table — value is u16 FIXED-POINT on the need's authored `min..max` domain
//!   ([`resonantdust_codec::value`]) and is NEVER ticked: the current value is
//!   `v0 − ∫ base_rate · multipliers`, integrated PIECEWISE across condition expiries;
//! - a DERIVED condition is a band `lo <= sat < hi` on its need — computed, never granted;
//! - a TIMED condition is a stored row `(remaining_at_write:16 | key:16, written_tic)`
//!   alive while `remaining_at_write − elapsed > 0` (expiry DERIVED, never stored — F3);
//! - traits and TIMED conditions modify a need's depletion RATE (multipliers form a
//!   product) under the RE-STAMP LAW (F7): every modifier-set mutation re-stamps the
//!   affected rows, so inside a row's window modifiers only ever EXPIRE — the expiry tic
//!   is derivable, and the eval integrates across it with no write;
//! - mood = `clamp(0.5 + Σ active offsets, 0..1)`.
//!
//! Tics are u16 and WRAP (~3 h at 6 Hz); every elapsed is `now.wrapping_sub(then)` under
//! the half-window guard: a row stamped AHEAD of the observer's learned clock reads as
//! elapsed 0, never as ancient (the phantom-Dehydrated fix, interactions P4).

use crate::loader::{Bundle, NeedParams};
use crate::stat_eval::condition_remaining;
use resonantdust_codec::object::{
    gameplay_row_data, gameplay_row_reference, GAMEPLAY_CONDITION, GAMEPLAY_NEED, GAMEPLAY_TRAIT,
};
use resonantdust_codec::value::dequantize;

/// The base mood an unburdened pawn sits at (F5).
pub const MOOD_BASE: f64 = 0.5;

/// One ACTIVE condition: the gameplay def ref, its mood offset, the tics it has left
/// (`0` = DERIVED, alive exactly while its band holds), and its authored display priority.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveCondition {
    pub condition_id: u32,
    pub mood: f64,
    pub remaining: u16,
    /// The corpus `priority`, carried so consumers can render but never re-decide the order —
    /// [`active_conditions`] has already sorted (conditions F3).
    pub priority: i32,
}

/// One rate-multiplier window over a need row's elapsed time (stat-model F6/F7): the
/// multiplier holds from the row's `set_tic` until `until` elapsed tics (`None` = forever —
/// a trait's contribution). The re-stamp law guarantees no window STARTS inside the row's
/// lifetime — grants re-stamp — so windows only end.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RateWindow {
    pub rate: f64,
    pub until: Option<f64>,
}

/// The elapsed tics of `now` since `then`, under the future-stamp half-window guard.
fn elapsed_guarded(then: u16, now: u16) -> f64 {
    let raw = now.wrapping_sub(then);
    if raw > u16::MAX / 2 {
        0.0
    } else {
        f64::from(raw)
    }
}

/// The rate-multiplier windows for one need row, from the pawn's trait rows + stored
/// condition rows, relative to the row's `set_tic`. Rows resolving to nothing are skipped.
pub fn rate_windows(
    bundle: &Bundle,
    need_name: &str,
    trait_rows: &[u32],
    condition_rows: &[(u32, u16)],
    set_tic: u16,
) -> Vec<RateWindow> {
    let mut out = Vec::new();
    for &row in trait_rows {
        let reference = gameplay_row_reference(GAMEPLAY_TRAIT, row);
        let Some(tp) = bundle.trait_params_by_ref(reference) else { continue };
        let level = gameplay_row_data(row) as usize;
        let Some(entry) = level.checked_sub(1).and_then(|i| tp.levels.get(i)) else { continue };
        for m in entry.needs.iter().filter(|m| m.need == need_name && m.rate != 1.0) {
            out.push(RateWindow { rate: m.rate, until: None });
        }
    }
    for &(row, written) in condition_rows {
        let reference = gameplay_row_reference(GAMEPLAY_CONDITION, row);
        let Some(cp) = bundle.condition_params_by_ref(reference) else { continue };
        if cp.duration <= 0.0 {
            continue; // a stored row of a DERIVED condition is inert (F2/F13)
        }
        let remaining_at_set = condition_remaining(row, written, set_tic);
        if remaining_at_set == 0 {
            continue; // already expired when the row was stamped
        }
        for m in cp.needs.iter().filter(|m| m.need == need_name && m.rate != 1.0) {
            out.push(RateWindow { rate: m.rate, until: Some(f64::from(remaining_at_set)) });
        }
    }
    out
}

/// The product of window multipliers active at elapsed `t` (a window covers `[0, until)`).
fn multiplier_at(windows: &[RateWindow], t: f64) -> f64 {
    windows
        .iter()
        .filter(|w| w.until.is_none_or(|u| t < u))
        .map(|w| w.rate)
        .product::<f64>()
        .max(0.0)
}

/// Total depletion (in need units) over `elapsed` tics at `base_rate`, integrated
/// PIECEWISE across the windows' ends (stat-model I4).
fn depletion(base_rate: f64, windows: &[RateWindow], elapsed: f64) -> f64 {
    let mut cuts: Vec<f64> =
        windows.iter().filter_map(|w| w.until).filter(|&u| u > 0.0 && u < elapsed).collect();
    cuts.sort_by(f64::total_cmp);
    cuts.dedup();
    cuts.push(elapsed);
    let mut total = 0.0;
    let mut prev = 0.0;
    for cut in cuts {
        // the multiplier is constant inside (prev, cut); sample at its midpoint
        total += (cut - prev) * base_rate * multiplier_at(windows, (prev + cut) / 2.0);
        prev = cut;
    }
    total
}

/// The current satisfaction of a need row at `now`, on the need's authored domain:
/// `clamp(v0) − ∫ rate`, floored at `min`. `deplete` 0 (unauthored) never drains.
/// `value` is the DEQUANTIZED authored-units value ([`resonantdust_codec::value`]);
/// `windows` from [`rate_windows`] (pass `&[]` for an unmodified need).
pub fn satisfaction_at(
    value: f64,
    set_tic: u16,
    np: &NeedParams,
    now: u16,
    windows: &[RateWindow],
) -> f64 {
    let s0 = value.clamp(np.min, np.max);
    if np.deplete <= 0.0 {
        return s0;
    }
    let base_rate = (np.max - np.min) / np.deplete;
    let elapsed = elapsed_guarded(set_tic, now);
    (s0 - depletion(base_rate, windows, elapsed)).max(np.min)
}

/// Decode + evaluate one packed need row: resolve its [`NeedParams`], dequantize its value,
/// build its windows, and return `(need_reference, params, satisfaction_at(now))`.
fn eval_need_row(
    bundle: &Bundle,
    row: u32,
    set_tic: u16,
    trait_rows: &[u32],
    condition_rows: &[(u32, u16)],
    now: u16,
) -> Option<(u32, NeedParams, f64)> {
    let reference = gameplay_row_reference(GAMEPLAY_NEED, row);
    let np = bundle.need_params_by_ref(reference)?;
    let value = f64::from(dequantize(gameplay_row_data(row), np.min as f32, np.max as f32));
    let (_, name) = bundle.gameplay_lookup(reference)?;
    let windows = rate_windows(bundle, &name, trait_rows, condition_rows, set_tic);
    let sat = satisfaction_at(value, set_tic, &np, now, &windows);
    Some((reference, np, sat))
}

/// Every condition active at `now`: band-DERIVED conditions from the need rows + unexpired
/// stored rows. Rows naming an unknown need/condition are skipped (a corpus/state version
/// skew reads as "no condition", never a panic).
///
/// `trait_rows` = decoded `TRAIT` payload rows; `need_rows` = the `needs` sub-table's
/// `(packed, set_tic)` pairs; `condition_rows` = decoded `CONDITION` payload rows
/// `(packed, written_tic)` (`resonantdust_codec::payload`).
///
/// **The result is SORTED** — `priority` desc, then `|mood|` desc, then `condition_id` asc
/// (conditions F2/F3). The order is a corpus rule, so it is decided here, once, for every
/// observer. No consumer re-sorts.
pub fn active_conditions(
    bundle: &Bundle,
    trait_rows: &[u32],
    need_rows: &[(u32, u16)],
    condition_rows: &[(u32, u16)],
    now: u16,
) -> Vec<ActiveCondition> {
    let mut out = Vec::new();
    for &(row, set_tic) in need_rows {
        let Some((_, np, sat)) = eval_need_row(bundle, row, set_tic, trait_rows, condition_rows, now)
        else {
            continue;
        };
        for band in &np.bands {
            if sat >= band.lo && sat < band.hi {
                if let Some(cref) = bundle.gameplay_reference("condition", &band.condition) {
                    if let Some(cp) = bundle.condition_params_by_ref(cref) {
                        out.push(ActiveCondition {
                            condition_id: cref,
                            mood: cp.mood,
                            remaining: 0,
                            priority: cp.priority,
                        });
                    }
                }
            }
        }
    }
    for &(row, written) in condition_rows {
        let cref = gameplay_row_reference(GAMEPLAY_CONDITION, row);
        let Some(cp) = bundle.condition_params_by_ref(cref) else { continue };
        if cp.duration <= 0.0 {
            continue; // a stored row of a DERIVED condition is inert (F2)
        }
        let remaining = condition_remaining(row, written, now);
        if remaining > 0 {
            out.push(ActiveCondition {
                condition_id: cref,
                mood: cp.mood,
                remaining,
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
/// earliest band-threshold crossing of any need (under its PIECEWISE rate) + the earliest
/// stored-row expiry. `None` when nothing ahead can change. This is what lets observers
/// sleep between crossings instead of sampling (F4).
pub fn next_crossing_tic(
    bundle: &Bundle,
    trait_rows: &[u32],
    need_rows: &[(u32, u16)],
    condition_rows: &[(u32, u16)],
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
    for &(row, set_tic) in need_rows {
        let reference = gameplay_row_reference(GAMEPLAY_NEED, row);
        let Some(np) = bundle.need_params_by_ref(reference) else { continue };
        if np.deplete <= 0.0 {
            continue;
        }
        let Some((_, name)) = bundle.gameplay_lookup(reference) else { continue };
        let windows = rate_windows(bundle, &name, trait_rows, condition_rows, set_tic);
        let base_rate = (np.max - np.min) / np.deplete;
        let value = f64::from(dequantize(gameplay_row_data(row), np.min as f32, np.max as f32));
        let s0 = value.clamp(np.min, np.max);
        for band in &np.bands {
            for threshold in [band.lo, band.hi] {
                // A threshold at (or below) the domain floor is never crossed: satisfaction
                // CLAMPS there, so a bottom band (`lo == min`) holds forever.
                if threshold <= np.min || s0 <= threshold {
                    continue;
                }
                if let Some(e) = crossing_elapsed(s0 - threshold, base_rate, &windows) {
                    // First integer elapsed with `sat < threshold`: at floor(e) sat is still
                    // >= threshold (== at an exact integer crossing), so +1.
                    consider(set_tic.wrapping_add(e.floor() as u16 + 1));
                }
            }
        }
    }
    for &(row, written) in condition_rows {
        let cref = gameplay_row_reference(GAMEPLAY_CONDITION, row);
        let Some(cp) = bundle.condition_params_by_ref(cref) else { continue };
        if cp.duration > 0.0 && condition_remaining(row, written, now) > 0 {
            consider(written.wrapping_add(gameplay_row_data(row)));
        }
    }
    best
}

/// The REAL elapsed at which cumulative depletion reaches `drop` (> 0), walking the
/// piecewise segments; `None` if it never does (every live segment rate 0 with no end).
fn crossing_elapsed(drop: f64, base_rate: f64, windows: &[RateWindow]) -> Option<f64> {
    let mut cuts: Vec<f64> = windows.iter().filter_map(|w| w.until).filter(|&u| u > 0.0).collect();
    cuts.sort_by(f64::total_cmp);
    cuts.dedup();
    let mut prev = 0.0;
    let mut left = drop;
    for cut in cuts {
        let rate = base_rate * multiplier_at(windows, (prev + cut) / 2.0);
        let span = cut - prev;
        if rate > 0.0 && left <= rate * span {
            return Some(prev + left / rate);
        }
        left -= rate * span;
        prev = cut;
    }
    // the unbounded tail: every finite window has ended
    let rate = base_rate * multiplier_at(windows, prev + 1.0);
    (rate > 0.0).then(|| prev + left / rate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::load;
    use resonantdust_codec::object::pack_gameplay_row;
    use resonantdust_codec::value::quantize;

    /// thirst: deplete 1000 tics on the DEFAULT `0..1` domain, Thirsty [0.10, 0.35) −0.15,
    /// Dehydrated [0, 0.10) −0.40; quenched: +0.20 timed 100 tics.
    fn fixture() -> Bundle {
        let src = r#"
[[need]]
name = "thirst"
deplete = 1000
band = [
  { condition = "thirsty", lo = 0.10, hi = 0.35 },
  { condition = "dehydrated", lo = 0.0, hi = 0.10 },
]

[[condition]]
name = "thirsty"
mood = -0.15

[[condition]]
name = "dehydrated"
mood = -0.40

[[condition]]
name = "quenched"
mood = 0.20
duration = 100
"#;
        load(&[("needs.toml".into(), src.into())]).expect("fixture loads")
    }

    fn need_row(b: &Bundle, name: &str, value: f64) -> u32 {
        let np = b.need_params(name).expect(name);
        let q = quantize(value as f32, np.min as f32, np.max as f32);
        pack_gameplay_row(b.gameplay_reference("need", name).expect(name), q)
    }
    fn cond_row(b: &Bundle, name: &str, remaining: u16) -> u32 {
        pack_gameplay_row(b.gameplay_reference("condition", name).expect(name), remaining)
    }
    fn cond_ref(b: &Bundle, name: &str) -> u32 {
        b.gameplay_reference("condition", name).expect(name)
    }
    fn np(b: &Bundle, name: &str) -> NeedParams {
        b.need_params(name).expect(name)
    }

    /// A fixture built to make EVERY sort key decide something, and to make the
    /// insertion order deliberately wrong on all three.
    fn sort_fixture() -> Bundle {
        let src = r#"
[[need]]
name = "n"
deplete = 0
band = [
  { condition = "mid_a", lo = 0.0, hi = 1.01 },
  { condition = "mid_b", lo = 0.0, hi = 1.01 },
  { condition = "tie_b", lo = 0.0, hi = 1.01 },
  { condition = "low",   lo = 0.0, hi = 1.01 },
]

[[condition]]
name = "low"
mood = -0.01
priority = 99

[[condition]]
name = "mid_a"
mood = -0.10
priority = 10

[[condition]]
name = "mid_b"
mood = 0.50
priority = 10

[[condition]]
name = "tie_b"
mood = -0.20
priority = 5

[[condition]]
name = "tie_a"
mood = 0.20
priority = 5
duration = 100
"#;
        load(&[("needs.toml".into(), src.into())]).expect("sort fixture loads")
    }

    #[test]
    fn the_order_is_priority_then_magnitude_then_ref() {
        let b = sort_fixture();
        // Bands are non-exclusive here ON PURPOSE — this test is about ORDER, not membership.
        let rows = [(need_row(&b, "n", 1.0), 0u16)];
        let grants = [(cond_row(&b, "tie_a", 100), 0u16)];
        let order: Vec<u32> = active_conditions(&b, &[], &rows, &grants, 50)
            .iter()
            .map(|c| c.condition_id)
            .collect();
        let expect: Vec<u32> =
            ["low", "mid_b", "mid_a", "tie_b", "tie_a"].iter().map(|c| cond_ref(&b, c)).collect();
        assert_eq!(
            order, expect,
            "priority desc (low first), then |mood| desc (mid_b 0.50 > mid_a 0.10), \
             then ref asc (tie_b before tie_a — equal priority AND equal |mood| 0.20)"
        );
        let by_ref = |r: u32| {
            active_conditions(&b, &[], &rows, &grants, 50)
                .into_iter()
                .find(|c| c.condition_id == r)
                .unwrap()
        };
        assert_eq!(by_ref(cond_ref(&b, "low")).priority, 99);
        assert_eq!(by_ref(cond_ref(&b, "tie_a")).priority, 5, "a stored row carries priority too");
    }

    #[test]
    fn the_crossing_tic_is_exact() {
        // Full (1.0) at tic 0, deplete 1000: sat < 0.35 first at elapsed
        // floor(0.65·1000)+1 = 651 — AT 650 sat == 0.35 exactly and the band is NOT active.
        let b = fixture();
        let needs = [(need_row(&b, "thirst", 1.0), 0u16)];
        assert!(active_conditions(&b, &[], &needs, &[], 650).is_empty(), "sat == hi is not < hi");
        let at651 = active_conditions(&b, &[], &needs, &[], 651);
        assert_eq!(at651.len(), 1);
        assert_eq!(at651[0].condition_id, cond_ref(&b, "thirsty"));
        assert_eq!(next_crossing_tic(&b, &[], &needs, &[], 0), Some(651), "the computed crossing");
        // From inside Thirsty the next crossing is the 0.10 edge: floor(0.90·1000)+1 = 901.
        assert_eq!(next_crossing_tic(&b, &[], &needs, &[], 651), Some(901));
        let at901 = active_conditions(&b, &[], &needs, &[], 901);
        assert_eq!(at901[0].condition_id, cond_ref(&b, "dehydrated"), "bands are exclusive");
        assert_eq!(at901.len(), 1);
    }

    #[test]
    fn the_p0_worked_window_reproduces_exactly() {
        // VARIABLES.md §Pawn gameplay state, the quenched piecewise window: thirst 0..100
        // deplete 21600; stamped 40.0 at tic 1000 with quenched (rate 0.5, remaining 3600);
        // read at tic 6000 → 40.0 − 8.3333 − 6.4815 ≈ 25.1852.
        let src = r#"
[[need]]
name = "thirst"
min = 0
max = 100
deplete = 21600

[[condition]]
name = "quenched"
mood = 0.20
duration = 3600
needs = [ { need = "thirst", rate = 0.5 } ]
"#;
        let b = load(&[("needs.toml".into(), src.into())]).expect("loads");
        let p = np(&b, "thirst");
        let conditions = [(cond_row(&b, "quenched", 3600), 1000u16)];
        let windows = rate_windows(&b, "thirst", &[], &conditions, 1000);
        assert_eq!(windows, vec![RateWindow { rate: 0.5, until: Some(3600.0) }]);
        let sat = satisfaction_at(40.0, 1000, &p, 6000, &windows);
        assert!((sat - 25.1852).abs() < 1e-3, "the worked window (got {sat})");
        // Inside the quenched window the slope is HALVED: at tic 4600 (expiry) only
        // 8.3333 has drained.
        let at_expiry = satisfaction_at(40.0, 1000, &p, 4600, &windows);
        assert!((at_expiry - (40.0 - 8.3333)).abs() < 1e-3, "got {at_expiry}");
        // ... and the full pipeline (packed row → dequantize → windows) agrees.
        let needs = [(need_row(&b, "thirst", 40.0), 1000u16)];
        let active = active_conditions(&b, &[], &needs, &conditions, 6000);
        assert!(active.is_empty(), "25.18 is band-free and quenched expired at 4600");
    }

    #[test]
    fn a_trait_rate_window_never_ends() {
        // A camel-ish trait halves thirst depletion FOREVER (no expiry cut).
        let src = r#"
[[need]]
name = "thirst"
min = 0
max = 100
deplete = 1000

[[trait]]
name = "camel"
needs = [ { need = "thirst", rate = [0.5] } ]
"#;
        let b = load(&[("needs.toml".into(), src.into())]).expect("loads");
        let p = np(&b, "thirst");
        let tr = pack_gameplay_row(b.gameplay_reference("trait", "camel").unwrap(), 1);
        let windows = rate_windows(&b, "thirst", &[tr], &[], 0);
        assert_eq!(windows, vec![RateWindow { rate: 0.5, until: None }]);
        // 1000 tics at half of 0.1/tic → 50 drained, not 100.
        let sat = satisfaction_at(100.0, 0, &p, 1000, &windows);
        assert!((sat - 50.0).abs() < 1e-9, "got {sat}");
    }

    #[test]
    fn a_piecewise_crossing_lands_where_the_slope_says() {
        // thirst 0..100, deplete 1000 (0.1/tic), band edge at 35. From 40.0 with quenched
        // halving the first 100 tics: drop 5 needs 100·0.05 = 5 → crossing exactly at the
        // expiry boundary elapsed 100 → first-below = 101. Without the window: 50+1 = 51.
        let src = r#"
[[need]]
name = "thirst"
min = 0
max = 100
deplete = 1000
band = [ { condition = "thirsty", lo = 10, hi = 35 } ]

[[condition]]
name = "thirsty"
mood = -0.15

[[condition]]
name = "quenched"
mood = 0.20
duration = 3600
needs = [ { need = "thirst", rate = 0.5 } ]
"#;
        let b = load(&[("needs.toml".into(), src.into())]).expect("loads");
        let needs = [(need_row(&b, "thirst", 40.0), 0u16)];
        assert_eq!(next_crossing_tic(&b, &[], &needs, &[], 0), Some(51), "unmodified");
        let conditions = [(cond_row(&b, "quenched", 100), 0u16)];
        // wakes: the quenched EXPIRY at 100 comes before the band crossing at 101.
        assert_eq!(next_crossing_tic(&b, &[], &needs, &conditions, 0), Some(100));
        // past the expiry, the band crossing is the next wake.
        assert_eq!(next_crossing_tic(&b, &[], &needs, &conditions, 100), Some(101));
        let at100 = active_conditions(&b, &[], &needs, &conditions, 100);
        assert!(at100.is_empty(), "at the boundary sat == 35 exactly — not yet Thirsty");
        let at101 = active_conditions(&b, &[], &needs, &conditions, 101);
        assert_eq!(at101.len(), 1, "one tic later the band holds");
    }

    #[test]
    fn empty_clamps_and_stays_dehydrated() {
        // Past full depletion sat clamps at min, which the [0, 0.10) band CONTAINS.
        let b = fixture();
        let needs = [(need_row(&b, "thirst", 1.0), 0u16)];
        let c = active_conditions(&b, &[], &needs, &[], 5000);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].condition_id, cond_ref(&b, "dehydrated"));
        assert_eq!(satisfaction_at(1.0, 0, &np(&b, "thirst"), 5000, &[]), 0.0);
        assert!((mood(&c) - 0.10).abs() < 1e-9, "0.5 − 0.40");
        // …and nothing ahead can change without a write.
        assert_eq!(next_crossing_tic(&b, &[], &needs, &[], 5000), None);
        // Inside Dehydrated but not yet at min: the only threshold below is the CLAMP
        // (band.lo == min), which is never crossed — no spurious wake.
        assert_eq!(next_crossing_tic(&b, &[], &needs, &[], 950), None);
    }

    #[test]
    fn absent_rows_and_unknown_refs_are_calm() {
        let b = fixture();
        assert!(active_conditions(&b, &[], &[], &[], 100).is_empty());
        assert_eq!(mood(&[]), MOOD_BASE);
        assert_eq!(next_crossing_tic(&b, &[], &[], &[], 100), None);
        // an unknown need/condition KEY skips, never panics.
        let ghost = pack_gameplay_row(0x8001_0FF0, 30000); // gameplay/need, kind past the registry
        assert!(active_conditions(&b, &[], &[(ghost, 0)], &[(ghost, 0)], 100).is_empty());
    }

    #[test]
    fn stored_rows_expire_and_stack_with_bands() {
        let b = fixture();
        let q = cond_ref(&b, "quenched");
        let rows = [(cond_row(&b, "quenched", 100), 1000u16)];
        let live = active_conditions(&b, &[], &[], &rows, 1099);
        assert_eq!(live.len(), 1);
        assert_eq!((live[0].condition_id, live[0].remaining), (q, 1));
        assert!(active_conditions(&b, &[], &[], &rows, 1100).is_empty(), "expiry at remaining 0");
        assert_eq!(next_crossing_tic(&b, &[], &[], &rows, 1050), Some(1100));
        // Thirsty (−0.15) + Quenched (+0.20) sum: 0.5 − 0.15 + 0.20 = 0.55.
        let needs = [(need_row(&b, "thirst", 0.251), 1000u16)];
        let both = active_conditions(&b, &[], &needs, &rows, 1050);
        assert_eq!(both.len(), 2);
        assert!((mood(&both) - 0.55).abs() < 1e-9);
    }

    #[test]
    fn a_future_stamped_row_reads_as_fresh_not_ancient() {
        // An effect lands at its COMPOSE tic, which can be a few tics AHEAD of an
        // observer's learned clock. It must read as elapsed-0, never as wrapped-past.
        let b = fixture();
        let p = np(&b, "thirst");
        let s = satisfaction_at(0.8, 105, &p, 100, &[]);
        assert!((s - 0.8).abs() < 1e-6, "5 tics in the future = fresh (got {s})");
        let c = active_conditions(&b, &[], &[(need_row(&b, "thirst", 0.8), 105)], &[], 100);
        assert!(c.is_empty(), "0.8 is band-free — no phantom Dehydrated");
    }

    #[test]
    fn tic_wrap_is_handled() {
        // set near the top of the u16 window; `now` wrapped past 0.
        let b = fixture();
        let needs = [(need_row(&b, "thirst", 1.0), 65000u16)];
        // elapsed 1187 → sat 1 − 1.187 < 0 → clamped to min → Dehydrated.
        let c = active_conditions(&b, &[], &needs, &[], 65000u16.wrapping_add(1187));
        assert_eq!(c[0].condition_id, cond_ref(&b, "dehydrated"));
        // elapsed 651 crosses into Thirsty exactly as unwrapped.
        let c = active_conditions(&b, &[], &needs, &[], 65000u16.wrapping_add(651));
        assert_eq!(c[0].condition_id, cond_ref(&b, "thirsty"));
    }

    #[test]
    fn quantization_is_invisible_at_domain_scale() {
        // The 0..100 fixed-point step is ~0.0015 — a stamped 33.0 reads back within it.
        let b = fixture();
        let src = r#"
[[need]]
name = "thirst"
min = 0
max = 100
deplete = 1000
"#;
        let b2 = load(&[("needs.toml".into(), src.into())]).expect("loads");
        let row = need_row(&b2, "thirst", 33.0);
        let p = np(&b2, "thirst");
        let v = f64::from(dequantize(gameplay_row_data(row), p.min as f32, p.max as f32));
        assert!((v - 33.0).abs() < 0.002, "got {v}");
        drop(b);
    }
}
