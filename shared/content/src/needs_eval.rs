//! `needs_eval` — THE evaluation of needs → conditions (needs-moodlets F3).
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
//! - the active EMOTION is the argmax over the summed emotion modifiers of the active
//!   set (emotions F3) — `emotion_eval`; MOOD is retired (emotions F4).
//!
//! Tics are u16 and WRAP (~3 h at 6 Hz); every elapsed is `now.wrapping_sub(then)` under
//! the half-window guard: a row stamped AHEAD of the observer's learned clock reads as
//! elapsed 0, never as ancient (the phantom-Dehydrated fix, interactions P4).

use crate::loader::{Bundle, NeedParams};
use crate::stat_eval::condition_remaining;
use resonantdust_codec::object::{
    def_variant_id, row_data, row_reference,
};
use resonantdust_codec::value::dequantize;

/// One ACTIVE condition: the gameplay def ref, the Σ of its emotion-modifier magnitudes
/// (the sort's tie-break — emotions F4), the tics it has left (`0` = DERIVED, alive
/// exactly while its band holds), and its authored display priority.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveCondition {
    pub condition_id: u32,
    pub magnitude_sum: u8,
    pub remaining: u16,
    /// The corpus `priority`, carried so consumers can render but never re-decide the order —
    /// [`active_conditions`] has already sorted (conditions F3).
    pub priority: i32,
}

/// One rate-multiplier window over a need row's elapsed time (stat-model F6/F7/I12): the
/// multiplier holds for elapsed `start <= t < until` (`until` `None` = forever — a trait's
/// contribution). `start` is normally 0 (the re-stamp law pairs grants with re-stamps), but
/// a lone FIRST grant landing after the row's `set_tic` is derivable from its `written_tic`
/// and gets a mid-row start — exact without a re-stamp (I12). Re-grants overwrite their own
/// history, which is why the law still binds composers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RateWindow {
    pub rate: f64,
    pub start: f64,
    pub until: Option<f64>,
    /// survival F3: an ABSOLUTE depletion contribution (need units/tic) while this
    /// window is active — the deplete modifier's lane. Multipliers scale the BASE rate
    /// only; adds are absolute and SUM (quenched halving thirst does not halve
    /// starving's corpus drain). A pure multiplier window carries 0.
    pub add: f64,
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

/// The add contribution (units/tic) a deplete modifier authors for `np`'s domain.
fn add_of(np: &NeedParams, deplete: f64) -> f64 {
    if deplete > 0.0 { (np.max - np.min) / deplete } else { 0.0 }
}

