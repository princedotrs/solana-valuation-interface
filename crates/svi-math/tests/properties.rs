use {
    proptest::prelude::*,
    svi_math::{
        deviation_bps, mul_div_ceil, mul_div_floor, pow10, pyth_band, quote_amount, rescale,
        scale_by_pow10, Band, Rounding,
    },
};

// ---------- worked examples from the spec ----------

#[test]
fn the_classic_hundred_over_three() {
    assert_eq!(mul_div_floor(100, 1, 3), Some(33));
    assert_eq!(mul_div_ceil(100, 1, 3), Some(34));
}

#[test]
fn real_xsol_nav_rescale() {
    // hylo-core returns redeem NAV in N9: 64_640_132 == $0.064640132
    let nav_n9 = 64_640_132u64;
    assert_eq!(rescale(nav_n9, 9, 6, Rounding::Down), Some(64_640));
    assert_eq!(rescale(nav_n9, 9, 6, Rounding::Up), Some(64_641));

    // one whole xSOL (9 decimals) valued in USD at 6 decimals
    assert_eq!(
        quote_amount(nav_n9, 9, 1_000_000_000, 9, 6, Rounding::Down),
        Some(64_640)
    );
}

#[test]
fn division_by_zero_is_none_not_panic() {
    assert_eq!(mul_div_floor(1, 1, 0), None);
    assert_eq!(mul_div_ceil(1, 1, 0), None);
}

#[test]
fn overflowing_result_is_none() {
    assert_eq!(mul_div_floor(u64::MAX, 2, 1), None);
    assert_eq!(rescale(u64::MAX, 0, 1, Rounding::Down), None);
}

#[test]
fn pow10_limits() {
    assert_eq!(pow10(19), Some(10_000_000_000_000_000_000));
    assert_eq!(pow10(20), None);
}

#[test]
fn band_rejects_inverted_bounds() {
    assert!(Band::new(10, 5, 20).is_none());
    assert!(Band::new(5, 10, 20).is_some());
}

#[test]
fn band_tolerance() {
    let b = Band::new(99, 100, 101).unwrap();
    assert_eq!(b.spread(), 2);
    assert_eq!(b.within_tolerance(200), Some(true));
    assert_eq!(b.within_tolerance(100), Some(false)); // 2/100 = 200 bps > 100
}

// ---------- oracle-price helpers ----------

#[test]
fn scale_by_pow10_moves_both_directions() {
    assert_eq!(scale_by_pow10(123, 2, Rounding::Down), Some(12_300));
    assert_eq!(scale_by_pow10(12_345, -2, Rounding::Down), Some(123));
    assert_eq!(scale_by_pow10(12_345, -2, Rounding::Up), Some(124));
    assert_eq!(scale_by_pow10(999, 0, Rounding::Down), Some(999));
}

/// A synthetic feed, not a real one: mantissa 19_234_567_800 at expo -8 is
/// 192.345678, published here at 9 decimals.
#[test]
fn pyth_band_worked_example() {
    let b = pyth_band(19_234_567_800, 1_500_000, -8, 9).unwrap();
    assert_eq!(b.mid, 192_345_678_000);
    assert_eq!(b.lower, 192_330_678_000); // (price - conf) * 10
    assert_eq!(b.upper, 192_360_678_000); // (price + conf) * 10
    assert!(b.lower <= b.mid && b.mid <= b.upper);
}

#[test]
fn pyth_band_zero_confidence_is_a_point() {
    let b = pyth_band(19_234_567_800, 0, -8, 9).unwrap();
    assert_eq!(b.lower, b.mid);
    assert_eq!(b.upper, b.mid);
    assert_eq!(b.spread(), 0);
}

/// Confidence wider than the price is a real state of the world. We keep the
/// band constructible with a floor of zero and let the caller's tolerance
/// reject it, rather than failing here and losing the information.
#[test]
fn pyth_band_confidence_wider_than_price_saturates_at_zero() {
    let b = pyth_band(1_000, 5_000, -2, 2).unwrap();
    assert_eq!(b.lower, 0);
    assert!(b.mid > 0);
    assert_eq!(b.within_tolerance(100), Some(false));
}

