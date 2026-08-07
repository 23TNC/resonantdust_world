//! `stat_eval` — DERIVED stats + affordance predicates (stat-model F5/F6/F8).
//!
//! A stat is never stored and never fanned: it is `clamp(Σ contributions, combined bounds)`
//! computed HERE from a pawn's packed trait rows + its active conditions, for every consumer
//! (worker gate, npc availability, client pie menu). A second implementation anywhere is the
//! drift class stat-model I2 names — ghost refusals between the npc's check and the worker's.
//!
//! The combiner (F6, user): additive contributions SUM (base 0); range contributions
//! INTERSECT (max-of-mins / min-of-maxes) inside the def's authored global bounds; an EMPTY
//! intersection is decided by the def's authored winner (`min_wins`).

use crate::loader::{Bundle, StatModifier};
use crate::needs_eval::ActiveCondition;
use resonantdust_codec::object::{gameplay_row_data, gameplay_row_reference, GAMEPLAY_TRAIT};

/// A stored condition row's remaining tics at `now`: `remaining_at_write − elapsed`
/// (stat-model F3 — backward-looking, never a stored countdown). A row written AHEAD of the
/// observer's clock (within the wrap half-window) reads as elapsed 0, not as ancient — the
/// same future-stamp guard the needs eval carries (I6).
pub fn condition_remaining(row: u32, written_tic: u16, now: u16) -> u16 {
    let raw = now.wrapping_sub(written_tic);
    let elapsed = if raw > u16::MAX / 2 { 0 } else { raw };
    gameplay_row_data(row).saturating_sub(elapsed)
}

/// The combiner's RANGE half (F6): intersect `contribs`' optional lo/hi pairs inside the
/// authored `base` bounds; an empty intersection pins to the winning bound (`min_wins`).
/// Returns `(lo, hi)` with `lo <= hi` always.
pub fn combine_bounds(
    base: (f64, f64),
    contribs: impl Iterator<Item = (Option<f64>, Option<f64>)>,
    min_wins: bool,
) -> (f64, f64) {
    let (mut lo, mut hi) = base;
    for (cmin, cmax) in contribs {
        if let Some(m) = cmin {
            lo = lo.max(m);
        }
        if let Some(m) = cmax {
            hi = hi.min(m);
        }
    }
    // The authored global bounds are the OUTERMOST clamp — a contribution cannot push past them.
    lo = lo.clamp(base.0, base.1);
    hi = hi.clamp(base.0, base.1);
    if lo > hi {
        let pin = if min_wins { lo } else { hi };
        (pin, pin)
    } else {
        (lo, hi)
    }
}

/// Every stat modifier this pawn carries for `stat`: its trait rows' level entries + its
/// ACTIVE conditions' entries (derived band conditions included — `active` comes from
/// [`crate::needs_eval::active_conditions`]).
fn modifiers_for<'a>(
    bundle: &'a Bundle,
    stat: &'a str,
    trait_rows: &'a [u32],
    active: &'a [ActiveCondition],
) -> Vec<StatModifier> {
    let mut out = Vec::new();
    for &row in trait_rows {
        let reference = gameplay_row_reference(GAMEPLAY_TRAIT, row);
        let Some(tp) = bundle.trait_params_by_ref(reference) else { continue };
        let level = gameplay_row_data(row) as usize;
        let Some(entry) = level.checked_sub(1).and_then(|i| tp.levels.get(i)) else { continue };
        out.extend(entry.stats.iter().filter(|m| m.stat == stat).cloned());
    }
    for c in active {
        let Some(cp) = bundle.condition_params_by_ref(c.condition_id) else { continue };
        out.extend(cp.stats.iter().filter(|m| m.stat == stat).cloned());
    }
    out
}

/// Derive one stat (F8): `clamp(Σ add, combined bounds)` from the pawn's trait rows +
/// active conditions. An unknown stat derives to `0.0`.
pub fn stat_value(
    bundle: &Bundle,
    stat: &str,
    trait_rows: &[u32],
    active: &[ActiveCondition],
) -> f64 {
    let Some(sp) = bundle.stat_params(stat) else { return 0.0 };
    let mods = modifiers_for(bundle, stat, trait_rows, active);
    let sum: f64 = mods.iter().map(|m| m.add).sum();
    let (lo, hi) = combine_bounds((sp.min, sp.max), mods.iter().map(|m| (m.min, m.max)), sp.min_wins);
    sum.clamp(lo, hi)
}