/// The rate windows for one need row — multipliers AND deplete-modifier adds — from the
/// pawn's trait rows, stored condition rows, and (survival F3) the OTHER need rows whose
/// BAND-DERIVED conditions author a deplete on this need (starving drains corpus while
/// hunger sits in its band; the in-band interval derives from the SOURCE's own
/// trajectory). Relative to this row's `set_tic`. Rows resolving to nothing are skipped.
///
/// ACYCLICITY LAW: source trajectories are computed WITHOUT cross-need drains (depth 1) —
/// a drain TARGET must not itself carry bands that drain others. Corpus (the target) has
/// no bands; thirst/hunger (the sources) are drained by nothing. A future cycle is a
/// content bug this eval answers with the depth-1 approximation, not a hang.
pub fn rate_windows(
    bundle: &Bundle,
    need_name: &str,
    trait_rows: &[u64],
    condition_rows: &[(u64, u16)],
    need_rows: &[(u64, u16)],
    set_tic: u16,
) -> Vec<RateWindow> {
    let np_target = bundle.need_params(need_name);
    let mut out = Vec::new();
    for &row in trait_rows {
        let reference = row_reference(row);
        let Some(tp) = bundle.trait_params_by_ref(reference) else { continue };
        let tier = def_variant_id(reference) as usize;
        let Some(entry) = tp.levels.get(tier) else { continue };
        for m in entry.needs.iter().filter(|m| m.need == need_name) {
            let add = np_target.as_ref().and_then(|np| m.deplete.map(|d| add_of(np, d))).unwrap_or(0.0);
            if m.rate != 1.0 || add > 0.0 {
                out.push(RateWindow { rate: m.rate, start: 0.0, until: None, add });
            }
        }
    }
    for &(row, written) in condition_rows {
        let reference = row_reference(row);
        let Some(cp) = bundle.condition_params_by_ref(reference) else { continue };
        if cp.duration <= 0.0 {
            continue; // a stored row of a DERIVED condition is inert (F2/F13)
        }
        // Where does this grant sit relative to the row's stamp? Written at-or-before the
        // stamp (the paired-re-stamp common case, or older): starts at 0 with the remaining
        // it had AT the stamp. Written AFTER the stamp (a lone unpaired grant — I12): starts
        // at the derivable offset with its full remaining.
        let offset = written.wrapping_sub(set_tic);
        let (start, remaining) = if offset > u16::MAX / 2 {
            (0.0, condition_remaining(row, written, set_tic))
        } else {
            (f64::from(offset), row_data(row))
        };
        if remaining == 0 {
            continue; // already expired when the row was stamped
        }
        for m in cp.needs.iter().filter(|m| m.need == need_name) {
            let add = np_target.as_ref().and_then(|np| m.deplete.map(|d| add_of(np, d))).unwrap_or(0.0);
            if m.rate != 1.0 || add > 0.0 {
                out.push(RateWindow {
                    rate: m.rate, start, until: Some(start + f64::from(remaining)), add,
                });
            }
        }
    }
    // survival F3: cross-need DERIVED drains — a source need whose band condition
    // authors a deplete on THIS need drains it while the source sits in-band.
    let Some(np_target) = np_target else { return out };
    for &(src_row, src_set) in need_rows {
        let src_ref = row_reference(src_row);
        let Some(src_np) = bundle.need_params_by_ref(src_ref) else { continue };
        let Some((_, src_name)) = bundle.gameplay_lookup(src_ref) else { continue };
        if src_name == need_name {
            continue; // self-drain rides the row's own windows, not the derived lane
        }
        for band in &src_np.bands {
            let Some(cref) = bundle.gameplay_reference("condition", &band.condition) else { continue };
            let Some(cp) = bundle.condition_params_by_ref(cref) else { continue };
            for m in cp.needs.iter().filter(|m| m.need == need_name) {
                let Some(d) = m.deplete else { continue };
                let add = add_of(&np_target, d);
                if add <= 0.0 {
                    continue;
                }
                // The source's own trajectory (depth 1 — no cross-need drains).
                let src_windows =
                    rate_windows(bundle, &src_name, trait_rows, condition_rows, &[], src_set);
                let s0 = f64::from(dequantize(
                    row_data(src_row), src_np.min as f32, src_np.max as f32,
                ))
                .clamp(src_np.min, src_np.max);
                let Some((enter, exit)) =
                    in_band_interval(s0, &src_np, &src_windows, band.lo, band.hi)
                else {
                    continue;
                };
                // Convert source-elapsed → THIS row's elapsed frame (signed wrapped
                // offset between the two stamps, exact inside the half-window).
                let off = f64::from(src_set.wrapping_sub(set_tic) as i16);
                let start = (enter + off).max(0.0);
                let until = exit.map(|e| e + off).filter(|&e| e > start);
                if exit.is_some() && until.is_none() {
                    continue; // the whole interval predates this row's stamp
                }
                out.push(RateWindow { rate: 1.0, start, until, add });
            }
        }
    }
    out
}

/// When does a DECREASING lazy value sit inside `[lo, hi)`? Returns the source-elapsed
/// `(enter, exit)` — `exit` `None` when the domain floor clamps it in-band forever (the
/// bottom band holds). `None` = never enters (already below, or never reaches the band).
fn in_band_interval(
    s0: f64,
    np: &NeedParams,
    windows: &[RateWindow],
    lo: f64,
    hi: f64,
) -> Option<(f64, Option<f64>)> {
    let base_rate = if np.deplete > 0.0 { (np.max - np.min) / np.deplete } else { 0.0 };
    if s0 < lo {
        return None; // below the band already; a decreasing value never re-enters
    }
    let enter = if s0 < hi {
        0.0
    } else {
        crossing_elapsed(s0 - hi, base_rate, windows)?
    };
    let exit = if lo <= np.min {
        None // the clamp holds the value in the bottom band forever
    } else {
        crossing_elapsed(s0 - lo, base_rate, windows)
    };
    Some((enter, exit))
}

