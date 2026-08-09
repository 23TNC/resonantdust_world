//! `emotion_eval` — THE active-emotion argmax (emotions F3).
//!
//! One implementation, every consumer: the pawn's emotion is `argmax` over the summed
//! `emotions` modifiers of its trait levels + ACTIVE conditions. No contributions →
//! `fine` (index 0); a tie takes the LOWEST index — deterministic everywhere, and
//! fine-first declaration order biases ties toward calm. Emotions never ride the wire:
//! every observer computes this from its corpus, so two observers with one corpus and
//! one active set CANNOT disagree (the one-eval law).

use crate::loader::Bundle;
use crate::needs_eval::ActiveCondition;
use resonantdust_codec::object::{def_variant_id, row_reference};

/// The 16 per-emotion magnitude sums (index = the u4 declaration index) for a pawn's
/// trait rows + active conditions. Rows naming an unknown trait/condition are skipped —
/// version skew reads as "no contribution", never a panic.
pub fn emotion_sums(bundle: &Bundle, trait_rows: &[u64], active: &[ActiveCondition]) -> [u16; 16] {
    let mut sums = [0u16; 16];
    for &row in trait_rows {
        let reference = row_reference(row);
        let Some(tp) = bundle.trait_params_by_ref(reference) else { continue };
        let tier = def_variant_id(reference) as usize;
        let Some(entry) = tp.levels.get(tier) else { continue };
        for m in &entry.emotions {
            sums[(m.emotion & 0x0f) as usize] += m.magnitude as u16;
        }
    }
    for c in active {
        let Some(cp) = bundle.condition_params_by_ref(c.condition_id) else { continue };
        for m in &cp.emotions {
            sums[(m.emotion & 0x0f) as usize] += m.magnitude as u16;
        }
    }
    sums
}

/// The ACTIVE emotion (F3): `(u4 index, the 16 sums)`. Empty → `fine` (0); ties → the
/// LOWEST index (`>` scan from 0 keeps the first maximum).
pub fn active_emotion(
    bundle: &Bundle,
    trait_rows: &[u64],
    active: &[ActiveCondition],
) -> (u8, [u16; 16]) {
    let sums = emotion_sums(bundle, trait_rows, active);
    let mut best = 0usize;
    for (i, &s) in sums.iter().enumerate() {
        if s > sums[best] {
            best = i;
        }
    }
    (best as u8, sums)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::load;
    use resonantdust_codec::object::pack_gameplay_row;

    /// The user's worked oracle (emotions I6): 3 playful + 5 uncomfortable + 2 focused
    /// + 6 happy → HAPPY. Declaration order fine/happy/focused/uncomfortable/playful.
    fn fixture() -> Bundle {
        let src = r##"
[[emotion]]
name = "fine"
color = "#9aa4b0"

[[emotion]]
name = "happy"
color = "#e8a33a"

[[emotion]]
name = "focused"
color = "#2fb8d8"

[[emotion]]
name = "uncomfortable"
color = "#8a8f3c"

[[emotion]]
name = "playful"
color = "#c93cb8"

[[condition]]
name = "c_playful"
emotions = [ { emotion = "playful", magnitude = 3 } ]
duration = 100

[[condition]]
name = "c_uncomfortable"
emotions = [ { emotion = "uncomfortable", magnitude = 5 } ]
duration = 100

[[condition]]
name = "c_focused"
emotions = [ { emotion = "focused", magnitude = 2 } ]
duration = 100

[[condition]]
name = "c_happy"
emotions = [ { emotion = "happy", magnitude = 6 } ]
duration = 100

[[condition]]
name = "c_sadless"
duration = 100

[[trait]]
name = "puppyish"
emotions = [ { emotion = "playful", magnitude = [1, 4] } ]
"##;
        load(&[("emotions.toml".into(), src.into())]).expect("fixture loads")
    }

    fn active(b: &Bundle, names: &[&str]) -> Vec<ActiveCondition> {
        names
            .iter()
            .map(|n| ActiveCondition {
                condition_id: b.gameplay_reference("condition", n).expect(n),
                magnitude_sum: 0,
                remaining: 50,
                priority: 0,
            })
            .collect()
    }

    #[test]
    fn the_users_oracle_lands_on_happy() {
        let b = fixture();
        let set = active(&b, &["c_playful", "c_uncomfortable", "c_focused", "c_happy"]);
        let (e, sums) = active_emotion(&b, &[], &set);
        assert_eq!(e, b.emotion_index("happy").unwrap(), "3/5/2/6 → happy (the user's example)");
        assert_eq!(sums[b.emotion_index("playful").unwrap() as usize], 3);
        assert_eq!(sums[b.emotion_index("uncomfortable").unwrap() as usize], 5);
        assert_eq!(sums[b.emotion_index("focused").unwrap() as usize], 2);
        assert_eq!(sums[b.emotion_index("happy").unwrap() as usize], 6);
    }

    #[test]
    fn empty_is_fine_and_ties_take_the_lowest_index() {
        let b = fixture();
        assert_eq!(active_emotion(&b, &[], &[]).0, 0, "no contributions → fine");
        assert_eq!(active_emotion(&b, &[], &active(&b, &["c_sadless"])).0, 0, "+0 set → fine");
        // Trait levels sum in beside conditions: puppyish level 2 (+4 playful) +
        // c_playful (+3) = 7, beating c_happy's 6.
        let t = pack_gameplay_row(b.gameplay_reference("trait", "puppyish").unwrap(), 2);
        let set = active(&b, &["c_playful", "c_happy"]);
        let (e, sums) = active_emotion(&b, &[t], &set);
        assert_eq!(sums[b.emotion_index("playful").unwrap() as usize], 7, "trait levels sum in");
        assert_eq!(e, b.emotion_index("playful").unwrap());
        // An exact tie: focused 2+2+2 = 6 vs happy 6 — the LOWEST declaration index
        // (happy, 1) wins over focused (2).
        let tie = active(&b, &["c_focused", "c_focused", "c_focused", "c_happy"]);
        let (e, sums) = active_emotion(&b, &[], &tie);
        assert_eq!(sums[b.emotion_index("focused").unwrap() as usize], 6);
        assert_eq!(sums[b.emotion_index("happy").unwrap() as usize], 6);
        assert_eq!(e, b.emotion_index("happy").unwrap(), "a 6=6 tie takes the LOWEST index");
    }
}