/// Does `affordance` pass for this pawn (F5/F10; food-chain F4)? A STAT check is
/// EXCLUSIVE (`value > above` / `value < below`); a NEED check compares the lazy
/// satisfaction at `now` (a pawn with no row for the need never passes). An unknown
/// affordance never passes. `need_rows`/`cond_rows` are the same raw slices `active`
/// was derived from — the stat path reads `active`, the need path re-evaluates lazily.
pub fn affordance_passes(
    bundle: &Bundle,
    affordance: &str,
    trait_rows: &[u32],
    need_rows: &[(u32, u16)],
    cond_rows: &[(u32, u16)],
    active: &[ActiveCondition],
    now: u16,
) -> bool {
    let Some(ap) = bundle.affordance_params(affordance) else { return false };
    match &ap.check {
        crate::loader::AffordanceCheck::Stat { stat, above, below } => {
            let v = stat_value(bundle, stat, trait_rows, active);
            match (above, below) {
                (Some(t), None) => v > *t,
                (None, Some(t)) => v < *t,
                _ => false, // refused at load; unreachable from a loaded corpus
            }
        }
        crate::loader::AffordanceCheck::Need { need, cmp, value } => {
            match crate::needs_eval::need_satisfaction(
                bundle, need, trait_rows, need_rows, cond_rows, now,
            ) {
                Some(sat) => cmp.pass(sat, *value),
                None => false,
            }
        }
    }
}