/// The product of window multipliers active at elapsed `t` (a window covers `[start, until)`).
fn multiplier_at(windows: &[RateWindow], t: f64) -> f64 {
    windows
        .iter()
        .filter(|w| t >= w.start && w.until.is_none_or(|u| t < u))
        .map(|w| w.rate)
        .product::<f64>()
        .max(0.0)
}

/// The SEGMENT rate at elapsed `t` (survival F3): the base under its multipliers, plus
/// the absolute deplete-modifier adds — multipliers never scale the adds.
fn segment_rate(base_rate: f64, windows: &[RateWindow], t: f64) -> f64 {
    let adds: f64 = windows
        .iter()
        .filter(|w| w.add > 0.0 && t >= w.start && w.until.is_none_or(|u| t < u))
        .map(|w| w.add)
        .sum();
    base_rate * multiplier_at(windows, t) + adds
}

/// Every elapsed at which some window's multiplier switches on or off, ascending.
fn window_cuts(windows: &[RateWindow]) -> Vec<f64> {
    let mut cuts: Vec<f64> = windows
        .iter()
        .flat_map(|w| [Some(w.start), w.until])
        .flatten()
        .filter(|&c| c > 0.0)
        .collect();
    cuts.sort_by(f64::total_cmp);
    cuts.dedup();
    cuts
}

/// Total depletion (in need units) over `elapsed` tics at `base_rate`, integrated
/// PIECEWISE across the windows' edges (stat-model I4).
fn depletion(base_rate: f64, windows: &[RateWindow], elapsed: f64) -> f64 {
    let mut cuts: Vec<f64> =
        window_cuts(windows).into_iter().filter(|&c| c < elapsed).collect();
    cuts.push(elapsed);
    let mut total = 0.0;
    let mut prev = 0.0;
    for cut in cuts {
        // the rate is constant inside (prev, cut); sample at its midpoint
        total += (cut - prev) * segment_rate(base_rate, windows, (prev + cut) / 2.0);
        prev = cut;
    }
    total
}

/// A need's EFFECTIVE clamp at `now` (stat-model F6): the authored domain intersected with
/// the min/max contributions of trait levels + stored conditions ALIVE at `now`, under the
/// need's authored `winner`. Rate is not consulted here — this is the write-side clamp.
pub fn need_bounds(
    bundle: &Bundle,
    need_name: &str,
    np: &NeedParams,
    trait_rows: &[u64],
    condition_rows: &[(u64, u16)],
    now: u16,
) -> (f64, f64) {
    let mut contribs: Vec<(Option<f64>, Option<f64>)> = Vec::new();
    for &row in trait_rows {
        let reference = row_reference(row);
        let Some(tp) = bundle.trait_params_by_ref(reference) else { continue };
        let tier = def_variant_id(reference) as usize;
        let Some(entry) = tp.levels.get(tier) else { continue };
        contribs.extend(entry.needs.iter().filter(|m| m.need == need_name).map(|m| (m.min, m.max)));
    }
    for &(row, written) in condition_rows {
        let reference = row_reference(row);
        let Some(cp) = bundle.condition_params_by_ref(reference) else { continue };
        if cp.duration <= 0.0 || condition_remaining(row, written, now) == 0 {
            continue;
        }
        contribs.extend(cp.needs.iter().filter(|m| m.need == need_name).map(|m| (m.min, m.max)));
    }
    // food-chain F2: need CAPS TIER UPWARD — among authored MAX modifiers the HIGHEST
    // wins (unlike stat ranges, which intersect), so a leveled cap trait (`corpus`)
    // raises the ceiling by leveling and a second source can never silently shrink it.
    // No max authored anywhere = the authored domain max. Mins keep the max-of-mins
    // floor; the authored domain stays the OUTERMOST clamp (the encoding bound).
    let mut lo = np.min;
    let mut hi: Option<f64> = None;
    for (cmin, cmax) in contribs {
        if let Some(m) = cmin {
            lo = lo.max(m);
        }
        if let Some(m) = cmax {
            hi = Some(hi.map_or(m, |h: f64| h.max(m)));
        }
    }
    let hi = hi.unwrap_or(np.max).clamp(np.min, np.max);
    let lo = lo.clamp(np.min, np.max);
    if lo > hi {
        let pin = if np.min_wins { lo } else { hi };
        (pin, pin)
    } else {
        (lo, hi)
    }
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
    // survival F3: a deplete-0 need still moves under deplete-MODIFIER adds (corpus
    // under starving); with no adds either, it never moves — the old fast path.
    if np.deplete <= 0.0 && windows.iter().all(|w| w.add <= 0.0) {
        return s0;
    }
    let base_rate = if np.deplete > 0.0 { (np.max - np.min) / np.deplete } else { 0.0 };
    let elapsed = elapsed_guarded(set_tic, now);
    (s0 - depletion(base_rate, windows, elapsed)).max(np.min)
}

