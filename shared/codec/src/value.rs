//! The ONE f32 ↔ u16 fixed-point pair (stat-model F4/I2).
//!
//! A need's stored value is u16 fixed-point on its AUTHORED `min..max` domain: `0..65535`
//! maps linearly onto `min..max`. The encoding domain is ALWAYS the authored bounds —
//! modifiers narrow the effective clamp *within* it, never re-scale the encoding — so a
//! quantized word never changes meaning when a condition comes or goes.
//!
//! Rounding is half away from zero (`f32::round`), defined HERE and nowhere else: the
//! quantization happens ONCE at write (worker / reducer composers), and every consumer
//! dequantizes with the exact inverse. A second rounding rule anywhere is a drift lane
//! between the npc's availability math and the worker's gate.

/// Quantize an authored-units value onto its need's `min..max` domain. Out-of-domain values
/// clamp to the ends; a degenerate domain (`min >= max`) quantizes to 0.
pub fn quantize(v: f32, min: f32, max: f32) -> u16 {
    if !(max > min) || !v.is_finite() {
        return 0;
    }
    let t = (v - min) / (max - min) * 65535.0;
    t.round().clamp(0.0, 65535.0) as u16
}

/// The exact inverse: a stored u16 back to authored units.
pub fn dequantize(q: u16, min: f32, max: f32) -> f32 {
    min + (q as f32 / 65535.0) * (max - min)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_domain_ends_are_exact() {
        assert_eq!(quantize(0.0, 0.0, 100.0), 0);
        assert_eq!(quantize(100.0, 0.0, 100.0), 65535);
        assert_eq!(dequantize(0, 0.0, 100.0), 0.0);
        assert_eq!(dequantize(65535, 0.0, 100.0), 100.0);
    }

    #[test]
    fn the_p0_worked_window_stamp_quantizes_to_26214() {
        // VARIABLES.md §Pawn gameplay state: 40.0 on thirst's 0..100 → q = 26214 (exactly
        // 40% of 65535), dequantizing back to 40.0000.
        let q = quantize(40.0, 0.0, 100.0);
        assert_eq!(q, 26214);
        let v = dequantize(q, 0.0, 100.0);
        assert!((v - 40.0).abs() < 1e-4, "dequantized {v}");
    }

    #[test]
    fn out_of_domain_clamps_and_junk_is_zero() {
        assert_eq!(quantize(-5.0, 0.0, 100.0), 0);
        assert_eq!(quantize(250.0, 0.0, 100.0), 65535);
        assert_eq!(quantize(f32::NAN, 0.0, 100.0), 0);
        assert_eq!(quantize(50.0, 100.0, 100.0), 0, "degenerate domain");
    }

    #[test]
    fn a_negative_min_domain_round_trips() {
        // min can be negative — min/max IS the sign treatment (interactions F7).
        let q = quantize(-25.0, -100.0, 100.0);
        let v = dequantize(q, -100.0, 100.0);
        assert!((v + 25.0).abs() < 0.01, "round-tripped {v}");
    }

    #[test]
    fn quantize_dequantize_is_stable() {
        // Re-quantizing a dequantized value must be the identity — re-stamps (F7) pass a
        // need's value through this pair repeatedly and must not creep.
        for q in [0u16, 1, 12345, 26214, 65534, 65535] {
            let v = dequantize(q, 0.0, 100.0);
            assert_eq!(quantize(v, 0.0, 100.0), q, "q {q} crept");
        }
    }
}