/// Every affordance gate on `interaction` passes (stat-model F5) — the availability
/// question the npc, the worker, and the pie menu all ask identically.
#[allow(clippy::too_many_arguments)]
pub fn interaction_available(
    bundle: &Bundle,
    interaction: &str,
    trait_rows: &[u32],
    need_rows: &[(u32, u16)],
    cond_rows: &[(u32, u16)],
    active: &[ActiveCondition],
    now: u16,
) -> bool {
    match bundle.interaction_params(interaction) {
        Some(ip) => ip.affordances.iter().all(|a| {
            affordance_passes(bundle, a, trait_rows, need_rows, cond_rows, active, now)
        }),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::load;
    use resonantdust_codec::object::pack_gameplay_row;

    fn bundle(src: &str) -> Bundle {
        load(&[("gameplay.toml".into(), src.into())]).expect("fixture loads")
    }

    fn trait_row(b: &Bundle, name: &str, level: u16) -> u32 {
        pack_gameplay_row(b.gameplay_reference("trait", name).expect(name), level)
    }

    /// The stream's real shape: walks' leveled tics/tile into ground_speed,
    /// biological_lifeform's marker into metabolism, both predicates.
    const CORPUS: &str = r#"
[[stat]]
name = "ground_speed"
min = 0
max = 240
winner = "min"

[[stat]]
name = "metabolism"
min = 0
max = 10
winner = "min"

[[trait]]
name = "walks"
stats = [ { stat = "ground_speed", add = [24, 12, 6] } ]

[[trait]]
name = "biological_lifeform"
stats = [ { stat = "metabolism", add = [1] } ]

[[affordance]]
name = "can_move_ground"
check = { stat = "ground_speed", above = 0.0 }

[[affordance]]
name = "can_drink"
check = { stat = "metabolism", above = 0.0 }

[[need]]
name = "thirst"
min = 0
max = 100

[[interaction]]
name = "drink"
affordances = ["can_drink"]
inputs = ["pawn", "amount"]
satisfy = { target = "@pawn", need = "thirst", amount = "@amount" }
"#;

    #[test]
    fn a_leveled_trait_contributes_its_level_value() {
        let b = bundle(CORPUS);
        let rows = [trait_row(&b, "walks", 2), trait_row(&b, "biological_lifeform", 1)];
        assert_eq!(stat_value(&b, "ground_speed", &rows, &[]), 12.0, "walks level 2");
        assert_eq!(stat_value(&b, "metabolism", &rows, &[]), 1.0);
        assert_eq!(stat_value(&b, "ground_speed", &[trait_row(&b, "walks", 3)], &[]), 6.0);
        // No walks → ground_speed 0 → can_move_ground fails, can_drink still passes.
        let biolife = [trait_row(&b, "biological_lifeform", 1)];
        assert!(!affordance_passes(&b, "can_move_ground", &biolife, &[], &[], &[], 0));
        assert!(affordance_passes(&b, "can_drink", &biolife, &[], &[], &[], 0));
        assert!(interaction_available(&b, "drink", &biolife, &[], &[], &[], 0));
        assert!(!interaction_available(&b, "drink", &[trait_row(&b, "walks", 1)], &[], &[], &[], 0));
    }

    #[test]
    fn the_users_combiner_cases_hold() {
        // F6 verbatim: 3..7 ∧ 4..8 → 4..7; 2..3 ∧ 4..5 → winner ("min" pins 4, "max" pins 3).
        let overlapping = [(Some(3.0), Some(7.0)), (Some(4.0), Some(8.0))];
        assert_eq!(combine_bounds((0.0, 100.0), overlapping.iter().copied(), true), (4.0, 7.0));
        let disjoint = [(Some(2.0), Some(3.0)), (Some(4.0), Some(5.0))];
        assert_eq!(combine_bounds((0.0, 100.0), disjoint.iter().copied(), true), (4.0, 4.0));
        assert_eq!(combine_bounds((0.0, 100.0), disjoint.iter().copied(), false), (3.0, 3.0));
    }

    #[test]
    fn range_contributions_clamp_the_sum_and_respect_global_bounds() {
        let b = bundle(
            r#"
[[stat]]
name = "s"
min = 0
max = 10
winner = "min"

[[trait]]
name = "floor_four"
stats = [ { stat = "s", add = [1], min = [4] } ]

[[trait]]
name = "cap_beyond_global"
stats = [ { stat = "s", add = [0], max = [50] } ]
"#,
        );
        // add 1 but floor 4 → 4.
        assert_eq!(stat_value(&b, "s", &[trait_row(&b, "floor_four", 1)], &[]), 4.0);
        // a contributed max above the global bound clamps to the global (outermost) bound.
        let rows = [trait_row(&b, "floor_four", 1), trait_row(&b, "cap_beyond_global", 1)];
        assert_eq!(stat_value(&b, "s", &rows, &[]), 4.0);
        assert_eq!(
            combine_bounds((0.0, 10.0), [(None, Some(50.0))].iter().copied(), true),
            (0.0, 10.0)
        );
    }

    #[test]
    fn conditions_contribute_while_active() {
        let b = bundle(
            r#"
[[stat]]
name = "s"
min = 0
max = 10

[[condition]]
name = "boosted"
duration = 100
stats = [ { stat = "s", add = 2.0 } ]
"#,
        );
        let cref = b.gameplay_reference("condition", "boosted").unwrap();
        let active =
            [ActiveCondition { condition_id: cref, magnitude_sum: 0, remaining: 50, priority: 0 }];
        assert_eq!(stat_value(&b, "s", &[], &active), 2.0);
        assert_eq!(stat_value(&b, "s", &[], &[]), 0.0, "expired = gone");
    }

    #[test]
    fn condition_remaining_derives_and_guards_the_seams() {
        let row = pack_gameplay_row(0x8002_0030, 3600);
        assert_eq!(condition_remaining(row, 1000, 1000), 3600);
        assert_eq!(condition_remaining(row, 1000, 2000), 2600);
        assert_eq!(condition_remaining(row, 1000, 4600), 0, "expiry at exactly remaining");
        assert_eq!(condition_remaining(row, 1000, 995), 3600, "future-stamped = fresh");
        // u16 wrap: written near the top, now wrapped past 0.
        assert_eq!(condition_remaining(row, 65000, 65000u16.wrapping_add(600)), 3000);
    }

    #[test]
    fn unknown_names_and_absent_levels_are_calm() {
        let b = bundle(CORPUS);
        assert_eq!(stat_value(&b, "nonsense", &[], &[]), 0.0);
        assert!(!affordance_passes(&b, "nonsense", &[], &[], &[], &[], 0));
        assert!(!interaction_available(&b, "nonsense", &[], &[], &[], &[], 0));
        // a level past the authored table contributes nothing (not a panic).
        assert_eq!(stat_value(&b, "ground_speed", &[trait_row(&b, "walks", 9)], &[]), 0.0);
        // a level-0 row (never minted, but defensively) contributes nothing.
        assert_eq!(stat_value(&b, "ground_speed", &[trait_row(&b, "walks", 0)], &[]), 0.0);
    }
}