/// Decode + evaluate one packed need row: resolve its [`NeedParams`], dequantize its value,
/// build its windows, and return `(need_reference, params, satisfaction_at(now))`.
fn eval_need_row(
    bundle: &Bundle,
    row: u64,
    set_tic: u16,
    trait_rows: &[u64],
    condition_rows: &[(u64, u16)],
    need_rows: &[(u64, u16)],
    now: u16,
) -> Option<(u32, NeedParams, f64)> {
    let reference = row_reference(row);
    let np = bundle.need_params_by_ref(reference)?;
    let value = f64::from(dequantize(row_data(row), np.min as f32, np.max as f32));
    let (_, name) = bundle.gameplay_lookup(reference)?;
    let windows = rate_windows(bundle, &name, trait_rows, condition_rows, need_rows, set_tic);
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
/// **The result is SORTED** — `priority` desc, then Σ emotion magnitude desc, then
/// `condition_id` asc (conditions F2/F3, emotions F4). The order is a corpus rule, so it
/// is decided here, once, for every observer. No consumer re-sorts.
pub fn active_conditions(
    bundle: &Bundle,
    trait_rows: &[u64],
    need_rows: &[(u64, u16)],
    condition_rows: &[(u64, u16)],
    now: u16,
) -> Vec<ActiveCondition> {
    let mut out = Vec::new();
    for &(row, set_tic) in need_rows {
        let Some((_, np, sat)) =
            eval_need_row(bundle, row, set_tic, trait_rows, condition_rows, need_rows, now)
        else {
            continue;
        };
        for band in &np.bands {
            if sat >= band.lo && sat < band.hi {
                if let Some(cref) = bundle.gameplay_reference("condition", &band.condition) {
                    if let Some(cp) = bundle.condition_params_by_ref(cref) {
                        out.push(ActiveCondition {
                            condition_id: cref,
                            magnitude_sum: cp.emotions.iter().map(|e| e.magnitude).sum(),
                            remaining: 0,
                            priority: cp.priority,
                        });
                    }
                }
            }
        }
    }
    for &(row, written) in condition_rows {
        let cref = row_reference(row);
        let Some(cp) = bundle.condition_params_by_ref(cref) else { continue };
        if cp.duration <= 0.0 {
            continue; // a stored row of a DERIVED condition is inert (F2)
        }
        let remaining = condition_remaining(row, written, now);
        if remaining > 0 {
            out.push(ActiveCondition {
                condition_id: cref,
                magnitude_sum: cp.emotions.iter().map(|e| e.magnitude).sum(),
                remaining,
                priority: cp.priority,
            });
        }
    }
    // `sort_by` (stable) with a total comparator — see the doc comment.
    out.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| b.magnitude_sum.cmp(&a.magnitude_sum))
            .then_with(|| a.condition_id.cmp(&b.condition_id))
    });
    out
}

