use {
    proptest::prelude::*,
    svi_math::{mul_div_ceil, mul_div_floor, pow10, quote_amount, rescale, Band, Rounding},
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
}