#[test]
fn pyth_band_refuses_non_positive_prices() {
    assert!(pyth_band(0, 10, -8, 9).is_none());
    assert!(pyth_band(-1, 10, -8, 9).is_none());
    assert!(pyth_band(i64::MIN, 10, -8, 9).is_none());
}

#[test]
fn deviation_worked_examples() {
    assert_eq!(deviation_bps(100, 100), Some(0));
    assert_eq!(deviation_bps(102, 100), Some(200)); // 2% premium
    assert_eq!(deviation_bps(98, 100), Some(200));  // 2% discount, same magnitude
    assert_eq!(deviation_bps(1, 0), None);
}

/// Rounding up matters: a deviation of 0.005% must not report as zero, or a
/// threshold of "flag anything non-trivial" would never fire.
#[test]
fn deviation_rounds_up_so_small_drifts_are_visible() {
    assert_eq!(deviation_bps(1_000_001, 1_000_000), Some(1));
    assert_eq!(deviation_bps(999_999, 1_000_000), Some(1));
}

// ---------- properties ----------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    /// Floor never exceeds ceil, and they differ by at most one unit.
    #[test]
    fn floor_le_ceil_and_within_one(a: u64, b: u64, d in 1u64..) {
        if let (Some(f), Some(c)) = (mul_div_floor(a, b, d), mul_div_ceil(a, b, d)) {
            prop_assert!(f <= c);
            prop_assert!(c - f <= 1);
        }
    }

    /// They agree exactly when the division has no remainder.
    #[test]
    fn agree_iff_exact(a: u64, b: u64, d in 1u64..) {
        let exact = (a as u128 * b as u128) % d as u128 == 0;
        if let (Some(f), Some(c)) = (mul_div_floor(a, b, d), mul_div_ceil(a, b, d)) {
            prop_assert_eq!(f == c, exact);
        }
    }

    /// Dividing by the same factor you multiplied by is the identity.
    #[test]
    fn multiply_then_divide_is_identity(a: u64, d in 1u64..) {
        prop_assert_eq!(mul_div_floor(a, d, d), Some(a));
        prop_assert_eq!(mul_div_ceil(a, d, d), Some(a));
    }

    /// Monotone in the first argument: a bigger input never yields a smaller output.
    #[test]
    fn monotone_in_a(a1: u32, a2: u32, b: u32, d in 1u32..) {
        let (lo, hi) = if a1 <= a2 { (a1, a2) } else { (a2, a1) };
        let f_lo = mul_div_floor(lo.into(), b.into(), d.into()).unwrap();
        let f_hi = mul_div_floor(hi.into(), b.into(), d.into()).unwrap();
        prop_assert!(f_lo <= f_hi);
    }

    /// Scaling up then back down is lossless.
    #[test]
    fn rescale_up_then_down_roundtrips(amount in 0u64..u64::MAX / 1_000_000, k in 0u8..6) {
        let up = rescale(amount, 0, k, Rounding::Down).unwrap();
        prop_assert_eq!(rescale(up, k, 0, Rounding::Down), Some(amount));
    }

    /// Scaling down loses information, and the direction is respected.
    #[test]
    fn rescale_down_is_bounded(amount: u64, k in 1u8..10) {
        let down = rescale(amount, k, 0, Rounding::Down).unwrap();
        let up = rescale(amount, k, 0, Rounding::Up).unwrap();
        prop_assert!(down <= up);
        prop_assert!(up - down <= 1);
        prop_assert!(rescale(down, 0, k, Rounding::Down).unwrap() <= amount);
    }

    /// THE INVARIANT THE WHOLE DESIGN RESTS ON:
    /// the conservative quote is never larger than the optimistic one.
    #[test]
    fn conservative_never_exceeds_optimistic(
        unit_value in 0u64..1_000_000_000_000,
        base_amount in 0u64..10_000_000_000_000,
    ) {
        let lo = quote_amount(unit_value, 9, base_amount, 9, 6, Rounding::Down);
        let hi = quote_amount(unit_value, 9, base_amount, 9, 6, Rounding::Up);
        if let (Some(lo), Some(hi)) = (lo, hi) {
            prop_assert!(lo <= hi);
        }
    }

    /// A Band that constructs always satisfies the core program's check.
    #[test]
    fn band_invariant_holds(a: u64, b: u64, c: u64) {
        if let Some(band) = Band::new(a, b, c) {
            prop_assert!(band.lower <= band.mid);
            prop_assert!(band.mid <= band.upper);
        }
    }

    /// Whatever Pyth reports, the band we publish satisfies svi-core's
    /// `lower <= quote <= upper` check. If this can fail, the adapter can
    /// build an update the core will reject at runtime.
    #[test]
    fn pyth_band_always_satisfies_the_core_invariant(
        price in 1i64..1_000_000_000_000,
        conf in 0u64..1_000_000_000,
        expo in -12i32..0,
        out_decimals in 0u8..12,
    ) {
        if let Some(b) = pyth_band(price, conf, expo, out_decimals) {
            prop_assert!(b.lower <= b.mid);
            prop_assert!(b.mid <= b.upper);
        }
    }

    /// Wider confidence can never produce a narrower band.
    #[test]
    fn wider_confidence_never_narrows_the_band(
        price in 1i64..1_000_000_000,
        c1 in 0u64..1_000_000,
        c2 in 0u64..1_000_000,
    ) {
        let (lo, hi) = if c1 <= c2 { (c1, c2) } else { (c2, c1) };
        if let (Some(a), Some(b)) = (pyth_band(price, lo, -8, 9), pyth_band(price, hi, -8, 9)) {
            prop_assert!(a.spread() <= b.spread());
        }
    }

    /// Deviation is symmetric about the reference: a premium and a discount of
    /// the same size report the same magnitude.
    ///
    /// Only meaningful while the discount side does not clamp — a price cannot
    /// fall more than 100%, so `delta` is capped at the reference. Without the
    /// cap the test compares a premium of `delta` against a discount of only
    /// `reference`, which are different distances and rightly differ.
    #[test]
    fn deviation_is_symmetric(reference in 1u64..1_000_000_000, raw_delta in 0u64..1_000_000) {
        let delta = raw_delta.min(reference);
        let up = reference.saturating_add(delta);
        let down = reference - delta;
        prop_assert_eq!(deviation_bps(up, reference), deviation_bps(down, reference));
    }

    /// A total loss reads as exactly 100%.
    #[test]
    fn deviation_of_a_worthless_market_price_is_10000_bps(reference in 1u64..1_000_000_000) {
        prop_assert_eq!(deviation_bps(0, reference), Some(10_000));
    }

    /// Zero deviation if and only if the two values are equal.
    #[test]
    fn deviation_zero_iff_equal(market in 0u64..1_000_000_000, reference in 1u64..1_000_000_000) {
        let d = deviation_bps(market, reference).unwrap();
        prop_assert_eq!(d == 0, market == reference);
    }

    /// Never under-reports: the returned bps, applied back to the reference,
    /// always covers the actual difference. This is what makes it safe to
    /// compare against a threshold.
    #[test]
    fn deviation_never_under_reports(market in 0u64..1_000_000_000, reference in 1u64..1_000_000_000) {
        let d = deviation_bps(market, reference).unwrap();
        let covered = u128::from(d) * u128::from(reference);
        let actual = u128::from(market.abs_diff(reference)) * 10_000u128;
        prop_assert!(covered >= actual);
    }

    /// Scaling by a positive then the matching negative shift is lossless.
    #[test]
    fn scale_by_pow10_roundtrips(amount in 0u64..u64::MAX / 1_000_000_000, k in 0i32..9) {
        let up = scale_by_pow10(amount, k, Rounding::Down).unwrap();
        prop_assert_eq!(scale_by_pow10(up, -k, Rounding::Down), Some(amount));
    }
}