/// A NAMED need's lazy satisfaction for this pawn at `now` (food-chain F4 — the
/// need-check affordance's read; the same PIECEWISE eval every band uses). `None` =
/// the pawn carries no row for that need (a pawn without `corpus` cannot `can_die`).
pub fn need_satisfaction(
    bundle: &Bundle,
    need: &str,
    trait_rows: &[u64],
    need_rows: &[(u64, u16)],
    condition_rows: &[(u64, u16)],
    now: u16,
) -> Option<f64> {
    let nref = bundle.gameplay_reference("need", need)?;
    let &(row, set_tic) = need_rows
        .iter()
        .find(|(row, _)| row_reference(*row) == nref)?;
    eval_need_row(bundle, row, set_tic, trait_rows, condition_rows, need_rows, now)
        .map(|(_, _, sat)| sat)
}

/// The next FUTURE tic at which the active set can change without any new write: the
/// earliest band-threshold crossing of any need (under its PIECEWISE rate) + the earliest
/// stored-row expiry. `None` when nothing ahead can change. This is what lets observers
/// sleep between crossings instead of sampling (F4).
pub fn next_crossing_tic(
    bundle: &Bundle,
    trait_rows: &[u64],
    need_rows: &[(u64, u16)],
    condition_rows: &[(u64, u16)],
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
        let reference = row_reference(row);
        let Some(np) = bundle.need_params_by_ref(reference) else { continue };
        let Some((_, name)) = bundle.gameplay_lookup(reference) else { continue };
        let windows = rate_windows(bundle, &name, trait_rows, condition_rows, need_rows, set_tic);
        // survival F3: a deplete-0 need can still cross under deplete-modifier adds.
        if np.deplete <= 0.0 && windows.iter().all(|w| w.add <= 0.0) {
            continue;
        }
        let base_rate = if np.deplete > 0.0 { (np.max - np.min) / np.deplete } else { 0.0 };
        let value = f64::from(dequantize(row_data(row), np.min as f32, np.max as f32));
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
        let cref = row_reference(row);
        let Some(cp) = bundle.condition_params_by_ref(cref) else { continue };
        if cp.duration > 0.0 && condition_remaining(row, written, now) > 0 {
            consider(written.wrapping_add(row_data(row)));
        }
    }
    best
}

/// The FLOOR-crossing of one named need (survival F4 — the crossing scheduler's read):
/// the first tic strictly after `now`... at which the need's lazy value reaches
/// `threshold` (death reads corpus at its domain min). `None` = the row is absent, the
/// value already sits at/below the threshold's write-visible side, or the trajectory
/// never reaches it. Same eval as everything else — the worker schedules with this and
/// re-validates at fire.
pub fn floor_crossing_tic(
    bundle: &Bundle,
    need: &str,
    threshold: f64,
    trait_rows: &[u64],
    need_rows: &[(u64, u16)],
    condition_rows: &[(u64, u16)],
) -> Option<u16> {
    let nref = bundle.gameplay_reference("need", need)?;
    let &(row, set_tic) = need_rows
        .iter()
        .find(|(row, _)| row_reference(*row) == nref)?;
    let np = bundle.need_params_by_ref(nref)?;
    let windows = rate_windows(bundle, need, trait_rows, condition_rows, need_rows, set_tic);
    let base_rate = if np.deplete > 0.0 { (np.max - np.min) / np.deplete } else { 0.0 };
    let s0 = f64::from(dequantize(row_data(row), np.min as f32, np.max as f32))
        .clamp(np.min, np.max);
    if s0 <= threshold {
        return None; // already there — a write would see it now, nothing to schedule
    }
    let e = crossing_elapsed(s0 - threshold, base_rate, &windows)?;
    Some(set_tic.wrapping_add(e.ceil() as u16))
}

/// The REAL elapsed at which cumulative depletion reaches `drop` (> 0), walking the
/// piecewise segments; `None` if it never does (every live segment rate 0 with no end).
fn crossing_elapsed(drop: f64, base_rate: f64, windows: &[RateWindow]) -> Option<f64> {
    let cuts = window_cuts(windows);
    let mut prev = 0.0;
    let mut left = drop;
    for cut in cuts {
        let rate = segment_rate(base_rate, windows, (prev + cut) / 2.0);
        let span = cut - prev;
        if rate > 0.0 && left <= rate * span {
            return Some(prev + left / rate);
        }
        left -= rate * span;
        prev = cut;
    }
    // the unbounded tail: every finite window has switched off
    let rate = segment_rate(base_rate, windows, prev + 1.0);
    (rate > 0.0).then(|| prev + left / rate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::load;
    use resonantdust_codec::object::pack_row;
    use resonantdust_codec::value::quantize;

    /// thirst: deplete 1000 tics on the DEFAULT `0..1` domain, Thirsty [0.10, 0.35)
    /// +2 uncomfortable, Dehydrated [0, 0.10) +5 uncomfortable; quenched: +2 happy
    /// timed 100 tics.
    fn fixture() -> Bundle {
        let src = r##"
[[emotion]]
name = "fine"
color = "#9aa4b0"

[[emotion]]
name = "happy"
color = "#e8b23a"

[[emotion]]
name = "uncomfortable"
color = "#8a8f3c"

[[need]]
name = "thirst"
deplete = 1000
band = [
  { condition = "thirsty", lo = 0.10, hi = 0.35 },
  { condition = "dehydrated", lo = 0.0, hi = 0.10 },
]

[[condition]]
name = "thirsty"
emotions = [ { emotion = "uncomfortable", magnitude = 2 } ]

[[condition]]
name = "dehydrated"
emotions = [ { emotion = "uncomfortable", magnitude = 5 } ]

[[condition]]
name = "quenched"
emotions = [ { emotion = "happy", magnitude = 2 } ]
duration = 100
"##;
        load(&[("needs.toml".into(), src.into())]).expect("fixture loads")
    }

    fn need_row(b: &Bundle, name: &str, value: f64) -> u64 {
        let np = b.need_params(name).expect(name);
        let q = quantize(value as f32, np.min as f32, np.max as f32);
        pack_row(b.gameplay_reference("need", name).expect(name), q)
    }
    fn cond_row(b: &Bundle, name: &str, remaining: u16) -> u64 {
        pack_row(b.gameplay_reference("condition", name).expect(name), remaining)
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
        let src = r##"
[[need]]
name = "n"
deplete = 0
band = [
  { condition = "mid_a", lo = 0.0, hi = 1.01 },
  { condition = "mid_b", lo = 0.0, hi = 1.01 },
  { condition = "tie_b", lo = 0.0, hi = 1.01 },
  { condition = "low",   lo = 0.0, hi = 1.01 },
]

[[emotion]]
name = "fine"
color = "#9aa4b0"

[[emotion]]
name = "happy"
color = "#e8b23a"

[[emotion]]
name = "sad"
color = "#3a6ee8"

[[condition]]
name = "low"
emotions = [ { emotion = "sad", magnitude = 1 } ]
priority = 99

[[condition]]
name = "mid_a"
emotions = [ { emotion = "sad", magnitude = 1 } ]
priority = 10

[[condition]]
name = "mid_b"
emotions = [ { emotion = "happy", magnitude = 5 } ]
priority = 10

[[condition]]
name = "tie_b"
emotions = [ { emotion = "sad", magnitude = 2 } ]
priority = 5

[[condition]]
name = "tie_a"
emotions = [ { emotion = "happy", magnitude = 2 } ]
priority = 5
duration = 100
"##;
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
            "priority desc (low first), then Σ magnitude desc (mid_b 5 > mid_a 1), \
             then ref asc (tie_b before tie_a — equal priority AND equal Σ magnitude 2)"
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
duration = 3600
needs = [ { need = "thirst", rate = 0.5 } ]
"#;
        let b = load(&[("needs.toml".into(), src.into())]).expect("loads");
        let p = np(&b, "thirst");
        let conditions = [(cond_row(&b, "quenched", 3600), 1000u16)];
        let windows = rate_windows(&b, "thirst", &[], &conditions, &[], 1000);
        assert_eq!(windows, vec![RateWindow { rate: 0.5, start: 0.0, until: Some(3600.0), add: 0.0 }]);
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
    fn a_lone_late_grant_gets_a_mid_row_window() {
        // I12: quenched granted 100 tics AFTER the row's stamp, with NO paired re-stamp.
        // The window is derivable from written_tic: base rate for [0,100), halved for
        // [100,200), base after. thirst 0..100, deplete 1000 → 0.1/tic.
        let src = r#"
[[need]]
name = "thirst"
min = 0
max = 100
deplete = 1000

[[condition]]
name = "quenched"
duration = 3600
needs = [ { need = "thirst", rate = 0.5 } ]
"#;
        let b = load(&[("needs.toml".into(), src.into())]).expect("loads");
        let p = np(&b, "thirst");
        let conditions = [(cond_row(&b, "quenched", 100), 100u16)]; // written at 100, row set at 0
        let windows = rate_windows(&b, "thirst", &[], &conditions, &[], 0);
        assert_eq!(windows, vec![RateWindow { rate: 0.5, start: 100.0, until: Some(200.0), add: 0.0 }]);
        // read at 300: 100·0.1 + 100·0.05 + 100·0.1 = 25 drained.
        let sat = satisfaction_at(80.0, 0, &p, 300, &windows);
        assert!((sat - 55.0).abs() < 1e-9, "got {sat}");
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

[[pawn_trait_passive]]
name = "camel"
needs = [ { need = "thirst", rate = [0.5] } ]
"#;
        let b = load(&[("needs.toml".into(), src.into())]).expect("loads");
        let p = np(&b, "thirst");
        let tr = pack_row(b.gameplay_reference("pawn_trait_passive", "camel").unwrap(), 1);
        let windows = rate_windows(&b, "thirst", &[tr], &[], &[], 0);
        assert_eq!(windows, vec![RateWindow { rate: 0.5, start: 0.0, until: None, add: 0.0 }]);
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

[[condition]]
name = "quenched"
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
        assert_eq!(c[0].magnitude_sum, 5, "Dehydrated carries its Σ emotion magnitude");
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
        assert_eq!(next_crossing_tic(&b, &[], &[], &[], 100), None);
        // an unknown need/condition KEY skips, never panics.
        let ghost = pack_row(0x8001_0FF0, 30000); // gameplay/need, kind past the registry
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
        // Thirsty (+2 uncomfortable, DERIVED) stacks with Quenched (+2 happy, stored).
        let needs = [(need_row(&b, "thirst", 0.251), 1000u16)];
        let both = active_conditions(&b, &[], &needs, &rows, 1050);
        assert_eq!(both.len(), 2);
        assert!(both.iter().all(|c| c.magnitude_sum == 2));
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
    fn need_caps_tier_upward_by_the_leveled_trait() {
        // food-chain F2: corpus authors the 0..2 ENCODING domain; the leveled corpus
        // trait's per-level max caps it — level 1 → 1, level 2 → 2 (highest wins).
        let src = r##"
[[need]]
name = "corpus"
min = 0
max = 2

[[pawn_trait_passive]]
name = "corpus"
needs = [ { need = "corpus", max = [1.0, 2.0] } ]
"##;
        let b = load(&[("t.toml".into(), src.into())]).expect("loads");
        let np = b.need_params("corpus").expect("corpus");
        let tref = b.gameplay_reference("pawn_trait_passive", "corpus").expect("trait ref");
        let level1 = [pack_row(tref, 0)]; // tier 1 = variant 0 (F6)
        let level2 = [pack_row(tref | 1, 0)]; // tier 2 = variant 1
        assert_eq!(need_bounds(&b, "corpus", &np, &level1, &[], 0), (0.0, 1.0), "level 1 caps 1");
        assert_eq!(need_bounds(&b, "corpus", &np, &level2, &[], 0), (0.0, 2.0), "level 2 caps 2");
        assert_eq!(need_bounds(&b, "corpus", &np, &[], &[], 0), (0.0, 2.0), "unraited = domain");
        // Two sources: the HIGHEST authored cap stands (a second source cannot shrink).
        let both = [pack_row(tref, 0), pack_row(tref | 1, 0)];
        assert_eq!(need_bounds(&b, "corpus", &np, &both, &[], 0), (0.0, 2.0));
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
        let v = f64::from(dequantize(row_data(row), p.min as f32, p.max as f32));
        assert!((v - 33.0).abs() < 0.002, "got {v}");
        drop(b);
    }

    /// survival F3/F4 — the pinned trajectory (I1's acceptance): hunger banded
    /// [0,10) → starving, whose `deplete = 1000` drains corpus (deplete 0, domain
    /// 0..2). Hunger 20 at deplete 2000 (rate 100/2000 = 0.05/tic) enters the band at
    /// elapsed 200; corpus (full, 2.0) then drains at 2/1000 = 0.002/tic. Read at
    /// elapsed 700: 500 in-band tics → corpus = 2 − 500·0.002 = 1.0. Dehydration's
    /// twin band SUMS: with thirst also in-band from 0, corpus loses 0.004/tic while
    /// both run. The floor crossing predicts the zero write-tic the scheduler queues.
    #[test]
    fn deplete_modifiers_drain_and_predict_the_floor() {
        let src = r#"
[[need]]
name = "hunger"
min = 0
max = 100
deplete = 2000
band = [ { condition = "starving", lo = 0, hi = 10 } ]

[[need]]
name = "thirst"
min = 0
max = 100
deplete = 2000
band = [ { condition = "dehydrated", lo = 0, hi = 10 } ]

[[need]]
name = "corpus"
min = 0
max = 2

[[condition]]
name = "starving"
needs = [ { need = "corpus", deplete = 1000 } ]

[[condition]]
name = "dehydrated"
needs = [ { need = "corpus", deplete = 1000 } ]
"#;
        let b = load(&[("needs.toml".into(), src.into())]).expect("loads");
        // Hunger 20, thirst FULL (out of band the whole test), corpus full. All rows
        // stamped at tic 0.
        let needs = vec![
            (need_row(&b, "hunger", 20.0), 0u16),
            (need_row(&b, "thirst", 100.0), 0u16),
            (need_row(&b, "corpus", 2.0), 0u16),
        ];
        // Hunger reaches its band (< 10) at elapsed 200 (drop 10 at 0.05/tic).
        let sat = need_satisfaction(&b, "corpus", &[], &needs, &[], 700).expect("corpus row");
        assert!((sat - 1.0).abs() < 0.01, "single drain: got {sat}");
        // BOTH sources in-band: thirst 5 sits in dehydrated from elapsed 0 — corpus
        // loses 0.002/tic (dehydrated alone) for 200 tics, then 0.004/tic summed.
        let needs2 = vec![
            (need_row(&b, "hunger", 20.0), 0u16),
            (need_row(&b, "thirst", 5.0), 0u16),
            (need_row(&b, "corpus", 2.0), 0u16),
        ];
        let sat2 = need_satisfaction(&b, "corpus", &[], &needs2, &[], 500).expect("corpus row");
        // 200·0.002 + 300·0.004 = 0.4 + 1.2 = 1.6 → corpus 0.4
        assert!((sat2 - 0.4).abs() < 0.01, "summed drains: got {sat2}");
        // The floor crossing: from the single-drain trajectory, corpus reaches 0 at
        // elapsed 200 + 2.0/0.002 = 1200.
        let cross = floor_crossing_tic(&b, "corpus", 0.0, &[], &needs, &[]).expect("crossing");
        assert_eq!(cross, 1200, "the scheduler's write tic");
        // A need with no drains and deplete 0 still never moves (the old fast path).
        let sat3 = need_satisfaction(&b, "corpus", &[], &[(need_row(&b, "corpus", 2.0), 0)], &[], 5000)
            .expect("corpus row");
        assert!((sat3 - 2.0).abs() < 0.001, "undrained corpus moved: {sat3}");
    }
}
